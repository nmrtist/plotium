use egui::{Context, Ui};
use egui_phosphor::regular as icon;
use plotium_core::automation::ResourceRef;
use plotium_core::state::{
    ActivitySourceId, BoardFrameId, CanvasId, DatasetId, FrameRef, PlotiumApp, PrimaryView,
    ProjectContextId, ResultFollowMode,
};

use super::commands::{self, CommandId};

const WORKSPACE: &str = "plotium.result_follow.workspace";
const EDITABLE_LAYERS: &str = "plotium.result_follow.editable_layers";
const COLLECTION: &str = "plotium.result_follow.collection";

#[derive(Clone, Default)]
struct Collection {
    context: Option<(ProjectContextId, ActivitySourceId)>,
    frames: Vec<BoardFrameId>,
}

pub(super) fn begin_frame(ctx: &Context) {
    ctx.data_mut(|data| {
        data.insert_temp(egui::Id::new(EDITABLE_LAYERS), Vec::<egui::LayerId>::new())
    });
}

pub(super) fn register_editable_layer(ctx: &Context, layer: egui::LayerId) {
    ctx.data_mut(|data| {
        let layers =
            data.get_temp_mut_or_default::<Vec<egui::LayerId>>(egui::Id::new(EDITABLE_LAYERS));
        if !layers.contains(&layer) {
            layers.push(layer);
        }
    });
}

pub(super) fn register_editor(ui: &Ui) {
    register_editable_layer(ui.ctx(), ui.layer_id());
}

pub(crate) fn remember_workspace(ui: &Ui) {
    ui.ctx().data_mut(|data| {
        data.insert_temp(
            egui::Id::new(WORKSPACE),
            (ui.available_rect_before_wrap(), ui.layer_id()),
        );
    });
}

pub(crate) fn pause_on_input(app: &mut PlotiumApp, ctx: &Context, input: &egui::RawInput) {
    let workspace =
        ctx.data(|data| data.get_temp::<(egui::Rect, egui::LayerId)>(egui::Id::new(WORKSPACE)));
    let editors = ctx
        .data(|data| data.get_temp::<Vec<egui::LayerId>>(egui::Id::new(EDITABLE_LAYERS)))
        .unwrap_or_default();
    let in_workspace = |pos: egui::Pos2| {
        let top = ctx.layer_id_at(pos);
        top.is_some_and(|layer| editors.contains(&layer))
            || workspace.is_some_and(|(rect, layer)| rect.contains(pos) && top == Some(layer))
    };
    let mut pointer = ctx.pointer_hover_pos();
    let focused = ctx
        .memory(|memory| memory.focused())
        .and_then(|id| ctx.read_response(id));
    let keyboard_workspace = focused.as_ref().map_or_else(
        || pointer.is_some_and(in_workspace),
        |response| in_workspace(response.rect.center()),
    );
    let mut manual = !input.dropped_files.is_empty();
    for event in &input.events {
        match event {
            egui::Event::PointerMoved(pos) => pointer = Some(*pos),
            egui::Event::PointerButton {
                pos, pressed: true, ..
            } => manual |= in_workspace(*pos),
            egui::Event::MouseWheel { .. } | egui::Event::Zoom(_) | egui::Event::Rotate(_) => {
                manual |= pointer.is_some_and(in_workspace);
            }
            egui::Event::Touch {
                pos,
                phase: egui::TouchPhase::Start | egui::TouchPhase::Move,
                ..
            } => manual |= in_workspace(*pos),
            egui::Event::Text(_) | egui::Event::Paste(_) | egui::Event::Cut => {
                manual |= keyboard_workspace
            }
            egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } => {
                let modified = modifiers.command || modifiers.ctrl;
                let observation = modified
                    && (matches!(key, egui::Key::K | egui::Key::C)
                        || (*key == egui::Key::P && modifiers.shift));
                manual |= keyboard_workspace
                    && !observation
                    && !matches!(key, egui::Key::Escape | egui::Key::F1);
            }
            _ => {}
        }
    }
    if manual {
        app.session
            .result_follow
            .pause("You navigated or edited the workspace.");
    }
}

pub(super) fn pause_for_command(app: &mut PlotiumApp, command: CommandId) {
    if !matches!(
        command,
        CommandId::FollowResults
            | CommandId::PauseResultFollow
            | CommandId::StopResultFollow
            | CommandId::OperationHistory
            | CommandId::CommandPalette
            | CommandId::About
            | CommandId::HelpManual
            | CommandId::CheckUpdates
            | CommandId::ImportResults
            | CommandId::ClearRecentFiles
            | CommandId::SaveProject
            | CommandId::TogglePrimarySidebar
            | CommandId::ToggleSecondarySidebar
            | CommandId::UiScaleUp
            | CommandId::UiScaleDown
            | CommandId::UiScaleReset
            | CommandId::AgentConnections
            | CommandId::Preferences
    ) {
        app.session
            .result_follow
            .pause("You opened, selected, or edited content.");
    }
}

pub(super) fn gate(app: &PlotiumApp, command: CommandId) -> Result<(), &'static str> {
    let requires = |ok, reason| if ok { Ok(()) } else { Err(reason) };
    match command {
        CommandId::FollowResults => requires(
            app.session.result_follow.source().is_some(),
            "Enable an AI connection before following its results.",
        ),
        CommandId::PauseResultFollow => requires(
            app.session.result_follow.mode() == ResultFollowMode::Following,
            "Resume following before pausing it.",
        ),
        CommandId::StopResultFollow => requires(
            app.session.result_follow.mode() != ResultFollowMode::Off,
            "Resume following before stopping it.",
        ),
        CommandId::ViewLatestResults => requires(
            !available_results(app, false).is_empty(),
            "Wait for the current AI connection to produce an available result.",
        ),
        _ => Ok(()),
    }
}

pub(super) fn execute(app: &mut PlotiumApp, ctx: &Context, command: CommandId) {
    if gate(app, command).is_err() {
        return;
    }
    match command {
        CommandId::FollowResults => {
            app.session.result_follow.resume();
            let resources = available_results(app, false);
            open_collection(app, ctx, &resources, true);
        }
        CommandId::PauseResultFollow => app.session.result_follow.pause("You paused following."),
        CommandId::StopResultFollow => app.session.result_follow.stop(),
        CommandId::ViewLatestResults => {
            let resources = available_results(app, false);
            open_collection(app, ctx, &resources, true);
        }
        _ => {}
    }
}

pub(super) fn button(app: &mut PlotiumApp, ui: &mut Ui) {
    let mode = app.session.result_follow.mode();
    let (glyph, label, command) = match mode {
        ResultFollowMode::Following => (
            icon::PLAY_CIRCLE,
            "Following results",
            CommandId::PauseResultFollow,
        ),
        ResultFollowMode::Paused => (
            icon::PAUSE_CIRCLE,
            "Following paused",
            CommandId::FollowResults,
        ),
        ResultFollowMode::Off => (icon::STOP_CIRCLE, "Following off", CommandId::FollowResults),
    };
    let descriptor = commands::describe(app, command);
    let label =
        if mode == ResultFollowMode::Following && app.session.result_follow.source().is_none() {
            "Waiting for AI connection"
        } else {
            label
        };
    let mut tooltip = format!("{label}\n{}", descriptor.label);
    if let Some(reason) = app.session.result_follow.pause_reason() {
        tooltip.push_str(&format!("\n{reason}"));
    }
    let response = ui.add_enabled(
        descriptor.enabled,
        egui::Button::new(glyph)
            .min_size(egui::vec2(
                super::ribbon_chrome::SIDEBAR_TOGGLE_WIDTH,
                ui.spacing().interact_size.y,
            ))
            .frame_when_inactive(false)
            .selected(mode == ResultFollowMode::Following),
    );
    let response = response.on_hover_text(tooltip);
    let response = if let Some(reason) = descriptor.disabled_reason {
        response.on_disabled_hover_text(reason)
    } else {
        response
    };
    if response.clicked() {
        commands::execute_without_clipboard(command, app, ui.ctx());
    }
}

pub(super) fn controls(app: &mut PlotiumApp, ui: &mut Ui) {
    let mode = app.session.result_follow.mode();
    ui.horizontal_wrapped(|ui| {
        ui.label(match (mode, app.session.result_follow.source().is_some()) {
            (ResultFollowMode::Following, false) => "Waiting for AI connection",
            (ResultFollowMode::Following, true) => "Following AI connection results",
            (ResultFollowMode::Paused, _) => "Following paused",
            (ResultFollowMode::Off, _) => "Following off",
        });
        for command in [
            if mode == ResultFollowMode::Following {
                CommandId::PauseResultFollow
            } else {
                CommandId::FollowResults
            },
            CommandId::StopResultFollow,
            CommandId::ViewLatestResults,
        ] {
            let descriptor = commands::describe(app, command);
            let label = match command {
                CommandId::FollowResults => "Resume",
                CommandId::PauseResultFollow => "Pause",
                CommandId::StopResultFollow => "Stop following",
                _ => "View latest",
            };
            let response = ui.add_enabled(descriptor.enabled, egui::Button::new(label));
            let response = if let Some(reason) = descriptor.disabled_reason {
                response.on_disabled_hover_text(reason)
            } else {
                response
            };
            if response.clicked() {
                commands::execute_without_clipboard(command, app, ui.ctx());
            }
        }
    });
    if let Some(reason) = app.session.result_follow.pause_reason() {
        ui.weak(reason);
    }
    ui.weak("Pausing only holds your view; running tasks continue.");
    ui.separator();
}

fn available_results(app: &PlotiumApp, unseen: bool) -> Vec<ResourceRef> {
    let Some(source) = app.session.result_follow.source() else {
        return Vec::new();
    };
    app.session
        .background_activity
        .results()
        .iter()
        .filter(|row| {
            row.source == Some(source)
                && (!unseen || row.operation.0 > app.session.result_follow.cursor().0)
        })
        .filter(|row| app.activity_result_available(&row.resource))
        .map(|row| row.resource.clone())
        .collect()
}

pub(crate) fn apply(app: &mut PlotiumApp, ui: &Ui) {
    if app.session.result_follow.mode() != ResultFollowMode::Following {
        return;
    }
    let resources = available_results(app, true);
    let latest = app.session.background_activity.latest_id();
    if !resources.is_empty() {
        open_collection(app, ui.ctx(), &resources, false);
    }
    app.session.result_follow.advance(latest);
}

fn result_frame(app: &PlotiumApp, resource: &ResourceRef) -> Option<BoardFrameId> {
    let canvas = CanvasId::try_from(resource).ok().or_else(|| {
        (resource.kind.0 == plotium_core::automation::KIND_CANVAS_OBJECT)
            .then(|| resource.parent_id.as_deref()?.parse::<CanvasId>().ok())
            .flatten()
    });
    if let Some(id) = canvas {
        return app.doc.canvas_index(id).map(|_| BoardFrameId::Page(id));
    }
    let dataset = DatasetId::try_from(resource)
        .ok()
        .or_else(|| resource.parent_id.as_deref()?.parse::<DatasetId>().ok())?;
    let index = app.doc.dataset_index(dataset)?;
    let frame = plotium_core::state::page_frame_showing_dataset(app, index).or_else(|| {
        app.doc.datasets[index]
            .as_table()
            .filter(|table| table.board_sheet_visible())
            .map(|_| FrameRef::Sheet(index))
    })?;
    plotium_core::state::board_frame_id(app, frame)
}

fn open_collection(app: &mut PlotiumApp, ctx: &Context, resources: &[ResourceRef], force: bool) {
    let key = egui::Id::new(COLLECTION);
    let context = app
        .session
        .result_follow
        .source()
        .map(|source| (app.session.project_context, source));
    let mut collection = ctx
        .data(|data| data.get_temp::<Collection>(key))
        .unwrap_or_default();
    if collection.context != context {
        collection = Collection {
            context,
            frames: Vec::new(),
        };
    }
    collection
        .frames
        .retain(|id| plotium_core::state::board_frame_ref(app, *id).is_some());
    let has_frames = resources
        .iter()
        .any(|resource| result_frame(app, resource).is_some());
    let mut added = false;
    for resource in resources {
        if let Some(frame) = result_frame(app, resource)
            && !collection.frames.contains(&frame)
        {
            collection.frames.push(frame);
            added = true;
        }
    }
    if has_frames && !collection.frames.is_empty() {
        if added || force || app.session.view != PrimaryView::Canvas {
            if let Some(index) = collection.frames.iter().find_map(|frame| match frame {
                BoardFrameId::Page(id) => app.doc.canvas_index(*id),
                _ => None,
            }) {
                app.activate_canvas(index);
            }
            app.session.ui.frame_selection = collection.frames.clone();
            plotium_core::state::sync_frame_selection_to_data(app);
            app.session.view = PrimaryView::Canvas;
            app.session.board_reveal = None;
            super::canvas::zoom_to_selection(app, ctx);
        }
    } else if let Some(dataset) = resources.iter().rev().find_map(|resource| {
        DatasetId::try_from(resource)
            .ok()
            .or_else(|| resource.parent_id.as_deref()?.parse::<DatasetId>().ok())
            .and_then(|id| app.doc.dataset_index(id))
    }) && (force
        || app.active_dataset() != Some(dataset)
        || app.session.view != PrimaryView::Data)
    {
        app.focus_datasets(&[dataset], Some(dataset));
        app.session.view = PrimaryView::Data;
    }
    ctx.data_mut(|data| data.insert_temp(key, collection));
}

#[cfg(test)]
#[path = "result_follow_tests.rs"]
mod tests;
