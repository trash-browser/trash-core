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
}

struct Shared {
    tx: Sender<CefEvent>,
    browser: Option<Browser>,
    /// Parent X11 window + last content bounds (device pixels).
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

    let ok = initialize(Some(main_args), Some(&settings), Some(&mut app), sandbox_info);
    assert_eq!(ok, 1, "cef initialize failed");

    // Keep the App wrapper alive for the process lifetime (CEF also refs it).
    std::mem::forget(app);

    // Build start-page data URI only after initialize (uses CEF string helpers).
    let start_uri = data_uri(include_str!("ui/start.html"));
    if let Ok(mut s) = shared().lock() {
        s.start_uri = start_uri;
    }

    Some(rx)
}

pub fn shutdown_cef() {
    if let Ok(mut s) = shared().lock() {
        if let Some(browser) = s.browser.take() {
            if let Some(host) = browser.host() {
                host.close_browser(1);
            }
        }
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
    let Ok(mut s) = shared.lock() else { return };
    let bounds_changed = s.bounds.x != bounds.x
        || s.bounds.y != bounds.y
        || s.bounds.width != bounds.width
        || s.bounds.height != bounds.height;
    s.bounds = bounds.clone();

    if s.parent != Some(parent_xid) {
        s.parent = Some(parent_xid);
        if s.context_ready && s.browser.is_none() {
            create_browser_locked(&mut s);
        }
    } else if bounds_changed {
        if let Some(browser) = s.browser.as_ref() {
            resize_child(browser, &bounds);
        }
    }
}

pub fn navigate(url: &str) {
    let shared = shared();
    let Ok(mut s) = shared.lock() else { return };
    if let Some(frame) = s.browser.as_ref().and_then(|b| b.main_frame()) {
        frame.load_url(Some(&CefString::from(url)));
    } else {
        s.pending_url = Some(url.to_string());
    }
}

pub fn go_back() {
    if let Ok(s) = shared().lock() {
        if let Some(b) = s.browser.as_ref() {
            b.go_back();
        }
    }
}

pub fn go_forward() {
    if let Ok(s) = shared().lock() {
        if let Some(b) = s.browser.as_ref() {
            b.go_forward();
        }
    }
}

pub fn reload() {
    if let Ok(s) = shared().lock() {
        if let Some(b) = s.browser.as_ref() {
            b.reload();
        }
    }
}

pub fn stop_load() {
    if let Ok(s) = shared().lock() {
        if let Some(b) = s.browser.as_ref() {
            b.stop_load();
        }
    }
}

pub fn start_uri() -> String {
    shared()
        .lock()
        .map(|s| s.start_uri.clone())
        .unwrap_or_default()
}

pub fn close_browser() {
    if let Ok(mut s) = shared().lock() {
        if let Some(browser) = s.browser.take() {
            if let Some(host) = browser.host() {
                host.close_browser(1);
            }
        }
    }
}

fn emit(ev: CefEvent) {
    if let Ok(s) = shared().lock() {
        let _ = s.tx.send(ev);
    }
}

fn create_browser_locked(s: &mut Shared) {
    let Some(parent) = s.parent else { return };
    if s.browser.is_some() || !s.context_ready {
        return;
    }

    let url = s
        .pending_url
        .take()
        .unwrap_or_else(|| s.start_uri.clone());

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

fn cache_path() -> std::path::PathBuf {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".cache")))
        .unwrap_or_else(|| std::env::temp_dir());
    base.join("trashbrowser-gpui")
}

fn data_uri(html: &str) -> String {
    let encoded = CefString::from(&base64_encode(Some(html.as_bytes())));
    let encoded = CefString::from(&uriencode(Some(&encoded), 0)).to_string();
    format!("data:text/html;base64,{encoded}")
}

// ---- CEF App / Client ----------------------------------------------------

wrap_app! {
    struct TrashCefApp;

    impl App {
        fn browser_process_handler(&self) -> Option<BrowserProcessHandler> {
            Some(TrashBrowserProcessHandler::new())
        }

        fn on_before_command_line_processing(
            &self,
            _process_type: Option<&CefString>,
            command_line: Option<&mut CommandLine>,
        ) {
            let Some(command_line) = command_line else { return };
            let ozone = CefString::from("ozone-platform");
            if command_line.has_switch(Some(&ozone)) == 0 {
                command_line.append_switch_with_value(Some(&ozone), Some(&CefString::from("x11")));
            }
        }
    }
}

wrap_browser_process_handler! {
    struct TrashBrowserProcessHandler;

    impl BrowserProcessHandler {
        fn on_context_initialized(&self) {
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
    }
}

wrap_life_span_handler! {
    struct ContentLifeSpan;

    impl LifeSpanHandler {
        fn on_after_created(&self, browser: Option<&mut Browser>) {
            if let (Some(browser), Ok(mut s)) = (browser.cloned(), shared().lock()) {
                // Apply current bounds in case create used a stale rect.
                resize_child(&browser, &s.bounds);
                s.browser = Some(browser);
                let _ = s.tx.send(CefEvent::Created);
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
        fn on_loading_state_change(
            &self,
            _browser: Option<&mut Browser>,
            is_loading: i32,
            can_go_back: i32,
            can_go_forward: i32,
        ) {
            emit(CefEvent::Loading {
                loading: is_loading != 0,
                can_back: can_go_back != 0,
                can_forward: can_go_forward != 0,
            });
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
