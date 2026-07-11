use gpui::{
    App, AppContext, Bounds, Context, IntoElement, ParentElement, Render, SharedString, Styled,
    TitlebarOptions, Window, WindowBounds, WindowOptions, div, hsla, px, rgb, rgba, size,
};
use pyo3::prelude::*;

#[cfg(target_os = "windows")]
fn enable_process_dpi_awareness() {
    use windows::Win32::UI::HiDpi::{
        DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
    };

    // Python extension modules cannot embed GPUI's executable manifest into
    // python.exe, so opt the host process into the same PerMonitorV2 mode at
    // runtime. This may fail if the host process already chose a DPI mode;
    // ignoring that failure is safe and matches Windows API guidance.
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
}

#[cfg(not(target_os = "windows"))]
fn enable_process_dpi_awareness() {}

#[derive(Clone)]
enum ChildNode {
    Element(PyElement),
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

/// A Python-owned GPUI element tree node.
///
/// This ports GPUI's fluent `div()` builder into Python. The object stores a
/// lightweight Python-side tree, then renders it into real GPUI elements when
/// the root view paints.
#[pyclass(name = "Element")]
#[derive(Clone, Default)]
struct PyElement {
    children: Vec<ChildNode>,
    styles: Vec<StyleOp>,
}

#[pymethods]
impl PyElement {
    /// Add another element as a child.
    fn child(&self, child: &PyElement) -> Self {
        let mut element = self.clone();
        element.children.push(ChildNode::Element(child.clone()));
        element
    }

    /// Add a text child.
    fn child_text(&self, text: &str) -> Self {
        let mut element = self.clone();
        element.children.push(ChildNode::Text(text.to_string()));
        element
    }

    /// Set absolute positioning.
    fn absolute(&self) -> Self {
        self.with_style(StyleOp::Absolute)
    }

    /// Set the background color from a `0xRRGGBB` integer.
    fn bg(&self, hex: u32) -> Self {
        self.with_style(StyleOp::Background(hex))
    }

    /// Set the background color from a `0xRRGGBBAA` integer.
    fn bg_rgba(&self, hex: u32) -> Self {
        self.with_style(StyleOp::BackgroundRgba(hex))
    }

    /// Set a one-pixel border.
    fn border_1(&self) -> Self {
        self.with_style(StyleOp::Border1)
    }

    /// Set the border color from a `0xRRGGBB` integer.
    fn border_color(&self, hex: u32) -> Self {
        self.with_style(StyleOp::BorderColor(hex))
    }

    /// Set the border style to dashed.
    fn border_dashed(&self) -> Self {
        self.with_style(StyleOp::BorderDashed)
    }

    /// Use flexbox layout.
    fn flex(&self) -> Self {
        self.with_style(StyleOp::Flex)
    }

    /// Use GPUI's `flex_1` helper.
    fn flex_1(&self) -> Self {
        self.with_style(StyleOp::Flex1)
    }

    /// Set flex direction to column.
    fn flex_col(&self) -> Self {
        self.with_style(StyleOp::FlexCol)
    }

    /// Set flex direction to row.
    fn flex_row(&self) -> Self {
        self.with_style(StyleOp::FlexRow)
    }

    /// Set horizontal and vertical flex gap in pixels.
    fn gap(&self, value: f32) -> Self {
        self.with_style(StyleOp::Gap(value))
    }

    /// Set flex column gap in pixels.
    fn gap_x(&self, value: f32) -> Self {
        self.with_style(StyleOp::GapX(value))
    }

    /// Set flex row gap in pixels.
    fn gap_y(&self, value: f32) -> Self {
        self.with_style(StyleOp::GapY(value))
    }

    /// Use grid layout.
    fn grid(&self) -> Self {
        self.with_style(StyleOp::Grid)
    }

    /// Hide this element with `display: none`.
    fn hidden(&self) -> Self {
        self.with_style(StyleOp::Hidden)
    }

    /// Set width and height to the same pixel value.
    fn size(&self, value: f32) -> Self {
        self.with_style(StyleOp::Width(value))
            .with_style(StyleOp::Height(value))
    }

    /// Set width in pixels.
    fn width(&self, value: f32) -> Self {
        self.with_style(StyleOp::Width(value))
    }

    /// Alias for GPUI's `w(...)` helper.
    fn w(&self, value: f32) -> Self {
        self.width(value)
    }

    /// Set height in pixels.
    fn height(&self, value: f32) -> Self {
        self.with_style(StyleOp::Height(value))
    }

    /// Alias for GPUI's `h(...)` helper.
    fn h(&self, value: f32) -> Self {
        self.height(value)
    }

    /// Align flex items to the start of the cross axis.
    fn items_start(&self) -> Self {
        self.with_style(StyleOp::ItemsStart)
    }

    /// Center children on the flex cross axis.
    fn items_center(&self) -> Self {
        self.with_style(StyleOp::ItemsCenter)
    }

    /// Align flex items to the end of the cross axis.
    fn items_end(&self) -> Self {
        self.with_style(StyleOp::ItemsEnd)
    }

    /// Justify flex items to the start of the main axis.
    fn justify_start(&self) -> Self {
        self.with_style(StyleOp::JustifyStart)
    }

    /// Center children on the flex main axis.
    fn justify_center(&self) -> Self {
        self.with_style(StyleOp::JustifyCenter)
    }

    /// Justify flex items to the end of the main axis.
    fn justify_end(&self) -> Self {
        self.with_style(StyleOp::JustifyEnd)
    }

    /// Distribute children with space between them.
    fn justify_between(&self) -> Self {
        self.with_style(StyleOp::JustifyBetween)
    }

    /// Set margin in pixels.
    fn margin(&self, value: f32) -> Self {
        self.with_style(StyleOp::Margin(value))
    }

    /// Alias for GPUI's `m(...)` helper.
    fn m(&self, value: f32) -> Self {
        self.margin(value)
    }

    /// Set horizontal margin in pixels.
    fn mx(&self, value: f32) -> Self {
        self.with_style(StyleOp::MarginX(value))
    }

    /// Set vertical margin in pixels.
    fn my(&self, value: f32) -> Self {
        self.with_style(StyleOp::MarginY(value))
    }

    /// Set opacity from 0.0 to 1.0.
    fn opacity(&self, value: f32) -> Self {
        self.with_style(StyleOp::Opacity(value))
    }

    /// Set padding in pixels.
    fn padding(&self, value: f32) -> Self {
        self.with_style(StyleOp::Padding(value))
    }

    /// Alias for GPUI's `p(...)` helper.
    fn p(&self, value: f32) -> Self {
        self.padding(value)
    }

    /// Set horizontal padding in pixels.
    fn px(&self, value: f32) -> Self {
        self.with_style(StyleOp::PaddingX(value))
    }

    /// Set vertical padding in pixels.
    fn py(&self, value: f32) -> Self {
        self.with_style(StyleOp::PaddingY(value))
    }

    /// Set relative positioning.
    fn relative(&self) -> Self {
        self.with_style(StyleOp::Relative)
    }

    /// Apply GPUI's medium border radius helper.
    fn rounded_md(&self) -> Self {
        self.with_style(StyleOp::RoundedMd)
    }

    /// Apply GPUI's full border radius helper.
    fn rounded_full(&self) -> Self {
        self.with_style(StyleOp::RoundedFull)
    }

    /// Apply GPUI's large shadow helper.
    fn shadow_lg(&self) -> Self {
        self.with_style(StyleOp::ShadowLg)
    }

    /// Set text color from a `0xRRGGBB` integer.
    fn text_color(&self, hex: u32) -> Self {
        self.with_style(StyleOp::TextColor(hex))
    }

    /// Apply GPUI's small text size helper.
    fn text_sm(&self) -> Self {
        self.with_style(StyleOp::TextSm)
    }

    /// Apply GPUI's large text size helper.
    fn text_lg(&self) -> Self {
        self.with_style(StyleOp::TextLg)
    }

    /// Apply GPUI's extra-large text size helper.
    fn text_xl(&self) -> Self {
        self.with_style(StyleOp::TextXl)
    }

    /// Apply GPUI's 2xl text size helper.
    fn text_2xl(&self) -> Self {
        self.with_style(StyleOp::Text2xl)
    }
}

impl PyElement {
    fn with_style(&self, style: StyleOp) -> Self {
        let mut element = self.clone();
        element.styles.push(style);
        element
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

struct PythonRootView {
    root: PyElement,
}

impl Render for PythonRootView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        self.root.to_gpui_element()
    }
}

/// Create a GPUI `div` element builder.
#[pyfunction(name = "div")]
fn div_() -> PyElement {
    PyElement::default()
}

/// Create a GPUI element containing a single text child.
#[pyfunction]
fn text(value: &str) -> PyElement {
    PyElement::default().child_text(value)
}

/// Create a `0xRRGGBB` color integer.
#[pyfunction(name = "rgb")]
fn rgb_(r: u8, g: u8, b: u8) -> u32 {
    ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

/// Create a `0xRRGGBBAA` color integer.
#[pyfunction(name = "rgba")]
fn rgba_(r: u8, g: u8, b: u8, a: u8) -> u32 {
    ((r as u32) << 24) | ((g as u32) << 16) | ((b as u32) << 8) | (a as u32)
}

/// Validate and normalize HSLA components. Returns a `(h, s, l, a)` tuple.
#[pyfunction(name = "hsla")]
fn hsla_(h: f32, s: f32, l: f32, a: f32) -> (f32, f32, f32, f32) {
    let color = hsla(h, s, l, a);
    (color.h, color.s, color.l, color.a)
}

/// Run a GPUI application with `root` as the first window's root view.
///
/// This blocks until the platform application exits, mirroring GPUI's Rust
/// `application().run(...)` API.
#[pyfunction]
#[pyo3(signature = (root, width = 500.0, height = 500.0, title = None))]
fn run(
    py: Python<'_>,
    root: PyElement,
    width: f32,
    height: f32,
    title: Option<String>,
) -> PyResult<()> {
    py.detach(move || {
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
                cx.new(|_| PythonRootView { root: root.clone() })
            }) {
                eprintln!("failed to open GPUI window from Python: {error:?}");
                cx.quit();
                return;
            }

            cx.activate(true);
        });
    });

    Ok(())
}

/// Python bindings for a small, growing subset of GPUI.
#[pymodule]
fn gpui_python(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyElement>()?;
    module.add_function(wrap_pyfunction!(div_, module)?)?;
    module.add_function(wrap_pyfunction!(text, module)?)?;
    module.add_function(wrap_pyfunction!(rgb_, module)?)?;
    module.add_function(wrap_pyfunction!(rgba_, module)?)?;
    module.add_function(wrap_pyfunction!(hsla_, module)?)?;
    module.add_function(wrap_pyfunction!(run, module)?)?;
    module.add("__doc__", "Python bindings for GPUI")?;

    Ok(())
}
