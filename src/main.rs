//! Trash Browser — GPUI chrome + CEF content (X11 child window).

mod cef_host;
mod content;
mod omnibox;

use std::sync::mpsc::Receiver;
use std::time::Duration;

use gpui::{
    App, Bounds, Context, Entity, MouseButton, SharedString, TitlebarOptions, Window, WindowBounds,
    WindowOptions, div, prelude::*, px, rgb, size,
};
use gpui_platform::application;
use raw_window_handle::{HasWindowHandle as RawHasWindowHandle, RawWindowHandle};

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

struct Tab {
    id: u64,
    title: SharedString,
    url: String,
}

impl Tab {
    fn blank(id: u64, title: impl Into<SharedString>) -> Self {
        Self {
            id,
            title: title.into(),
            url: String::new(),
        }
    }
}

struct Root {
    tabs: Vec<Tab>,
    active: u64,
    next_id: u64,
    omnibox: Entity<Omnibox>,
    can_back: bool,
    can_forward: bool,
    loading: bool,
    /// Don't clobber the omnibox while the user is editing.
    url_dirty: bool,
    cef_rx: Receiver<CefEvent>,
    start_uri: String,
}

impl Root {
    fn new(omnibox: Entity<Omnibox>, cef_rx: Receiver<CefEvent>, cx: &mut Context<Self>) -> Self {
        cx.subscribe(&omnibox, |this, _emitter, event: &OmniboxEvent, cx| {
            match event {
                OmniboxEvent::Submit(text) => {
                    this.url_dirty = false;
                    this.navigate(text, cx);
                }
            }
        })
        .detach();

        // External CEF message pump + UI event drain.
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(10))
                    .await;
                this.update(cx, |this, cx| {
                    this.pump_cef(cx);
                })
                .ok();
            }
        })
        .detach();

        Self {
            tabs: vec![Tab::blank(1, "Start")],
            active: 1,
            next_id: 2,
            omnibox,
            can_back: false,
            can_forward: false,
            loading: false,
            url_dirty: false,
            cef_rx,
            start_uri: cef_host::start_uri(),
        }
    }

    fn pump_cef(&mut self, cx: &mut Context<Self>) {
        let mut events = Vec::new();
        cef_host::pump(&self.cef_rx, &mut events);
        for ev in events {
            self.on_cef_event(ev, cx);
        }
    }

    fn on_cef_event(&mut self, ev: CefEvent, cx: &mut Context<Self>) {
        match ev {
            CefEvent::Ready | CefEvent::Created => {
                let url = self.active_content_url();
                cef_host::navigate(&url);
            }
            CefEvent::Closed => {}
            CefEvent::Address(url) => {
                let is_start = url == self.start_uri || url.starts_with("data:text/html");
                let shown = if is_start { String::new() } else { url };
                if let Some(tab) = self.active_tab_mut() {
                    tab.url = shown.clone();
                }
                if !self.url_dirty {
                    self.omnibox
                        .update(cx, |omni, cx| omni.set_text(shown, cx));
                }
                cx.notify();
            }
            CefEvent::Title(title) => {
                if let Some(tab) = self.active_tab_mut() {
                    tab.title = if title.is_empty() {
                        "New Tab".into()
                    } else {
                        title.into()
                    };
                }
                cx.notify();
            }
            CefEvent::Loading {
                loading,
                can_back,
                can_forward,
            } => {
                self.loading = loading;
                self.can_back = can_back;
                self.can_forward = can_forward;
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

    fn active_content_url(&self) -> String {
        match self.active_tab() {
            Some(t) if t.url.is_empty() => self.start_uri.clone(),
            Some(t) => t.url.clone(),
            None => self.start_uri.clone(),
        }
    }

    fn sync_omnibox(&self, cx: &mut Context<Self>) {
        let url = self
            .active_tab()
            .map(|t| t.url.clone())
            .unwrap_or_default();
        self.omnibox.update(cx, |omni, cx| omni.set_text(url, cx));
    }

    fn navigate(&mut self, input: &str, cx: &mut Context<Self>) {
        let target = normalize(input);
        let load = if target.is_empty() {
            if let Some(tab) = self.active_tab_mut() {
                tab.url.clear();
                tab.title = "Start".into();
            }
            self.start_uri.clone()
        } else {
            if let Some(tab) = self.active_tab_mut() {
                tab.url = target.clone();
                tab.title = title_for_url(&target).into();
            }
            target
        };
        self.url_dirty = false;
        self.sync_omnibox(cx);
        cef_host::navigate(&load);
        cx.notify();
    }

    fn new_tab(&mut self, cx: &mut Context<Self>) {
        let id = self.next_id;
        self.next_id += 1;
        self.tabs.push(Tab::blank(id, "New Tab"));
        self.active = id;
        self.url_dirty = false;
        self.sync_omnibox(cx);
        cef_host::navigate(&self.start_uri);
        cx.notify();
    }

    fn select_tab(&mut self, id: u64, cx: &mut Context<Self>) {
        if !self.tabs.iter().any(|t| t.id == id) {
            return;
        }
        self.active = id;
        self.url_dirty = false;
        self.sync_omnibox(cx);
        let url = self.active_content_url();
        cef_host::navigate(&url);
        cx.notify();
    }

    fn close_tab(&mut self, id: u64, cx: &mut Context<Self>) {
        let Some(i) = self.tabs.iter().position(|t| t.id == id) else {
            return;
        };
        self.tabs.remove(i);
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
            cef_host::navigate(&url);
        }
        cx.notify();
    }

    fn go_back(&mut self, _cx: &mut Context<Self>) {
        cef_host::go_back();
    }

    fn go_forward(&mut self, _cx: &mut Context<Self>) {
        cef_host::go_forward();
    }

    fn reload(&mut self, _cx: &mut Context<Self>) {
        if self.loading {
            cef_host::stop_load();
        } else {
            cef_host::reload();
        }
    }
}

impl Render for Root {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let can_back = self.can_back;
        let can_forward = self.can_forward;

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(BG))
            .text_color(rgb(TEXT))
            .text_sm()
            .child(self.titlebar(cx))
            .child(self.toolbar(can_back, can_forward, cx))
            .child(content_surface())
    }
}

impl Root {
    fn titlebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let active = self.active;
        let mut tabs = div().flex().items_end().gap_1().pl_2();
        for tab in &self.tabs {
            let id = tab.id;
            let is_active = id == active;
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
                            .child("✕")
                            .on_click(cx.listener(move |this, _, _w, cx| this.close_tab(id, cx))),
                    )
                    .on_click(cx.listener(move |this, _, _w, cx| this.select_tab(id, cx))),
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
                    .child("+")
                    .on_click(cx.listener(|this, _, _w, cx| this.new_tab(cx))),
            )
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .on_mouse_down(MouseButton::Left, |_, window, _| window.start_window_move()),
            )
            .child(window_controls())
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
                nav_btn("back", "‹", can_back).when(can_back, |el| {
                    el.on_click(cx.listener(|this, _, _w, cx| this.go_back(cx)))
                }),
            )
            .child(
                nav_btn("fwd", "›", can_forward).when(can_forward, |el| {
                    el.on_click(cx.listener(|this, _, _w, cx| this.go_forward(cx)))
                }),
            )
            .child(
                nav_btn("reload", if self.loading { "✕" } else { "⟳" }, true)
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
                    .child(div().text_color(rgb(MUTED)).child("🔒"))
                    .child(self.omnibox.clone())
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _w, _cx| {
                            this.url_dirty = true;
                        }),
                    ),
            )
            .child(
                nav_btn("share", "⤴", true)
                    .on_click(cx.listener(|this, _, _w, cx| this.reload(cx))),
            )
    }
}

fn nav_btn(id: &'static str, glyph: &'static str, enabled: bool) -> gpui::Stateful<gpui::Div> {
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
        .when(enabled, |el| el.hover(|s| s.bg(rgb(HOVER)).text_color(rgb(TEXT))))
        .child(glyph)
}

fn window_controls() -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap_1()
        .child(
            div()
                .id("min")
                .w(px(34.0))
                .h(px(28.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded_md()
                .text_color(rgb(MUTED))
                .hover(|s| s.bg(rgb(HOVER)).text_color(rgb(TEXT)))
                .child("—")
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
                .rounded_md()
                .text_color(rgb(MUTED))
                .hover(|s| s.bg(rgb(HOVER)).text_color(rgb(TEXT)))
                .child("▢")
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
                .rounded_md()
                .text_color(rgb(MUTED))
                .hover(|s| s.bg(rgb(0xe81123)).text_color(rgb(0xffffff)))
                .child("✕")
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
    let has_scheme = [
        "http://",
        "https://",
        "file://",
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
    let looks_like_domain = !s.contains(' ')
        && s.contains('.')
        && !s.starts_with('.')
        && !s.ends_with('.');
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

fn main() {
    // CEF child windows need an X11 parent. Prefer X11/Xwayland over Wayland.
    unsafe {
        std::env::remove_var("WAYLAND_DISPLAY");
    }

    // Must live for the whole process — MainArgs points into its argv storage.
    let args = cef::args::Args::new();
    let Some(cef_rx) = cef_host::init(&args) else {
        // Helper process — work already done inside execute_process.
        return;
    };

    application().run(|cx: &mut App| {
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
                    appears_transparent: true,
                    ..Default::default()
                }),
                ..Default::default()
            },
            |window, cx| {
                // Seed parent XID early if available.
                if let Ok(handle) = RawHasWindowHandle::window_handle(window) {
                    let xid = match handle.as_raw() {
                        RawWindowHandle::Xlib(h) => Some(h.window as u64),
                        RawWindowHandle::Xcb(h) => Some(h.window.get() as u64),
                        _ => None,
                    };
                    if let Some(xid) = xid {
                        cef_host::set_parent_bounds(
                            xid,
                            cef::Rect {
                                x: 0,
                                y: 92,
                                width: 1280,
                                height: 728,
                            },
                        );
                    }
                }

                let omnibox = cx.new(Omnibox::new);
                cx.new(|cx| Root::new(omnibox, cef_rx, cx))
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
