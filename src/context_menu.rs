//! Page context menu (right-click) shown as a GPUI popup.
//!
//! CEF's native context menu is unreliable under Linux OSR because OS focus
//! stays on the chrome window, so the menu cannot be dismissed. We intercept
//! `run_context_menu`, render a dismissible GPUI popup, and complete the CEF
//! callback when the user picks an item or clicks away.

use gpui::{
    Context, FocusHandle, Focusable, IntoElement, MouseButton, ParentElement, Pixels, Render, Size,
    Styled, Window, div, prelude::*, px, rgb, rgba, size,
};
use std::sync::{Arc, Mutex};

const ROW_H: f32 = 30.0;
const SEP_H: f32 = 9.0;
const PAD: f32 = 12.0;
const WIDTH: f32 = 220.0;

#[derive(Debug, Clone)]
pub struct ContextMenuEntry {
    pub command_id: i32,
    pub label: String,
    pub enabled: bool,
    pub separator: bool,
}

#[derive(Clone, Copy)]
pub enum ContextMenuResult {
    Command(i32),
    Cancelled,
}

pub fn popup_size(entries: &[ContextMenuEntry]) -> Size<Pixels> {
    let height = entries
        .iter()
        .map(|e| if e.separator { SEP_H } else { ROW_H })
        .sum::<f32>()
        + PAD;
    size(px(WIDTH), px(height.max(ROW_H + PAD)))
}

pub struct ContextMenuView {
    entries: Vec<ContextMenuEntry>,
    result: Arc<Mutex<Option<ContextMenuResult>>>,
    focus: FocusHandle,
}

impl ContextMenuView {
    pub fn new(
        entries: Vec<ContextMenuEntry>,
        result: Arc<Mutex<Option<ContextMenuResult>>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        cx.on_blur(&focus, window, |this, window, _cx| {
            if let Ok(mut slot) = this.result.lock() {
                if slot.is_none() {
                    *slot = Some(ContextMenuResult::Cancelled);
                }
            }
            window.remove_window();
        })
        .detach();
        cx.observe_window_activation(window, |this, window, _cx| {
            if !window.is_window_active() {
                if let Ok(mut slot) = this.result.lock() {
                    if slot.is_none() {
                        *slot = Some(ContextMenuResult::Cancelled);
                    }
                }
                window.remove_window();
            }
        })
        .detach();
        Self {
            entries,
            result,
            focus,
        }
    }
}

impl Focusable for ContextMenuView {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for ContextMenuView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let height = self
            .entries
            .iter()
            .map(|e| if e.separator { SEP_H } else { ROW_H })
            .sum::<f32>()
            + PAD;

        let mut menu = div()
            .flex()
            .flex_col()
            .w(px(WIDTH))
            .h(px(height))
            .p_1()
            .bg(rgba(0x202020f5))
            .border_1()
            .border_color(rgba(0xffffff20))
            .rounded_lg()
            .shadow_lg()
            .text_color(rgb(0xe8eaed))
            .text_sm();

        for (index, entry) in self.entries.iter().enumerate() {
            if entry.separator {
                menu = menu.child(div().h(px(1.0)).my_1().mx_1().bg(rgba(0xffffff24)));
                continue;
            }
            let result = self.result.clone();
            let command_id = entry.command_id;
            let enabled = entry.enabled;
            let label = entry.label.clone();
            menu = menu.child(
                div()
                    .id(("ctx-menu-row", index))
                    .flex()
                    .items_center()
                    .h(px(ROW_H))
                    .px_3()
                    .rounded_md()
                    .opacity(if enabled { 1.0 } else { 0.42 })
                    .when(enabled, |row| row.hover(|s| s.bg(rgba(0xffffff1f))))
                    .child(div().flex_1().child(label))
                    .when(enabled, |row| {
                        row.on_mouse_down(MouseButton::Left, move |_, window, _| {
                            if let Ok(mut slot) = result.lock() {
                                *slot = Some(ContextMenuResult::Command(command_id));
                            }
                            window.remove_window();
                        })
                    }),
            );
        }

        div()
            .flex()
            .track_focus(&self.focus)
            .on_key_down(|event, window, cx| {
                if event.keystroke.key == "escape" {
                    window.remove_window();
                    cx.stop_propagation();
                }
            })
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
            .child(menu)
    }
}
