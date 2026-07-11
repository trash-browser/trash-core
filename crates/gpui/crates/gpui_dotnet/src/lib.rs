use std::{ffi::CStr, os::raw::c_char, ptr};

use gpui::{
    App, AppContext, Bounds, Context, IntoElement, ParentElement, Render, SharedString, Styled,
    TitlebarOptions, Window, WindowBounds, WindowOptions, div, px, rgb, rgba, size,
};

#[cfg(target_os = "windows")]
fn enable_process_dpi_awareness() {
    use windows::Win32::UI::HiDpi::{
        DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
    };

    // .NET hosts do not use GPUI's Rust executable manifest. Opt the process
    // into the same PerMonitorV2 DPI mode at runtime. This can fail if the host
    // already picked a DPI mode; ignoring that failure is safe.
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
}

#[cfg(not(target_os = "windows"))]
fn enable_process_dpi_awareness() {}

#[derive(Clone)]
enum ChildNode {
    Element(DotnetElement),
    Text(String),
}

#[derive(Clone)]
enum StyleOp {
    Absolute,
    Background(u32),
    BackgroundRgba(u32),
    Border1,
    BorderColor(u32),
    BorderDashed,
    Flex,
    Flex1,
    FlexCol,
    FlexRow,
    Gap(f32),
    GapX(f32),
    GapY(f32),
    Grid,
    Height(f32),
    Hidden,
    ItemsCenter,
    ItemsEnd,
    ItemsStart,
    JustifyBetween,
    JustifyCenter,
    JustifyEnd,
    JustifyStart,
    Margin(f32),
    MarginX(f32),
    MarginY(f32),
    Opacity(f32),
    Padding(f32),
    PaddingX(f32),
    PaddingY(f32),
    Relative,
    RoundedFull,
    RoundedMd,
    ShadowLg,
    Text2xl,
    TextColor(u32),
    TextLg,
    TextSm,
    TextXl,
    Width(f32),
}

#[derive(Clone, Default)]
pub struct DotnetElement {
    children: Vec<ChildNode>,
    styles: Vec<StyleOp>,
}

impl DotnetElement {
    fn push_style(&mut self, style: StyleOp) {
        self.styles.push(style);
    }

    fn to_gpui_element(&self) -> gpui::AnyElement {
        let mut element = div();

        for style in &self.styles {
            element = match style {
                StyleOp::Absolute => element.absolute(),
                StyleOp::Background(hex) => element.bg(rgb(*hex)),
                StyleOp::BackgroundRgba(hex) => element.bg(rgba(*hex)),
                StyleOp::Border1 => element.border_1(),
                StyleOp::BorderColor(hex) => element.border_color(rgb(*hex)),
                StyleOp::BorderDashed => element.border_dashed(),
                StyleOp::Flex => element.flex(),
                StyleOp::Flex1 => element.flex_1(),
                StyleOp::FlexCol => element.flex_col(),
                StyleOp::FlexRow => element.flex_row(),
                StyleOp::Gap(value) => element.gap(px(*value)),
                StyleOp::GapX(value) => element.gap_x(px(*value)),
                StyleOp::GapY(value) => element.gap_y(px(*value)),
                StyleOp::Grid => element.grid(),
                StyleOp::Height(value) => element.h(px(*value)),
                StyleOp::Hidden => element.hidden(),
                StyleOp::ItemsCenter => element.items_center(),
                StyleOp::ItemsEnd => element.items_end(),
                StyleOp::ItemsStart => element.items_start(),
                StyleOp::JustifyBetween => element.justify_between(),
                StyleOp::JustifyCenter => element.justify_center(),
                StyleOp::JustifyEnd => element.justify_end(),
                StyleOp::JustifyStart => element.justify_start(),
                StyleOp::Margin(value) => element.m(px(*value)),
                StyleOp::MarginX(value) => element.mx(px(*value)),
                StyleOp::MarginY(value) => element.my(px(*value)),
                StyleOp::Opacity(value) => element.opacity(*value),
                StyleOp::Padding(value) => element.p(px(*value)),
                StyleOp::PaddingX(value) => element.px(px(*value)),
                StyleOp::PaddingY(value) => element.py(px(*value)),
                StyleOp::Relative => element.relative(),
                StyleOp::RoundedFull => element.rounded_full(),
                StyleOp::RoundedMd => element.rounded_md(),
                StyleOp::ShadowLg => element.shadow_lg(),
                StyleOp::Text2xl => element.text_2xl(),
                StyleOp::TextColor(hex) => element.text_color(rgb(*hex)),
                StyleOp::TextLg => element.text_lg(),
                StyleOp::TextSm => element.text_sm(),
                StyleOp::TextXl => element.text_xl(),
                StyleOp::Width(value) => element.w(px(*value)),
            };
        }

        for child in &self.children {
            element = match child {
                ChildNode::Element(child) => element.child(child.to_gpui_element()),
                ChildNode::Text(text) => element.child(text.clone()),
            };
        }

        element.into_any_element()
    }
}

struct DotnetRootView {
    root: DotnetElement,
}

impl Render for DotnetRootView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        self.root.to_gpui_element()
    }
}

unsafe fn element_from_ptr<'a>(element: *mut DotnetElement) -> Option<&'a mut DotnetElement> {
    unsafe { element.as_mut() }
}

unsafe fn element_ref_from_ptr<'a>(element: *const DotnetElement) -> Option<&'a DotnetElement> {
    unsafe { element.as_ref() }
}

unsafe fn string_from_ptr(value: *const c_char) -> Option<String> {
    if value.is_null() {
        return None;
    }

    unsafe { CStr::from_ptr(value) }
        .to_str()
        .ok()
        .map(ToString::to_string)
}

fn with_element(
    element: *mut DotnetElement,
    f: impl FnOnce(&mut DotnetElement),
) -> *mut DotnetElement {
    unsafe {
        if let Some(element) = element_from_ptr(element) {
            f(element);
        }
    }

    element
}

fn with_style(element: *mut DotnetElement, style: StyleOp) -> *mut DotnetElement {
    with_element(element, |element| element.push_style(style))
}

#[unsafe(no_mangle)]
pub extern "C" fn gpui_dotnet_element_new() -> *mut DotnetElement {
    Box::into_raw(Box::new(DotnetElement::default()))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn gpui_dotnet_element_clone(
    element: *const DotnetElement,
) -> *mut DotnetElement {
    let Some(element) = (unsafe { element_ref_from_ptr(element) }) else {
        return ptr::null_mut();
    };

    Box::into_raw(Box::new(element.clone()))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn gpui_dotnet_element_free(element: *mut DotnetElement) {
    if !element.is_null() {
        drop(unsafe { Box::from_raw(element) });
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn gpui_dotnet_element_child(
    element: *mut DotnetElement,
    child: *const DotnetElement,
) -> *mut DotnetElement {
    with_element(element, |element| unsafe {
        if let Some(child) = element_ref_from_ptr(child) {
            element.children.push(ChildNode::Element(child.clone()));
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn gpui_dotnet_element_child_text(
    element: *mut DotnetElement,
    text: *const c_char,
) -> *mut DotnetElement {
    with_element(element, |element| unsafe {
        if let Some(text) = string_from_ptr(text) {
            element.children.push(ChildNode::Text(text));
        }
    })
}

macro_rules! style_fn {
    ($name:ident, $style:expr) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn $name(element: *mut DotnetElement) -> *mut DotnetElement {
            with_style(element, $style)
        }
    };
    ($name:ident, $value:ident: $value_ty:ty, $style:expr) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn $name(
            element: *mut DotnetElement,
            $value: $value_ty,
        ) -> *mut DotnetElement {
            with_style(element, $style)
        }
    };
}

style_fn!(gpui_dotnet_element_absolute, StyleOp::Absolute);
style_fn!(gpui_dotnet_element_bg, hex: u32, StyleOp::Background(hex));
style_fn!(gpui_dotnet_element_bg_rgba, hex: u32, StyleOp::BackgroundRgba(hex));
style_fn!(gpui_dotnet_element_border_1, StyleOp::Border1);
style_fn!(gpui_dotnet_element_border_color, hex: u32, StyleOp::BorderColor(hex));
style_fn!(gpui_dotnet_element_border_dashed, StyleOp::BorderDashed);
style_fn!(gpui_dotnet_element_flex, StyleOp::Flex);
style_fn!(gpui_dotnet_element_flex_1, StyleOp::Flex1);
style_fn!(gpui_dotnet_element_flex_col, StyleOp::FlexCol);
style_fn!(gpui_dotnet_element_flex_row, StyleOp::FlexRow);
style_fn!(gpui_dotnet_element_gap, value: f32, StyleOp::Gap(value));
style_fn!(gpui_dotnet_element_gap_x, value: f32, StyleOp::GapX(value));
style_fn!(gpui_dotnet_element_gap_y, value: f32, StyleOp::GapY(value));
style_fn!(gpui_dotnet_element_grid, StyleOp::Grid);
style_fn!(gpui_dotnet_element_hidden, StyleOp::Hidden);
style_fn!(gpui_dotnet_element_height, value: f32, StyleOp::Height(value));
style_fn!(gpui_dotnet_element_items_center, StyleOp::ItemsCenter);
style_fn!(gpui_dotnet_element_items_end, StyleOp::ItemsEnd);
style_fn!(gpui_dotnet_element_items_start, StyleOp::ItemsStart);
style_fn!(gpui_dotnet_element_justify_between, StyleOp::JustifyBetween);
style_fn!(gpui_dotnet_element_justify_center, StyleOp::JustifyCenter);
style_fn!(gpui_dotnet_element_justify_end, StyleOp::JustifyEnd);
style_fn!(gpui_dotnet_element_justify_start, StyleOp::JustifyStart);
style_fn!(gpui_dotnet_element_margin, value: f32, StyleOp::Margin(value));
style_fn!(gpui_dotnet_element_margin_x, value: f32, StyleOp::MarginX(value));
style_fn!(gpui_dotnet_element_margin_y, value: f32, StyleOp::MarginY(value));
style_fn!(gpui_dotnet_element_opacity, value: f32, StyleOp::Opacity(value));
style_fn!(gpui_dotnet_element_padding, value: f32, StyleOp::Padding(value));
style_fn!(gpui_dotnet_element_padding_x, value: f32, StyleOp::PaddingX(value));
style_fn!(gpui_dotnet_element_padding_y, value: f32, StyleOp::PaddingY(value));
style_fn!(gpui_dotnet_element_relative, StyleOp::Relative);
style_fn!(gpui_dotnet_element_rounded_full, StyleOp::RoundedFull);
style_fn!(gpui_dotnet_element_rounded_md, StyleOp::RoundedMd);
style_fn!(gpui_dotnet_element_shadow_lg, StyleOp::ShadowLg);
style_fn!(gpui_dotnet_element_text_2xl, StyleOp::Text2xl);
style_fn!(gpui_dotnet_element_text_color, hex: u32, StyleOp::TextColor(hex));
style_fn!(gpui_dotnet_element_text_lg, StyleOp::TextLg);
style_fn!(gpui_dotnet_element_text_sm, StyleOp::TextSm);
style_fn!(gpui_dotnet_element_text_xl, StyleOp::TextXl);
style_fn!(gpui_dotnet_element_width, value: f32, StyleOp::Width(value));

#[unsafe(no_mangle)]
pub unsafe extern "C" fn gpui_dotnet_run(
    root: *const DotnetElement,
    width: f32,
    height: f32,
    title: *const c_char,
) -> bool {
    let Some(root) = (unsafe { element_ref_from_ptr(root) }) else {
        return false;
    };

    let root = root.clone();
    let title = unsafe { string_from_ptr(title) };

    enable_process_dpi_awareness();

    let app = gpui_platform::application();
    app.run(move |cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(width), px(height)), cx);
        let mut options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            ..Default::default()
        };

        if let Some(title) = title.as_ref() {
            options.titlebar = Some(TitlebarOptions {
                title: Some(SharedString::from(title.clone())),
                ..Default::default()
            });
        }

        if let Err(error) = cx.open_window(options, |_, cx| {
            cx.new(|_| DotnetRootView { root: root.clone() })
        }) {
            eprintln!("failed to open GPUI window from .NET: {error:?}");
            cx.quit();
            return;
        }

        cx.activate(true);
    });

    true
}

#[unsafe(no_mangle)]
pub extern "C" fn gpui_dotnet_rgb(r: u8, g: u8, b: u8) -> u32 {
    ((r as u32) << 16) | ((g as u32) << 8) | b as u32
}

#[unsafe(no_mangle)]
pub extern "C" fn gpui_dotnet_rgba(r: u8, g: u8, b: u8, a: u8) -> u32 {
    ((r as u32) << 24) | ((g as u32) << 16) | ((b as u32) << 8) | a as u32
}
