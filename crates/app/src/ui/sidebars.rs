use super::*;

const MIN_WORKSPACE_WIDTH: f32 = 320.0;
/// Logical pixels from the window's outer edge, independent of sidebar width.
const HIDE_EDGE_DISTANCE: f32 = 24.0;

/// How a sidebar takes part in the frame's layout.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Placement {
    /// At rest beside the workspace, resizable.
    Docked,
    Hidden,
    /// Sliding over the workspace with `shown` of its width in view. The
    /// workspace keeps the layout it had when the slide began (`reserved`)
    /// and lays out again once, when the slide ends, so a heavy canvas is
    /// never re-laid out at every width in between.
    Sliding {
        shown: f32,
        reserved: bool,
    },
}

impl Placement {
    fn resolve(visible: bool, shown: f32, was_reserved: bool) -> Self {
        let settled = if visible { shown >= 1.0 } else { shown <= 0.0 };
        match (settled, visible) {
            (true, true) => Self::Docked,
            (true, false) => Self::Hidden,
            (false, _) => Self::Sliding {
                shown,
                reserved: was_reserved,
            },
        }
    }

    fn reserves_space(self) -> bool {
        matches!(self, Self::Docked | Self::Sliding { reserved: true, .. })
    }
}

pub(super) fn render(app: &mut PlotiumApp, ui: &mut Ui, dark: bool, workspace_width: f32) {
    let compact = workspace_width < 1200.0;
    let inspector_visible = app.session.secondary_sidebar_visible;
    if !inspector_visible {
        app.finish_axis_overrides_edit();
    }
    object_inspector::finish_series_edit_if_inactive(app, inspector_visible);
    let primary_rect = render_side(app, ui, dark, workspace_width, compact, true);
    let secondary_rect = render_side(app, ui, dark, workspace_width, compact, false);
    super::workspace_geometry::set_sidebar_rects(ui.ctx(), primary_rect, secondary_rect);
}

/// Shows one sidebar and returns the rectangle the workspace lays out around.
fn render_side(
    app: &mut PlotiumApp,
    ui: &mut Ui,
    dark: bool,
    workspace_width: f32,
    compact: bool,
    primary: bool,
) -> Option<Rect> {
    let id = sidebar_id(primary);
    let session = &app.session;
    let (visible, stored_width, other_width) = if primary {
        (
            session.primary_sidebar_visible,
            session.primary_sidebar_width,
            session
                .secondary_sidebar_visible
                .then_some(session.secondary_sidebar_width),
        )
    } else {
        (
            session.secondary_sidebar_visible,
            session.secondary_sidebar_width,
            session
                .primary_sidebar_visible
                .then_some(session.primary_sidebar_width),
        )
    };
    let shown = motion::slide(ui.ctx(), id, visible);
    let was_reserved = reserves_space(ui.ctx(), primary);
    let placement = Placement::resolve(visible, shown, was_reserved);
    let reserved_id = reserved_id(ui.ctx(), primary);
    ui.ctx()
        .data_mut(|data| data.insert_temp(reserved_id, placement.reserves_space()));

    let (min_width, width_cap) = match (primary, compact) {
        (true, true) => (150.0, 420.0),
        (true, false) => (190.0, 420.0),
        (false, true) => (180.0, 460.0),
        (false, false) => (230.0, 460.0),
    };
    let max_width = (workspace_width - other_width.unwrap_or(0.0) - MIN_WORKSPACE_WIDTH)
        .clamp(min_width, width_cap);
    let width = stored_width.clamp(min_width, max_width);
    let panel = if primary {
        egui::Panel::left("primary_sidebar")
    } else {
        egui::Panel::right("secondary_sidebar")
    }
    .frame(egui::Frame::NONE)
    .show_separator_line(false);
    let edge = if primary {
        SidebarEdge::Right
    } else {
        SidebarEdge::Left
    };

    match placement {
        Placement::Hidden => {
            prime_sliding_area(ui.ctx(), primary);
            // A shown panel consumes one auto-id slot from this ui. Burn the
            // same slot while hidden so the later siblings' container ids
            // (secondary panel, central panel) do not shift when a sidebar
            // toggles.
            ui.skip_ahead_auto_ids(1);
            None
        }
        Placement::Docked => {
            prime_sliding_area(ui.ctx(), primary);
            let panel = panel
                .resizable(true)
                .default_size(width)
                .size_range(min_width..=max_width);
            let response = show_sidebar(panel, app, ui, dark, primary, min_width..=max_width);
            paint_sidebar_resize_edge(ui, id, response.inner, edge, dark);
            // Only a docked sidebar reports its width: the in-between widths
            // of a slide are not the user's choice.
            let actual = response.response.rect.width();
            if primary {
                app.session.primary_sidebar_width = actual;
            } else {
                app.session.secondary_sidebar_width = actual;
            }
            if resize_released_at_window_edge(ui.ctx(), id, edge) {
                // The user already dragged it to the edge; do not slide.
                motion::land_slide(ui.ctx(), id, false);
                if primary {
                    app.session.primary_sidebar_visible = false;
                    sidebar_hidden_status(app, commands::CommandId::TogglePrimarySidebar, "Left");
                } else {
                    app.session.secondary_sidebar_visible = false;
                    sidebar_hidden_status(
                        app,
                        commands::CommandId::ToggleSecondarySidebar,
                        "Right",
                    );
                }
            }
            Some(response.inner)
        }
        Placement::Sliding { shown, reserved } => {
            let slot = if reserved {
                // Keep the docked width reserved and empty; the sidebar slides
                // over it in its own layer.
                panel
                    .resizable(false)
                    .exact_size(width)
                    .show_inside(ui, |_| {})
                    .response
                    .rect
            } else {
                ui.skip_ahead_auto_ids(1);
                let available = ui.available_rect_before_wrap();
                if primary {
                    Rect::from_min_max(
                        available.min,
                        egui::pos2(available.left() + width, available.bottom()),
                    )
                } else {
                    Rect::from_min_max(
                        egui::pos2(available.right() - width, available.top()),
                        available.max,
                    )
                }
            };
            show_sliding(app, ui, dark, primary, slot, shown);
            reserved.then_some(slot)
        }
    }
}

/// Whether the workspace currently lays out around this sidebar, which lags
/// its visibility until a slide ends.
pub(super) fn reserves_space(ctx: &egui::Context, primary: bool) -> bool {
    ctx.data(|data| data.get_temp::<bool>(reserved_id(ctx, primary)))
        .unwrap_or(false)
}

fn reserved_id(ctx: &egui::Context, primary: bool) -> Id {
    super::viewport_local_id(
        ctx,
        if primary {
            "plotium_primary_sidebar_reserved"
        } else {
            "plotium_secondary_sidebar_reserved"
        },
    )
}

fn sidebar_id(primary: bool) -> Id {
    Id::new(if primary {
        "primary_sidebar"
    } else {
        "secondary_sidebar"
    })
}

/// Draws the sidebar in an area just above the workspace panels and below
/// floating windows, shifted toward its window edge by the hidden fraction
/// and clipped to its slot. As an area it owns the pointer over its visible
/// part, so canvas gestures that test the layer under the pointer stop there.
fn show_sliding(app: &mut PlotiumApp, ui: &Ui, dark: bool, primary: bool, slot: Rect, shown: f32) {
    let outward = if primary { -1.0 } else { 1.0 };
    let rect = slot.translate(egui::vec2(outward * (1.0 - shown) * slot.width(), 0.0));
    let ctx = ui.ctx();
    let area = sliding_area(primary);
    ctx.set_sublayer(ui.layer_id(), area.layer());
    area.fixed_pos(rect.min).show(ctx, |area_ui| {
        area_ui.set_clip_rect(slot.intersect(ui.clip_rect()));
        area_ui.scope_builder(UiBuilder::new().max_rect(rect), |content| {
            sidebar_contents(app, content, dark, primary);
        });
        // A moving sidebar takes no input: this cover, added last, wins the
        // hit test over its contents.
        area_ui.interact(
            rect.intersect(slot),
            area_ui.id().with("cover"),
            egui::Sense::click_and_drag(),
        );
    });
}

fn sliding_area(primary: bool) -> egui::Area {
    egui::Area::new(sidebar_id(primary).with("sliding"))
        .order(egui::Order::Background)
        .constrain(false)
        .movable(false)
        .fade_in(false)
        .sense(egui::Sense::hover())
}

/// A new area spends its first frame on an invisible sizing pass. Run that
/// pass once, empty, inert and off screen, before any slide so the first
/// slide shows at once.
fn prime_sliding_area(ctx: &egui::Context, primary: bool) {
    let area = sliding_area(primary);
    if egui::AreaState::load(ctx, area.layer().id).is_none() {
        area.interactable(false)
            .default_size(Vec2::ZERO)
            .fixed_pos(egui::pos2(-1.0e4, -1.0e4))
            .show(ctx, |_| {});
    }
}

/// Preview and commit use the same current pointer position: dragging back
/// from the window edge cancels hiding without any latched state.
pub(super) fn resize_released_at_window_edge(
    ctx: &egui::Context,
    panel_id: Id,
    edge: SidebarEdge,
) -> bool {
    let Some(resize) = ctx.read_response(panel_id.with("__resize")) else {
        return false;
    };
    if !resize.dragged() && !resize.drag_stopped() {
        return false;
    }
    let Some(pointer) = resize
        .interact_pointer_pos()
        .or_else(|| ctx.pointer_interact_pos())
    else {
        return false;
    };
    let window = ctx.content_rect();
    let near_edge = match edge {
        SidebarEdge::Right => pointer.x <= window.left() + HIDE_EDGE_DISTANCE,
        SidebarEdge::Left => pointer.x >= window.right() - HIDE_EDGE_DISTANCE,
    };
    if near_edge && resize.dragged() {
        // Keep the hint visible even when the pointer is outside the window.
        egui::Tooltip::always_open(
            ctx.clone(),
            resize.layer_id,
            panel_id.with("hide_hint"),
            window.shrink(12.0).clamp(pointer),
        )
        .show(|ui| {
            ui.label("Release to hide sidebar");
        });
    }
    near_edge && resize.drag_stopped()
}

fn sidebar_hidden_status(app: &mut PlotiumApp, id: commands::CommandId, side: &str) {
    let recovery = match shortcuts::shortcut_label(id) {
        Some(chord) => format!("the layout buttons in the title row or {chord}"),
        None => "the layout buttons in the title row".to_owned(),
    };
    app.session.status = format!("{side} sidebar hidden. Show it again with {recovery}.");
}

fn show_sidebar(
    panel: egui::Panel,
    app: &mut PlotiumApp,
    ui: &mut Ui,
    dark: bool,
    primary: bool,
    width_range: std::ops::RangeInclusive<f32>,
) -> InnerResponse<Rect> {
    let (id, edge) = if primary {
        (sidebar_id(true), SidebarEdge::Right)
    } else {
        (sidebar_id(false), SidebarEdge::Left)
    };
    show_resizable_sidebar(panel, ui, id, edge, width_range, |ui| {
        sidebar_contents(app, ui, dark, primary)
    })
}

fn sidebar_contents(app: &mut PlotiumApp, ui: &mut Ui, dark: bool, primary: bool) -> Rect {
    let id = sidebar_id(primary);
    // Anchor the content ids globally: a Ui's per-pass unique id folds in
    // the parent's auto-id counter, so without this every widget in this
    // sidebar changes id whenever an earlier sibling panel toggles. That
    // dropped focus mid-edit and tripped egui's rect-changed-id debug
    // overlay (one-frame red boxes) on the unmoved sidebar. The global scope
    // also keeps the ids when the sidebar moves into its sliding layer.
    ui.scope_builder(
        UiBuilder::new()
            .id_salt(id.with("stable_scope"))
            .global_scope(true),
        |ui| {
            let size = ui.available_size();
            let frame = window_shell::edge_frame(dark, window_shell::EDGE_INSET);
            let inset = frame.total_margin().sum();
            let rect = frame
                .show(ui, |ui| {
                    ui.set_min_size((size - inset).max(Vec2::ZERO));
                    if primary {
                        primary_sidebar::render(app, ui);
                    } else {
                        secondary_sidebar::render(app, ui);
                    }
                })
                .response
                .rect;
            let divider = if primary {
                window_shell::Side::Right
            } else {
                window_shell::Side::Left
            };
            window_shell::paint_separator(ui, rect, divider, dark);
            rect
        },
    )
    .inner
}

#[cfg(test)]
#[path = "sidebars_tests.rs"]
mod tests;
