//! Shared-x stack commands on top of the generic panel layout actions.
use super::{Action, PanelActionError, PanelState, apply_panel_layout, merge_panels};
use crate::state::{CanvasDocument, PanelId, PanelLayout, PlotiumApp};

impl PlotiumApp {
    /// Turn a panel's shared-x stack on or off. Entering the mode closes the
    /// gap unless `gap` is given; leaving it returns the panel to free layout
    /// and keeps the members where the stack placed them.
    pub fn set_shared_x_stack_action(
        &self,
        canvas: usize,
        panel: PanelId,
        shared_x: bool,
        gap: Option<f32>,
    ) -> Result<Action, PanelActionError> {
        let current = self
            .doc
            .canvases
            .get(canvas)
            .ok_or(PanelActionError::MissingCanvas(canvas))?
            .panel(panel)
            .ok_or(PanelActionError::MissingPanel(panel))?;
        let stacked = current.layout == PanelLayout::SharedXStack;
        let (layout, default_gap) = match (shared_x, stacked) {
            (true, true) => (PanelLayout::SharedXStack, current.layout_gap),
            (true, false) => (PanelLayout::SharedXStack, 0.0),
            (false, true) => (PanelLayout::Free, current.layout_gap),
            (false, false) => (current.layout, current.layout_gap),
        };
        self.set_panel_layout_action(
            canvas,
            panel,
            layout,
            gap.unwrap_or(default_gap),
            current.layout_padding,
            current.layout_alignment,
        )
    }

    /// Merge several panels into the first and stack their content on one x
    /// axis, top to bottom in their page order, as a single undo step.
    pub fn stack_panels_shared_x_action(
        &self,
        canvas: usize,
        primary: PanelId,
        others: &[PanelId],
    ) -> Result<Action, PanelActionError> {
        let before = self
            .doc
            .canvases
            .get(canvas)
            .map(PanelState::of)
            .ok_or(PanelActionError::MissingCanvas(canvas))?;
        let mut page = self.doc.canvases[canvas].clone();
        stack_panels(&mut page, primary, others, 0.0)?;
        page.validate_structure()
            .map_err(PanelActionError::Invalid)?;
        Ok(Action::ReplacePanelState {
            canvas,
            before,
            after: PanelState::of(&page),
        })
    }
}

/// Make `primary` a shared-x stack. Panels in `others` are merged into it
/// first, and the merged members then run top to bottom in the order their
/// frames sat on the page; without a merge the content order is kept.
pub(crate) fn stack_panels(
    page: &mut CanvasDocument,
    primary: PanelId,
    others: &[PanelId],
    gap: f32,
) -> Result<(), PanelActionError> {
    if !others.is_empty() {
        merge_panels(page, primary, others)?;
        let mut order = page
            .panel(primary)
            .ok_or(PanelActionError::MissingPanel(primary))?
            .item_order
            .clone();
        // Merged frames are relative to the merged panel, so their tops keep
        // the page order the panels were arranged in.
        order.sort_by(|a, b| {
            let top = |id| page.object(id).map_or(f32::INFINITY, |item| item.frame.y);
            top(*a).total_cmp(&top(*b))
        });
        page.panel_mut(primary)
            .ok_or(PanelActionError::MissingPanel(primary))?
            .item_order = order;
    }
    let target = page
        .panel_mut(primary)
        .ok_or(PanelActionError::MissingPanel(primary))?;
    target.layout = PanelLayout::SharedXStack;
    target.layout_gap = gap;
    apply_panel_layout(page, primary)
}
