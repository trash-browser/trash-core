//! CEF browser-process host: init, external message pump, and a single content
//! browser parented as an X11 child of the GPUI window.

use cef::{args::Args, *};
use std::collections::{HashMap, VecDeque};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{
    Arc, Mutex, OnceLock,
    atomic::{AtomicI64, AtomicU64, Ordering},
};
use trash_adblock::{
    AdBlockAdapter, AdBlockDecision, AdBlockService, AdBlockServiceBuilder, AdapterRequest,
    BraveAdBlockEngine, FilterListStatus, FilterListUpdater, FrameContext, FrameKey,
    NavigationContextStore, RequestContext, ResourceType as AdBlockResourceType,
    StatisticsRecorder, StatisticsSnapshot, YoutubeResponseKind, classify_youtube_response,
    sanitize_youtube_response, statistics_channel,
};

/// Events pushed from CEF callbacks (UI thread) and drained by the GPUI pump.
/// Every per-page event carries the owning `tab_id` so the chrome can update the
/// right tab even when the event comes from a background browser.
#[derive(Debug, Clone)]
pub enum CefEvent {
    Ready,
    Created {
        tab_id: u64,
    },
    Closed {
        tab_id: u64,
    },
    Address {
        tab_id: u64,
        url: String,
    },
    Title {
        tab_id: u64,
        title: String,
    },
    Loading {
        tab_id: u64,
        loading: bool,
        can_back: bool,
        can_forward: bool,
    },
    /// PNG bytes for the page favicon (empty clears it).
    Favicon {
        tab_id: u64,
        bytes: Vec<u8>,
    },
    /// The CEF content browser took keyboard focus (e.g. user clicked the page).
    ContentFocus,
}

/// A tab whose CEF browser has not been created yet.
struct PendingTab {
    tab_id: u64,
    url: String,
}

struct Shared {
    tx: Sender<CefEvent>,
    /// Created content browsers, keyed by GPUI tab id.
    browsers: HashMap<u64, Browser>,
    /// CEF browser identifier -> owning tab id (for routing callbacks).
    identifier_to_tab: HashMap<i32, u64>,
    /// The visible tab. Its browser fills the content area; others are hidden.
    active_tab: Option<u64>,
    /// Tabs waiting for context/parent before their browser can be created.
    to_create: VecDeque<PendingTab>,
    /// Tabs whose create call was issued, awaiting `on_after_created` (FIFO so we
    /// can correlate the created browser back to the requesting tab).
    creating: VecDeque<PendingTab>,
    /// Per-tab URL to load once the browser exists (a navigation that arrived
    /// before creation finished).
    pending_nav: HashMap<u64, String>,
    /// Native parent window + last content bounds (device pixels).
    parent: Option<u64>,
    /// On Linux, a plain X11 child we own under `parent`. CEF parents under this
    /// host instead of the Vulkan/WGPU surface window — Chromium's child window
    /// creation fails against the GPUI window directly (XGetWindowAttributes
    /// BadDrawable on the CEF xid).
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    host_xid: Option<u64>,
    bounds: Rect,
    start_uri: String,
    context_ready: bool,
    /// Linux OSR: GPUI owns OS focus; keep CEF browser focus in sync for typing.
    content_input_active: bool,
}

impl Shared {
    fn active_browser(&self) -> Option<Browser> {
        self.active_tab
            .and_then(|id| self.browsers.get(&id).cloned())
    }
}

/// Shared CEF state (browser process only).
static SHARED: OnceLock<Arc<Mutex<Shared>>> = OnceLock::new();

struct AdBlockRuntime {
    service: Arc<AdBlockService>,
    adapter: AdBlockAdapter,
    navigation: Arc<NavigationContextStore>,
    filter_updater: Mutex<FilterListUpdater>,
    recorder: Mutex<Option<StatisticsRecorder>>,
    fallback_statistics: StatisticsSnapshot,
    last_pushed_statistics_version: AtomicU64,
    last_pushed_filter_version: AtomicU64,
}

static ADBLOCK: OnceLock<Arc<AdBlockRuntime>> = OnceLock::new();

fn shared() -> Arc<Mutex<Shared>> {
    SHARED.get().expect("CEF not initialized").clone()
}

fn adblock_runtime() -> Arc<AdBlockRuntime> {
    ADBLOCK.get().expect("ad blocker not initialized").clone()
}

/// Bootstrap CEF. `args` must outlive the whole process (MainArgs points into it).
/// Returns `None` when this process is a CEF helper (already finished).
pub fn init(args: &Args) -> Option<Receiver<CefEvent>> {
    let _ = api_hash(sys::CEF_API_VERSION_LAST, 0);

    let main_args = args.as_main_args();
    let Some(cmd_line) = args.as_cmd_line() else {
        eprintln!("[cef] failed to parse command line");
        std::process::exit(1);
    };

    let sandbox_info = std::ptr::null_mut();
    let type_switch = CefString::from("type");
    let is_browser = cmd_line.has_switch(Some(&type_switch)) != 1;

    let mut app = TrashCefApp::new();
    let code = execute_process(Some(main_args), Some(&mut app), sandbox_info);

    if !is_browser {
        assert!(code >= 0, "CEF sub-process failed");
        return None;
    }
    assert_eq!(code, -1, "unexpected browser-process execute_process code");

    let cache_dir = cache_path();
    let _ = std::fs::create_dir_all(&cache_dir);

    let (tx, rx) = mpsc::channel();
    let (statistics_sink, statistics_receiver) = statistics_channel(4096);
    let service = Arc::new(
        AdBlockServiceBuilder::default()
            .statistics(statistics_sink)
            .build(),
    );
    let navigation = Arc::new(NavigationContextStore::default());
    let filter_updater =
        FilterListUpdater::start(cache_dir.join("adblock").join("lists"), service.clone());
    let database_path = cache_dir.join("adblock.dat");
    let recorder = match StatisticsRecorder::open(&database_path, statistics_receiver) {
        Ok(recorder) => {
            eprintln!(
                "[adblock] recording statistics in {}",
                recorder.database_path().display()
            );
            Some(recorder)
        }
        Err(error) => {
            eprintln!(
                "[adblock] failed to open statistics database {}: {error}",
                database_path.display()
            );
            None
        }
    };
    let _ = ADBLOCK.set(Arc::new(AdBlockRuntime {
        adapter: AdBlockAdapter::new(service.clone(), navigation.clone()),
        service,
        navigation,
        filter_updater: Mutex::new(filter_updater),
        recorder: Mutex::new(recorder),
        fallback_statistics: StatisticsSnapshot::unavailable(),
        last_pushed_statistics_version: AtomicU64::new(0),
        last_pushed_filter_version: AtomicU64::new(0),
    }));
    let _ = SHARED.set(Arc::new(Mutex::new(Shared {
        tx,
        browsers: HashMap::new(),
        identifier_to_tab: HashMap::new(),
        active_tab: None,
        to_create: VecDeque::new(),
        creating: VecDeque::new(),
        pending_nav: HashMap::new(),
        parent: None,
        #[cfg(any(target_os = "linux", target_os = "freebsd"))]
        host_xid: None,
        bounds: Rect {
            x: 0,
            y: 0,
            width: 1,
            height: 1,
        },
        start_uri: String::new(),
        context_ready: false,
        content_input_active: false,
    })));

    let settings = Settings {
        no_sandbox: 1,
        external_message_pump: 1,
        #[cfg(any(target_os = "linux", target_os = "freebsd"))]
        windowless_rendering_enabled: 1,
        root_cache_path: CefString::from(cache_dir.to_string_lossy().as_ref()),
        ..Default::default()
    };

    let ok = initialize(
        Some(main_args),
        Some(&settings),
        Some(&mut app),
        sandbox_info,
    );
    if ok != 1 {
        // Chromium's process singleton may refuse a second instance and ask the
        // existing one to focus — treat that as a clean exit, not a panic.
        let code = get_exit_code();
        eprintln!("[cef] initialize failed (ok={ok}, exit_code={code})");
        std::process::exit(if code != 0 { code } else { 1 });
    }

    // Keep the App wrapper alive for the process lifetime (CEF also refs it).
    std::mem::forget(app);

    let start_uri = shell_uri();
    if let Ok(mut s) = shared().lock() {
        s.start_uri = start_uri;
    }

    Some(rx)
}

pub fn shutdown_cef() {
    let browsers: Vec<Browser> = shared()
        .lock()
        .ok()
        .map(|mut s| s.browsers.drain().map(|(_, b)| b).collect())
        .unwrap_or_default();
    for browser in &browsers {
        if let Some(host) = browser.host() {
            host.close_browser(1);
        }
    }
    // Drain a few pump iterations so close completes.
    for _ in 0..20 {
        do_message_loop_work();
    }
    if let Some(runtime) = ADBLOCK.get()
        && let Ok(mut updater) = runtime.filter_updater.lock()
    {
        updater.shutdown();
    }
    if let Some(runtime) = ADBLOCK.get()
        && let Ok(mut recorder) = runtime.recorder.lock()
        && let Some(mut recorder) = recorder.take()
    {
        recorder.shutdown();
    }
    shutdown();
}

/// Drive CEF from the GPUI main thread.
pub fn pump(rx: &Receiver<CefEvent>, out: &mut Vec<CefEvent>) {
    do_message_loop_work();
    // Adblock stats are for the shell new-tab page only; check infrequently.
    static AD_TICK: AtomicU64 = AtomicU64::new(0);
    if AD_TICK.fetch_add(1, Ordering::Relaxed) % 8 == 0 {
        push_adblock_statistics(false);
    }
    loop {
        match rx.try_recv() {
            Ok(ev) => out.push(ev),
            Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => break,
        }
    }
}

/// Suggested delay until the next `pump` call (from CEF's external pump schedule).
pub fn next_pump_delay_ms() -> u64 {
    // Clamp so we stay responsive to input even when CEF asks for a long sleep.
    NEXT_PUMP_DELAY_MS
        .load(Ordering::Relaxed)
        .clamp(1, 16) as u64
}

static NEXT_PUMP_DELAY_MS: AtomicI64 = AtomicI64::new(10);

fn cef_debug() -> bool {
    static DEBUG: OnceLock<bool> = OnceLock::new();
    *DEBUG.get_or_init(|| std::env::var_os("TRASH_CEF_DEBUG").is_some())
}

macro_rules! cef_debug {
    ($($t:tt)*) => {
        if cef_debug() {
            eprintln!($($t)*);
        }
    };
}

fn push_adblock_statistics(force: bool) {
    let Some(runtime) = ADBLOCK.get() else {
        return;
    };
    let (version, statistics, error) = runtime
        .recorder
        .lock()
        .ok()
        .and_then(|recorder| recorder.as_ref().map(StatisticsRecorder::latest))
        .unwrap_or_else(|| (1, runtime.fallback_statistics.clone(), None));
    let (filter_version, filter_status) = runtime
        .filter_updater
        .lock()
        .map(|updater| updater.latest())
        .unwrap_or_else(|_| {
            (
                1,
                FilterListStatus::unavailable("filter updater lock poisoned"),
            )
        });
    if !force
        && runtime
            .last_pushed_statistics_version
            .load(Ordering::Acquire)
            == version
        && runtime.last_pushed_filter_version.load(Ordering::Acquire) == filter_version
    {
        return;
    }
    let browser = shared().lock().ok().and_then(|shared| shared.active_browser());
    let Some(frame) = browser.as_ref().and_then(Browser::main_frame) else {
        return;
    };
    let frame_url = CefString::from(&frame.url()).to_string();
    if !frame_url.starts_with("miko://shell/") {
        return;
    }
    let Ok(json) = statistics.to_json() else {
        return;
    };
    let Ok(filter_json) = filter_status.to_json() else {
        return;
    };
    if let Some(error) = error {
        eprintln!("[adblock] statistics recorder stopped: {error}");
    }
    let script = format!(
        "(() => {{ \
         const detail = {json}; \
         window.__trashAdblockStats = detail; \
         window.dispatchEvent(new CustomEvent('trash-adblock-stats', {{ detail }})); \
         const filterDetail = {filter_json}; \
         window.__trashAdblockStatus = filterDetail; \
         window.dispatchEvent(new CustomEvent('trash-adblock-status', {{ detail: filterDetail }})); \
         }})();"
    );
    frame.execute_java_script(
        Some(&CefString::from(script.as_str())),
        Some(&CefString::from("miko://shell/adblock-stats-bridge.js")),
        1,
    );
    runtime
        .last_pushed_statistics_version
        .store(version, Ordering::Release);
    runtime
        .last_pushed_filter_version
        .store(filter_version, Ordering::Release);
}

/// Compile complete filter-list texts outside the CEF IO callback and publish
/// the resulting engine atomically. Callers that handle large subscriptions
/// should invoke this function from their background worker.
#[allow(dead_code)] // Updater hook; subscription storage lands in a later phase.
pub fn replace_adblock_filter_lists(lists: Vec<String>) {
    let engine = BraveAdBlockEngine::from_filter_lists(lists);
    adblock_runtime()
        .service
        .replace_engine(trash_adblock::EngineSnapshot::new(engine));
}

/// Attach the content browsers under `parent_xid` and keep the active browser
/// sized to `bounds` (device pixels, relative to the parent window).
pub fn set_parent_bounds(parent_xid: u64, bounds: Rect) {
    let shared = shared();
    let browser_to_resize = {
        let Ok(mut s) = shared.lock() else { return };
        let bounds_changed = s.bounds.x != bounds.x
            || s.bounds.y != bounds.y
            || s.bounds.width != bounds.width
            || s.bounds.height != bounds.height;
        s.bounds = bounds.clone();
        #[cfg(any(target_os = "linux", target_os = "freebsd"))]
        if let Ok(mut cached) = CACHED_BOUNDS.lock() {
            *cached = bounds.clone();
        }

        if s.parent != Some(parent_xid) || bounds_changed {
            cef_debug!(
                "[cef] layout parent=0x{parent_xid:x} rect=({},{} {}x{})",
                bounds.x, bounds.y, bounds.width, bounds.height
            );
        }

        #[cfg(any(target_os = "linux", target_os = "freebsd"))]
        {
            if let Some(host) = ensure_linux_host_window(&mut s, parent_xid, &bounds) {
                let _ = host;
            }
        }

        if s.parent != Some(parent_xid) {
            s.parent = Some(parent_xid);
            drain_to_create_locked(&mut s);
            None
        } else if bounds_changed {
            s.active_browser()
        } else {
            None
        }
    };
    if let Some(browser) = browser_to_resize {
        resize_child(&browser, &bounds);
    }
}

/// Make `tab_id` the visible tab. Creates its browser on first activation
/// (loading `url`); on later activations it just reveals the existing browser
/// so the page keeps its state instead of reloading.
pub fn show_tab(tab_id: u64, url: &str) {
    let shared = shared();
    let (to_hide, to_show) = {
        let Ok(mut s) = shared.lock() else { return };
        if s.active_tab == Some(tab_id) && s.browsers.contains_key(&tab_id) {
            return;
        }
        let to_hide = s
            .active_tab
            .filter(|id| *id != tab_id)
            .and_then(|id| s.browsers.get(&id).cloned());
        s.active_tab = Some(tab_id);

        let to_show = s.browsers.get(&tab_id).cloned();
        if to_show.is_none()
            && !s.creating.iter().any(|p| p.tab_id == tab_id)
            && !s.to_create.iter().any(|p| p.tab_id == tab_id)
        {
            // First time we see this tab — queue a browser for it.
            s.to_create.push_back(PendingTab {
                tab_id,
                url: url.to_string(),
            });
            drain_to_create_locked(&mut s);
        }
        (to_hide, to_show)
    };
    // Touch CEF only after releasing the lock (callbacks re-enter this module).
    if let Some(browser) = to_hide {
        set_child_visible(&browser, false);
    }
    if let Some(browser) = to_show {
        let bounds = shared.lock().ok().map(|s| s.bounds.clone());
        if let Some(bounds) = bounds {
            resize_child(&browser, &bounds); // resize_child shows + repositions
        }
        focus_content();
    }
}

/// Close the browser backing `tab_id` (if any) and forget its bookkeeping.
pub fn close_tab(tab_id: u64) {
    let shared = shared();
    let browser = {
        let Ok(mut s) = shared.lock() else { return };
        s.to_create.retain(|p| p.tab_id != tab_id);
        s.creating.retain(|p| p.tab_id != tab_id);
        s.pending_nav.remove(&tab_id);
        if s.active_tab == Some(tab_id) {
            s.active_tab = None;
        }
        let browser = s.browsers.remove(&tab_id);
        if let Some(browser) = browser.as_ref() {
            s.identifier_to_tab.remove(&browser.identifier());
        }
        browser
    };
    if let Some(host) = browser.as_ref().and_then(Browser::host) {
        host.close_browser(1);
    }
}

pub fn navigate(url: &str) {
    let shared = shared();
    let frame = {
        let Ok(mut s) = shared.lock() else { return };
        let Some(tab_id) = s.active_tab else { return };
        match s.browsers.get(&tab_id).and_then(Browser::main_frame) {
            Some(frame) => Some(frame),
            None => {
                // Browser not created yet — apply the load once it exists.
                s.pending_nav.insert(tab_id, url.to_string());
                None
            }
        }
    };
    if let Some(frame) = frame {
        frame.load_url(Some(&CefString::from(url)));
    }
}

/// Pull Win32 keyboard focus back to the GPUI chrome window. The CEF content
/// browser is a native child HWND that otherwise keeps OS keyboard focus, so
/// key events never reach GPUI and the omnibox can't be typed into.
#[cfg(target_os = "windows")]
pub fn focus_chrome() {
    unsafe extern "system" {
        fn SetFocus(hwnd: isize) -> isize;
    }
    if let Ok(mut s) = shared().lock() {
        s.content_input_active = false;
    }
    let parent = shared().lock().ok().and_then(|s| s.parent);
    if let Some(parent) = parent {
        unsafe { SetFocus(parent as isize) };
    }
}

#[cfg(not(target_os = "windows"))]
pub fn focus_chrome() {
    if let Ok(mut s) = shared().lock() {
        s.content_input_active = false;
    }
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    retain_chrome_os_focus();
}

/// Hand keyboard focus to the active CEF content browser (page becomes typeable).
pub fn focus_content() {
    if let Ok(mut s) = shared().lock() {
        s.content_input_active = true;
    }
    let browser = shared().lock().ok().and_then(|s| s.active_browser());
    let Some(host) = browser.as_ref().and_then(Browser::host) else {
        return;
    };
    // Tell CEF the browser is focused, then move Win32 focus onto the child so
    // OS key events actually reach Chromium (set_focus alone is not enough when
    // the GPUI parent HWND currently owns keyboard focus).
    host.set_focus(1);
    #[cfg(target_os = "windows")]
    {
        unsafe extern "system" {
            fn SetFocus(hwnd: isize) -> isize;
        }
        let hwnd = host.window_handle();
        if !hwnd.0.is_null() {
            unsafe { SetFocus(hwnd.0 as isize) };
        }
    }
    // Linux OSR: keep OS focus on the GPUI parent so key events still arrive in
    // GPUI and can be forwarded; the host X11 paint window must not steal focus.
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    retain_chrome_os_focus();
}

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
fn retain_chrome_os_focus() {
    let display = get_xdisplay();
    if display.is_null() {
        return;
    }
    let Ok(xlib) = x11_dl::xlib::Xlib::open() else {
        return;
    };
    let parent = shared().lock().ok().and_then(|s| s.parent);
    let Some(parent) = parent else { return };
    unsafe {
        // RevertToParent = 2 — keep focus on the GPUI chrome window.
        (xlib.XSetInputFocus)(display as *mut _, parent, 2, x11_dl::xlib::CurrentTime);
        (xlib.XFlush)(display as *mut _);
    }
}

/// Forward a GPUI key-down into the active CEF browser (needed for Linux OSR).
pub fn send_key_down(event: &gpui::KeyDownEvent) {
    #[cfg(target_os = "windows")]
    {
        let _ = event;
    }
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    {
        send_key_inner(&event.keystroke, KeyPhase::Down, event.prefer_character_input);
    }
}

/// Forward a GPUI key-up into the active CEF browser (needed for Linux OSR).
pub fn send_key_up(event: &gpui::KeyUpEvent) {
    #[cfg(target_os = "windows")]
    {
        let _ = event;
    }
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    {
        send_key_inner(&event.keystroke, KeyPhase::Up, false);
    }
}

/// Forward a GPUI mouse-down into the active CEF browser (needed for Linux OSR).
pub fn send_mouse_click(event: &gpui::MouseDownEvent, window: &gpui::Window, _unused: bool) {
    send_mouse_click_inner(
        event.button,
        event.position,
        event.modifiers,
        event.click_count,
        false,
        window,
    );
}

/// Forward a GPUI mouse-up into the active CEF browser (needed for Linux OSR).
pub fn send_mouse_click_up(event: &gpui::MouseUpEvent, window: &gpui::Window) {
    send_mouse_click_inner(
        event.button,
        event.position,
        event.modifiers,
        event.click_count,
        true,
        window,
    );
}

/// Forward mouse movement into CEF (Linux OSR).
pub fn send_mouse_move(event: &gpui::MouseMoveEvent, window: &gpui::Window) {
    #[cfg(target_os = "windows")]
    {
        let _ = (event, window);
        return;
    }
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    {
        let Some((x, y, inside, mut modifiers)) = view_mouse_point(event.position, window) else {
            return;
        };
        // Drop duplicate moves — GPUI can spam identical positions.
        if event.pressed_button.is_none()
            && let Ok(last) = LAST_MOUSE.lock()
            && last.0 == x
            && last.1 == y
            && last.2 == inside
        {
            return;
        }
        if let Ok(mut last) = LAST_MOUSE.lock() {
            *last = (x, y, inside);
        }
        if let Some(button) = event.pressed_button {
            modifiers |= mouse_button_flag(button);
        }
        modifiers |= modifier_flags(event.modifiers);
        with_active_host(|host| {
            let ev = MouseEvent { x, y, modifiers };
            host.send_mouse_move_event(Some(&ev), if inside { 0 } else { 1 });
        });
    }
}

/// Forward scroll wheel into CEF (Linux OSR).
pub fn send_mouse_wheel(event: &gpui::ScrollWheelEvent, window: &gpui::Window) {
    #[cfg(target_os = "windows")]
    {
        let _ = (event, window);
        return;
    }
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    {
        let Some((x, y, inside, mut modifiers)) = view_mouse_point(event.position, window) else {
            return;
        };
        if !inside {
            return;
        }
        modifiers |= modifier_flags(event.modifiers);
        let scale = window.scale_factor();
        let (dx, dy) = match event.delta {
            gpui::ScrollDelta::Pixels(pt) => (
                (f32::from(pt.x) * scale).round() as i32,
                (f32::from(pt.y) * scale).round() as i32,
            ),
            gpui::ScrollDelta::Lines(pt) => {
                // Chromium expects pixel deltas; approximate a line as 40 device px.
                (
                    (pt.x * 40.0 * scale).round() as i32,
                    (pt.y * 40.0 * scale).round() as i32,
                )
            }
        };
        // Pass GPUI's sign through unchanged (negating dy scrolled backwards).
        with_active_host(|host| {
            let ev = MouseEvent { x, y, modifiers };
            host.send_mouse_wheel_event(Some(&ev), dx, dy);
        });
    }
}

fn send_mouse_click_inner(
    button: gpui::MouseButton,
    position: gpui::Point<gpui::Pixels>,
    modifiers: gpui::Modifiers,
    click_count: usize,
    mouse_up: bool,
    window: &gpui::Window,
) {
    #[cfg(target_os = "windows")]
    {
        let _ = (button, position, modifiers, click_count, mouse_up, window);
        return;
    }
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    {
        let Some((x, y, inside, mut mods)) = view_mouse_point(position, window) else {
            return;
        };
        if !inside && !mouse_up {
            return;
        }
        mods |= modifier_flags(modifiers);
        // On mouse-down the pressed button is included; on mouse-up it is not.
        if !mouse_up {
            mods |= mouse_button_flag(button);
        }
        let Some(btn) = cef_mouse_button(button) else {
            return;
        };
        with_active_host(|host| {
            let ev = MouseEvent {
                x,
                y,
                modifiers: mods,
            };
            // CEF hit-testing expects a move to the click point first.
            if !mouse_up {
                host.send_mouse_move_event(Some(&ev), 0);
                retain_chrome_os_focus();
            }
            host.send_mouse_click_event(
                Some(&ev),
                btn,
                if mouse_up { 1 } else { 0 },
                click_count.max(1) as i32,
            );
        });
    }
}

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
static LAST_MOUSE: Mutex<(i32, i32, bool)> = Mutex::new((i32::MIN, i32::MIN, false));

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
static CACHED_BOUNDS: Mutex<Rect> = Mutex::new(Rect {
    x: 0,
    y: 0,
    width: 1,
    height: 1,
});

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
fn with_active_host(f: impl FnOnce(&BrowserHost)) {
    let browser = shared().lock().ok().and_then(|s| s.active_browser());
    let Some(host) = browser.as_ref().and_then(Browser::host) else {
        return;
    };
    f(&host);
}

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
fn view_mouse_point(
    position: gpui::Point<gpui::Pixels>,
    window: &gpui::Window,
) -> Option<(i32, i32, bool, u32)> {
    let scale = window.scale_factor();
    let device_x = (f32::from(position.x) * scale).round() as i32;
    let device_y = (f32::from(position.y) * scale).round() as i32;
    let bounds = CACHED_BOUNDS
        .lock()
        .ok()
        .map(|b| (*b).clone())
        .or_else(|| shared().lock().ok().map(|s| s.bounds.clone()))?;
    let x = device_x - bounds.x;
    let y = device_y - bounds.y;
    let inside = x >= 0 && y >= 0 && x < bounds.width && y < bounds.height;
    let x = x.clamp(0, bounds.width.saturating_sub(1).max(0));
    let y = y.clamp(0, bounds.height.saturating_sub(1).max(0));
    Some((x, y, inside, 0))
}

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
fn cef_mouse_button(button: gpui::MouseButton) -> Option<MouseButtonType> {
    match button {
        gpui::MouseButton::Left => Some(MouseButtonType::LEFT),
        gpui::MouseButton::Right => Some(MouseButtonType::RIGHT),
        gpui::MouseButton::Middle => Some(MouseButtonType::MIDDLE),
        _ => None,
    }
}

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
fn mouse_button_flag(button: gpui::MouseButton) -> u32 {
    match button {
        gpui::MouseButton::Left => sys::cef_event_flags_t::EVENTFLAG_LEFT_MOUSE_BUTTON.0,
        gpui::MouseButton::Middle => sys::cef_event_flags_t::EVENTFLAG_MIDDLE_MOUSE_BUTTON.0,
        gpui::MouseButton::Right => sys::cef_event_flags_t::EVENTFLAG_RIGHT_MOUSE_BUTTON.0,
        _ => 0,
    }
}

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
fn modifier_flags(modifiers: gpui::Modifiers) -> u32 {
    let mut flags = 0u32;
    if modifiers.shift {
        flags |= sys::cef_event_flags_t::EVENTFLAG_SHIFT_DOWN.0;
    }
    if modifiers.control {
        flags |= sys::cef_event_flags_t::EVENTFLAG_CONTROL_DOWN.0;
    }
    if modifiers.alt {
        flags |= sys::cef_event_flags_t::EVENTFLAG_ALT_DOWN.0;
    }
    if modifiers.platform {
        flags |= sys::cef_event_flags_t::EVENTFLAG_COMMAND_DOWN.0;
    }
    flags
}

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
enum KeyPhase {
    Down,
    Up,
}

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
fn send_key_inner(keystroke: &gpui::Keystroke, phase: KeyPhase, prefer_character_input: bool) {
    let Some((windows_key_code, mut character)) = map_windows_key(keystroke) else {
        return;
    };
    // Shortcuts (Ctrl+Backspace, Ctrl+A, …) must not carry a character or Blink
    // treats them as text insertion instead of editing commands.
    let shortcut = (keystroke.modifiers.control || keystroke.modifiers.platform)
        && !prefer_character_input;
    let alt_menu =
        keystroke.modifiers.alt && !prefer_character_input && !keystroke.modifiers.control;
    if shortcut || alt_menu {
        character = 0;
    }

    let modifiers = modifier_flags(keystroke.modifiers);
    let is_system_key = i32::from(keystroke.modifiers.alt && !keystroke.modifiers.control);
    let base = KeyEvent {
        size: std::mem::size_of::<sys::cef_key_event_t>(),
        type_: KeyEventType::RAWKEYDOWN,
        modifiers,
        windows_key_code,
        native_key_code: windows_key_code,
        is_system_key,
        character,
        unmodified_character: character,
        focus_on_editable_field: 0,
    };
    with_active_host(|host| match phase {
        KeyPhase::Down => {
            let mut down = base.clone();
            down.type_ = KeyEventType::RAWKEYDOWN;
            host.send_key_event(Some(&down));

            // Editing shortcuts need KEYDOWN as well on Linux OSR.
            if shortcut {
                let mut keydown = base.clone();
                keydown.type_ = KeyEventType::KEYDOWN;
                host.send_key_event(Some(&keydown));
            }

            if character != 0 && !shortcut && !alt_menu {
                let mut ch = base.clone();
                ch.type_ = KeyEventType::CHAR;
                // CHAR events use the character as windows_key_code on Linux OSR.
                ch.windows_key_code = character as i32;
                if windows_key_code == 0x0D {
                    ch.character = b'\r' as u16;
                    ch.unmodified_character = b'\r' as u16;
                    ch.windows_key_code = b'\r' as i32;
                }
                host.send_key_event(Some(&ch));
            }
        }
        KeyPhase::Up => {
            let mut up = base;
            up.type_ = KeyEventType::KEYUP;
            host.send_key_event(Some(&up));
        }
    });
}

/// Map a GPUI keystroke to a Windows VK code + UTF-16 character (0 = non-char).
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
fn map_windows_key(keystroke: &gpui::Keystroke) -> Option<(i32, u16)> {
    let key = keystroke.key.as_str();
    let from_name = match key {
        "enter" => Some((0x0D, b'\r' as u16)),
        "backspace" => Some((0x08, 0x08)),
        "tab" => Some((0x09, b'\t' as u16)),
        "escape" => Some((0x1B, 0x1B)),
        "space" => Some((0x20, b' ' as u16)),
        "pageup" => Some((0x21, 0)),
        "pagedown" => Some((0x22, 0)),
        "end" => Some((0x23, 0)),
        "home" => Some((0x24, 0)),
        "left" => Some((0x25, 0)),
        "up" => Some((0x26, 0)),
        "right" => Some((0x27, 0)),
        "down" => Some((0x28, 0)),
        "insert" => Some((0x2D, 0)),
        "delete" => Some((0x2E, 0x7F)),
        "f1" => Some((0x70, 0)),
        "f2" => Some((0x71, 0)),
        "f3" => Some((0x72, 0)),
        "f4" => Some((0x73, 0)),
        "f5" => Some((0x74, 0)),
        "f6" => Some((0x75, 0)),
        "f7" => Some((0x76, 0)),
        "f8" => Some((0x77, 0)),
        "f9" => Some((0x78, 0)),
        "f10" => Some((0x79, 0)),
        "f11" => Some((0x7A, 0)),
        "f12" => Some((0x7B, 0)),
        _ => None,
    };
    if let Some(mapped) = from_name {
        return Some(mapped);
    }

    if let Some(text) = keystroke.key_char.as_deref() {
        let mut chars = text.chars();
        if let Some(ch) = chars.next()
            && chars.next().is_none()
        {
            let utf16 = ch as u32;
            if utf16 <= 0xFFFF {
                let vk = if ch.is_ascii_alphabetic() {
                    ch.to_ascii_uppercase() as i32
                } else if ch.is_ascii() {
                    // Digits / punctuation: VK matches the unshifted key when possible.
                    keystroke
                        .key
                        .chars()
                        .next()
                        .filter(|c| c.is_ascii())
                        .map(|c| {
                            if c.is_ascii_alphabetic() {
                                c.to_ascii_uppercase() as i32
                            } else {
                                c as i32
                            }
                        })
                        .unwrap_or(ch as i32)
                } else {
                    0
                };
                return Some((vk, utf16 as u16));
            }
        }
    }

    // Fallback: single-key labels like "a", "1", "," without key_char.
    let mut chars = key.chars();
    if let Some(ch) = chars.next()
        && chars.next().is_none()
        && ch.is_ascii()
    {
        let vk = if ch.is_ascii_alphabetic() {
            ch.to_ascii_uppercase() as i32
        } else {
            ch as i32
        };
        let character = if keystroke.modifiers.control
            || keystroke.modifiers.alt
            || keystroke.modifiers.platform
        {
            0
        } else {
            ch as u16
        };
        return Some((vk, character));
    }
    None
}

pub fn go_back() {
    let browser = shared().lock().ok().and_then(|s| s.active_browser());
    if let Some(browser) = browser {
        browser.go_back();
    }
}

pub fn go_forward() {
    let browser = shared().lock().ok().and_then(|s| s.active_browser());
    if let Some(browser) = browser {
        browser.go_forward();
    }
}

pub fn reload() {
    let browser = shared().lock().ok().and_then(|s| s.active_browser());
    if let Some(browser) = browser {
        browser.reload();
    }
}

pub fn stop_load() {
    let browser = shared().lock().ok().and_then(|s| s.active_browser());
    if let Some(browser) = browser {
        browser.stop_load();
    }
}

pub fn start_uri() -> String {
    shared()
        .lock()
        .map(|s| s.start_uri.clone())
        .unwrap_or_default()
}

/// Close every content browser (app is quitting).
pub fn close_browser() {
    let browsers: Vec<Browser> = shared()
        .lock()
        .ok()
        .map(|mut s| {
            s.active_tab = None;
            s.identifier_to_tab.clear();
            s.browsers.drain().map(|(_, b)| b).collect()
        })
        .unwrap_or_default();
    for browser in &browsers {
        if let Some(host) = browser.host() {
            host.close_browser(1);
        }
    }
}

fn emit(ev: CefEvent) {
    let tx = shared().lock().ok().map(|s| s.tx.clone());
    if let Some(tx) = tx {
        let _ = tx.send(ev);
    }
}

/// Resolve the GPUI tab id that owns `browser`, if it is a tracked content
/// browser. Callbacks from a browser mid-teardown return `None`.
fn tab_of(browser: Option<&Browser>) -> Option<u64> {
    let identifier = browser?.identifier();
    shared()
        .lock()
        .ok()
        .and_then(|s| s.identifier_to_tab.get(&identifier).copied())
}

/// Issue `browser_host_create_browser` for every queued tab, once the CEF
/// context and native parent are both ready. Each request moves to `creating`
/// so `on_after_created` can correlate the new browser (FIFO) back to its tab.
fn drain_to_create_locked(s: &mut Shared) {
    let Some(parent_raw) = s.parent else { return };
    if !s.context_ready {
        return;
    }

    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    let parent = {
        let bounds = s.bounds.clone();
        let Some(host) = ensure_linux_host_window(s, parent_raw, &bounds) else {
            eprintln!("[cef] linux host window unavailable; deferring create");
            return;
        };
        host as cef::sys::cef_window_handle_t
    };
    #[cfg(target_os = "windows")]
    let parent = cef::sys::HWND(parent_raw as usize as *mut _);

    // Relative to the Linux host window (which already sits at content bounds).
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    let child_bounds = Rect {
        x: 0,
        y: 0,
        width: s.bounds.width.max(1),
        height: s.bounds.height.max(1),
    };
    #[cfg(target_os = "windows")]
    let child_bounds = s.bounds.clone();

    while let Some(pending) = s.to_create.pop_front() {
        eprintln!(
            "[cef] create tab={} gpui=0x{parent_raw:x} host=0x{parent:x} rect=({},{} {}x{}) url={}",
            pending.tab_id,
            child_bounds.x,
            child_bounds.y,
            child_bounds.width,
            child_bounds.height,
            pending.url
        );

        #[cfg(any(target_os = "linux", target_os = "freebsd"))]
        let window_info = WindowInfo::default().set_as_windowless(parent);
        #[cfg(target_os = "windows")]
        let window_info = WindowInfo {
            runtime_style: RuntimeStyle::ALLOY,
            ..Default::default()
        }
        .set_as_child(parent, &child_bounds);

        let mut client = ContentClient::new();
        #[cfg(any(target_os = "linux", target_os = "freebsd"))]
        let settings = {
            // Opaque background so OSR doesn't stay transparent/black forever.
            let mut s = BrowserSettings::default();
            s.background_color = 0xff_0f_0f_10;
            s
        };
        #[cfg(target_os = "windows")]
        let settings = BrowserSettings::default();
        let ok = browser_host_create_browser(
            Some(&window_info),
            Some(&mut client),
            Some(&CefString::from(pending.url.as_str())),
            Some(&settings),
            None,
            None,
        );
        if ok == 0 {
            eprintln!("[cef] browser_host_create_browser failed (tab={})", pending.tab_id);
            continue;
        }
        s.creating.push_back(pending);
    }
}

/// Create (or resize) a plain X11 child under the GPUI window for CEF to parent
/// into. Returns the host xid.
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
fn ensure_linux_host_window(s: &mut Shared, parent_xid: u64, bounds: &Rect) -> Option<u64> {
    let display = get_xdisplay();
    if display.is_null() {
        eprintln!("[cef] ensure_linux_host: cef_get_xdisplay null");
        return None;
    }
    let Ok(xlib) = x11_dl::xlib::Xlib::open() else {
        eprintln!("[cef] ensure_linux_host: libX11 open failed");
        return None;
    };

    let width = bounds.width.max(1) as u32;
    let height = bounds.height.max(1) as u32;

    if let Some(host) = s.host_xid {
        unsafe {
            (xlib.XMoveResizeWindow)(
                display as *mut _,
                host,
                bounds.x,
                bounds.y,
                width,
                height,
            );
            (xlib.XMapRaised)(display as *mut _, host);
            (xlib.XFlush)(display as *mut _);
        }
        return Some(host);
    }

    let host = unsafe {
        let screen = (xlib.XDefaultScreen)(display as *mut _);
        let black = (xlib.XBlackPixel)(display as *mut _, screen);
        // Dark fill matching the GPUI placeholder until CEF paints.
        let background = 0x000f0f10_u64;
        let win = (xlib.XCreateSimpleWindow)(
            display as *mut _,
            parent_xid,
            bounds.x,
            bounds.y,
            width,
            height,
            0,
            black,
            background,
        );
        if win == 0 {
            eprintln!("[cef] ensure_linux_host: XCreateSimpleWindow failed");
            return None;
        }
        // Paint-only child: never take keyboard focus (GPUI must keep OS focus
        // so OSR key/mouse forwarding keeps working).
        let mut hints: x11_dl::xlib::XWMHints = std::mem::zeroed();
        hints.flags = x11_dl::xlib::InputHint;
        hints.input = 0;
        (xlib.XSetWMHints)(display as *mut _, win, &mut hints);
        (xlib.XSelectInput)(
            display as *mut _,
            win,
            x11_dl::xlib::ExposureMask | x11_dl::xlib::StructureNotifyMask,
        );
        (xlib.XMapRaised)(display as *mut _, win);
        (xlib.XFlush)(display as *mut _);
        win
    };
    eprintln!("[cef] linux host window=0x{host:x} under parent=0x{parent_xid:x}");
    s.host_xid = Some(host);
    OSR_HOST_XID.store(host, Ordering::Relaxed);
    // Drop cached GC/visual so the next paint rebinds to the new host.
    if let Ok(mut target) = OSR_TARGET.lock() {
        *target = None;
    }
    Some(host)
}

/// Show or hide a content browser's native child window.
#[cfg(target_os = "windows")]
fn set_child_visible(browser: &Browser, visible: bool) {
    const SW_HIDE: i32 = 0;
    const SW_SHOWNA: i32 = 8; // show without activating (keeps chrome focus rules)
    unsafe extern "system" {
        fn ShowWindow(hwnd: isize, cmd: i32) -> i32;
    }
    let Some(host) = browser.host() else { return };
    let hwnd = host.window_handle();
    if hwnd.0.is_null() {
        return;
    }
    unsafe { ShowWindow(hwnd.0 as isize, if visible { SW_SHOWNA } else { SW_HIDE }) };
}

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
fn resize_child(browser: &Browser, bounds: &Rect) {
    // OSR: only the host X11 window is real; tell CEF the view size changed.
    if let Ok(mut s) = shared().lock()
        && let Some(parent) = s.parent
    {
        let _ = ensure_linux_host_window(&mut s, parent, bounds);
    }
    if let Some(host) = browser.host() {
        host.was_resized();
        host.invalidate(PaintElementType::VIEW);
    }
}

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
fn set_child_visible(browser: &Browser, visible: bool) {
    let display = get_xdisplay();
    if display.is_null() {
        return;
    }
    let Ok(xlib) = x11_dl::xlib::Xlib::open() else {
        return;
    };
    let host_xid = shared().lock().ok().and_then(|s| s.host_xid);
    let Some(host_xid) = host_xid else { return };
    unsafe {
        if visible {
            (xlib.XMapRaised)(display as *mut _, host_xid);
        } else {
            (xlib.XUnmapWindow)(display as *mut _, host_xid);
        }
        (xlib.XFlush)(display as *mut _);
    }
    let _ = browser;
}

#[cfg(target_os = "windows")]
fn resize_child(browser: &Browser, bounds: &Rect) {
    const SWP_NOACTIVATE: u32 = 0x0010;
    const SWP_SHOWWINDOW: u32 = 0x0040;

    unsafe extern "system" {
        fn SetWindowPos(
            hwnd: isize,
            hwnd_insert_after: isize,
            x: i32,
            y: i32,
            width: i32,
            height: i32,
            flags: u32,
        ) -> i32;
    }

    let Some(host) = browser.host() else { return };
    let hwnd = host.window_handle();
    if hwnd.0.is_null() {
        return;
    }
    let ok = unsafe {
        SetWindowPos(
            hwnd.0 as isize,
            0,
            bounds.x,
            bounds.y,
            bounds.width.max(1),
            bounds.height.max(1),
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        )
    };
    if ok == 0 {
        eprintln!("[cef] SetWindowPos failed hwnd={:?}", hwnd.0);
    }
    log_windows_browser("resize", browser);
    host.notify_move_or_resize_started();
}

#[cfg(target_os = "windows")]
fn log_windows_browser(event: &str, browser: &Browser) {
    #[repr(C)]
    struct WinRect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }
    unsafe extern "system" {
        fn GetParent(hwnd: isize) -> isize;
        fn GetClientRect(hwnd: isize, rect: *mut WinRect) -> i32;
        fn IsWindowVisible(hwnd: isize) -> i32;
    }

    let Some(host) = browser.host() else { return };
    let hwnd = host.window_handle();
    if hwnd.0.is_null() {
        eprintln!("[cef] {event} id={} hwnd=null", browser.identifier());
        return;
    }
    let raw = hwnd.0 as isize;
    let mut rect = WinRect {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    unsafe { GetClientRect(raw, &mut rect) };
    eprintln!(
        "[cef] {event} id={} hwnd=0x{raw:x} parent=0x{:x} visible={} client={}x{}",
        browser.identifier(),
        unsafe { GetParent(raw) },
        unsafe { IsWindowVisible(raw) },
        rect.right - rect.left,
        rect.bottom - rect.top,
    );
}

fn cache_path() -> std::path::PathBuf {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".cache")))
        .unwrap_or_else(|| std::env::temp_dir());
    base.join("trashbrowser-gpui")
}

const SHELL_HTML: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/shell/dist/index.html"
));

fn shell_uri() -> String {
    "miko://shell/#/new-tab".to_string()
}

wrap_scheme_handler_factory! {
    struct MikoSchemeHandlerFactory;

    impl SchemeHandlerFactory {
        fn create(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            _scheme_name: Option<&CefString>,
            request: Option<&mut Request>,
        ) -> Option<ResourceHandler> {
            let url = CefString::from(&request?.url()).to_string();
            eprintln!("[cef] scheme-request url={url}");
            if !url.starts_with("miko://shell/") {
                return None;
            }
            let stream = stream_reader_create_for_data(
                SHELL_HTML.as_ptr() as *mut u8,
                SHELL_HTML.len(),
            )?;
            Some(cef::wrapper::stream_resource_handler::StreamResourceHandler::new_with_stream(
                // CEF's set_mime_type expects only the media type. Including a
                // charset parameter makes Chromium fall back to text/plain.
                "text/html".to_string(),
                stream,
            ))
        }
    }
}

// ---- CEF App / Client ----------------------------------------------------

wrap_app! {
    struct TrashCefApp;

    impl App {
        fn browser_process_handler(&self) -> Option<BrowserProcessHandler> {
            Some(TrashBrowserProcessHandler::new())
        }

        fn on_register_custom_schemes(&self, registrar: Option<&mut SchemeRegistrar>) {
            const OPTIONS: i32 = sys::cef_scheme_options_t::CEF_SCHEME_OPTION_STANDARD as i32
                | sys::cef_scheme_options_t::CEF_SCHEME_OPTION_LOCAL as i32
                | sys::cef_scheme_options_t::CEF_SCHEME_OPTION_SECURE as i32
                | sys::cef_scheme_options_t::CEF_SCHEME_OPTION_CORS_ENABLED as i32;
            if let Some(registrar) = registrar {
                let ok = registrar.add_custom_scheme(Some(&CefString::from("miko")), OPTIONS);
                assert_eq!(ok, 1, "failed to register miko scheme");
            }
        }

        fn on_before_command_line_processing(
            &self,
            _process_type: Option<&CefString>,
            command_line: Option<&mut CommandLine>,
        ) {
            #[cfg(any(target_os = "linux", target_os = "freebsd"))]
            if let Some(command_line) = command_line {
                let ozone = CefString::from("ozone-platform");
                if command_line.has_switch(Some(&ozone)) == 0 {
                    command_line
                        .append_switch_with_value(Some(&ozone), Some(&CefString::from("x11")));
                }
                // Windowless (OSR) + software paint into our X11 host window.
                // Avoids fighting GPUI's Vulkan surface and the broken Alloy
                // native-child path under Xwayland (invalid Ozone xids).
                for switch in ["disable-gpu", "disable-gpu-compositing", "disable-gpu-sandbox"] {
                    let name = CefString::from(switch);
                    if command_line.has_switch(Some(&name)) == 0 {
                        command_line.append_switch(Some(&name));
                    }
                }
            }
            #[cfg(target_os = "windows")]
            let _ = command_line;
        }
    }
}

wrap_browser_process_handler! {
    struct TrashBrowserProcessHandler;

    impl BrowserProcessHandler {
        fn on_context_initialized(&self) {
            let mut factory = MikoSchemeHandlerFactory::new();
            let registered = register_scheme_handler_factory(
                Some(&CefString::from("miko")),
                Some(&CefString::from("shell")),
                Some(&mut factory),
            );
            assert_eq!(registered, 1, "failed to install miko scheme handler");

            if let Ok(mut s) = shared().lock() {
                s.context_ready = true;
                let _ = s.tx.send(CefEvent::Ready);
                drain_to_create_locked(&mut s);
            }
        }

        fn on_schedule_message_pump_work(&self, delay_ms: i64) {
            NEXT_PUMP_DELAY_MS.store(delay_ms.max(0), Ordering::Relaxed);
        }
    }
}

wrap_client! {
    struct ContentClient;

    impl Client {
        fn life_span_handler(&self) -> Option<LifeSpanHandler> {
            Some(ContentLifeSpan::new())
        }
        fn load_handler(&self) -> Option<LoadHandler> {
            Some(ContentLoadHandler::new())
        }
        fn display_handler(&self) -> Option<DisplayHandler> {
            Some(ContentDisplayHandler::new())
        }
        fn request_handler(&self) -> Option<RequestHandler> {
            Some(ContentRequestHandler::new())
        }
        fn focus_handler(&self) -> Option<FocusHandler> {
            Some(ContentFocusHandler::new())
        }
        fn render_handler(&self) -> Option<RenderHandler> {
            linux_render_handler()
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
fn linux_render_handler() -> Option<RenderHandler> {
    Some(ContentRenderHandler::new())
}

#[cfg(not(any(target_os = "linux", target_os = "freebsd")))]
fn linux_render_handler() -> Option<RenderHandler> {
    None
}

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
wrap_render_handler! {
    struct ContentRenderHandler;

    impl RenderHandler {
        fn view_rect(&self, _browser: Option<&mut Browser>, rect: Option<&mut Rect>) {
            let Some(rect) = rect else { return };
            let bounds = CACHED_BOUNDS
                .lock()
                .ok()
                .map(|b| (*b).clone())
                .unwrap_or(Rect {
                    x: 0,
                    y: 0,
                    width: 1,
                    height: 1,
                });
            rect.x = 0;
            rect.y = 0;
            rect.width = bounds.width.max(1);
            rect.height = bounds.height.max(1);
        }

        fn on_paint(
            &self,
            _browser: Option<&mut Browser>,
            type_: PaintElementType,
            dirty_rects: Option<&[Rect]>,
            buffer: *const u8,
            width: ::std::os::raw::c_int,
            height: ::std::os::raw::c_int,
        ) {
            if type_ != PaintElementType::VIEW || buffer.is_null() || width <= 0 || height <= 0 {
                return;
            }
            static PAINTS: AtomicU64 = AtomicU64::new(0);
            let n = PAINTS.fetch_add(1, Ordering::Relaxed);
            if n == 0 {
                eprintln!("[cef] osr-paint first frame {width}x{height}");
            }
            let host_xid = OSR_HOST_XID.load(Ordering::Relaxed);
            let host_xid = if host_xid != 0 {
                host_xid
            } else {
                let xid = shared().lock().ok().and_then(|s| s.host_xid).unwrap_or(0);
                if xid != 0 {
                    OSR_HOST_XID.store(xid, Ordering::Relaxed);
                }
                xid
            };
            if host_xid == 0 {
                return;
            }
            paint_host_window(
                host_xid,
                buffer,
                width as u32,
                height as u32,
                dirty_rects,
            );
        }
    }
}

/// Cached X11 blit targets so OSR paints avoid per-frame Xlib open / attrs / GC.
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
struct OsrPaintTarget {
    host_xid: u64,
    visual: *mut x11_dl::xlib::Visual,
    depth: u32,
    gc: x11_dl::xlib::GC,
}

// SAFETY: CEF paints on the browser UI thread; we only touch this from that path.
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
unsafe impl Send for OsrPaintTarget {}
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
unsafe impl Sync for OsrPaintTarget {}

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
static OSR_XLIB: OnceLock<x11_dl::xlib::Xlib> = OnceLock::new();
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
static OSR_TARGET: Mutex<Option<OsrPaintTarget>> = Mutex::new(None);
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
static OSR_PAINT_N: AtomicU64 = AtomicU64::new(0);
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
static OSR_HOST_XID: AtomicU64 = AtomicU64::new(0);

/// Blit a CEF OSR BGRA frame into our plain X11 host window.
///
/// Zero-copy: wrap CEF's paint buffer in an `XImage` for the duration of this
/// callback (valid until `on_paint` returns) and only push dirty rectangles.
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
fn paint_host_window(
    host_xid: u64,
    buffer: *const u8,
    width: u32,
    height: u32,
    dirty_rects: Option<&[Rect]>,
) {
    let display = get_xdisplay();
    if display.is_null() || width == 0 || height == 0 {
        return;
    }
    let xlib = OSR_XLIB.get_or_init(|| {
        x11_dl::xlib::Xlib::open().expect("libX11 required for OSR paint")
    });

    let mut guard = match OSR_TARGET.lock() {
        Ok(g) => g,
        Err(_) => return,
    };
    let needs_init = guard.as_ref().is_none_or(|t| t.host_xid != host_xid);
    if needs_init {
        unsafe {
            if let Some(old) = guard.take() {
                (xlib.XFreeGC)(display as *mut _, old.gc);
            }
            let mut attrs: x11_dl::xlib::XWindowAttributes = std::mem::zeroed();
            if (xlib.XGetWindowAttributes)(display as *mut _, host_xid, &mut attrs) == 0 {
                eprintln!("[cef] osr XGetWindowAttributes failed for host=0x{host_xid:x}");
                return;
            }
            let visual = attrs.visual;
            let depth = attrs.depth as u32;
            if visual.is_null() || depth == 0 {
                eprintln!("[cef] osr host visual/depth invalid");
                return;
            }
            let gc = (xlib.XCreateGC)(display as *mut _, host_xid, 0, std::ptr::null_mut());
            if gc.is_null() {
                eprintln!("[cef] osr XCreateGC failed");
                return;
            }
            *guard = Some(OsrPaintTarget {
                host_xid,
                visual,
                depth,
                gc,
            });
        }
    }
    let Some(target) = guard.as_ref() else {
        return;
    };

    unsafe {
        let mut image: x11_dl::xlib::XImage = std::mem::zeroed();
        image.width = width as i32;
        image.height = height as i32;
        image.xoffset = 0;
        image.format = x11_dl::xlib::ZPixmap;
        // CEF buffer is valid for this callback only — never XDestroyImage.
        image.data = buffer as *mut i8;
        image.byte_order = x11_dl::xlib::LSBFirst;
        image.bitmap_unit = 32;
        image.bitmap_bit_order = x11_dl::xlib::LSBFirst;
        image.bitmap_pad = 32;
        image.depth = target.depth as i32;
        image.bytes_per_line = (width * 4) as i32;
        image.bits_per_pixel = 32;
        image.red_mask = (*target.visual).red_mask;
        image.green_mask = (*target.visual).green_mask;
        image.blue_mask = (*target.visual).blue_mask;
        if (xlib.XInitImage)(&mut image) == 0 {
            return;
        }

        let full = Rect {
            x: 0,
            y: 0,
            width: width as i32,
            height: height as i32,
        };
        let rects: &[Rect] = match dirty_rects {
            Some(rects) if !rects.is_empty() => rects,
            _ => std::slice::from_ref(&full),
        };

        for rect in rects {
            let x = rect.x.max(0);
            let y = rect.y.max(0);
            if x >= width as i32 || y >= height as i32 {
                continue;
            }
            let w = (rect.width.min(width as i32 - x)).max(0) as u32;
            let h = (rect.height.min(height as i32 - y)).max(0) as u32;
            if w == 0 || h == 0 {
                continue;
            }
            let _ = (xlib.XPutImage)(
                display as *mut _,
                host_xid,
                target.gc,
                &mut image,
                x,
                y,
                x,
                y,
                w,
                h,
            );
        }

        // Flush every few frames — full flush each paint is expensive under Xwayland.
        let n = OSR_PAINT_N.fetch_add(1, Ordering::Relaxed);
        if n % 4 == 0 {
            (xlib.XFlush)(display as *mut _);
        }
    }
}

wrap_focus_handler! {
    struct ContentFocusHandler;

    impl FocusHandler {
        fn on_got_focus(&self, _browser: Option<&mut Browser>) {
            emit(CefEvent::ContentFocus);
        }
    }
}

wrap_life_span_handler! {
    struct ContentLifeSpan;

    impl LifeSpanHandler {
        fn on_after_created(&self, browser: Option<&mut Browser>) {
            let Some(browser) = browser.cloned() else { return };
            // Correlate this browser with the oldest outstanding create request.
            let state = shared().lock().ok().and_then(|mut s| {
                let pending = s.creating.pop_front()?;
                let tab_id = pending.tab_id;
                s.browsers.insert(tab_id, browser.clone());
                s.identifier_to_tab.insert(browser.identifier(), tab_id);
                let is_active = s.active_tab == Some(tab_id);
                let bounds = s.bounds.clone();
                let pending_nav = s.pending_nav.remove(&tab_id);
                Some((tab_id, is_active, bounds, pending_nav, s.tx.clone()))
            });
            let Some((tab_id, is_active, bounds, pending_nav, tx)) = state else {
                return;
            };

            // Never call into CEF while holding SHARED: Win32 callbacks can
            // synchronously re-enter this module and emit an event.
            if is_active {
                resize_child(&browser, &bounds); // positions + shows the child
            } else {
                set_child_visible(&browser, false);
            }
            #[cfg(any(target_os = "linux", target_os = "freebsd"))]
            {
                // OSR needs an explicit resize/invalidate after create or the
                // first frames may never be requested.
                if let Some(host) = browser.host() {
                    host.was_resized();
                    host.invalidate(PaintElementType::VIEW);
                }
            }
            if let Some(url) = pending_nav
                && let Some(frame) = browser.main_frame()
            {
                frame.load_url(Some(&CefString::from(url.as_str())));
            }
            #[cfg(target_os = "windows")]
            log_windows_browser("created", &browser);
            let _ = tx.send(CefEvent::Created { tab_id });
        }

        fn do_close(&self, _browser: Option<&mut Browser>) -> i32 {
            0 // allow
        }

        fn on_before_close(&self, _browser: Option<&mut Browser>) {
            let Some(browser) = _browser.as_deref() else { return };
            let identifier = browser.identifier();
            adblock_runtime().navigation.remove_browser(identifier);
            if let Ok(mut s) = shared().lock() {
                if let Some(tab_id) = s.identifier_to_tab.remove(&identifier) {
                    s.browsers.remove(&tab_id);
                    if s.active_tab == Some(tab_id) {
                        s.active_tab = None;
                    }
                    let _ = s.tx.send(CefEvent::Closed { tab_id });
                }
            }
        }

        fn on_before_popup(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            _popup_id: i32,
            target_url: Option<&CefString>,
            _target_frame_name: Option<&CefString>,
            _target_disposition: WindowOpenDisposition,
            _user_gesture: i32,
            _popup_features: Option<&PopupFeatures>,
            _window_info: Option<&mut WindowInfo>,
            _client: Option<&mut Option<Client>>,
            _settings: Option<&mut BrowserSettings>,
            _extra_info: Option<&mut Option<DictionaryValue>>,
            _no_javascript_access: Option<&mut i32>,
        ) -> i32 {
            // Open popups as same-tab navigations.
            if let Some(url) = target_url {
                let url = url.to_string();
                if !url.is_empty() {
                    navigate(&url);
                }
            }
            1 // cancel popup window
        }
    }
}

wrap_load_handler! {
    struct ContentLoadHandler;

    impl LoadHandler {
        fn on_load_start(
            &self,
            browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            _transition_type: TransitionType,
        ) {
            if let (Some(browser), Some(frame)) = (browser.as_deref(), frame.as_deref()) {
                update_navigation_context(browser, frame);
            }
            eprintln!(
                "[cef] load-start id={} main={} url={}",
                browser.map(|b| b.identifier()).unwrap_or(-1),
                frame.as_deref().map(|f| f.is_main()).unwrap_or(0),
                frame
                    .as_deref()
                    .map(|f| CefString::from(&f.url()).to_string())
                    .unwrap_or_default(),
            );
        }

        fn on_load_end(
            &self,
            browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            http_status_code: i32,
        ) {
            let should_push_statistics = frame
                .as_deref()
                .is_some_and(|frame| frame.is_main() != 0)
                && frame
                    .as_deref()
                    .map(|frame| CefString::from(&frame.url()).to_string())
                    .is_some_and(|url| url.starts_with("miko://shell/"));
            let is_main = frame.as_deref().is_some_and(|f| f.is_main() != 0);
            let browser_id = browser.as_deref().map(|b| b.identifier()).unwrap_or(-1);
            eprintln!(
                "[cef] load-end id={} main={} status={} url={}",
                browser_id,
                i32::from(is_main),
                http_status_code,
                frame
                    .as_deref()
                    .map(|f| CefString::from(&f.url()).to_string())
                    .unwrap_or_default(),
            );
            if should_push_statistics {
                push_adblock_statistics(true);
            }
            // After navigation, re-assert CEF focus if the page still owns input
            // (Linux OSR otherwise loses keystrokes until the next click).
            if is_main
                && shared()
                    .lock()
                    .ok()
                    .is_some_and(|s| s.content_input_active)
                && let Some(browser) = browser.as_ref()
                && let Some(host) = browser.host()
            {
                host.set_focus(1);
                #[cfg(any(target_os = "linux", target_os = "freebsd"))]
                retain_chrome_os_focus();
            }
        }

        fn on_loading_state_change(
            &self,
            _browser: Option<&mut Browser>,
            is_loading: i32,
            can_go_back: i32,
            can_go_forward: i32,
        ) {
            if let Some(browser) = _browser.as_deref() {
                eprintln!(
                    "[cef] load-state id={} loading={} back={} forward={}",
                    browser.identifier(), is_loading, can_go_back, can_go_forward
                );
            }
            if let Some(tab_id) = tab_of(_browser.as_deref()) {
                emit(CefEvent::Loading {
                    tab_id,
                    loading: is_loading != 0,
                    can_back: can_go_back != 0,
                    can_forward: can_go_forward != 0,
                });
            }
        }

        fn on_load_error(
            &self,
            browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            error_code: Errorcode,
            error_text: Option<&CefString>,
            failed_url: Option<&CefString>,
        ) {
            eprintln!(
                "[cef] load-error id={} code={:?} text={} url={}",
                browser.map(|b| b.identifier()).unwrap_or(-1),
                error_code,
                error_text.map(CefString::to_string).unwrap_or_default(),
                failed_url.map(CefString::to_string).unwrap_or_default(),
            );
        }
    }
}

wrap_request_handler! {
    struct ContentRequestHandler;

    impl RequestHandler {
        fn resource_request_handler(
            &self,
            browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            request: Option<&mut Request>,
            is_navigation: i32,
            _is_download: i32,
            request_initiator: Option<&CefString>,
            _disable_default_handling: Option<&mut i32>,
        ) -> Option<ResourceRequestHandler> {
            let runtime = adblock_runtime();
            let request_context = match (
                browser.as_deref(),
                frame.as_deref(),
                request.as_deref(),
            ) {
                (Some(browser), Some(frame), Some(request)) => Some(RequestContext {
                    browser_id: browser.identifier(),
                    frame_id: CefString::from(&frame.identifier()).to_string(),
                    request_id: request.identifier(),
                }),
                _ => None,
            };
            let resource_type = request
                .as_deref()
                .map(|request| map_cef_resource_type(request.resource_type()))
                .unwrap_or(AdBlockResourceType::Other);
            Some(ContentResourceRequestHandler::new(
                runtime.adapter.clone(),
                runtime.service.clone(),
                request_context,
                resource_type,
                request_initiator.map(CefString::to_string),
                is_navigation != 0,
            ))
        }

        fn on_render_view_ready(&self, browser: Option<&mut Browser>) {
            if let Some(browser) = browser {
                eprintln!("[cef] render-view-ready id={}", browser.identifier());
                #[cfg(target_os = "windows")]
                log_windows_browser("render-view-ready", browser);
            }
        }

        fn on_render_process_terminated(
            &self,
            browser: Option<&mut Browser>,
            status: TerminationStatus,
            error_code: i32,
            error_string: Option<&CefString>,
        ) {
            eprintln!(
                "[cef] renderer-terminated id={} status={:?} code={} error={}",
                browser.map(|b| b.identifier()).unwrap_or(-1),
                status,
                error_code,
                error_string.map(CefString::to_string).unwrap_or_default(),
            );
        }
    }
}

wrap_resource_request_handler! {
    struct ContentResourceRequestHandler {
        adapter: AdBlockAdapter,
        service: Arc<AdBlockService>,
        request_context: Option<RequestContext>,
        resource_type: AdBlockResourceType,
        request_initiator: Option<String>,
        is_navigation: bool,
    }

    impl ResourceRequestHandler {
        fn on_before_resource_load(
            &self,
            browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            request: Option<&mut Request>,
            _callback: Option<&mut Callback>,
        ) -> ReturnValue {
            let (Some(browser), Some(frame), Some(request)) =
                (browser.as_deref(), frame.as_deref(), request.as_deref())
            else {
                // Missing CEF metadata must fail open instead of breaking browsing.
                return ReturnValue::CONTINUE;
            };

            update_navigation_context(browser, frame);

            let decision = self.adapter.evaluate(AdapterRequest {
                browser_id: browser.identifier(),
                frame_id: CefString::from(&frame.identifier()).to_string(),
                request_id: request.identifier(),
                url: CefString::from(&request.url()).to_string(),
                method: CefString::from(&request.method()).to_string(),
                resource_type: map_cef_resource_type(request.resource_type()),
                is_navigation: self.is_navigation,
                request_initiator: self.request_initiator.clone(),
            });

            match decision {
                AdBlockDecision::Allow => ReturnValue::CONTINUE,
                AdBlockDecision::Block => ReturnValue::CANCEL,
                AdBlockDecision::RewriteUrl(url) | AdBlockDecision::Redirect(url) => {
                    request.set_url(Some(&CefString::from(url.as_str())));
                    ReturnValue::CONTINUE
                }
            }
        }

        fn resource_response_filter(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            request: Option<&mut Request>,
            response: Option<&mut Response>,
        ) -> Option<ResponseFilter> {
            let request = request?;
            let response = response?;
            if response.status() != 200 {
                return None;
            }
            let url = CefString::from(&request.url()).to_string();
            let mime_type = CefString::from(&response.mime_type()).to_string();
            let kind = classify_youtube_response(&url, &mime_type)?;
            Some(YoutubeAdResponseFilter::new(Arc::new(Mutex::new(
                YoutubeFilterState::new(
                    kind,
                    self.service.clone(),
                    self.request_context.clone(),
                    self.resource_type,
                ),
            ))))
        }
    }
}

const MAX_YOUTUBE_RESPONSE_BYTES: usize = 32 * 1024 * 1024;

#[derive(Clone, Copy, PartialEq, Eq)]
enum YoutubeFilterMode {
    Buffering,
    Passthrough,
    Finished,
}

struct YoutubeFilterState {
    kind: YoutubeResponseKind,
    mode: YoutubeFilterMode,
    input: Vec<u8>,
    pending: Vec<u8>,
    pending_offset: usize,
    service: Arc<AdBlockService>,
    request_context: Option<RequestContext>,
    resource_type: AdBlockResourceType,
}

impl YoutubeFilterState {
    fn new(
        kind: YoutubeResponseKind,
        service: Arc<AdBlockService>,
        request_context: Option<RequestContext>,
        resource_type: AdBlockResourceType,
    ) -> Self {
        Self {
            kind,
            mode: YoutubeFilterMode::Buffering,
            input: Vec::new(),
            pending: Vec::new(),
            pending_offset: 0,
            service,
            request_context,
            resource_type,
        }
    }

    fn process(&mut self, data_in: Option<&[u8]>, data_out: &mut [u8]) -> (usize, usize, bool) {
        let input_size = data_in.map_or(0, <[u8]>::len);
        let end_of_input = data_in.is_none();

        if let Some(data) = data_in {
            match self.mode {
                YoutubeFilterMode::Buffering
                    if self.input.len().saturating_add(data.len())
                        <= MAX_YOUTUBE_RESPONSE_BYTES =>
                {
                    self.input.extend_from_slice(data);
                }
                YoutubeFilterMode::Buffering => {
                    self.mode = YoutubeFilterMode::Passthrough;
                    self.pending = std::mem::take(&mut self.input);
                    self.pending.extend_from_slice(data);
                }
                YoutubeFilterMode::Passthrough => {
                    self.compact_pending();
                    self.pending.extend_from_slice(data);
                }
                YoutubeFilterMode::Finished => {}
            }
        }

        if end_of_input {
            match self.mode {
                YoutubeFilterMode::Buffering => self.finalize_sanitized_response(),
                YoutubeFilterMode::Passthrough => self.mode = YoutubeFilterMode::Finished,
                YoutubeFilterMode::Finished => {}
            }
        }

        let available = self.pending.len().saturating_sub(self.pending_offset);
        let written = available.min(data_out.len());
        if written > 0 {
            let end = self.pending_offset + written;
            data_out[..written].copy_from_slice(&self.pending[self.pending_offset..end]);
            self.pending_offset = end;
        }
        if self.pending_offset == self.pending.len() {
            self.pending.clear();
            self.pending_offset = 0;
        }

        let done = end_of_input && self.pending.is_empty();
        (input_size, written, done)
    }

    fn finalize_sanitized_response(&mut self) {
        let original = std::mem::take(&mut self.input);
        let result = sanitize_youtube_response(self.kind, &original);
        if result.removed_items > 0
            && let Some(context) = self.request_context.clone()
        {
            self.service
                .record_blocked_request(context, self.resource_type);
        }
        self.pending = result.bytes;
        self.pending_offset = 0;
        self.mode = YoutubeFilterMode::Finished;
    }

    fn compact_pending(&mut self) {
        if self.pending_offset > 0 {
            self.pending.drain(..self.pending_offset);
            self.pending_offset = 0;
        }
    }
}

wrap_response_filter! {
    struct YoutubeAdResponseFilter {
        state: Arc<Mutex<YoutubeFilterState>>,
    }

    impl ResponseFilter {
        fn init_filter(&self) -> i32 {
            1
        }

        fn filter(
            &self,
            data_in: Option<&mut Vec<u8>>,
            data_in_read: Option<&mut usize>,
            data_out: Option<&mut Vec<u8>>,
            data_out_written: Option<&mut usize>,
        ) -> ResponseFilterStatus {
            let Some(data_in_read) = data_in_read else {
                return ResponseFilterStatus::ERROR;
            };
            let Some(data_out_written) = data_out_written else {
                return ResponseFilterStatus::ERROR;
            };
            let Some(data_out) = data_out else {
                return ResponseFilterStatus::ERROR;
            };
            let Ok(mut state) = self.state.lock() else {
                return ResponseFilterStatus::ERROR;
            };
            let (read, written, done) =
                state.process(data_in.as_ref().map(|data| data.as_slice()), data_out.as_mut_slice());
            *data_in_read = read;
            *data_out_written = written;
            if done {
                ResponseFilterStatus::DONE
            } else {
                ResponseFilterStatus::NEED_MORE_DATA
            }
        }
    }
}

#[cfg(test)]
mod youtube_response_filter_tests {
    use super::*;

    #[test]
    fn buffers_sanitizes_and_flushes_chunked_player_response() {
        let service = Arc::new(AdBlockServiceBuilder::default().build());
        let context = RequestContext {
            browser_id: 1,
            frame_id: "main".into(),
            request_id: 9,
        };
        let mut state = YoutubeFilterState::new(
            YoutubeResponseKind::ApiJson,
            service.clone(),
            Some(context),
            AdBlockResourceType::XmlHttpRequest,
        );
        let input = br#"{"videoDetails":{"videoId":"abc"},"adPlacements":[1],"playerAds":[2]}"#;
        let split = input.len() / 2;
        let mut output_chunk = [0u8; 11];

        let (read, written, done) = state.process(Some(&input[..split]), &mut output_chunk);
        assert_eq!(read, split);
        assert_eq!(written, 0);
        assert!(!done);
        let (read, written, done) = state.process(Some(&input[split..]), &mut output_chunk);
        assert_eq!(read, input.len() - split);
        assert_eq!(written, 0);
        assert!(!done);

        let mut output = Vec::new();
        loop {
            let (_, written, done) = state.process(None, &mut output_chunk);
            output.extend_from_slice(&output_chunk[..written]);
            if done {
                break;
            }
        }
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("videoDetails"));
        assert!(!output.contains("adPlacements"));
        assert!(!output.contains("playerAds"));
        assert_eq!(service.blocked_total(), 1);
    }
}

fn update_navigation_context(browser: &Browser, frame: &Frame) {
    let frame_url = CefString::from(&frame.url()).to_string();
    let top_level_url = if frame.is_main() != 0 {
        frame_url.clone()
    } else {
        browser
            .main_frame()
            .map(|main| CefString::from(&main.url()).to_string())
            .filter(|url| !url.is_empty())
            .unwrap_or_else(|| frame_url.clone())
    };
    adblock_runtime().navigation.upsert(FrameContext {
        key: FrameKey {
            browser_id: browser.identifier(),
            frame_id: CefString::from(&frame.identifier()).to_string(),
        },
        frame_url,
        top_level_url,
    });
}

fn map_cef_resource_type(resource_type: cef::ResourceType) -> AdBlockResourceType {
    match resource_type {
        cef::ResourceType::MAIN_FRAME => AdBlockResourceType::MainFrame,
        cef::ResourceType::SUB_FRAME => AdBlockResourceType::SubFrame,
        cef::ResourceType::STYLESHEET => AdBlockResourceType::Stylesheet,
        cef::ResourceType::SCRIPT => AdBlockResourceType::Script,
        cef::ResourceType::IMAGE => AdBlockResourceType::Image,
        cef::ResourceType::FONT_RESOURCE => AdBlockResourceType::Font,
        cef::ResourceType::OBJECT | cef::ResourceType::PLUGIN_RESOURCE => {
            AdBlockResourceType::Object
        }
        cef::ResourceType::MEDIA => AdBlockResourceType::Media,
        cef::ResourceType::WORKER => AdBlockResourceType::Worker,
        cef::ResourceType::SHARED_WORKER => AdBlockResourceType::SharedWorker,
        cef::ResourceType::PREFETCH => AdBlockResourceType::Prefetch,
        cef::ResourceType::FAVICON => AdBlockResourceType::Favicon,
        cef::ResourceType::XHR => AdBlockResourceType::XmlHttpRequest,
        cef::ResourceType::PING => AdBlockResourceType::Ping,
        cef::ResourceType::SERVICE_WORKER => AdBlockResourceType::ServiceWorker,
        cef::ResourceType::CSP_REPORT => AdBlockResourceType::CspReport,
        _ => AdBlockResourceType::Other,
    }
}

wrap_display_handler! {
    struct ContentDisplayHandler;

    impl DisplayHandler {
        fn on_address_change(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            url: Option<&CefString>,
        ) {
            if let Some(tab_id) = tab_of(_browser.as_deref()) {
                emit(CefEvent::Address {
                    tab_id,
                    url: url.map(CefString::to_string).unwrap_or_default(),
                });
            }
        }

        fn on_title_change(&self, _browser: Option<&mut Browser>, title: Option<&CefString>) {
            if let Some(tab_id) = tab_of(_browser.as_deref()) {
                emit(CefEvent::Title {
                    tab_id,
                    title: title.map(CefString::to_string).unwrap_or_default(),
                });
            }
        }

        fn on_favicon_urlchange(
            &self,
            browser: Option<&mut Browser>,
            icon_urls: Option<&mut CefStringList>,
        ) {
            let Some(tab_id) = tab_of(browser.as_deref()) else {
                return;
            };
            let Some(url) = pick_favicon_url(icon_urls) else {
                emit(CefEvent::Favicon {
                    tab_id,
                    bytes: Vec::new(),
                });
                return;
            };
            let Some(host) = browser.as_deref().and_then(Browser::host) else {
                return;
            };
            let mut callback = FaviconDownloadCallback::new(tab_id);
            host.download_image(
                Some(&CefString::from(url.as_str())),
                1,  // is_favicon
                32, // max DIP size
                0,  // use cache
                Some(&mut callback),
            );
        }
    }
}

fn pick_favicon_url(icon_urls: Option<&mut CefStringList>) -> Option<String> {
    let list = icon_urls?;
    let raw: *mut sys::_cef_string_list_t = (&mut *list).into();
    if raw.is_null() {
        return None;
    }
    unsafe {
        let count = sys::cef_string_list_size(raw);
        let mut best = None;
        for i in 0..count {
            let mut value = std::mem::zeroed();
            if sys::cef_string_list_value(raw, i, &mut value) != 0 {
                let url = CefString::from(std::ptr::from_ref(&value)).to_string();
                if !url.is_empty() {
                    // Prefer the last URL — CEF usually lists larger icons last.
                    best = Some(url);
                }
            }
        }
        best
    }
}

wrap_download_image_callback! {
    struct FaviconDownloadCallback {
        tab_id: u64,
    }

    impl DownloadImageCallback {
        fn on_download_image_finished(
            &self,
            _image_url: Option<&CefString>,
            http_status_code: i32,
            image: Option<&mut Image>,
        ) {
            if http_status_code != 200 {
                return;
            }
            let Some(image) = image else {
                return;
            };
            let mut width = 0;
            let mut height = 0;
            let Some(png) = image.as_png(1.0, 1, Some(&mut width), Some(&mut height)) else {
                return;
            };
            let size = png.size();
            if size == 0 {
                return;
            }
            let mut buf = vec![0u8; size];
            png.data(Some(&mut buf), 0);
            emit(CefEvent::Favicon {
                tab_id: self.tab_id,
                bytes: buf,
            });
        }
    }
}
