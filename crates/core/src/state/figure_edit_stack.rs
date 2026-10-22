//! `panel_stack`: turn a panel's shared-x stack on or off for agents.
use super::*;
use crate::automation::{TargetRef, canvas_object_ref, panel_resource};
use serde_json::{Value, json};

/// Widest gap the panel inspector offers, in points.
const MAX_STACK_GAP_PT: f32 = 144.0;

pub(super) struct StackRequest<'a> {
    pub target: &'a TargetRef,
    pub shared_x: bool,
    pub gap_pt: Option<f32>,
    /// Panels merged into `target` before it is stacked.
    pub merge: &'a [TargetRef],
}

/// Resolve an unlocked Panel whose layout a stack may replace.
fn stackable_panel(page: &CanvasDocument, target: &TargetRef) -> Result<PanelId, String> {
    let panel = page
        .panels
        .iter()
        .find(|p| *target == TargetRef::resource(panel_resource(page, p)))
        .ok_or("Discover an exact Panel target on this canvas.")?;
    if panel.locked
        || panel
            .item_order
            .iter()
            .any(|id| page.object(*id).is_some_and(|o| o.locked))
    {
        return Err("Panel and its contents must be unlocked.".into());
    }
    if !matches!(panel.layout, PanelLayout::Free | PanelLayout::SharedXStack) {
        return Err("panel_stack starts from a free-layout Panel.".into());
    }
    Ok(panel.id)
}

pub(super) fn apply(
    page: &mut CanvasDocument,
    request: &StackRequest<'_>,
    index: usize,
) -> Result<Value, String> {
    let id = stackable_panel(page, request.target)?;
    let mut merged = Vec::with_capacity(request.merge.len());
    for target in request.merge {
        let other = stackable_panel(page, target)?;
        if other == id || merged.contains(&other) {
            return Err("merge lists distinct Panels other than the stacked Panel.".into());
        }
        merged.push(other);
    }
    if !request.shared_x && !merged.is_empty() {
        return Err("merge applies only with shared_x true.".into());
    }
    if let Some(gap) = request.gap_pt
        && (!request.shared_x || !gap.is_finite() || !(0.0..=MAX_STACK_GAP_PT).contains(&gap))
    {
        return Err(format!(
            "gap_pt applies with shared_x true and must be 0–{MAX_STACK_GAP_PT} pt."
        ));
    }
    let panel = page.panel(id).ok_or("Panel missing.")?;
    let stacked = panel.layout == PanelLayout::SharedXStack;
    let plots = std::iter::once(id)
        .chain(merged.iter().copied())
        .filter_map(|panel| page.panel(panel))
        .map(|panel| page.shared_x_stack_plots(panel).len())
        .sum::<usize>();
    if request.shared_x && plots < 2 {
        return Err("A shared-x stack needs at least two plots; list the Panels of the other plots in merge.".into());
    }
    let gap = request
        .gap_pt
        .unwrap_or(if stacked { panel.layout_gap } else { 0.0 });
    if request.shared_x {
        crate::actions::stack_panels(page, id, &merged, gap).map_err(|e| e.to_string())?;
    } else {
        // Members stay where the stack placed them.
        page.panel_mut(id).ok_or("Panel missing.")?.layout = PanelLayout::Free;
    }
    let panel = page.panel(id).ok_or("Panel missing.")?;
    let members = panel
        .item_order
        .iter()
        .map(|item| TargetRef::resource(canvas_object_ref(page.resource_id, *item)))
        .collect::<Vec<_>>();
    Ok(json!({
        "op": "panel_stack",
        "edit_index": index,
        "target": request.target,
        "layout": if request.shared_x { "shared_x_stack" } else { "free" },
        "gap_pt": panel.layout_gap,
        "members_top_to_bottom": members,
        "merged_panels": request.merge,
    }))
}
