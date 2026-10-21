use egui::Ui;
use egui_phosphor::regular as icon;
use plotium_core::actions::{Action, PanelState, ZOrder};
use plotium_core::state::{ContentId, ObjectId, Panel, PanelId, PlotiumApp, SelectionPath};

use super::layer_controls::{
    kind_glyph, kind_label, lock_button, row as layer_row, truncated_selectable, visibility_button,
};
use super::selection::{claim_list_keyboard_focus, select_layer_range, select_modifiers};

pub(super) fn render_panels(app: &mut PlotiumApp, ci: usize, ui: &mut Ui) {
    let panels: Vec<_> = app.doc.canvases[ci]
        .panels
        .iter()
        .map(|panel| panel.id)
        .rev()
        .collect();
    for panel in panels {
        // egui auto-ids are positional, so anchor each row to a global egui
        // scope keyed by the typed panel id: otherwise every row after a list
        // change (new canvas, dataset, or result above) is assigned fresh
        // widget ids at unchanged rects, which drops focus and trips egui's
        // debug id-change overlay. A salted non-global scope is not enough:
        // its unique id still embeds the parent's positional auto-id counter.
        let row_scope = egui::UiBuilder::new().id(egui::Id::new(("layers_panel_row", panel)));
        ui.scope_builder(row_scope, |ui| {
            render_panel(app, ci, panel, ui);
        });
    }
    if app.session.ui.layers_drag_content.is_some()
        && ui.input(|input| input.pointer.primary_released())
    {
        app.session.ui.layers_drag_content = None;
        app.session.ui.panel_drop_target = None;
    }
}

fn render_panel(app: &mut PlotiumApp, ci: usize, panel_id: PanelId, ui: &mut Ui) {
    let Some(panel) = app.doc.canvases[ci].panel(panel_id).cloned() else {
        return;
    };
    let collapse_id = egui::Id::new(("figure-layer-panel-open", panel_id));
    let mut open = ui.data(|data| data.get_temp::<bool>(collapse_id).unwrap_or(true));
    let selected_path = SelectionPath::panel(app.doc.canvases[ci].resource_id, panel_id);
    let selected = app
        .session
        .ui
        .hierarchical_selection
        .contains(selected_path);
    let mut select_panel = false;
    let mut enter_panel = false;
    let mut flags = None;
    let mut drop_content = None;
    let display_name = panel_tree_name(app, ci, &panel);
    let (visible_flags, locked_flags) = super::layer_controls::row(
        ui,
        |ui| {
            if ui
                .small_button(if open {
                    icon::CARET_DOWN
                } else {
                    icon::CARET_RIGHT
                })
                .clicked()
            {
                open = !open;
                ui.data_mut(|data| data.insert_temp(collapse_id, open));
            }
            let mut visible = panel.visible;
            if super::layer_controls::visibility_button(ui, &mut visible).changed() {
                flags = Some((visible, panel.locked));
            }
            ui.weak(icon::RECTANGLE).on_hover_text("Panel");
            let response = super::layer_controls::truncated_selectable(ui, selected, display_name)
                .interact(egui::Sense::click_and_drag());
            if response.drag_started() {
                app.session.ui.panel_drop_target = None;
            }
            if let Some(content) = app.session.ui.layers_drag_content
                && response.hovered()
                && !panel.locked
                && app.doc.canvases[ci].parent_panel(content) != Some(panel_id)
            {
                app.session.ui.panel_drop_target = Some(panel_id);
                if ui.input(|input| input.pointer.primary_released()) {
                    drop_content = Some(content);
                }
            }
            if app.session.ui.panel_drop_target == Some(panel_id) {
                ui.painter().rect_stroke(
                    response.rect,
                    2.0,
                    egui::Stroke::new(1.5_f32, ui.visuals().selection.stroke.color),
                    egui::StrokeKind::Inside,
                );
            }
            if response.clicked() || (response.secondary_clicked() && !selected) {
                select_panel = true;
            }
            if response.double_clicked() {
                enter_panel = true;
            }
            response.context_menu(|ui| panel_context_menu(app, ui));
            flags
        },
        |ui| {
            let mut locked = panel.locked;
            if super::layer_controls::lock_button(ui, &mut locked).changed() {
                Some((panel.visible, locked))
            } else {
                None
            }
        },
    );
    flags = visible_flags.or(locked_flags);
    if let Some(content) = drop_content {
        app.select_content(ci, content);
        crate::ui::commands::execute_without_clipboard(
            crate::ui::commands::CommandId::MoveContentToPanel(Some(panel_id)),
            app,
            ui.ctx(),
        );
        app.session.ui.layers_drag_content = None;
        app.session.ui.panel_drop_target = None;
    }
    if let Some((visible, locked)) = flags {
        replace_panel(app, ci, panel_id, |panel| {
            panel.visible = visible;
            panel.locked = locked;
        });
    }
    if select_panel {
        app.session.ui.selection_scope = plotium_core::state::SelectionScope::Layers;
        if ui.input(|input| input.modifiers.command || input.modifiers.shift) {
            if let Err(reason) = app.toggle_panel_sibling(ci, panel_id) {
                app.session.status = reason.to_owned();
            }
        } else {
            app.select_panel(ci, panel_id);
        }
    }
    if enter_panel {
        app.enter_panel(ci, panel_id);
    }
    if open {
        // `ContentId` is canvas-local, so the typed row identity needs the
        // canvas resource id as well; see `render_panels` for why rows get
        // global scopes at all.
        let canvas = app.doc.canvases[ci].resource_id;
        for content in panel.item_order.iter().rev().copied() {
            let row_scope =
                egui::UiBuilder::new().id(egui::Id::new(("layers_content_row", canvas, content)));
            ui.scope_builder(row_scope, |ui| {
                render_content(app, ci, panel_id, content, ui);
            });
        }
        if panel.item_order.is_empty() {
            super::layer_controls::row(
                ui,
                |ui| {
                    ui.add_space(36.0);
                    ui.add(egui::Label::new("Empty panel — add or move content here.").truncate());
                },
                |_| {},
            );
        }
    }
}

pub(super) fn panel_tree_name(app: &PlotiumApp, ci: usize, panel: &Panel) -> String {
    let label = app.doc.canvases[ci].panel_label_text(panel);
    if panel.label.visible && !label.is_empty() {
        format!("{label}  {}", panel.name)
    } else {
        panel.name.clone()
    }
}

fn render_content(
    app: &mut PlotiumApp,
    ci: usize,
    panel: PanelId,
    content: ContentId,
    ui: &mut Ui,
) {
    let Some(item) = app.doc.canvases[ci].object(content).cloned() else {
        return;
    };
    let path = SelectionPath::content(app.doc.canvases[ci].resource_id, Some(panel), content);
    let selected = app.session.ui.hierarchical_selection.contains(path);
    let mut select = false;
    let mut flags = None;
    let destinations: Vec<_> = app.doc.canvases[ci]
        .panels
        .iter()
        .filter(|candidate| candidate.id != panel && !candidate.locked)
        .map(|candidate| (candidate.id, candidate.name.clone()))
        .collect();
    let (visible_flags, locked_flags) = super::layer_controls::row(
        ui,
        |ui| {
            ui.add_space(24.0);
            let mut visible = item.visible;
            if super::layer_controls::visibility_button(ui, &mut visible).changed() {
                flags = Some((visible, item.locked));
            }
            ui.weak(super::layer_controls::kind_glyph(&item.kind))
                .on_hover_text(super::layer_controls::kind_label(&item.kind));
            let response =
                super::layer_controls::truncated_selectable(ui, selected, item.name.clone())
                    .interact(egui::Sense::click_and_drag());
            if response.drag_started() {
                app.session.ui.layers_drag_content = Some(content);
            }
            if response.clicked() || (response.secondary_clicked() && !selected) {
                select = true;
            }
            response.context_menu(|ui| {
                if ui.button("Move out of panel").clicked() {
                    app.select_content(ci, content);
                    crate::ui::commands::execute_without_clipboard(
                        crate::ui::commands::CommandId::MoveContentToPanel(None),
                        app,
                        ui.ctx(),
                    );
                    ui.close();
                }
                if !destinations.is_empty() {
                    ui.menu_button("Move to panel", |ui| {
                        for (target, name) in &destinations {
                            if ui.button(name).clicked() {
                                app.select_content(ci, content);
                                crate::ui::commands::execute_without_clipboard(
                                    crate::ui::commands::CommandId::MoveContentToPanel(Some(
                                        *target,
                                    )),
                                    app,
                                    ui.ctx(),
                                );
                                ui.close();
                            }
                        }
                    });
                }
            });
            flags
        },
        |ui| {
            let mut locked = item.locked;
            if super::layer_controls::lock_button(ui, &mut locked).changed() {
                Some((item.visible, locked))
            } else {
                None
            }
        },
    );
    flags = visible_flags.or(locked_flags);
    if let Some((visible, locked)) = flags {
        app.execute_action(Action::set_object_flags(
            ci,
            content,
            (item.visible, item.locked),
            (visible, locked),
        ));
    }
    if select {
        app.session.ui.selection_scope = plotium_core::state::SelectionScope::Layers;
        let additive = ui.input(|input| input.modifiers.command || input.modifiers.shift);
        if additive {
            if let Err(reason) = app.toggle_content_sibling(ci, content) {
                app.session.status = reason.to_owned();
            }
        } else {
            app.select_content(ci, content);
        }
    }
}

fn panel_context_menu(app: &mut PlotiumApp, ui: &mut Ui) {
    ui.menu_button("Order", |ui| {
        for (label, order) in [
            ("Bring to Front", plotium_core::actions::ZOrder::Front),
            ("Bring Forward", plotium_core::actions::ZOrder::Forward),
            ("Send Backward", plotium_core::actions::ZOrder::Backward),
            ("Send to Back", plotium_core::actions::ZOrder::Back),
        ] {
            let command = crate::ui::commands::CommandId::ZOrder(order);
            let descriptor = crate::ui::commands::describe(app, command);
            if ui
                .add_enabled(descriptor.enabled, egui::Button::new(label))
                .clicked()
            {
                crate::ui::commands::execute_without_clipboard(command, app, ui.ctx());
                ui.close();
            }
        }
    });
    for command in [
        crate::ui::commands::CommandId::DuplicatePanel,
        crate::ui::commands::CommandId::DissolvePanel,
        crate::ui::commands::CommandId::DeletePanel,
    ] {
        let descriptor = crate::ui::commands::describe(app, command);
        if ui
            .add_enabled(descriptor.enabled, egui::Button::new(descriptor.label))
            .clicked()
        {
            crate::ui::commands::execute_without_clipboard(command, app, ui.ctx());
            ui.close();
        }
    }
}

fn replace_panel(
    app: &mut PlotiumApp,
    ci: usize,
    id: PanelId,
    edit: impl FnOnce(&mut plotium_core::state::Panel),
) {
    let before = PanelState::of(&app.doc.canvases[ci]);
    let mut page = app.doc.canvases[ci].clone();
    if let Some(panel) = page.panel_mut(id) {
        edit(panel);
        app.execute_action(Action::ReplacePanelState {
            canvas: ci,
            before,
            after: PanelState::of(&page),
        });
    }
}

pub(super) fn object_list(app: &mut PlotiumApp, ci: usize, ui: &mut Ui) {
    render_panels(app, ci, ui);
    let mut select = None;
    let mut reorder: Option<(ObjectId, ZOrder)> = None;
    let mut transfer: Option<(ObjectId, usize, bool)> = None;
    let others = crate::ui::menus::other_canvas_destinations(app, ci);
    let panel_destinations: Vec<_> = app.doc.canvases[ci]
        .panels
        .iter()
        .filter(|panel| !panel.locked)
        .map(|panel| (panel.id, panel_tree_name(app, ci, panel)))
        .collect();
    let count = app.doc.canvases[ci].objects.len();
    // `ObjectId` is canvas-local, so the typed row identity includes the
    // canvas resource id; see `render_panels`.
    let canvas = app.doc.canvases[ci].resource_id;
    for row in 0..count {
        let oi = count - 1 - row;
        let object_id = app.doc.canvases[ci].objects[oi].id;
        if app.doc.canvases[ci].parent_panel(object_id).is_some() {
            continue;
        }
        let locked_before = app.doc.canvases[ci].objects[oi].locked;
        let row_scope =
            egui::UiBuilder::new().id(egui::Id::new(("layers_object_row", canvas, object_id)));
        let (_, (lock_change, row_reorder)) = ui
            .scope_builder(row_scope, |ui| {
                layer_row(
                    ui,
                    |ui| {
                        let mut visible = app.doc.canvases[ci].objects[oi].visible;
                        if visibility_button(ui, &mut visible).changed() {
                            let before = (
                                app.doc.canvases[ci].objects[oi].visible,
                                app.doc.canvases[ci].objects[oi].locked,
                            );
                            app.execute_action(Action::set_object_flags(
                                ci,
                                object_id,
                                before,
                                (visible, before.1),
                            ));
                        }
                        ui.weak(kind_glyph(&app.doc.canvases[ci].objects[oi].kind))
                            .on_hover_text(kind_label(&app.doc.canvases[ci].objects[oi].kind));
                        if app.doc.canvases[ci]
                            .content_group(app.doc.canvases[ci].objects[oi].id)
                            .is_some()
                        {
                            ui.weak(egui::RichText::new("⛓").small())
                                .on_hover_text("Grouped");
                        }
                        let selected = app.session.ui.selection.contains(object_id)
                            || app.session.ui.selection.object() == Some(object_id);
                        let resp = truncated_selectable(
                            ui,
                            selected,
                            app.doc.canvases[ci].objects[oi].name.clone(),
                        )
                        .interact(egui::Sense::click_and_drag());
                        if resp.drag_started() {
                            app.session.ui.layers_drag_content = Some(object_id);
                        }
                        if resp.clicked() || (resp.secondary_clicked() && !selected) {
                            claim_list_keyboard_focus(ui, &resp);
                            select = Some((object_id, select_modifiers(ui)));
                        }
                        resp.context_menu(|ui| {
                            object_transfer_menu(ui, object_id, &others, &mut transfer);
                            if !panel_destinations.is_empty() {
                                ui.menu_button("Move into panel", |ui| {
                                    for (panel, name) in &panel_destinations {
                                        if ui.button(name).clicked() {
                                            app.select_content(ci, object_id);
                                            crate::ui::commands::execute_without_clipboard(
                                                crate::ui::commands::CommandId::MoveContentToPanel(
                                                    Some(*panel),
                                                ),
                                                app,
                                                ui.ctx(),
                                            );
                                            ui.close();
                                        }
                                    }
                                });
                            }
                        });
                    },
                    |ui| {
                        let mut locked = locked_before;
                        let lock_change = lock_button(ui, &mut locked).changed().then_some(locked);
                        let mut row_reorder = None;
                        if ui
                            .add_enabled(
                                row + 1 < count,
                                egui::Button::new(icon::CARET_DOWN).small().frame(false),
                            )
                            .on_hover_text("Send backward")
                            .clicked()
                        {
                            row_reorder = Some(ZOrder::Backward);
                        }
                        if ui
                            .add_enabled(
                                row > 0,
                                egui::Button::new(icon::CARET_UP).small().frame(false),
                            )
                            .on_hover_text("Bring forward")
                            .clicked()
                        {
                            row_reorder = Some(ZOrder::Forward);
                        }
                        (lock_change, row_reorder)
                    },
                )
            })
            .inner;
        if let Some(locked) = lock_change
            && let Some(target) = app.object_target(ci, object_id)
            && let Ok(commit) = app.plan_property_write(
                plotium_core::properties::object::LOCKED,
                std::slice::from_ref(&target),
                &plotium_core::properties::PropertyValue::Bool(locked),
            )
        {
            app.commit_property(commit);
        }
        if let Some(operation) = row_reorder {
            reorder = Some((object_id, operation));
        }
    }
    if let Some((object_id, modifiers)) = select {
        app.session.ui.selection_scope = plotium_core::state::SelectionScope::Layers;
        select_layer_range(app, ci, object_id, modifiers);
        let active = app.doc.canvases[ci]
            .object(object_id)
            .and_then(|object| object.dataset())
            .and_then(|id| app.doc.dataset_index(id));
        app.set_active_dataset(active);
        app.reset_interaction();
        app.session.ui.panel_note_inline_edit = None;
        app.session.ui.panel_note_edit = None;
    }
    if let Some((object_id, op)) = reorder {
        app.apply_z_order(ci, &[object_id], op);
    }
    if let Some((object_id, to, is_move)) = transfer {
        app.transfer_objects_to_canvas(ci, &[object_id], to, is_move);
    }
}
fn object_transfer_menu(
    ui: &mut Ui,
    object_id: ObjectId,
    others: &[(usize, String)],
    transfer: &mut Option<(ObjectId, usize, bool)>,
) {
    let mut picked = None;
    crate::ui::menus::transfer_to_canvas_menu(
        ui,
        others,
        "Move to canvas",
        "Copy to canvas",
        &mut picked,
    );
    if let Some((to, is_move)) = picked {
        *transfer = Some((object_id, to, is_move));
    }
}
