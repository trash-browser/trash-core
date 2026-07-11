use crate::popup::{MenuItem, MenuModel};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserMenuCommand {
    NewTab,
    NewWindow,
    History,
    Downloads,
    Bookmarks,
    ZoomOut,
    ZoomReset,
    ZoomIn,
    Print,
    Find,
    MoreTools,
    Settings,
    Help,
    Exit,
}

#[derive(Clone, Copy)]
enum Locale {
    En,
    Th,
    Zh,
    Ja,
}

fn locale() -> Locale {
    let value = sys_locale::get_locale().unwrap_or_default().to_lowercase();
    if value.starts_with("th") {
        Locale::Th
    } else if value.starts_with("zh") {
        Locale::Zh
    } else if value.starts_with("ja") {
        Locale::Ja
    } else {
        Locale::En
    }
}

fn labels(locale: Locale) -> [&'static str; 16] {
    match locale {
        Locale::En => [
            "New tab",
            "New window",
            "History",
            "Downloads",
            "Bookmarks and lists",
            "Zoom out",
            "100%",
            "Zoom in",
            "Print…",
            "Search with Google Lens",
            "Find and edit",
            "More tools",
            "Settings",
            "Help",
            "Exit",
            "Zoom",
        ],
        Locale::Th => [
            "แท็บใหม่",
            "หน้าต่างใหม่",
            "ประวัติการเข้าชม",
            "การดาวน์โหลด",
            "บุ๊กมาร์กและรายการ",
            "ซูมออก",
            "100%",
            "ซูมเข้า",
            "พิมพ์…",
            "ค้นหาด้วย Google Lens",
            "ค้นหาและแก้ไข",
            "เครื่องมือเพิ่มเติม",
            "การตั้งค่า",
            "ความช่วยเหลือ",
            "ออก",
            "ซูม",
        ],
        Locale::Zh => [
            "打开新的标签页",
            "打开新的窗口",
            "历史记录",
            "下载内容",
            "书签和清单",
            "缩小",
            "100%",
            "放大",
            "打印…",
            "使用 Google 智能镜头搜索",
            "查找和修改",
            "更多工具",
            "设置",
            "帮助",
            "退出",
            "缩放",
        ],
        Locale::Ja => [
            "新しいタブ",
            "新しいウィンドウ",
            "履歴",
            "ダウンロード",
            "ブックマークとリスト",
            "縮小",
            "100%",
            "拡大",
            "印刷…",
            "Google レンズで検索",
            "検索と編集",
            "その他のツール",
            "設定",
            "ヘルプ",
            "終了",
            "ズーム",
        ],
    }
}

pub fn model() -> MenuModel {
    let l = labels(locale());
    let action = |id: &str, label: &str, shortcut: Option<&str>, enabled| MenuItem::Action {
        id: id.into(),
        label: label.into(),
        icon: None,
        shortcut: shortcut.map(Into::into),
        enabled,
    };
    MenuModel {
        items: vec![
            action("new-tab", l[0], Some("Ctrl+T"), true),
            action("new-window", l[1], Some("Ctrl+N"), false),
            MenuItem::Separator,
            MenuItem::Submenu {
                label: l[2].into(),
                icon: None,
                items: vec![action("history", l[2], Some("Ctrl+H"), false)],
            },
            action("downloads", l[3], Some("Ctrl+J"), true),
            MenuItem::Submenu {
                label: l[4].into(),
                icon: None,
                items: vec![action("bookmarks", l[4], Some("Ctrl+Shift+O"), false)],
            },
            MenuItem::Separator,
            MenuItem::Submenu {
                label: l[15].into(),
                icon: None,
                items: vec![
                    action("zoom-out", l[5], Some("Ctrl+-"), true),
                    action("zoom-reset", l[6], Some("Ctrl+0"), true),
                    action("zoom-in", l[7], Some("Ctrl++"), true),
                ],
            },
            action("print", l[8], Some("Ctrl+P"), false),
            action("lens", l[9], None, false),
            MenuItem::Submenu {
                label: l[10].into(),
                icon: None,
                items: vec![action("find", l[10], Some("Ctrl+F"), false)],
            },
            MenuItem::Submenu {
                label: l[11].into(),
                icon: None,
                items: vec![action("more-tools", l[11], None, false)],
            },
            MenuItem::Separator,
            action("settings", l[12], None, true),
            MenuItem::Submenu {
                label: l[13].into(),
                icon: None,
                items: vec![action("help", l[13], None, false)],
            },
            MenuItem::Separator,
            action("exit", l[14], None, true),
        ],
    }
}

pub fn command(id: &str) -> Option<BrowserMenuCommand> {
    Some(match id {
        "new-tab" => BrowserMenuCommand::NewTab,
        "new-window" => BrowserMenuCommand::NewWindow,
        "history" => BrowserMenuCommand::History,
        "downloads" => BrowserMenuCommand::Downloads,
        "bookmarks" => BrowserMenuCommand::Bookmarks,
        "zoom-out" => BrowserMenuCommand::ZoomOut,
        "zoom-reset" => BrowserMenuCommand::ZoomReset,
        "zoom-in" => BrowserMenuCommand::ZoomIn,
        "print" => BrowserMenuCommand::Print,
        "find" => BrowserMenuCommand::Find,
        "more-tools" => BrowserMenuCommand::MoreTools,
        "settings" => BrowserMenuCommand::Settings,
        "help" => BrowserMenuCommand::Help,
        "exit" => BrowserMenuCommand::Exit,
        _ => return None,
    })
}

use gpui::{
    Context, FocusHandle, Focusable, IntoElement, MouseButton, ParentElement, Pixels, Render, Size,
    Styled, Window, div, prelude::*, px, rgb, rgba, size,
};
use std::sync::{Arc, Mutex};

const MENU_BODY_WIDTH: f32 = 328.0;
const MENU_ROW_HEIGHT: f32 = 32.0;
const MENU_SEPARATOR_HEIGHT: f32 = 9.0;
const MENU_BODY_PADDING: f32 = 16.0;
// The popup is a native top-level window. Padding the window to make room for
// a GPUI shadow exposes the compositor's clear colour on Windows (the black
// rectangle visible around the old menu). Edge lets DWM provide the outside
// shadow, so the RGBA surface should end exactly at the menu panel.
const SHADOW_MARGIN: f32 = 0.0;

pub fn popup_metrics() -> (Size<Pixels>, Pixels) {
    let model = model();
    let body_height = model
        .items
        .iter()
        .map(|item| match item {
            MenuItem::Separator => MENU_SEPARATOR_HEIGHT,
            _ => MENU_ROW_HEIGHT,
        })
        .sum::<f32>()
        + MENU_BODY_PADDING;
    (
        size(
            px(MENU_BODY_WIDTH + SHADOW_MARGIN * 2.0),
            px(body_height + SHADOW_MARGIN * 2.0),
        ),
        px(SHADOW_MARGIN),
    )
}

pub struct BrowserMenuView {
    model: MenuModel,
    selected: Arc<Mutex<Option<BrowserMenuCommand>>>,
    focus: FocusHandle,
}

impl BrowserMenuView {
    pub fn new(
        selected: Arc<Mutex<Option<BrowserMenuCommand>>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        cx.on_blur(&focus, window, |_this, window, _cx| {
            window.remove_window();
        })
        .detach();
        Self {
            model: model(),
            selected,
            focus,
        }
    }
}

impl Focusable for BrowserMenuView {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for BrowserMenuView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let body_height = self
            .model
            .items
            .iter()
            .map(|item| match item {
                MenuItem::Separator => MENU_SEPARATOR_HEIGHT,
                _ => MENU_ROW_HEIGHT,
            })
            .sum::<f32>()
            + MENU_BODY_PADDING;
        let mut menu = div()
            .flex()
            .flex_col()
            .w(px(MENU_BODY_WIDTH))
            .h(px(body_height))
            .p_2()
            // Match Edge's dark flyout. Keeping a little alpha preserves the
            // native backdrop without making page content legible through it.
            .bg(rgba(0x202020f5))
            .border_1()
            .border_color(rgba(0xffffff20))
            .rounded_lg()
            .shadow_lg()
            .text_color(rgb(0xe8eaed))
            .text_sm();

        for (index, item) in self.model.items.clone().into_iter().enumerate() {
            menu = match item {
                MenuItem::Separator => {
                    menu.child(div().h(px(1.0)).my_1().mx_1().bg(rgba(0xffffff24)))
                }
                MenuItem::Action {
                    id,
                    label,
                    shortcut,
                    enabled,
                    ..
                } => {
                    let selected = self.selected.clone();
                    let command = command(&id);
                    menu.child(menu_row(index, label, shortcut, enabled, None).when(
                        enabled && command.is_some(),
                        move |row| {
                            row.on_mouse_down(MouseButton::Left, move |_, window, _| {
                                if let Ok(mut slot) = selected.lock() {
                                    *slot = command;
                                }
                                window.remove_window();
                            })
                        },
                    ))
                }
                MenuItem::Submenu { label, items, .. } => {
                    let enabled = items.iter().any(MenuItem::enabled);
                    menu.child(menu_row(index, label, None, enabled, Some("›")))
                }
                MenuItem::Check {
                    label,
                    checked,
                    enabled,
                    ..
                } => menu.child(menu_row(
                    index,
                    label,
                    None,
                    enabled,
                    checked.then_some("✓"),
                )),
                MenuItem::Radio {
                    label,
                    selected,
                    enabled,
                    ..
                } => menu.child(menu_row(
                    index,
                    label,
                    None,
                    enabled,
                    selected.then_some("●"),
                )),
            };
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
            .child(menu)
    }
}

fn menu_row(
    index: usize,
    label: String,
    shortcut: Option<String>,
    enabled: bool,
    suffix: Option<&'static str>,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(("browser-menu-row", index))
        .flex()
        .items_center()
        .h(px(32.0))
        .px_3()
        .rounded_md()
        .opacity(if enabled { 1.0 } else { 0.42 })
        .when(enabled, |row| row.hover(|style| style.bg(rgba(0xffffff1f))))
        .child(div().flex_1().child(label))
        .when_some(shortcut, |row, shortcut| {
            row.child(div().ml_4().text_color(rgb(0x9aa0a6)).child(shortcut))
        })
        .when_some(suffix, |row, suffix| {
            row.child(div().ml_3().text_color(rgb(0x9aa0a6)).child(suffix))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn popup_bounds_match_menu_content_and_shadow() {
        let (popup, shadow) = popup_metrics();
        assert_eq!(popup.width, px(328.0));
        assert_eq!(popup.height, px(468.0));
        assert_eq!(shadow, px(0.0));
    }

    #[test]
    fn chrome_menu_contains_core_commands() {
        let model = model();
        assert!(
            model
                .items
                .iter()
                .any(|item| matches!(item, MenuItem::Action { id, .. } if id == "new-tab"))
        );
        assert!(
            model
                .items
                .iter()
                .any(|item| matches!(item, MenuItem::Action { id, .. } if id == "settings"))
        );
        assert!(
            model
                .items
                .iter()
                .any(|item| matches!(item, MenuItem::Action { id, .. } if id == "exit"))
        );
    }
}
