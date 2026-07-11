//! Browser chrome keyboard shortcuts (Ctrl+T, Ctrl+L, F5, …).
//!
//! Bound globally via GPUI's keymap, and also intercepted in the page
//! key-forward path so they still work while CEF/OSR content has focus.

use gpui::{App, KeyBinding, Keystroke, actions};

actions!(
    browser,
    [
        NewTab,
        CloseTab,
        NextTab,
        PrevTab,
        FocusOmnibox,
        Reload,
        HardReload,
        GoBack,
        GoForward,
        Downloads,
        Settings,
        Print,
        ZoomIn,
        ZoomOut,
        ZoomReset,
        Exit,
    ]
);

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        // Tabs / windows
        KeyBinding::new("ctrl-t", NewTab, None),
        KeyBinding::new("cmd-t", NewTab, None),
        KeyBinding::new("ctrl-w", CloseTab, None),
        KeyBinding::new("cmd-w", CloseTab, None),
        KeyBinding::new("ctrl-f4", CloseTab, None),
        KeyBinding::new("ctrl-tab", NextTab, None),
        KeyBinding::new("ctrl-pagedown", NextTab, None),
        KeyBinding::new("ctrl-shift-tab", PrevTab, None),
        KeyBinding::new("ctrl-pageup", PrevTab, None),
        // Navigation / address
        KeyBinding::new("ctrl-l", FocusOmnibox, None),
        KeyBinding::new("cmd-l", FocusOmnibox, None),
        KeyBinding::new("alt-d", FocusOmnibox, None),
        KeyBinding::new("f6", FocusOmnibox, None),
        KeyBinding::new("ctrl-r", Reload, None),
        KeyBinding::new("cmd-r", Reload, None),
        KeyBinding::new("f5", Reload, None),
        KeyBinding::new("ctrl-shift-r", HardReload, None),
        KeyBinding::new("cmd-shift-r", HardReload, None),
        KeyBinding::new("ctrl-f5", HardReload, None),
        KeyBinding::new("alt-left", GoBack, None),
        KeyBinding::new("alt-right", GoForward, None),
        // Chrome pages
        KeyBinding::new("ctrl-j", Downloads, None),
        KeyBinding::new("cmd-j", Downloads, None),
        KeyBinding::new("ctrl-,", Settings, None),
        KeyBinding::new("cmd-,", Settings, None),
        KeyBinding::new("ctrl-p", Print, None),
        KeyBinding::new("cmd-p", Print, None),
        // Zoom
        KeyBinding::new("ctrl-=", ZoomIn, None),
        KeyBinding::new("cmd-=", ZoomIn, None),
        KeyBinding::new("ctrl-+", ZoomIn, None),
        KeyBinding::new("cmd-+", ZoomIn, None),
        KeyBinding::new("ctrl--", ZoomOut, None),
        KeyBinding::new("cmd--", ZoomOut, None),
        KeyBinding::new("ctrl-0", ZoomReset, None),
        KeyBinding::new("cmd-0", ZoomReset, None),
        // App
        KeyBinding::new("ctrl-q", Exit, None),
        KeyBinding::new("cmd-q", Exit, None),
    ]);
}

/// Returns true when the keystroke is a chrome shortcut we handle (so the page
/// should not receive it).
pub fn is_browser_shortcut(keystroke: &Keystroke) -> bool {
    match_shortcut(keystroke).is_some()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shortcut {
    NewTab,
    CloseTab,
    NextTab,
    PrevTab,
    FocusOmnibox,
    Reload,
    HardReload,
    GoBack,
    GoForward,
    Downloads,
    Settings,
    Print,
    ZoomIn,
    ZoomOut,
    ZoomReset,
    Exit,
}

pub fn match_shortcut(keystroke: &Keystroke) -> Option<Shortcut> {
    let ctrl = keystroke.modifiers.control || keystroke.modifiers.platform;
    let shift = keystroke.modifiers.shift;
    let alt = keystroke.modifiers.alt;
    let key = keystroke.key.as_str();

    // Function keys (no modifiers required, shift optional for hard reload via Ctrl+F5)
    if !ctrl && !alt {
        return match key {
            "f5" if !shift => Some(Shortcut::Reload),
            "f6" => Some(Shortcut::FocusOmnibox),
            _ => None,
        };
    }

    if alt && !ctrl {
        return match key {
            "left" => Some(Shortcut::GoBack),
            "right" => Some(Shortcut::GoForward),
            "d" => Some(Shortcut::FocusOmnibox),
            _ => None,
        };
    }

    if !ctrl {
        return None;
    }

    match (shift, key) {
        (false, "t") => Some(Shortcut::NewTab),
        (false, "w") | (false, "f4") => Some(Shortcut::CloseTab),
        (false, "tab") | (false, "pagedown") => Some(Shortcut::NextTab),
        (true, "tab") | (false, "pageup") => Some(Shortcut::PrevTab),
        (false, "l") => Some(Shortcut::FocusOmnibox),
        (false, "r") => Some(Shortcut::Reload),
        (true, "r") | (false, "f5") => Some(Shortcut::HardReload),
        (false, "j") => Some(Shortcut::Downloads),
        (false, ",") => Some(Shortcut::Settings),
        (false, "p") => Some(Shortcut::Print),
        (false, "=") | (false, "+") => Some(Shortcut::ZoomIn),
        (false, "-") => Some(Shortcut::ZoomOut),
        (false, "0") => Some(Shortcut::ZoomReset),
        (false, "q") => Some(Shortcut::Exit),
        _ => None,
    }
}
