use super::*;
use crate::actions::NativePlotInsertion;
use crate::state::PanelLayout;

impl PlotiumApp {
    pub(super) fn validate_plot_insertion(
        &self,
        insertion: &NativePlotInsertion,
    ) -> Result<(), ActionApplyError> {
        match insertion {
            NativePlotInsertion::NewCanvas { page, .. } => {
                if self.doc.canvas_index(page.resource_id).is_some() {
                    return Err(ActionApplyError::StaleTarget(
                        "Created canvas already exists.".into(),
                    ));
                }
                page.validate_structure()
                    .map_err(ActionApplyError::InvalidValue)?;
            }
            NativePlotInsertion::ExistingCanvas {
                canvas,
                object,
                panel,
                new_panel,
                additional_panel,
                ..
            } => {
                let ci = self.doc.canvas_index(*canvas).ok_or_else(|| {
                    ActionApplyError::StaleTarget("Parent canvas is missing.".into())
                })?;
                let mut page = self.doc.canvases[ci].clone();
                if page.object(object.id).is_some()
                    || (!additional_panel && page.objects.iter().any(|o| o.plot().is_some()))
                {
                    return Err(ActionApplyError::StaleTarget(
                        "Parent canvas already contains a plot.".into(),
                    ));
                }
                if *new_panel {
                    if (!additional_panel && !page.panels.is_empty())
                        || page.panel(panel.id).is_some()
                    {
                        return Err(ActionApplyError::StaleTarget(
                            "Choose an empty canvas without panels.".into(),
                        ));
                    }
                    page.panels.push(panel.as_ref().clone());
                } else {
                    if page.panels.len() != 1
                        || page.panel(panel.id) != Some(panel.as_ref())
                        || panel.locked
                        || !panel.visible
                        || !panel.item_order.is_empty()
                        || panel.layout != PanelLayout::Free
                    {
                        return Err(ActionApplyError::StaleTarget("Parent panel changed or is not empty, visible, unlocked and free-layout.".into()));
                    }
                    page.panel_mut(panel.id)
                        .expect("validated panel")
                        .item_order
                        .push(object.id);
                }
                page.next_object_id =
                    page.next_object_id
                        .max(object.id.try_advance(1).ok_or_else(|| {
                            ActionApplyError::InvalidValue("Content identity exhausted.".into())
                        })?);
                if *new_panel {
                    let slot = match panel.label.mode {
                        crate::state::PanelLabelMode::Auto { slot } => slot,
                        _ => 0,
                    };
                    page.next_panel_label_slot =
                        page.next_panel_label_slot.max(slot.saturating_add(1));
                }
                page.objects.push(object.as_ref().clone());
                page.validate_structure()
                    .map_err(ActionApplyError::InvalidValue)?;
            }
        }
        Ok(())
    }

    pub(super) fn apply_plot_insertion(
        &mut self,
        insertion: &NativePlotInsertion,
    ) -> Result<(), ActionApplyError> {
        // Redo also validates before touching the page.
        self.validate_plot_insertion(insertion)?;
        match insertion {
            NativePlotInsertion::NewCanvas { page, .. } => {
                self.insert_canvas_value(
                    self.doc.canvases.len(),
                    page.as_ref().clone(),
                    false,
                    true,
                );
            }
            NativePlotInsertion::ExistingCanvas {
                canvas,
                object,
                panel,
                new_panel,
                next_label_slot,
                ..
            } => {
                let ci = self.doc.canvas_index(*canvas).expect("validated parent");
                let page = &mut self.doc.canvases[ci];
                page.next_object_id =
                    page.next_object_id
                        .max(object.id.try_advance(1).ok_or_else(|| {
                            ActionApplyError::InvalidValue("Content identity exhausted.".into())
                        })?);
                page.next_panel_label_slot = page.next_panel_label_slot.max(*next_label_slot);
                page.objects.push(object.as_ref().clone());
                if *new_panel {
                    page.panels.push(panel.as_ref().clone());
                } else {
                    page.panel_mut(panel.id)
                        .expect("validated panel")
                        .item_order
                        .push(object.id);
                }
            }
        }
        Ok(())
    }

    /// Later GUI edits may mint and then undo child identities before this creation
    /// is undone. Keep those high-water marks in the original subtree's Redo value.
    pub(super) fn retain_created_plot_allocators(&self, action: &mut Action) {
        match action {
            Action::Composite(actions) => {
                for child in actions {
                    self.retain_created_plot_allocators(child);
                }
            }
            Action::CreatePlot { insertion } => match insertion.as_mut() {
                NativePlotInsertion::NewCanvas { page, .. } => {
                    if let Some(ci) = self.doc.canvas_index(page.resource_id) {
                        let live = &self.doc.canvases[ci];
                        page.next_object_id = page.next_object_id.max(live.next_object_id);
                        page.next_group_id = page.next_group_id.max(live.next_group_id);
                        page.next_panel_label_slot =
                            page.next_panel_label_slot.max(live.next_panel_label_slot);
                        for object in &mut page.objects {
                            if let Some(current) = live.object(object.id) {
                                retain_series_allocator(object, current);
                            }
                        }
                    }
                }
                NativePlotInsertion::ExistingCanvas { canvas, object, .. } => {
                    if let Some(current) = self
                        .doc
                        .canvas_index(*canvas)
                        .and_then(|ci| self.doc.canvases[ci].object(object.id))
                    {
                        retain_series_allocator(object, current);
                    }
                }
            },
            Action::DeleteFigureTargets { change } => {
                if let Some(page) = self
                    .doc
                    .canvas_index(change.canvas)
                    .map(|ci| &self.doc.canvases[ci])
                {
                    change.retain_allocators(page);
                }
            }
            _ => {}
        }
    }

    pub(super) fn revert_plot_insertion(&mut self, insertion: &NativePlotInsertion) {
        match insertion {
            NativePlotInsertion::NewCanvas {
                page,
                active_before,
            } => {
                if let Some(ci) = self.doc.canvas_index(page.resource_id) {
                    let before = active_before
                        .and_then(|id| self.doc.canvas_index(id))
                        .map(|i| if i > ci { i - 1 } else { i });
                    self.remove_canvas_at(ci, before);
                }
            }
            NativePlotInsertion::ExistingCanvas {
                canvas,
                object,
                panel,
                new_panel,
                ..
            } => {
                if let Some(ci) = self.doc.canvas_index(*canvas) {
                    // This preserves the page's allocator high-water marks, including after Undo.
                    if let Some(parent) = self.doc.canvases[ci].panel_mut(panel.id) {
                        parent.item_order.retain(|id| *id != object.id);
                    }
                    let selection_elsewhere = (self.session.active_canvas != Some(ci))
                        .then(|| self.session.ui.selection.clone());
                    self.remove_object_value(ci, object.id);
                    if let Some(selection) = selection_elsewhere {
                        self.session.ui.selection = selection;
                    }
                    if *new_panel {
                        self.doc.canvases[ci].panels.retain(|p| p.id != panel.id);
                    }
                }
            }
        }
    }
}

pub(super) fn retain_series_allocator(saved: &mut CanvasObject, live: &CanvasObject) {
    if let (Some(saved), Some(live)) = (saved.plot_mut(), live.plot()) {
        saved.next_series_id = saved.next_series_id.max(live.next_series_id);
    }
}
