//! Trash Browser — GPUI chrome + CEF content in a native child window.
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod cef_host;
mod content;
mod omnibox;

use std::sync::mpsc::Receiver;
use std::time::Duration;

use gpui::{
    Animation, AnimationExt as _, App, AssetSource, Bounds, Context, Entity, FocusHandle,
    Focusable, Image, ImageFormat, KeyDownEvent, KeyUpEvent, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, ScrollWheelEvent, SharedString, TitlebarOptions, Transformation,
    Window, WindowBounds, WindowControlArea, WindowDecorations, WindowOptions, div, img,
    percentage, prelude::*, px, rgb, size, svg,
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
        let mut events = Vec::new();
        cef_host::pump(&self.cef_rx, &mut events);
        if events.is_empty() {
            return;
        }
        for ev in events {
            self.on_cef_event(ev, cx);
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
        cx.notify();
    }

    fn close_tab(&mut self, id: u64, cx: &mut Context<Self>) {
        let Some(i) = self.tabs.iter().position(|t| t.id == id) else {
            return;
        };
        self.tabs.remove(i);
        // Tear down this tab's browser regardless of whether it was active.
        cef_host::close_tab(id);
        if self.tabs.is_empty() {
            cef_host::close_browser();
            cx.quit();
            return;
        }
        if self.active == id {
            let neighbor = i.min(self.tabs.len() - 1);
            self.active = self.tabs[neighbor].id;
            self.url_dirty = false;
            self.sync_omnibox(cx);
            let url = self.active_content_url();
            cef_host::show_tab(self.active, &url);
        }
        cx.notify();
    }

    /// Retire the chrome tab whose browser closed itself. Its CEF browser is
    /// already gone, so unlike `close_tab` this does not call into `cef_host` to
    /// close anything — it only fixes up the tab strip and active selection.
    fn on_browser_closed(&mut self, tab_id: u64, cx: &mut Context<Self>) {
        let Some(i) = self.tabs.iter().position(|t| t.id == tab_id) else {
            return;
        };
        self.tabs.remove(i);
        if self.tabs.is_empty() {
            cef_host::close_browser();
            cx.quit();
            return;
        }
        if self.active == tab_id {
            let neighbor = i.min(self.tabs.len() - 1);
            self.active = self.tabs[neighbor].id;
            self.url_dirty = false;
            self.sync_omnibox(cx);
            let url = self.active_content_url();
            cef_host::show_tab(self.active, &url);
        }
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

    fn reload(&mut self, _cx: &mut Context<Self>) {
        if self.loading {
            cef_host::stop_load();
        } else {
            cef_host::reload();
        }
        cef_host::focus_content();
    }
}

impl Render for Root {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
                    .flex()
                    .flex_1()
                    .w_full()
                    .min_h_0()
                    .child(content_surface())
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, event: &MouseDownEvent, window, cx| {
                            this.page_pointer_down = true;
                            window.focus(&this.content_focus, cx);
                            cef_host::focus_content();
                            cef_host::send_mouse_click(event, window, false);
                        }),
                    )
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(|this, event: &MouseDownEvent, window, cx| {
                            this.page_pointer_down = true;
                            window.focus(&this.content_focus, cx);
                            cef_host::focus_content();
                            cef_host::send_mouse_click(event, window, false);
                        }),
                    )
                    .on_mouse_down(
                        MouseButton::Middle,
                        cx.listener(|this, event: &MouseDownEvent, window, cx| {
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
                    .on_scroll_wheel(cx.listener(|_this, event: &ScrollWheelEvent, window, _cx| {
                        cef_host::send_mouse_wheel(event, window);
                    }))
                    .on_key_down(cx.listener(|_this, event: &KeyDownEvent, _window, cx| {
                        cef_host::send_key_down(event);
                        cx.stop_propagation();
                    }))
                    .on_key_up(cx.listener(|_this, event: &KeyUpEvent, _window, cx| {
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
                    el.on_click(cx.listener(|this, _, _w, cx| this.go_back(cx)))
                }),
            )
            .child(
                nav_btn("fwd", "arrow-right", can_forward).when(can_forward, |el| {
                    el.on_click(cx.listener(|this, _, _w, cx| this.go_forward(cx)))
                }),
            )
            .child(
                nav_btn("reload", if self.loading { "x" } else { "refresh" }, true)
                    .on_click(cx.listener(|this, _, _w, cx| this.reload(cx))),
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
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, window, cx| {
                            this.url_dirty = true;
                            // Focus the field in GPUI and steal OS keyboard focus
                            // from the CEF child so typed keys reach the omnibox.
                            let handle = this.omnibox.read(cx).focus_handle(cx);
                            window.focus(&handle, cx);
                            cef_host::focus_chrome();
                        }),
                    ),
            )
            .child(nav_btn("more", "dots", true))
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
