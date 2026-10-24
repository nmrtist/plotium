//! The data sheet docked below the board: a resizable bottom panel whose
//! header strip names the table and holds collapse, maximize and close.

use egui::{Id, Margin, Ui, UiBuilder, Vec2};
use egui_phosphor::regular as icon;
use plotium_core::settings::DataSheetPanelSettings;

const MIN_HEIGHT: f32 = 140.0;
/// The board keeps at least this much height so it never disappears.
const MIN_BOARD_HEIGHT: f32 = 120.0;
const MAXIMIZED_BOARD_HEIGHT: f32 = 48.0;

pub(super) struct Header {
    pub title: String,
    pub state: Option<String>,
    pub size: Option<String>,
    /// Dataset index and name of the table this one was derived from.
    pub derived_from: Option<(usize, String)>,
}

#[derive(Clone, Copy, PartialEq)]
pub(super) enum HeaderAction {
    None,
    Close,
    Open(usize),
}

pub(super) struct DockResponse {
    pub action: HeaderAction,
    /// The persisted panel preferences changed and should be saved.
    pub persist: bool,
    pub rect: egui::Rect,
}

pub(super) fn dock_id() -> Id {
    Id::new("plotium_data_sheet_dock")
}

fn maximized_id() -> Id {
    Id::new("plotium_data_sheet_maximized")
}

pub(super) fn show(
    ui: &mut Ui,
    panel: &mut DataSheetPanelSettings,
    margin: Margin,
    dark: bool,
    header: &Header,
    body: impl FnOnce(&mut Ui),
) -> DockResponse {
    let ctx = ui.ctx().clone();
    let mut maximized = !panel.collapsed
        && ctx
            .data(|data| data.get_temp::<bool>(maximized_id()))
            .unwrap_or(false);
    let available = ui.available_height();
    let max_height = (available - MIN_BOARD_HEIGHT).max(MIN_HEIGHT);
    let strip = ui.spacing().interact_size.y + margin.sum().y + 12.0;
    // Collapsed and maximized use their own ids so they never disturb the
    // resizable panel's state.
    let resizable = !panel.collapsed && !maximized;
    let mut persist = resizable && finish_height_drag(&ctx, panel);
    let base = if panel.collapsed {
        egui::Panel::bottom(dock_id().with("collapsed")).exact_size(strip)
    } else if maximized {
        egui::Panel::bottom(dock_id().with("maximized"))
            .exact_size((available - MAXIMIZED_BOARD_HEIGHT).max(MIN_HEIGHT))
    } else {
        egui::Panel::bottom(dock_id())
            .default_size(panel.height.clamp(MIN_HEIGHT, max_height))
            .size_range(MIN_HEIGHT..=max_height)
    };
    let was_collapsed = panel.collapsed;
    let mut action = HeaderAction::None;
    let response = base
        .resizable(resizable)
        .show_separator_line(false)
        .frame(egui::Frame::NONE.inner_margin(margin))
        .show_inside(ui, |ui| {
            // Global ids keep the grid and editor ids stable when panels
            // around the dock toggle (see `sidebars::show_sidebar`).
            ui.scope_builder(
                UiBuilder::new()
                    .id_salt(dock_id().with("stable_scope"))
                    .global_scope(true),
                |ui| {
                    let size = ui.available_size();
                    let frame = super::super::card_frame(dark, Margin::ZERO)
                        .inner_margin(Margin::symmetric(8, 4));
                    let inset = frame.total_margin().sum();
                    frame.show(ui, |ui| {
                        ui.set_min_size((size - inset).max(Vec2::ZERO));
                        action = header_strip(ui, header, panel, &mut maximized);
                        if !panel.collapsed {
                            ui.separator();
                            body(ui);
                        }
                    });
                },
            );
        });
    ctx.data_mut(|data| data.insert_temp(maximized_id(), maximized));
    persist |= was_collapsed != panel.collapsed;
    let rect = response.response.rect;
    if resizable && ctx.is_being_dragged(resize_id()) {
        ctx.data_mut(|data| data.insert_temp(drag_height_id(), rect.height()));
    }
    DockResponse {
        action,
        persist,
        rect,
    }
}

/// egui's own id for the panel separator drag.
fn resize_id() -> Id {
    dock_id().with("__resize")
}

fn drag_height_id() -> Id {
    dock_id().with("drag_height")
}

/// The settings are the only source of the dock height: a separator drag
/// stores its last height once it ends, and otherwise egui's remembered panel
/// size is dropped so the panel shows the stored height, clamped to the
/// window. A clamp from a short window therefore never becomes the preference.
fn finish_height_drag(ctx: &egui::Context, panel: &mut DataSheetPanelSettings) -> bool {
    if ctx.is_being_dragged(resize_id()) {
        return false;
    }
    let dragged = ctx.data_mut(|data| {
        data.remove::<egui::containers::panel::PanelState>(dock_id());
        let height = data.get_temp::<f32>(drag_height_id());
        data.remove::<f32>(drag_height_id());
        height
    });
    match dragged {
        Some(height) if (panel.height - height).abs() > 0.5 => {
            panel.height = height;
            true
        }
        _ => false,
    }
}

fn header_strip(
    ui: &mut Ui,
    header: &Header,
    panel: &mut DataSheetPanelSettings,
    maximized: &mut bool,
) -> HeaderAction {
    let mut action = HeaderAction::None;
    ui.horizontal(|ui| {
        let (glyph, hint) = if panel.collapsed {
            (icon::CARET_RIGHT, "Expand the data sheet")
        } else {
            (icon::CARET_DOWN, "Collapse the data sheet")
        };
        if ui.small_button(glyph).on_hover_text(hint).clicked() {
            panel.collapsed = !panel.collapsed;
            *maximized = false;
        }
        ui.label(crate::typography::headline(&header.title));
        if let Some(state) = &header.state {
            ui.weak(state);
        }
        if let Some(size) = &header.size {
            ui.weak(size);
        }
        if let Some((index, name)) = &header.derived_from
            && ui
                .link(format!("Derived from {name}"))
                .on_hover_text("Show the source table")
                .clicked()
        {
            action = HeaderAction::Open(*index);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .small_button(icon::X)
                .on_hover_text("Close the data sheet")
                .clicked()
            {
                action = HeaderAction::Close;
            }
            if !panel.collapsed {
                let (glyph, hint) = if *maximized {
                    (icon::ARROWS_IN_SIMPLE, "Restore the data sheet height")
                } else {
                    (icon::ARROWS_OUT_SIMPLE, "Maximize the data sheet")
                };
                if ui.small_button(glyph).on_hover_text(hint).clicked() {
                    *maximized = !*maximized;
                }
            }
        });
    });
    action
}
