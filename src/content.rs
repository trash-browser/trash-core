//! Invisible flex-fill surface that reports its device-pixel bounds so the CEF
//! child window can be positioned under the GPUI chrome.

use gpui::{
    App, Bounds, Element, ElementId, GlobalElementId, IntoElement, LayoutId, Pixels, Style, Window,
    fill, relative, rgb,
};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

use crate::cef_host;

pub struct ContentSurface;

impl IntoElement for ContentSurface {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for ContentSurface {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        Some("cef-content".into())
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = relative(1.).into();
        style.flex_grow = 1.;
        style.flex_shrink = 1.;
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        _cx: &mut App,
    ) -> Self::PrepaintState {
        if let Some((parent, rect)) = parent_and_rect(window, bounds) {
            cef_host::set_parent_bounds(parent, rect);
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        _cx: &mut App,
    ) {
        // Dark placeholder under the native CEF child (visible until first paint).
        window.paint_quad(fill(bounds, rgb(0x0f0f10)));
    }
}

fn parent_and_rect(window: &Window, bounds: Bounds<Pixels>) -> Option<(u64, cef::Rect)> {
    let handle = HasWindowHandle::window_handle(window).ok()?;
    let parent = match handle.as_raw() {
        RawWindowHandle::Xlib(h) => h.window as u64,
        RawWindowHandle::Xcb(h) => h.window.get() as u64,
        RawWindowHandle::Win32(h) => h.hwnd.get() as u64,
        other => {
            // CEF can only parent under X11/Win32. A Wayland handle usually means
            // the gpui wayland feature was pulled in via Cargo feature unification.
            eprintln!("[content] unsupported window handle for CEF parent: {other:?}");
            return None;
        }
    };

    let device = bounds.to_device_pixels(window.scale_factor());
    Some((
        parent,
        cef::Rect {
            x: i32::from(device.origin.x),
            y: i32::from(device.origin.y),
            width: i32::from(device.size.width).max(1),
            height: i32::from(device.size.height).max(1),
        },
    ))
}

pub fn content_surface() -> ContentSurface {
    ContentSurface
}
