//! CEF browser-process host: init, external message pump, and a single content
//! browser parented as an X11 child of the GPUI window.

use cef::{args::Args, *};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex, OnceLock};

/// Events pushed from CEF callbacks (UI thread) and drained by the GPUI pump.
#[derive(Debug, Clone)]
pub enum CefEvent {
    Ready,
    Created,
    Closed,
    Address(String),
    Title(String),
    Loading {
        loading: bool,
        can_back: bool,
        can_forward: bool,
    },
    /// The CEF content browser took keyboard focus (e.g. user clicked the page).
    ContentFocus,
}

struct Shared {
    tx: Sender<CefEvent>,
    browser: Option<Browser>,
    /// Native parent window + last content bounds (device pixels).
    parent: Option<u64>,
    bounds: Rect,
    start_uri: String,
    pending_url: Option<String>,
    context_ready: bool,
}

/// Shared CEF state (browser process only).
static SHARED: OnceLock<Arc<Mutex<Shared>>> = OnceLock::new();

fn shared() -> Arc<Mutex<Shared>> {
    SHARED.get().expect("CEF not initialized").clone()
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
    let _ = SHARED.set(Arc::new(Mutex::new(Shared {
        tx,
        browser: None,
        parent: None,
        bounds: Rect {
            x: 0,
            y: 0,
            width: 1,
            height: 1,
        },
        start_uri: String::new(),
        pending_url: None,
        context_ready: false,
    })));

    let settings = Settings {
        no_sandbox: 1,
        external_message_pump: 1,
        root_cache_path: CefString::from(cache_dir.to_string_lossy().as_ref()),
        ..Default::default()
    };

    let ok = initialize(
        Some(main_args),
        Some(&settings),
        Some(&mut app),
        sandbox_info,
    );
    assert_eq!(ok, 1, "cef initialize failed");

    // Keep the App wrapper alive for the process lifetime (CEF also refs it).
    std::mem::forget(app);

    let start_uri = shell_uri();
    if let Ok(mut s) = shared().lock() {
        s.start_uri = start_uri;
    }

    Some(rx)
}

pub fn shutdown_cef() {
    let browser = shared().lock().ok().and_then(|mut s| s.browser.take());
    if let Some(host) = browser.as_ref().and_then(Browser::host) {
        host.close_browser(1);
    }
    // Drain a few pump iterations so close completes.
    for _ in 0..20 {
        do_message_loop_work();
    }
    shutdown();
}

/// Drive CEF from the GPUI main thread. Call ~every 10ms.
pub fn pump(rx: &Receiver<CefEvent>, out: &mut Vec<CefEvent>) {
    do_message_loop_work();
    loop {
        match rx.try_recv() {
            Ok(ev) => out.push(ev),
            Err(TryRecvError::Empty) => break,
            Err(TryRecvError::Disconnected) => break,
        }
    }
}

/// Attach (or re-attach) the content browser under `parent_xid` at `bounds`
/// (device pixels, relative to the parent window).
pub fn set_parent_bounds(parent_xid: u64, bounds: Rect) {
    let shared = shared();
    let browser_to_resize = {
        let Ok(mut s) = shared.lock() else { return };
        let bounds_changed = s.bounds.x != bounds.x
            || s.bounds.y != bounds.y
            || s.bounds.width != bounds.width
            || s.bounds.height != bounds.height;
        s.bounds = bounds.clone();

        if s.parent != Some(parent_xid) || bounds_changed {
            eprintln!(
                "[cef] layout parent=0x{parent_xid:x} rect=({},{} {}x{})",
                bounds.x, bounds.y, bounds.width, bounds.height
            );
        }

        if s.parent != Some(parent_xid) {
            s.parent = Some(parent_xid);
            if s.context_ready && s.browser.is_none() {
                create_browser_locked(&mut s);
            }
            None
        } else if bounds_changed {
            s.browser.clone()
        } else {
            None
        }
    };
    if let Some(browser) = browser_to_resize {
        resize_child(&browser, &bounds);
    }
}

pub fn navigate(url: &str) {
    let shared = shared();
    let frame = {
        let Ok(mut s) = shared.lock() else { return };
        let frame = s.browser.as_ref().and_then(Browser::main_frame);
        if frame.is_none() {
            s.pending_url = Some(url.to_string());
        }
        frame
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
    let parent = shared().lock().ok().and_then(|s| s.parent);
    if let Some(parent) = parent {
        unsafe { SetFocus(parent as isize) };
    }
}

#[cfg(not(target_os = "windows"))]
pub fn focus_chrome() {}

/// Hand keyboard focus to the CEF content browser (page becomes typeable).
pub fn focus_content() {
    let browser = shared().lock().ok().and_then(|s| s.browser.clone());
    if let Some(host) = browser.as_ref().and_then(Browser::host) {
        host.set_focus(1);
    }
}

pub fn go_back() {
    let browser = shared().lock().ok().and_then(|s| s.browser.clone());
    if let Some(browser) = browser {
        browser.go_back();
    }
}

pub fn go_forward() {
    let browser = shared().lock().ok().and_then(|s| s.browser.clone());
    if let Some(browser) = browser {
        browser.go_forward();
    }
}

pub fn reload() {
    let browser = shared().lock().ok().and_then(|s| s.browser.clone());
    if let Some(browser) = browser {
        browser.reload();
    }
}

pub fn stop_load() {
    let browser = shared().lock().ok().and_then(|s| s.browser.clone());
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

pub fn close_browser() {
    let browser = shared().lock().ok().and_then(|mut s| s.browser.take());
    if let Some(host) = browser.as_ref().and_then(Browser::host) {
        host.close_browser(1);
    }
}

fn emit(ev: CefEvent) {
    let tx = shared().lock().ok().map(|s| s.tx.clone());
    if let Some(tx) = tx {
        let _ = tx.send(ev);
    }
}

fn create_browser_locked(s: &mut Shared) {
    let Some(parent) = s.parent else { return };
    if s.browser.is_some() || !s.context_ready {
        return;
    }

    let url = s.pending_url.take().unwrap_or_else(|| s.start_uri.clone());

    eprintln!(
        "[cef] create parent=0x{parent:x} rect=({},{} {}x{}) url={url}",
        s.bounds.x, s.bounds.y, s.bounds.width, s.bounds.height
    );

    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    let parent = parent as cef::sys::cef_window_handle_t;
    #[cfg(target_os = "windows")]
    let parent = cef::sys::HWND(parent as usize as *mut _);

    let window_info = WindowInfo {
        runtime_style: RuntimeStyle::ALLOY,
        ..Default::default()
    }
    .set_as_child(parent, &s.bounds);

    let mut client = ContentClient::new();
    let settings = BrowserSettings::default();
    let ok = browser_host_create_browser(
        Some(&window_info),
        Some(&mut client),
        Some(&CefString::from(url.as_str())),
        Some(&settings),
        None,
        None,
    );
    if ok == 0 {
        eprintln!("[cef] browser_host_create_browser failed");
    }
}

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
fn resize_child(browser: &Browser, bounds: &Rect) {
    let Some(host) = browser.host() else { return };
    let xid = host.window_handle();
    if xid == 0 {
        return;
    }
    let display = get_xdisplay();
    if display.is_null() {
        return;
    }
    let Ok(xlib) = x11_dl::xlib::Xlib::open() else {
        return;
    };
    unsafe {
        (xlib.XMoveResizeWindow)(
            display as *mut _,
            xid,
            bounds.x,
            bounds.y,
            bounds.width.max(1) as u32,
            bounds.height.max(1) as u32,
        );
        (xlib.XFlush)(display as *mut _);
    }
    host.notify_move_or_resize_started();
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
                    command_line.append_switch_with_value(Some(&ozone), Some(&CefString::from("x11")));
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
                if s.parent.is_some() && s.browser.is_none() {
                    create_browser_locked(&mut s);
                }
            }
        }

        fn on_schedule_message_pump_work(&self, _delay_ms: i64) {
            // GPUI polls do_message_loop_work on a short timer; nothing to do.
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
            if let Some(browser) = browser.cloned() {
                let state = shared().lock().ok().map(|mut s| {
                    let bounds = s.bounds.clone();
                    s.browser = Some(browser.clone());
                    (bounds, s.tx.clone())
                });
                let Some((bounds, tx)) = state else { return };

                // Never call into CEF while holding SHARED: Win32 callbacks can
                // synchronously re-enter this module and emit an event.
                resize_child(&browser, &bounds);
                #[cfg(target_os = "windows")]
                log_windows_browser("created", &browser);
                let _ = tx.send(CefEvent::Created);
            }
        }

        fn do_close(&self, _browser: Option<&mut Browser>) -> i32 {
            0 // allow
        }

        fn on_before_close(&self, _browser: Option<&mut Browser>) {
            if let Ok(mut s) = shared().lock() {
                s.browser = None;
                let _ = s.tx.send(CefEvent::Closed);
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
            eprintln!(
                "[cef] load-end id={} main={} status={} url={}",
                browser.map(|b| b.identifier()).unwrap_or(-1),
                frame.as_deref().map(|f| f.is_main()).unwrap_or(0),
                http_status_code,
                frame
                    .as_deref()
                    .map(|f| CefString::from(&f.url()).to_string())
                    .unwrap_or_default(),
            );
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
            emit(CefEvent::Loading {
                loading: is_loading != 0,
                can_back: can_go_back != 0,
                can_forward: can_go_forward != 0,
            });
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

wrap_display_handler! {
    struct ContentDisplayHandler;

    impl DisplayHandler {
        fn on_address_change(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            url: Option<&CefString>,
        ) {
            emit(CefEvent::Address(
                url.map(CefString::to_string).unwrap_or_default(),
            ));
        }

        fn on_title_change(&self, _browser: Option<&mut Browser>, title: Option<&CefString>) {
            emit(CefEvent::Title(
                title.map(CefString::to_string).unwrap_or_default(),
            ));
        }
    }
}
