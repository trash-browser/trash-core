//! Platform-neutral native popup/menu ownership and placement primitives.
#![allow(dead_code)] // Backends and chrome integrations are enabled per platform.

use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OverlayId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnchorRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonitorWorkArea {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PopupSize {
    pub width: i32,
    pub height: i32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PopupPlacement {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl PopupPlacement {
    /// Places below/right of the anchor, flips at monitor edges, then clamps to
    /// the work area. Coordinates and sizes are physical pixels for mixed-DPI safety.
    pub fn calculate(anchor: AnchorRect, size: PopupSize, work: MonitorWorkArea) -> Self {
        let width = size.width.max(1).min((work.right - work.left).max(1));
        let height = size.height.max(1).min((work.bottom - work.top).max(1));
        let mut x = anchor.left;
        let mut y = anchor.bottom;
        if x + width > work.right {
            x = anchor.right - width;
        }
        if y + height > work.bottom {
            y = anchor.top - height;
        }
        x = x.clamp(work.left, work.right - width);
        y = y.clamp(work.top, work.bottom - height);
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuItem {
    Action {
        id: String,
        label: String,
        icon: Option<String>,
        shortcut: Option<String>,
        enabled: bool,
    },
    Check {
        id: String,
        label: String,
        checked: bool,
        enabled: bool,
    },
    Radio {
        id: String,
        label: String,
        group: String,
        selected: bool,
        enabled: bool,
    },
    Submenu {
        label: String,
        icon: Option<String>,
        items: Vec<MenuItem>,
    },
    Separator,
}

impl MenuItem {
    pub fn enabled(&self) -> bool {
        match self {
            Self::Action { enabled, .. }
            | Self::Check { enabled, .. }
            | Self::Radio { enabled, .. } => *enabled,
            Self::Submenu { items, .. } => items.iter().any(Self::enabled),
            Self::Separator => false,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MenuModel {
    pub items: Vec<MenuItem>,
}

pub trait PopupWindow: Send {
    fn show(
        &mut self,
        owner: u64,
        placement: PopupPlacement,
        model: &MenuModel,
    ) -> anyhow::Result<()>;
    fn close(&mut self);
    fn is_open(&self) -> bool;
}

pub trait PopupBackend: Send {
    fn create_window(&mut self, id: OverlayId) -> anyhow::Result<Box<dyn PopupWindow>>;
}

pub struct PopupManager<B: PopupBackend> {
    backend: B,
    windows: HashMap<OverlayId, Box<dyn PopupWindow>>,
    next_id: u64,
}

impl<B: PopupBackend> PopupManager<B> {
    pub fn new(backend: B) -> Self {
        Self {
            backend,
            windows: HashMap::new(),
            next_id: 1,
        }
    }
    pub fn open(
        &mut self,
        owner: u64,
        anchor: AnchorRect,
        size: PopupSize,
        work: MonitorWorkArea,
        model: &MenuModel,
    ) -> anyhow::Result<OverlayId> {
        self.close_all();
        let id = OverlayId(self.next_id);
        self.next_id += 1;
        let mut window = self.backend.create_window(id)?;
        window.show(owner, PopupPlacement::calculate(anchor, size, work), model)?;
        self.windows.insert(id, window);
        Ok(id)
    }
    pub fn close(&mut self, id: OverlayId) {
        if let Some(mut window) = self.windows.remove(&id) {
            window.close();
        }
    }
    pub fn close_all(&mut self) {
        for (_, mut window) in self.windows.drain() {
            window.close();
        }
    }
}

/// Keyboard selection state shared by native backends. Separators and disabled
/// entries are skipped; opening a submenu creates a separate owned popup.
#[derive(Debug, Default)]
pub struct MenuController {
    selected: Option<usize>,
}
impl MenuController {
    pub fn selected(&self) -> Option<usize> {
        self.selected
    }
    pub fn move_selection(&mut self, model: &MenuModel, delta: isize) {
        if model.items.is_empty() {
            self.selected = None;
            return;
        }
        let len = model.items.len() as isize;
        let mut index = self
            .selected
            .map_or(if delta >= 0 { -1 } else { 0 }, |v| v as isize);
        for _ in 0..model.items.len() {
            index = (index + delta).rem_euclid(len);
            if model.items[index as usize].enabled() {
                self.selected = Some(index as usize);
                return;
            }
        }
        self.selected = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn places_below_when_space_exists() {
        assert_eq!(
            PopupPlacement::calculate(
                AnchorRect {
                    left: 10,
                    top: 10,
                    right: 30,
                    bottom: 30
                },
                PopupSize {
                    width: 100,
                    height: 80
                },
                MonitorWorkArea {
                    left: 0,
                    top: 0,
                    right: 500,
                    bottom: 500
                }
            ),
            PopupPlacement {
                x: 10,
                y: 30,
                width: 100,
                height: 80
            }
        );
    }
    #[test]
    fn flips_at_bottom_right_edge() {
        assert_eq!(
            PopupPlacement::calculate(
                AnchorRect {
                    left: 480,
                    top: 480,
                    right: 500,
                    bottom: 500
                },
                PopupSize {
                    width: 100,
                    height: 80
                },
                MonitorWorkArea {
                    left: 0,
                    top: 0,
                    right: 500,
                    bottom: 500
                }
            ),
            PopupPlacement {
                x: 400,
                y: 400,
                width: 100,
                height: 80
            }
        );
    }
    #[test]
    fn clamps_oversized_popup_to_work_area() {
        assert_eq!(
            PopupPlacement::calculate(
                AnchorRect {
                    left: -20,
                    top: -20,
                    right: 0,
                    bottom: 0
                },
                PopupSize {
                    width: 800,
                    height: 600
                },
                MonitorWorkArea {
                    left: 0,
                    top: 0,
                    right: 500,
                    bottom: 400
                }
            ),
            PopupPlacement {
                x: 0,
                y: 0,
                width: 500,
                height: 400
            }
        );
    }
    #[test]
    fn keyboard_skips_disabled_and_separators() {
        let model = MenuModel {
            items: vec![
                MenuItem::Separator,
                MenuItem::Action {
                    id: "a".into(),
                    label: "A".into(),
                    icon: None,
                    shortcut: None,
                    enabled: false,
                },
                MenuItem::Check {
                    id: "b".into(),
                    label: "B".into(),
                    checked: false,
                    enabled: true,
                },
            ],
        };
        let mut c = MenuController::default();
        c.move_selection(&model, 1);
        assert_eq!(c.selected(), Some(2));
    }
}
