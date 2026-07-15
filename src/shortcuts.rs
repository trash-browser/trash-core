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
        History,
        Settings,
        Print,
        Find,
        FindNext,
        FindPrev,
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
        KeyBinding::new("ctrl-h", History, None),
        KeyBinding::new("cmd-h", History, None),
        KeyBinding::new("ctrl-,", Settings, None),
        KeyBinding::new("cmd-,", Settings, None),
        KeyBinding::new("ctrl-p", Print, None),
        KeyBinding::new("cmd-p", Print, None),
        // Find in page
        KeyBinding::new("ctrl-f", Find, None),
        KeyBinding::new("cmd-f", Find, None),
        KeyBinding::new("f3", FindNext, None),
        KeyBinding::new("ctrl-g", FindNext, None),
        KeyBinding::new("cmd-g", FindNext, None),
        KeyBinding::new("shift-f3", FindPrev, None),
        KeyBinding::new("ctrl-shift-g", FindPrev, None),
        KeyBinding::new("cmd-shift-g", FindPrev, None),
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
    History,
    Settings,
    Print,
    Find,
    FindNext,
    FindPrev,
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
            "f3" if !shift => Some(Shortcut::FindNext),
            "f3" if shift => Some(Shortcut::FindPrev),
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
        (false, "h") => Some(Shortcut::History),
        (false, ",") => Some(Shortcut::Settings),
        (false, "p") => Some(Shortcut::Print),
        (false, "f") => Some(Shortcut::Find),
        (false, "g") => Some(Shortcut::FindNext),
        (true, "g") => Some(Shortcut::FindPrev),
        // Shift may be held for Ctrl++ on layouts where + is Shift+=.
        (_, "=") | (_, "+") => Some(Shortcut::ZoomIn),
        (false, "-") => Some(Shortcut::ZoomOut),
        (false, "0") => Some(Shortcut::ZoomReset),
        (false, "q") => Some(Shortcut::Exit),
        _ => None,
    }
}

/// Match a chrome shortcut from a CEF `KeyEvent` (Windows VK + event flags).
/// Used when the native CEF child owns OS focus and GPUI never sees the key.
pub fn match_cef_key(
    is_keydown: bool,
    windows_vk: i32,
    control: bool,
    shift: bool,
    alt: bool,
    command: bool,
) -> Option<Shortcut> {
    if !is_keydown {
        return None;
    }
    let key = windows_vk_key_name(windows_vk)?;
    let keystroke = Keystroke {
        modifiers: gpui::Modifiers {
            control: control,
            alt,
            shift,
            platform: command,
            function: false,
        },
        key: key.into(),
        key_char: None,
    };
    match_shortcut(&keystroke)
}

fn windows_vk_key_name(vk: i32) -> Option<&'static str> {
    Some(match vk {
        0x09 => "tab",
        0x21 => "pageup",
        0x22 => "pagedown",
        0x25 => "left",
        0x27 => "right",
        0x30 => "0",
        0x31 => "1",
        0x32 => "2",
        0x33 => "3",
        0x34 => "4",
        0x35 => "5",
        0x36 => "6",
        0x37 => "7",
        0x38 => "8",
        0x39 => "9",
        0x41 => "a",
        0x42 => "b",
        0x43 => "c",
        0x44 => "d",
        0x45 => "e",
        0x46 => "f",
        0x47 => "g",
        0x48 => "h",
        0x49 => "i",
        0x4A => "j",
        0x4B => "k",
        0x4C => "l",
        0x4D => "m",
        0x4E => "n",
        0x4F => "o",
        0x50 => "p",
        0x51 => "q",
        0x52 => "r",
        0x53 => "s",
        0x54 => "t",
        0x55 => "u",
        0x56 => "v",
        0x57 => "w",
        0x58 => "x",
        0x59 => "y",
        0x5A => "z",
        0x70 => "f1",
        0x71 => "f2",
        0x72 => "f3",
        0x73 => "f4",
        0x74 => "f5",
        0x75 => "f6",
        0x76 => "f7",
        0x77 => "f8",
        0x78 => "f9",
        0x79 => "f10",
        0x7A => "f11",
        0x7B => "f12",
        // OEM_PLUS / numpad add → zoom in (= / +)
        0xBB | 0x6B => "=",
        // OEM_MINUS / numpad subtract
        0xBD | 0x6D => "-",
        // OEM_COMMA
        0xBC => ",",
        _ => return None,
    })
}
