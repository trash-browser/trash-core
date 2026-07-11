//! Trash Browser — GPUI chrome + CEF content in a native child window.
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod browser_menu;
mod cef_host;
mod content;
mod context_menu;
mod omnibox;
mod popup;
mod shell_ipc;
mod shortcuts;
mod tab_lifecycle;

use std::sync::mpsc::Receiver;
use std::time::Duration;

use gpui::{
    Animation, AnimationExt as _, App, AssetSource, Bounds, Context, Entity, FocusHandle,
    Focusable, Image, ImageFormat, KeyDownEvent, KeyUpEvent, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, ScrollDelta, ScrollWheelEvent, SharedString, TitlebarOptions,
    Transformation, Window, WindowBounds, WindowControlArea, WindowDecorations, WindowHandle,
    WindowOptions, div, img, percentage, prelude::*, px, rgb, size, svg,
};
use gpui_platform::application;

use cef_host::CefEvent;
use content::content_surface;
use omnibox::{Omnibox, OmniboxEvent};

// ---- palette (mirrors chrome.html) ---------------------------------------
const BG: u32 = 0x1c1c1e;
const BAR: u32 = 0x242426;
const TAB: u32 = 0x2b2b2e;
const TAB_ACTIVE: u32 = 0x3a3a3d;
const FIELD: u32 = 0x202022;
const BORDER: u32 = 0x3a3a3d;
const TEXT: u32 = 0xe6e6e6;
const MUTED: u32 = 0x8a8a8f;
const HOVER: u32 = 0x2f2f33;
const ICON_ROOT: &str = "assets/tabler_icons";

mod embedded_tabler_assets {
    include!(concat!(env!("OUT_DIR"), "/tabler_assets.rs"));
}

struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> anyhow::Result<Option<std::borrow::Cow<'static, [u8]>>> {
        let path = path.replace('\\', "/");
        Ok(embedded_tabler_assets::get(&path).map(std::borrow::Cow::Borrowed))
    }

    fn list(&self, path: &str) -> anyhow::Result<Vec<SharedString>> {
        let prefix = path.replace('\\', "/");
        Ok(embedded_tabler_assets::PATHS
            .iter()
            .filter(|asset| asset.starts_with(&prefix))
            .map(|asset| SharedString::from(*asset))
            .collect())
    }
}

fn tabler_icon(style: &str, name: &str, size: f32) -> gpui::Svg {
    svg()
        .path(SharedString::from(format!(
            "{ICON_ROOT}/{style}/{name}.svg"
        )))
        .size(px(size))
        // GPUI's Svg element does not inherit text color from its parent and
        // skips painting entirely when no color is set on the SVG itself.
        .text_color(rgb(MUTED))
}

fn filled_icon(name: &str, size: f32) -> gpui::Svg {
    tabler_icon("filled", name, size)
}

fn outline_icon(name: &str, size: f32) -> gpui::Svg {
    tabler_icon("outline", name, size)
}

struct Tab {
    id: u64,
    title: SharedString,
    url: String,
    favicon: Option<std::sync::Arc<Image>>,
}

impl Tab {
    fn blank(id: u64, title: impl Into<SharedString>) -> Self {
        Self {
            id,
            title: title.into(),
            url: String::new(),
            favicon: None,
        }
    }
}

struct Root {
    tabs: Vec<Tab>,
    active: u64,
    next_id: u64,
    omnibox: Entity<Omnibox>,
    /// Focus target for the page area so OSR key events reach CEF after click.
    content_focus: FocusHandle,
    /// True while a mouse button pressed on the page is held (drag outside page).
    page_pointer_down: bool,
    can_back: bool,
    can_forward: bool,
    loading: bool,
    /// Don't clobber the omnibox while the user is editing.
    url_dirty: bool,
    cef_rx: Receiver<CefEvent>,
    start_uri: String,
    /// Last OS window title pushed to the platform (avoids redundant updates).
    os_title: String,
    menu_command: std::sync::Arc<std::sync::Mutex<Option<browser_menu::BrowserMenuCommand>>>,
    /// Open chrome ⋮ menu popup, if any.
    menu_window: Option<WindowHandle<browser_menu::BrowserMenuView>>,
    /// Suppress reopen when the ⋮ button both dismisses (mouse-down) and fires click.
    menu_suppress_open: bool,
    context_menu_result:
        std::sync::Arc<std::sync::Mutex<Option<context_menu::ContextMenuResult>>>,
    context_menu_window: Option<WindowHandle<context_menu::ContextMenuView>>,
    /// Open on next render (needs a `Window` handle for placement).
    pending_context_menu: Option<(i32, i32, Vec<context_menu::ContextMenuEntry>)>,
    /// Chrome shortcut from CEF keyboard handler (needs Window for some actions).
    pending_shortcut: Option<shortcuts::Shortcut>,
    /// Current page zoom as a percentage (100 = default).
    zoom_percent: i32,
    /// Transient Chrome-style zoom bubble.
    zoom_hud_visible: bool,
    /// Bumped on each zoom change so older hide timers are ignored.
    zoom_hud_generation: u64,
}

impl Root {
    fn new(
        omnibox: Entity<Omnibox>,
        cef_rx: Receiver<CefEvent>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.subscribe(
            &omnibox,
            |this, _emitter, event: &OmniboxEvent, cx| match event {
                OmniboxEvent::Submit(text) => {
                    this.url_dirty = false;
                    this.navigate(text, cx);
                }
            },
        )
        .detach();

        // When the omnibox loses focus within the chrome (tab switch, nav button,
        // Escape), stop guarding it so the address bar tracks the page again.
        let omnibox_focus = omnibox.read(cx).focus_handle(cx);
        cx.on_blur(&omnibox_focus, window, |this, _window, cx| {
            if this.url_dirty {
                this.url_dirty = false;
                this.sync_omnibox(cx);
                cx.notify();
            }
        })
        .detach();

        // External CEF message pump + UI event drain (delay from CEF schedule).
        cx.spawn(async move |this, cx| {
            loop {
                let delay = cef_host::next_pump_delay_ms();
                cx.background_executor()
                    .timer(Duration::from_millis(delay))
                    .await;
                this.update(cx, |this, cx| {
                    this.pump_cef(cx);
                })
                .ok();
            }
        })
        .detach();

        let start_uri = cef_host::start_uri();
        // Queue the first tab's browser; it is created once the CEF context and
        // the native parent window are both ready (see cef_host::show_tab).
        cef_host::show_tab(1, &start_uri);

        let menu_command: std::sync::Arc<
            std::sync::Mutex<Option<browser_menu::BrowserMenuCommand>>,
        > = Default::default();

        Self {
            tabs: vec![Tab::blank(1, "Start")],
            active: 1,
            next_id: 2,
            omnibox,
            content_focus: cx.focus_handle(),
            page_pointer_down: false,
            can_back: false,
            can_forward: false,
            loading: false,
            url_dirty: false,
            cef_rx,
            start_uri,
            os_title: String::new(),
            menu_command,
            menu_window: None,
            menu_suppress_open: false,
            context_menu_result: Default::default(),
            context_menu_window: None,
            pending_context_menu: None,
            pending_shortcut: None,
            zoom_percent: 100,
            zoom_hud_visible: false,
            zoom_hud_generation: 0,
        }
    }

    /// OS window title (taskbar / Alt-Tab): "Trash Browser" on the start page,
    /// "Trash Browser - <tab>" once the active tab has a real title.
    fn window_title(&self) -> String {
        match self.active_tab() {
            Some(tab) => {
                let name = tab.title.trim();
                if tab.url.is_empty() || name.is_empty() || name == "Start" || name == "New Tab" {
                    "Trash Browser".to_string()
                } else {
                    format!("Trash Browser - {name}")
                }
            }
            None => "Trash Browser".to_string(),
        }
    }

    fn pump_cef(&mut self, cx: &mut Context<Self>) {
        let command = self
            .menu_command
            .lock()
            .ok()
            .and_then(|mut slot| slot.take());
        if let Some(command) = command {
            self.run_browser_menu_command(command, cx);
        }
        if let Some(result) = self
            .context_menu_result
            .lock()
            .ok()
            .and_then(|mut slot| slot.take())
        {
            self.context_menu_window = None;
            match result {
                context_menu::ContextMenuResult::Command(id) => {
                    cef_host::finish_context_menu(Some(id));
                }
                context_menu::ContextMenuResult::Cancelled => {
                    cef_host::finish_context_menu(None);
                }
            }
        }
        let mut events = Vec::new();
        cef_host::pump(&self.cef_rx, &mut events);
        for ev in events {
            self.on_cef_event(ev, cx);
        }
        // Windows: Chromium may change zoom via Ctrl+wheel while the child has
        // OS focus; sync the HUD whenever the live level drifts.
        let zoom = cef_host::zoom_percent();
        if zoom != self.zoom_percent {
            self.zoom_percent = zoom;
            self.show_zoom_hud(cx);
        }
    }

    fn on_cef_event(&mut self, ev: CefEvent, cx: &mut Context<Self>) {
        match ev {
            CefEvent::Ready => {}
            CefEvent::Created { tab_id } => {
                // Browser HWND exists — if it backs the visible tab, hand it
                // keyboard focus so the page is immediately typeable.
                if tab_id == self.active {
                    cef_host::focus_content();
                }
            }
            CefEvent::Closed { tab_id } => {
                // A browser that closed on its own (window.close(), renderer
                // crash). If its chrome tab still exists, retire it too. When the
                // tab was closed via close_tab the GPUI tab is already gone, so
                // this is a no-op in that path.
                self.on_browser_closed(tab_id, cx);
            }
            CefEvent::Address { tab_id, url } => {
                let is_start = url.starts_with("miko://");
                let shown = if is_start { String::new() } else { url };
                if let Some(tab) = self.tab_by_id_mut(tab_id) {
                    tab.url = shown.clone();
                }
                if tab_id == self.active && !self.url_dirty {
                    self.omnibox.update(cx, |omni, cx| omni.set_text(shown, cx));
                }
                cx.notify();
            }
            CefEvent::Title { tab_id, title } => {
                if let Some(tab) = self.tab_by_id_mut(tab_id) {
                    tab.title = if title.is_empty() {
                        "New Tab".into()
                    } else {
                        title.into()
                    };
                }
                cx.notify();
            }
            CefEvent::Loading {
                tab_id,
                loading,
                can_back,
                can_forward,
            } => {
                // Nav buttons and the omnibox reflect the visible tab only.
                if tab_id == self.active {
                    self.loading = loading;
                    self.can_back = can_back;
                    self.can_forward = can_forward;
                    cx.notify();
                }
            }
            CefEvent::Favicon { tab_id, bytes } => {
                if let Some(tab) = self.tab_by_id_mut(tab_id) {
                    tab.favicon = if bytes.is_empty() {
                        None
                    } else {
                        Some(std::sync::Arc::new(Image::from_bytes(
                            ImageFormat::Png,
                            bytes,
                        )))
                    };
                }
                cx.notify();
            }
            CefEvent::ContentFocus => {
                // User is now interacting with the page, not the omnibox; let
                // address updates flow again and show the current page URL.
                if self.url_dirty {
                    self.url_dirty = false;
                    self.sync_omnibox(cx);
                    cx.notify();
                }
            }
            CefEvent::ShellOverlay {
                action,
                overlay,
                payload,
            } => {
                eprintln!(
                    "[shell-ipc] overlay action={action} overlay={overlay} payload={payload}"
                );
            }
            CefEvent::PageContextMenu { x, y, entries } => {
                self.pending_context_menu = Some((x, y, entries));
                cx.notify();
            }
            CefEvent::ChromeShortcut(shortcut) => {
                self.pending_shortcut = Some(shortcut);
                cx.notify();
            }
        }
    }

    fn active_tab(&self) -> Option<&Tab> {
        self.tabs.iter().find(|t| t.id == self.active)
    }

    fn active_tab_mut(&mut self) -> Option<&mut Tab> {
        let id = self.active;
        self.tabs.iter_mut().find(|t| t.id == id)
    }

    fn tab_by_id_mut(&mut self, id: u64) -> Option<&mut Tab> {
        self.tabs.iter_mut().find(|t| t.id == id)
    }

    fn active_content_url(&self) -> String {
        match self.active_tab() {
            Some(t) if t.url.is_empty() => self.start_uri.clone(),
            Some(t) => t.url.clone(),
            None => self.start_uri.clone(),
        }
    }

    fn sync_omnibox(&self, cx: &mut Context<Self>) {
        let url = self.active_tab().map(|t| t.url.clone()).unwrap_or_default();
        self.omnibox.update(cx, |omni, cx| omni.set_text(url, cx));
    }

    fn navigate(&mut self, input: &str, cx: &mut Context<Self>) {
        let target = normalize(input);
        let load = if target.is_empty() {
            if let Some(tab) = self.active_tab_mut() {
                tab.url.clear();
                tab.title = "Start".into();
                tab.favicon = None;
            }
            self.start_uri.clone()
        } else {
            if let Some(tab) = self.active_tab_mut() {
                tab.url = target.clone();
                tab.title = title_for_url(&target).into();
                tab.favicon = None;
            }
            target
        };
        self.url_dirty = false;
        self.sync_omnibox(cx);
        cef_host::navigate(&load);
        // Return keyboard focus to the page so the loaded content is typeable.
        cef_host::focus_content();
        cx.notify();
    }

    fn new_tab(&mut self, cx: &mut Context<Self>) {
        let id = self.next_id;
        self.next_id += 1;
        self.tabs.push(Tab::blank(id, "New Tab"));
        self.active = id;
        self.url_dirty = false;
        // Reset nav state until the fresh browser reports it.
        self.loading = false;
        self.can_back = false;
        self.can_forward = false;
        self.sync_omnibox(cx);
        // Creates a dedicated browser for this tab (loading the start page) and
        // reveals it. show_tab also hands keyboard focus to the page.
        cef_host::show_tab(id, &self.start_uri);
        self.zoom_hud_visible = false;
        self.sync_zoom_percent();
        cx.notify();
    }

    fn select_tab(&mut self, id: u64, cx: &mut Context<Self>) {
        if self.active == id || !self.tabs.iter().any(|t| t.id == id) {
            return;
        }
        self.active = id;
        self.url_dirty = false;
        self.sync_omnibox(cx);
        let url = self.active_content_url();
        // Reveal this tab's existing browser (no reload); create it on first
        // visit. The previously visible browser is hidden by show_tab.
        cef_host::show_tab(id, &url);
        self.zoom_hud_visible = false;
        self.sync_zoom_percent();
        cx.notify();
    }

    fn close_tab(&mut self, id: u64, cx: &mut Context<Self>) {
        let mut ids: Vec<_> = self.tabs.iter().map(|tab| tab.id).collect();
        let outcome = tab_lifecycle::close_tab(&mut ids, self.active, id);
        if outcome == tab_lifecycle::CloseOutcome::Ignored {
            return;
        }
        self.tabs.retain(|tab| tab.id != id);
        // Activate (or create) the replacement before tearing down the old CEF
        // browser. In particular, closing the final tab must never leave CEF
        // with no active/pending browser: on Windows that transient zero-browser
        // state can run the native close path and take the app window with it.
        self.apply_close_outcome(outcome, cx);
        // This closes only the CefBrowser owned by the tab. Application/window
        // shutdown is handled exclusively by the top-level close command.
        cef_host::close_tab(id);
    }

    /// Retire the chrome tab whose browser closed itself. Its CEF browser is
    /// already gone, so unlike `close_tab` this does not call into `cef_host` to
    /// close anything — it only fixes up the tab strip and active selection.
    fn on_browser_closed(&mut self, tab_id: u64, cx: &mut Context<Self>) {
        let mut ids: Vec<_> = self.tabs.iter().map(|tab| tab.id).collect();
        let outcome = tab_lifecycle::close_tab(&mut ids, self.active, tab_id);
        if outcome == tab_lifecycle::CloseOutcome::Ignored {
            return;
        }
        self.tabs.retain(|tab| tab.id != tab_id);
        self.apply_close_outcome(outcome, cx);
    }

    fn apply_close_outcome(
        &mut self,
        outcome: tab_lifecycle::CloseOutcome,
        cx: &mut Context<Self>,
    ) {
        match outcome {
            tab_lifecycle::CloseOutcome::Ignored => return,
            tab_lifecycle::CloseOutcome::CreateBlank => {
                self.new_tab(cx);
                return;
            }
            tab_lifecycle::CloseOutcome::Activate(id) => self.active = id,
            tab_lifecycle::CloseOutcome::KeepActive(id) => self.active = id,
        }
        self.url_dirty = false;
        self.sync_omnibox(cx);
        let url = self.active_content_url();
        cef_host::show_tab(self.active, &url);
        cx.notify();
    }

    fn go_back(&mut self, _cx: &mut Context<Self>) {
        cef_host::go_back();
        cef_host::focus_content();
    }

    fn go_forward(&mut self, _cx: &mut Context<Self>) {
        cef_host::go_forward();
        cef_host::focus_content();
    }

    fn browser_menu_is_open(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(handle) = self.menu_window else {
            return false;
        };
        if handle.update(cx, |_, _, _| ()).is_ok() {
            true
        } else {
            self.menu_window = None;
            false
        }
    }

    /// Close the chrome menu if open. Returns true when a live menu was dismissed.
    fn close_browser_menu(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(handle) = self.menu_window.take() else {
            return false;
        };
        handle
            .update(cx, |_, window, _| {
                window.remove_window();
            })
            .is_ok()
    }

    fn dismiss_browser_menu(&mut self, cx: &mut Context<Self>) {
        let _ = self.close_browser_menu(cx);
        self.menu_suppress_open = false;
        self.close_context_menu(cx);
    }

    fn close_context_menu(&mut self, cx: &mut Context<Self>) {
        if let Some(handle) = self.context_menu_window.take() {
            let _ = handle.update(cx, |_, window, _| {
                window.remove_window();
            });
        }
        if let Ok(mut slot) = self.context_menu_result.lock() {
            if slot.is_none() {
                *slot = Some(context_menu::ContextMenuResult::Cancelled);
            }
        }
        if let Some(result) = self
            .context_menu_result
            .lock()
            .ok()
            .and_then(|mut slot| slot.take())
        {
            match result {
                context_menu::ContextMenuResult::Command(id) => {
                    cef_host::finish_context_menu(Some(id));
                }
                context_menu::ContextMenuResult::Cancelled => {
                    cef_host::finish_context_menu(None);
                }
            }
        }
    }

    fn open_page_context_menu(
        &mut self,
        view_x: i32,
        view_y: i32,
        entries: Vec<context_menu::ContextMenuEntry>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_context_menu(cx);
        if let Ok(mut slot) = self.context_menu_result.lock() {
            *slot = None;
        }

        let popup_size = context_menu::popup_size(&entries);
        let scale = window.scale_factor();
        let owner = window.bounds();
        let (ox, oy) = cef_host::content_device_origin();
        let mut origin = gpui::point(
            owner.origin.x + px((ox + view_x) as f32 / scale),
            owner.origin.y + px((oy + view_y) as f32 / scale),
        );

        if let Some(display) = window.display(cx) {
            let work = display.visible_bounds();
            let work_right = work.origin.x + work.size.width;
            let work_bottom = work.origin.y + work.size.height;
            if origin.x + popup_size.width > work_right {
                origin.x = (work_right - popup_size.width).max(work.origin.x);
            }
            if origin.y + popup_size.height > work_bottom {
                origin.y = (origin.y - popup_size.height).max(work.origin.y);
            }
            origin.x = origin.x.clamp(work.origin.x, work_right - popup_size.width);
            origin.y = origin
                .y
                .clamp(work.origin.y, work_bottom - popup_size.height);
        }

        let result = self.context_menu_result.clone();
        cef_host::focus_chrome();
        match cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(origin, popup_size))),
                focus: true,
                kind: gpui::WindowKind::PopUp,
                is_movable: false,
                is_resizable: false,
                is_minimizable: false,
                window_background: gpui::WindowBackgroundAppearance::Transparent,
                window_decorations: Some(WindowDecorations::Client),
                ..Default::default()
            },
            move |window, cx| {
                cx.new(|cx| context_menu::ContextMenuView::new(entries, result, window, cx))
            },
        ) {
            Ok(handle) => self.context_menu_window = Some(handle),
            Err(error) => {
                eprintln!("failed to open context menu: {error:?}");
                cef_host::finish_context_menu(None);
            }
        }
    }

    fn open_browser_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let _ = self.close_browser_menu(cx);

        let (popup_size, shadow) = browser_menu::popup_metrics();
        let owner = window.bounds();
        // The toolbar's menu anchor is fixed at the right inset. GPUI window
        // bounds and element positions are logical DIPs in global coordinates;
        // the Windows backend converts this WindowBounds to physical pixels once.
        let anchor_right = owner.origin.x + owner.size.width - px(12.0);
        let anchor_top = owner.origin.y + px(64.0);
        let anchor_bottom = owner.origin.y + px(94.0);
        let mut origin = gpui::point(anchor_right - popup_size.width + shadow, anchor_bottom);

        if let Some(display) = window.display(cx) {
            let work = display.visible_bounds();
            let work_right = work.origin.x + work.size.width;
            let work_bottom = work.origin.y + work.size.height;
            if origin.x + popup_size.width > work_right {
                origin.x = anchor_right - popup_size.width + shadow;
            }
            if origin.y + popup_size.height > work_bottom {
                origin.y = anchor_top - popup_size.height;
            }
            origin.x = origin
                .x
                .max(work.origin.x)
                .min(work_right - popup_size.width);
            origin.y = origin
                .y
                .max(work.origin.y)
                .min(work_bottom - popup_size.height);
        }

        let selected = self.menu_command.clone();
        cef_host::focus_chrome();
        match cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(origin, popup_size))),
                focus: true,
                kind: gpui::WindowKind::PopUp,
                is_movable: false,
                is_resizable: false,
                is_minimizable: false,
                window_background: gpui::WindowBackgroundAppearance::Transparent,
                window_decorations: Some(WindowDecorations::Client),
                ..Default::default()
            },
            move |window, cx| cx.new(|cx| browser_menu::BrowserMenuView::new(selected, window, cx)),
        ) {
            Ok(handle) => self.menu_window = Some(handle),
            Err(error) => eprintln!("failed to open browser menu: {error:?}"),
        }
    }

    /// ⋮ button: first press closes an open menu; otherwise opens one.
    fn on_menu_button_down(&mut self, cx: &mut Context<Self>) {
        if self.close_browser_menu(cx) {
            self.menu_suppress_open = true;
        }
    }

    fn on_menu_button_click(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.menu_suppress_open {
            self.menu_suppress_open = false;
            return;
        }
        if self.browser_menu_is_open(cx) {
            let _ = self.close_browser_menu(cx);
            return;
        }
        self.open_browser_menu(window, cx);
    }

    fn reload(&mut self, _cx: &mut Context<Self>) {
        if self.loading {
            cef_host::stop_load();
        } else {
            cef_host::reload();
        }
        cef_host::focus_content();
    }

    fn hard_reload(&mut self, _cx: &mut Context<Self>) {
        cef_host::reload_ignore_cache();
        cef_host::focus_content();
    }

    fn next_tab(&mut self, cx: &mut Context<Self>) {
        if self.tabs.len() < 2 {
            return;
        }
        let Some(idx) = self.tabs.iter().position(|t| t.id == self.active) else {
            return;
        };
        let next = self.tabs[(idx + 1) % self.tabs.len()].id;
        self.select_tab(next, cx);
    }

    fn prev_tab(&mut self, cx: &mut Context<Self>) {
        if self.tabs.len() < 2 {
            return;
        }
        let Some(idx) = self.tabs.iter().position(|t| t.id == self.active) else {
            return;
        };
        let prev = self.tabs[(idx + self.tabs.len() - 1) % self.tabs.len()].id;
        self.select_tab(prev, cx);
    }

    fn focus_omnibox(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.dismiss_browser_menu(cx);
        self.url_dirty = true;
        let handle = self.omnibox.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
        cef_host::focus_chrome();
        self.omnibox.update(cx, |omni, cx| omni.select_all_text(cx));
        cx.notify();
    }

    fn run_shortcut(
        &mut self,
        shortcut: shortcuts::Shortcut,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use shortcuts::Shortcut;
        match shortcut {
            Shortcut::NewTab => self.new_tab(cx),
            Shortcut::CloseTab => {
                let id = self.active;
                self.close_tab(id, cx);
            }
            Shortcut::NextTab => self.next_tab(cx),
            Shortcut::PrevTab => self.prev_tab(cx),
            Shortcut::FocusOmnibox => self.focus_omnibox(window, cx),
            Shortcut::Reload => self.reload(cx),
            Shortcut::HardReload => self.hard_reload(cx),
            Shortcut::GoBack => {
                cef_host::go_back();
                cef_host::focus_content();
            }
            Shortcut::GoForward => {
                cef_host::go_forward();
                cef_host::focus_content();
            }
            Shortcut::Downloads => self.navigate("miko://#/download-manager", cx),
            Shortcut::Settings => self.navigate("miko://#/settings", cx),
            Shortcut::Print => {
                cef_host::print_page();
                cef_host::focus_content();
            }
            Shortcut::ZoomIn => self.zoom_in(cx),
            Shortcut::ZoomOut => self.zoom_out(cx),
            Shortcut::ZoomReset => self.zoom_reset(cx),
            Shortcut::Exit => {
                cef_host::close_browser();
                cx.quit();
            }
        }
    }

    fn sync_zoom_percent(&mut self) {
        self.zoom_percent = cef_host::zoom_percent();
    }

    fn show_zoom_hud(&mut self, cx: &mut Context<Self>) {
        self.sync_zoom_percent();
        self.zoom_hud_visible = true;
        self.zoom_hud_generation = self.zoom_hud_generation.wrapping_add(1);
        let generation = self.zoom_hud_generation;
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(2200))
                .await;
            this.update(cx, |this, cx| {
                if this.zoom_hud_generation == generation {
                    this.zoom_hud_visible = false;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn zoom_in(&mut self, cx: &mut Context<Self>) {
        cef_host::zoom_in_step();
        self.show_zoom_hud(cx);
    }

    fn zoom_out(&mut self, cx: &mut Context<Self>) {
        cef_host::zoom_out_step();
        self.show_zoom_hud(cx);
    }

    fn zoom_reset(&mut self, cx: &mut Context<Self>) {
        cef_host::zoom_reset();
        self.show_zoom_hud(cx);
    }

    fn run_browser_menu_command(
        &mut self,
        command: browser_menu::BrowserMenuCommand,
        cx: &mut Context<Self>,
    ) {
        use browser_menu::BrowserMenuCommand;
        match command {
            BrowserMenuCommand::NewTab => self.new_tab(cx),
            BrowserMenuCommand::Downloads => self.navigate("miko://#/download-manager", cx),
            BrowserMenuCommand::Settings => self.navigate("miko://#/settings", cx),
            BrowserMenuCommand::ZoomOut => self.zoom_out(cx),
            BrowserMenuCommand::ZoomIn => self.zoom_in(cx),
            BrowserMenuCommand::ZoomReset => self.zoom_reset(cx),
            BrowserMenuCommand::Print => {
                cef_host::print_page();
                cef_host::focus_content();
            }
            BrowserMenuCommand::Exit => {
                cef_host::close_browser();
                cx.quit();
            }
            _ => {}
        }
    }
}

impl Render for Root {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Never open windows during paint — that clears the element arena mid-frame.
        if self.pending_context_menu.is_some() {
            cx.defer_in(window, |this, window, cx| {
                if let Some((x, y, entries)) = this.pending_context_menu.take() {
                    this.open_page_context_menu(x, y, entries, window, cx);
                }
            });
        }
        if self.pending_shortcut.is_some() {
            cx.defer_in(window, |this, window, cx| {
                if let Some(shortcut) = this.pending_shortcut.take() {
                    this.run_shortcut(shortcut, window, cx);
                }
            });
        }

        let can_back = self.can_back;
        let can_forward = self.can_forward;

        // Keep the OS window title (taskbar / Alt-Tab) in sync with the tab.
        let title = self.window_title();
        if title != self.os_title {
            self.os_title = title.clone();
            window.set_window_title(&title);
        }

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(BG))
            .text_color(rgb(TEXT))
            .text_sm()
            .key_context("Browser")
            .on_action(cx.listener(|this, _: &shortcuts::NewTab, window, cx| {
                this.run_shortcut(shortcuts::Shortcut::NewTab, window, cx);
            }))
            .on_action(cx.listener(|this, _: &shortcuts::CloseTab, window, cx| {
                this.run_shortcut(shortcuts::Shortcut::CloseTab, window, cx);
            }))
            .on_action(cx.listener(|this, _: &shortcuts::NextTab, window, cx| {
                this.run_shortcut(shortcuts::Shortcut::NextTab, window, cx);
            }))
            .on_action(cx.listener(|this, _: &shortcuts::PrevTab, window, cx| {
                this.run_shortcut(shortcuts::Shortcut::PrevTab, window, cx);
            }))
            .on_action(cx.listener(|this, _: &shortcuts::FocusOmnibox, window, cx| {
                this.run_shortcut(shortcuts::Shortcut::FocusOmnibox, window, cx);
            }))
            .on_action(cx.listener(|this, _: &shortcuts::Reload, window, cx| {
                this.run_shortcut(shortcuts::Shortcut::Reload, window, cx);
            }))
            .on_action(cx.listener(|this, _: &shortcuts::HardReload, window, cx| {
                this.run_shortcut(shortcuts::Shortcut::HardReload, window, cx);
            }))
            .on_action(cx.listener(|this, _: &shortcuts::GoBack, window, cx| {
                this.run_shortcut(shortcuts::Shortcut::GoBack, window, cx);
            }))
            .on_action(cx.listener(|this, _: &shortcuts::GoForward, window, cx| {
                this.run_shortcut(shortcuts::Shortcut::GoForward, window, cx);
            }))
            .on_action(cx.listener(|this, _: &shortcuts::Downloads, window, cx| {
                this.run_shortcut(shortcuts::Shortcut::Downloads, window, cx);
            }))
            .on_action(cx.listener(|this, _: &shortcuts::Settings, window, cx| {
                this.run_shortcut(shortcuts::Shortcut::Settings, window, cx);
            }))
            .on_action(cx.listener(|this, _: &shortcuts::Print, window, cx| {
                this.run_shortcut(shortcuts::Shortcut::Print, window, cx);
            }))
            .on_action(cx.listener(|this, _: &shortcuts::ZoomIn, window, cx| {
                this.run_shortcut(shortcuts::Shortcut::ZoomIn, window, cx);
            }))
            .on_action(cx.listener(|this, _: &shortcuts::ZoomOut, window, cx| {
                this.run_shortcut(shortcuts::Shortcut::ZoomOut, window, cx);
            }))
            .on_action(cx.listener(|this, _: &shortcuts::ZoomReset, window, cx| {
                this.run_shortcut(shortcuts::Shortcut::ZoomReset, window, cx);
            }))
            .on_action(cx.listener(|this, _: &shortcuts::Exit, window, cx| {
                this.run_shortcut(shortcuts::Shortcut::Exit, window, cx);
            }))
            // Window-level drag tracking: page handlers stop when the pointer
            // leaves the page hitbox, so keep forwarding while a button is down.
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, window, _cx| {
                if this.page_pointer_down {
                    cef_host::send_mouse_move(event, window);
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, event: &MouseUpEvent, window, _cx| {
                    if this.page_pointer_down {
                        this.page_pointer_down = false;
                        cef_host::send_mouse_click_up(event, window);
                    }
                }),
            )
            .on_mouse_up(
                MouseButton::Right,
                cx.listener(|this, event: &MouseUpEvent, window, _cx| {
                    if this.page_pointer_down {
                        this.page_pointer_down = false;
                        cef_host::send_mouse_click_up(event, window);
                    }
                }),
            )
            .on_mouse_up(
                MouseButton::Middle,
                cx.listener(|this, event: &MouseUpEvent, window, _cx| {
                    if this.page_pointer_down {
                        this.page_pointer_down = false;
                        cef_host::send_mouse_click_up(event, window);
                    }
                }),
            )
            .child(self.titlebar(window.is_maximized(), cx))
            .child(self.toolbar(can_back, can_forward, cx))
            .child(
                // Page hit-target: on Linux OSR, GPUI owns input and we forward
                // mouse/scroll/keys into CEF. On Windows the native CEF child gets
                // OS input directly; these handlers still steal focus back.
                div()
                    .id("page")
                    .track_focus(&self.content_focus)
                    .occlude()
                    .relative()
                    .flex()
                    .flex_1()
                    .w_full()
                    .min_h_0()
                    .child(content_surface())
                    .when(self.zoom_hud_visible, |el| el.child(self.zoom_hud(cx)))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, event: &MouseDownEvent, window, cx| {
                            this.dismiss_browser_menu(cx);
                            this.page_pointer_down = true;
                            window.focus(&this.content_focus, cx);
                            cef_host::focus_content();
                            cef_host::send_mouse_click(event, window, false);
                        }),
                    )
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(|this, event: &MouseDownEvent, window, cx| {
                            this.dismiss_browser_menu(cx);
                            this.page_pointer_down = true;
                            window.focus(&this.content_focus, cx);
                            cef_host::focus_content();
                            cef_host::send_mouse_click(event, window, false);
                        }),
                    )
                    .on_mouse_down(
                        MouseButton::Middle,
                        cx.listener(|this, event: &MouseDownEvent, window, cx| {
                            this.dismiss_browser_menu(cx);
                            this.page_pointer_down = true;
                            window.focus(&this.content_focus, cx);
                            cef_host::focus_content();
                            cef_host::send_mouse_click(event, window, false);
                        }),
                    )
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, event: &MouseUpEvent, window, _cx| {
                            this.page_pointer_down = false;
                            cef_host::send_mouse_click_up(event, window);
                        }),
                    )
                    .on_mouse_up(
                        MouseButton::Right,
                        cx.listener(|this, event: &MouseUpEvent, window, _cx| {
                            this.page_pointer_down = false;
                            cef_host::send_mouse_click_up(event, window);
                        }),
                    )
                    .on_mouse_up(
                        MouseButton::Middle,
                        cx.listener(|this, event: &MouseUpEvent, window, _cx| {
                            this.page_pointer_down = false;
                            cef_host::send_mouse_click_up(event, window);
                        }),
                    )
                    .on_mouse_move(cx.listener(|_this, event: &MouseMoveEvent, window, _cx| {
                        cef_host::send_mouse_move(event, window);
                    }))
                    .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, window, cx| {
                        // Ctrl/Cmd + wheel → zoom (Chrome behavior), including 100%.
                        if event.modifiers.control || event.modifiers.platform {
                            let dy = match event.delta {
                                ScrollDelta::Lines(pt) => pt.y,
                                ScrollDelta::Pixels(pt) => f32::from(pt.y),
                            };
                            // GPUI: scroll up is positive Y → zoom in.
                            if dy > 0.0 {
                                this.zoom_in(cx);
                            } else if dy < 0.0 {
                                this.zoom_out(cx);
                            }
                            cx.stop_propagation();
                            return;
                        }
                        cef_host::send_mouse_wheel(event, window);
                    }))
                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                        if let Some(shortcut) = shortcuts::match_shortcut(&event.keystroke) {
                            this.run_shortcut(shortcut, window, cx);
                            cx.stop_propagation();
                            return;
                        }
                        cef_host::send_key_down(event);
                        cx.stop_propagation();
                    }))
                    .on_key_up(cx.listener(|_this, event: &KeyUpEvent, _window, cx| {
                        // Don't forward key-up for chrome shortcuts we already ate.
                        if shortcuts::is_browser_shortcut(&event.keystroke) {
                            cx.stop_propagation();
                            return;
                        }
                        cef_host::send_key_up(event);
                        cx.stop_propagation();
                    })),
            )
    }
}

impl Root {
    fn titlebar(&self, is_maximized: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let active = self.active;
        let loading = self.loading;
        let mut tabs = div().flex().items_end().gap_1().pl_2();
        for tab in &self.tabs {
            let id = tab.id;
            let is_active = id == active;
            let show_spinner = is_active && loading;
            let favicon = tab.favicon.clone();
            tabs = tabs.child(
                div()
                    .id(SharedString::from(format!("tab{id}")))
                    .flex()
                    .items_center()
                    .gap_2()
                    .h(px(32.0))
                    .min_w(px(120.0))
                    .max_w(px(220.0))
                    .px_3()
                    .rounded_t_lg()
                    .bg(rgb(if is_active { TAB_ACTIVE } else { TAB }))
                    .text_color(rgb(if is_active { TEXT } else { MUTED }))
                    .child(tab_leading_icon(id, show_spinner, favicon))
                    .child(div().flex_1().truncate().child(tab.title.clone()))
                    .child(
                        div()
                            .id(SharedString::from(format!("x{id}")))
                            .size(px(16.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_sm()
                            .text_color(rgb(MUTED))
                            .hover(|s| s.bg(rgb(0x50505a)).text_color(rgb(TEXT)))
                            .child(filled_icon("x", 12.0))
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_click(cx.listener(move |this, _, _w, cx| this.close_tab(id, cx))),
                    )
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, window, cx| {
                            this.dismiss_browser_menu(cx);
                            this.select_tab(id, cx);
                            cx.stop_propagation();
                            window.start_window_move();
                        }),
                    ),
            );
        }

        div()
            .flex()
            .items_center()
            .h(px(44.0))
            .pt_1()
            .px_2()
            .child(tabs)
            .child(
                div()
                    .id("newtab")
                    .size(px(30.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .ml_1()
                    .rounded_md()
                    .text_color(rgb(MUTED))
                    .hover(|s| s.bg(rgb(HOVER)).text_color(rgb(TEXT)))
                    .child(filled_icon("plus", 16.0))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(|this, _, _w, cx| this.new_tab(cx))),
            )
            .child(
                // Empty strip between the tabs and the window controls: the main
                // drag handle. On Windows dragging goes through the native
                // WM_NCHITTEST/HTCAPTION path (window_control_area); on other
                // platforms start_window_move drives it.
                //
                // Do NOT put Drag on the outer titlebar — it would cover the
                // min/max/close buttons and turn maximize clicks into caption drags.
                div()
                    .flex_1()
                    .h_full()
                    .window_control_area(WindowControlArea::Drag)
                    .on_mouse_down(MouseButton::Left, |_, window, _| window.start_window_move()),
            )
            .child(window_controls(is_maximized))
    }

    fn toolbar(
        &self,
        can_back: bool,
        can_forward: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .gap_1()
            .h(px(48.0))
            .px_3()
            .bg(rgb(BAR))
            .child(
                nav_btn("back", "arrow-left", can_back).when(can_back, |el| {
                    el.on_click(cx.listener(|this, _, _w, cx| {
                        this.dismiss_browser_menu(cx);
                        this.go_back(cx);
                    }))
                }),
            )
            .child(
                nav_btn("fwd", "arrow-right", can_forward).when(can_forward, |el| {
                    el.on_click(cx.listener(|this, _, _w, cx| {
                        this.dismiss_browser_menu(cx);
                        this.go_forward(cx);
                    }))
                }),
            )
            .child(
                nav_btn("reload", if self.loading { "x" } else { "refresh" }, true)
                    .on_click(cx.listener(|this, _, _w, cx| {
                        this.dismiss_browser_menu(cx);
                        this.reload(cx);
                    })),
            )
            .child(
                div()
                    .flex()
                    .flex_1()
                    .items_center()
                    .gap_2()
                    .h(px(32.0))
                    .mx_1()
                    .px_3()
                    .rounded_lg()
                    .bg(rgb(FIELD))
                    .border_1()
                    .border_color(rgb(BORDER))
                    .child(
                        div()
                            .text_color(rgb(MUTED))
                            .child(filled_icon("lock", 14.0)),
                    )
                    .child(self.omnibox.clone())
                    .when(self.zoom_percent != 100, |el| {
                        el.child(self.zoom_omnibox_chip(cx))
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, window, cx| {
                            this.dismiss_browser_menu(cx);
                            this.url_dirty = true;
                            // Focus the field in GPUI and steal OS keyboard focus
                            // from the CEF child so typed keys reach the omnibox.
                            let handle = this.omnibox.read(cx).focus_handle(cx);
                            window.focus(&handle, cx);
                            cef_host::focus_chrome();
                        }),
                    ),
            )
            .child(
                nav_btn("more", "dots", true)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _window, cx| this.on_menu_button_down(cx)),
                    )
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.on_menu_button_click(window, cx);
                    })),
            )
    }

    /// Persistent Chrome-style zoom chip inside the omnibox when not at 100%.
    fn zoom_omnibox_chip(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let label = SharedString::from(format!("{}%", self.zoom_percent));
        div()
            .id("zoom-chip")
            .flex()
            .items_center()
            .gap_1()
            .h(px(22.0))
            .px_2()
            .rounded_md()
            .bg(rgb(TAB))
            .border_1()
            .border_color(rgb(BORDER))
            .text_xs()
            .text_color(rgb(TEXT))
            .cursor_pointer()
            .hover(|s| s.bg(rgb(HOVER)))
            .child(outline_icon("zoom", 14.0))
            .child(label)
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(|this, _, _window, cx| {
                this.dismiss_browser_menu(cx);
                this.zoom_reset(cx);
            }))
    }

    /// Transient zoom bubble (Chrome-like) over the page after Ctrl+/-/0.
    fn zoom_hud(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let label = SharedString::from(format!("{}%", self.zoom_percent));
        div()
            .id("zoom-hud")
            .absolute()
            .top_3()
            .right_3()
            .flex()
            .items_center()
            .gap_1()
            .h(px(36.0))
            .px_2()
            .rounded_lg()
            .bg(rgb(BAR))
            .border_1()
            .border_color(rgb(BORDER))
            .shadow_lg()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .id("zoom-hud-out")
                    .size(px(28.0))
                    .rounded_md()
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(HOVER)))
                    .child(outline_icon("zoom-out", 16.0))
                    .on_click(cx.listener(|this, _, _window, cx| {
                        this.dismiss_browser_menu(cx);
                        this.zoom_out(cx);
                    })),
            )
            .child(
                div()
                    .id("zoom-hud-label")
                    .min_w(px(52.0))
                    .px_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_sm()
                    .text_color(rgb(TEXT))
                    .cursor_pointer()
                    .child(label)
                    .on_click(cx.listener(|this, _, _window, cx| {
                        this.dismiss_browser_menu(cx);
                        this.zoom_reset(cx);
                    })),
            )
            .child(
                div()
                    .id("zoom-hud-in")
                    .size(px(28.0))
                    .rounded_md()
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(HOVER)))
                    .child(outline_icon("zoom-in", 16.0))
                    .on_click(cx.listener(|this, _, _window, cx| {
                        this.dismiss_browser_menu(cx);
                        this.zoom_in(cx);
                    })),
            )
    }
}

fn tab_leading_icon(
    tab_id: u64,
    loading: bool,
    favicon: Option<std::sync::Arc<Image>>,
) -> impl IntoElement {
    let icon_slot = div()
        .size(px(16.0))
        .flex()
        .items_center()
        .justify_center()
        .overflow_hidden();

    if loading {
        icon_slot.child(outline_icon("loader-2", 14.0).with_animation(
            SharedString::from(format!("tab-spin-{tab_id}")),
            Animation::new(Duration::from_millis(750)).repeat(),
            |svg, delta| svg.with_transformation(Transformation::rotate(percentage(delta))),
        ))
    } else if let Some(favicon) = favicon {
        icon_slot.child(
            img(favicon)
                .size(px(14.0))
                .object_fit(gpui::ObjectFit::Contain),
        )
    } else {
        icon_slot.child(outline_icon("world", 14.0))
    }
}

fn nav_btn(id: &'static str, icon: &'static str, enabled: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .w(px(30.0))
        .h(px(30.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded_lg()
        .text_color(rgb(MUTED))
        .opacity(if enabled { 1.0 } else { 0.35 })
        .when(enabled, |el| {
            el.hover(|s| s.bg(rgb(HOVER)).text_color(rgb(TEXT)))
        })
        .child(outline_icon(icon, 18.0))
}

fn window_controls(is_maximized: bool) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap_1()
        // Keep these as client-area clicks (no WindowControlArea). Mixing
        // HTMAXBUTTON with on_click double-toggles maximize/restore on Windows.
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(
            div()
                .id("min")
                .w(px(34.0))
                .h(px(28.0))
                .flex()
                .items_center()
                .justify_center()
                .font_family(if cfg!(target_os = "windows") {
                    "Segoe Fluent Icons"
                } else {
                    "sans-serif"
                })
                .text_size(px(10.0))
                .text_color(rgb(MUTED))
                .hover(|s| s.bg(rgb(HOVER)).text_color(rgb(TEXT)))
                .child(if cfg!(target_os = "windows") {
                    "\u{e921}"
                } else {
                    "—"
                })
                .on_click(|_, window, _| window.minimize_window()),
        )
        .child(
            div()
                .id("max")
                .w(px(34.0))
                .h(px(28.0))
                .flex()
                .items_center()
                .justify_center()
                .font_family(if cfg!(target_os = "windows") {
                    "Segoe Fluent Icons"
                } else {
                    "sans-serif"
                })
                .text_size(px(10.0))
                .text_color(rgb(MUTED))
                .hover(|s| s.bg(rgb(HOVER)).text_color(rgb(TEXT)))
                .child(if cfg!(target_os = "windows") {
                    if is_maximized { "\u{e923}" } else { "\u{e922}" }
                } else if is_maximized {
                    "❐"
                } else {
                    "▢"
                })
                .on_click(|_, window, _| window.zoom_window()),
        )
        .child(
            div()
                .id("close")
                .w(px(34.0))
                .h(px(28.0))
                .flex()
                .items_center()
                .justify_center()
                .font_family(if cfg!(target_os = "windows") {
                    "Segoe Fluent Icons"
                } else {
                    "sans-serif"
                })
                .text_size(px(10.0))
                .text_color(rgb(MUTED))
                .hover(|s| s.bg(rgb(0xe81123)).text_color(rgb(0xffffff)))
                .child(if cfg!(target_os = "windows") {
                    "\u{e8bb}"
                } else {
                    "✕"
                })
                .on_click(|_, _window, cx| {
                    cef_host::close_browser();
                    cx.quit();
                }),
        )
}

fn title_for_url(url: &str) -> String {
    if url.is_empty() {
        return "Start".into();
    }
    url.trim_start_matches("https://")
        .trim_start_matches("http://")
        .split('/')
        .next()
        .unwrap_or(url)
        .to_string()
}

fn normalize(input: &str) -> String {
    let s = input.trim();
    if s.is_empty() {
        return String::new();
    }
    if let Some(route) = s.strip_prefix("miko://#") {
        return format!("miko://shell/#{route}");
    }
    let has_scheme = [
        "http://",
        "https://",
        "file://",
        "miko://",
        "about:",
        "data:",
        "chrome://",
        "view-source:",
    ]
    .iter()
    .any(|p| s.starts_with(p));
    if has_scheme {
        return s.to_string();
    }
    let looks_like_domain =
        !s.contains(' ') && s.contains('.') && !s.starts_with('.') && !s.ends_with('.');
    if looks_like_domain {
        format!("https://{s}")
    } else {
        format!("https://www.google.com/search?q={}", urlencode(s))
    }
}

fn urlencode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// CEF's cefsimple installs these so a BadWindow / BadMatch from the shared
/// X display doesn't take down the whole browser process.
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
fn install_x11_error_handlers() {
    use std::os::raw::c_int;
    use x11_dl::xlib::{Display, XErrorEvent, Xlib};

    unsafe extern "C" fn ignore_x_error(_display: *mut Display, _event: *mut XErrorEvent) -> c_int {
        0
    }
    unsafe extern "C" fn ignore_x_io_error(_display: *mut Display) -> c_int {
        0
    }

    let Ok(xlib) = Xlib::open() else {
        eprintln!("[boot] warning: could not load libX11 to install error handlers");
        return;
    };
    unsafe {
        (xlib.XSetErrorHandler)(Some(ignore_x_error));
        (xlib.XSetIOErrorHandler)(Some(ignore_x_io_error));
    }
}

fn main() {
    // GPUI's Windows DirectComposition root visual covers native child HWNDs.
    // This browser embeds CEF as a child, so use GPUI's HWND swap-chain path.
    #[cfg(target_os = "windows")]
    unsafe {
        std::env::set_var("GPUI_DISABLE_DIRECT_COMPOSITION", "1");
    }

    // CEF child windows need an X11 parent. Prefer X11/Xwayland over Wayland.
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    unsafe {
        std::env::remove_var("WAYLAND_DISPLAY");
        std::env::remove_var("WAYLAND_SOCKET");
        // Match cefsimple: without these, non-fatal X errors abort the process
        // when CEF (Xlib) and GPUI (xcb) share the display.
        install_x11_error_handlers();
    }

    // Must live for the whole process — MainArgs points into its argv storage.
    let args = cef::args::Args::new();
    let Some(cef_rx) = cef_host::init(&args) else {
        // Helper process — work already done inside execute_process.
        return;
    };

    application().with_assets(Assets).run(|cx: &mut App| {
        omnibox::bind_keys(cx);
        shortcuts::bind_keys(cx);

        cx.on_app_quit(|_cx| async {
            cef_host::shutdown_cef();
        })
        .detach();

        let bounds = Bounds::centered(None, size(px(1280.0), px(820.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Trash Browser".into()),
                    appears_transparent: true,
                    ..Default::default()
                }),
                // Use our custom titlebar (tabs + window controls), not the
                // compositor's. On Windows appears_transparent already hides
                // the system caption; on Linux this requests client-side decor.
                window_decorations: Some(WindowDecorations::Client),
                ..Default::default()
            },
            |window, cx| {
                let omnibox = cx.new(Omnibox::new);
                cx.new(|cx| Root::new(omnibox, cef_rx, window, cx))
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
