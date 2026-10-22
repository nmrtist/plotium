//! Shared-x stacks: a panel layout whose plots sit top to bottom on one x axis.
//!
//! Only the layout mode is persisted. Member order is the panel's item order,
//! member proportions are their frame heights, and the x link, aligned margins
//! and joined inner boundaries are derived from those on demand, so leaving
//! the mode or removing a member never leaves stale state behind.
use super::{
    AxisOverrides, AxisRange, CanvasDocument, ContentId, ObjectFrame, ObjectId, Panel, PanelId,
    PanelLayout,
};
use plotium_figure::AxisFrame;
use plotium_render::StackPlacement;
use std::collections::BTreeMap;

/// Frames closer than this to the stack gap count as touching; it absorbs
/// float drift from proportional reflow and panel scaling.
const JOIN_TOLERANCE_PT: f32 = 0.5;

impl CanvasDocument {
    /// The shared-x stack panel that directly contains `content`, if any.
    pub fn shared_x_stack_of(&self, content: ContentId) -> Option<&Panel> {
        self.parent_panel(content)
            .and_then(|id| self.panel(id))
            .filter(|panel| panel.layout == PanelLayout::SharedXStack)
    }

    /// Plot members of a stack in top-to-bottom order.
    pub fn shared_x_stack_plots(&self, panel: &Panel) -> Vec<ObjectId> {
        panel
            .item_order
            .iter()
            .copied()
            .filter(|id| self.object(*id).is_some_and(|item| item.plot().is_some()))
            .collect()
    }

    /// Plots whose x axis settings follow `object` because they share its
    /// stack. Empty unless the stack holds at least two plots.
    pub fn shared_x_axis_peers(&self, object: ObjectId) -> Vec<ObjectId> {
        let Some(panel) = self.shared_x_stack_of(object) else {
            return Vec::new();
        };
        let plots = self.shared_x_stack_plots(panel);
        if plots.len() < 2 || !plots.contains(&object) {
            return Vec::new();
        }
        plots
    }

    /// Page frames of every item of a shared-x stack, relative to the panel.
    /// Items fill the inner width; heights keep the proportions of the current
    /// frames and fill the inner height between `gap`s.
    pub fn shared_x_stack_frames(&self, panel: &Panel) -> Result<Vec<ObjectFrame>, String> {
        let weights = panel
            .item_order
            .iter()
            .map(|id| {
                self.object(*id)
                    .map(|item| item.frame.height)
                    .ok_or_else(|| format!("panel {} references missing content {id}", panel.id))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let padding = panel.layout_padding;
        let gaps = panel.layout_gap * weights.len().saturating_sub(1) as f32;
        let width = panel.frame.width - padding * 2.0;
        let height = panel.frame.height - padding * 2.0 - gaps;
        if width <= 0.0 || height <= 0.0 {
            return Err("panel is too small for its layout padding and gap".to_owned());
        }
        let total: f32 = weights.iter().sum();
        if !total.is_finite() || total <= 0.0 {
            return Err("stack members need positive frame heights".to_owned());
        }
        let mut top = padding;
        Ok(weights
            .into_iter()
            .map(|weight| {
                let frame = ObjectFrame::new(padding, top, width, height * weight / total);
                top += frame.height + panel.layout_gap;
                frame
            })
            .collect())
    }

    /// Re-apply a stack's layout after a member's height or the panel frame
    /// changed. Other layouts are left untouched.
    pub fn reflow_shared_x_stack(&mut self, panel: PanelId) -> Result<(), String> {
        let Some(stack) = self
            .panel(panel)
            .filter(|panel| panel.layout == PanelLayout::SharedXStack)
            .cloned()
        else {
            return Ok(());
        };
        let frames = self.shared_x_stack_frames(&stack)?;
        for (id, frame) in stack.item_order.into_iter().zip(frames) {
            if let Some(item) = self.object_mut(id) {
                item.frame = frame;
            }
        }
        Ok(())
    }

    /// Runtime placements of every plot in a stack of two or more axis plots.
    pub fn shared_x_stack_placements(&self) -> BTreeMap<ObjectId, StackPlacement> {
        self.panels
            .iter()
            .filter(|panel| panel.layout == PanelLayout::SharedXStack)
            .flat_map(|panel| self.panel_placements(panel))
            .collect()
    }

    /// The runtime placement of one plot, computed from its own stack only.
    pub fn shared_x_stack_placement(&self, object: ObjectId) -> Option<StackPlacement> {
        let panel = self.shared_x_stack_of(object)?;
        self.panel_placements(panel)
            .into_iter()
            .find_map(|(id, placement)| (id == object).then_some(placement))
    }

    /// Make every plot of a stack draw its bottom plot's x direction, and
    /// return every other plot to the direction it was built with.
    pub fn sync_shared_x_direction(&mut self) {
        let mut directions: BTreeMap<ObjectId, bool> = BTreeMap::new();
        for panel in self
            .panels
            .iter()
            .filter(|panel| panel.layout == PanelLayout::SharedXStack)
        {
            let plots = self.shared_x_stack_plots(panel);
            let bottom = plots
                .last()
                .and_then(|id| self.object(*id))
                .and_then(|item| item.plot());
            if let (true, Some(bottom)) = (plots.len() >= 2, bottom) {
                let reversed = bottom.natural_x_reversed();
                directions.extend(plots.into_iter().map(|id| (id, reversed)));
            }
        }
        for item in &mut self.objects {
            let direction = directions.get(&item.id).copied();
            if let Some(plot) = item.plot_mut() {
                plot.set_stack_x_reversed(direction);
            }
        }
    }

    fn panel_placements(&self, panel: &Panel) -> Vec<(ObjectId, StackPlacement)> {
        // A joined boundary needs an axis plot on both sides; text or an image
        // between two plots keeps both plots' own margins at that boundary.
        let axis_plot = |id: &ObjectId| {
            self.object(*id)
                .and_then(|item| item.plot().map(|plot| (item.frame, plot.figure())))
                .filter(|(_, figure)| figure.axis_frame != AxisFrame::Hidden)
        };
        let members: Vec<_> = panel
            .item_order
            .iter()
            .map(|id| (*id, axis_plot(id)))
            .collect();
        let plots: Vec<_> = members
            .iter()
            .filter_map(|(id, plot)| plot.map(|plot| (*id, plot)))
            .collect();
        if plots.len() < 2 {
            return Vec::new();
        }
        let (left, right, y_tick_clearance, y2_tick_clearance) = plots
            .iter()
            .map(|(_, (frame, figure))| {
                let layout = plotium_render::axis_layout(figure, frame.width, frame.height);
                (
                    layout.y_tick_clearance(figure),
                    layout.y2_tick_clearance(figure),
                    layout.margins,
                )
            })
            .fold(
                (0.0f32, 0.0f32, 0.0f32, 0.0f32),
                |(left, right, ticks, right_ticks), (clearance, right_clearance, margins)| {
                    (
                        left.max(margins.left),
                        right.max(margins.right),
                        ticks.max(clearance),
                        right_ticks.max(right_clearance),
                    )
                },
            );
        // Two neighbouring plots are joined only while their frames still meet
        // across the gap: deleting or moving out a member leaves a hole until
        // the stack reflows, and the plots beside it must keep their own axes.
        let joined = |upper: Option<usize>, lower: Option<usize>| {
            let frame = |index: Option<usize>| {
                index
                    .and_then(|index| members.get(index))
                    .and_then(|(_, plot)| plot.map(|(frame, _)| frame))
            };
            frame(upper)
                .zip(frame(lower))
                .is_some_and(|(upper, lower)| {
                    (lower.y - (upper.y + upper.height) - panel.layout_gap).abs()
                        <= JOIN_TOLERANCE_PT
                })
        };
        members
            .iter()
            .enumerate()
            .filter(|(_, (_, plot))| plot.is_some())
            .map(|(index, (id, _))| {
                (
                    *id,
                    StackPlacement {
                        left,
                        right,
                        y_tick_clearance,
                        y2_tick_clearance,
                        joined_above: joined(index.checked_sub(1), Some(index)),
                        joined_below: joined(Some(index), Some(index + 1)),
                    },
                )
            })
            .collect()
    }
}

/// Convert an x value between two plots' coordinate units. A plot whose unit
/// is unknown shares the other's numbers, since the author stacked them on one
/// axis; two known but incompatible units cannot share one.
pub(crate) fn shared_x_value(
    value: f64,
    from: &Option<plotium_data::UnitSpec>,
    to: &Option<plotium_data::UnitSpec>,
) -> Result<f64, String> {
    if from.is_none() || to.is_none() {
        return Ok(value);
    }
    super::convert_curve_value(value, from, to, false)
}

impl CanvasDocument {
    /// Give the numeric-x plots of a stack one x window: the union of their
    /// current windows as a manual x range, written in each plot's own
    /// coordinate unit. Their x scales must already agree, because only a
    /// direct edit can check that a plot accepts a logarithmic axis.
    pub fn unify_shared_x_window(&mut self, panel: PanelId) -> Result<(), String> {
        let Some(stack) = self
            .panel(panel)
            .filter(|panel| panel.layout == PanelLayout::SharedXStack)
        else {
            return Ok(());
        };
        let members: Vec<_> = self
            .shared_x_stack_plots(stack)
            .into_iter()
            .filter_map(|id| {
                let plot = self.object(id)?.plot()?;
                let figure = plot.figure();
                (figure.x.categories.is_none() && figure.axis_frame != AxisFrame::Hidden).then(
                    || {
                        let unit = plot.binding.coordinates.as_ref().and_then(|c| c.x.clone());
                        (id, unit, plot.viewport.full_x, plot.axis_overrides.x_scale)
                    },
                )
            })
            .collect();
        let [(_, reference, _, scale), ..] = members.as_slice() else {
            return Ok(());
        };
        if members.len() < 2 {
            return Ok(());
        }
        let linear = |scale: &Option<plotium_figure::AxisScale>| {
            scale.unwrap_or_default() == plotium_figure::AxisScale::Linear
        };
        if members
            .iter()
            .any(|(_, _, _, other)| linear(other) != linear(scale))
        {
            return Err(
                "Give the plots the same x scale (linear or logarithmic) before stacking them on a shared x axis."
                    .into(),
            );
        }
        let incompatible =
            |error: String| format!("Plots in a shared-x stack need compatible x units: {error}");
        let mut union: Option<AxisRange> = None;
        for (_, unit, full, _) in &members {
            let min = shared_x_value(full.min, unit, reference).map_err(incompatible)?;
            let max = shared_x_value(full.max, unit, reference).map_err(incompatible)?;
            let range = AxisRange::new(min, max);
            union = Some(union.map_or(range, |u| {
                AxisRange::new(u.min.min(range.min), u.max.max(range.max))
            }));
        }
        let Some(union) = union.filter(|range| range.is_valid()) else {
            return Ok(());
        };
        if *scale == Some(plotium_figure::AxisScale::Log10) && union.min <= 0.0 {
            return Err(
                "The stacked plots' x data includes values at or below zero, which the top plot's logarithmic x axis cannot show."
                    .into(),
            );
        }
        let reference = reference.clone();
        for (id, unit, _, _) in members {
            let min = shared_x_value(union.min, &reference, &unit).map_err(incompatible)?;
            let max = shared_x_value(union.max, &reference, &unit).map_err(incompatible)?;
            if let Some(plot) = self.object_mut(id).and_then(|item| item.plot_mut()) {
                let range = AxisRange::new(min, max);
                plot.axis_overrides.x_range = Some(range);
                // Rebuilding keeps a narrower view inside the wider window, so
                // start every member from the whole shared window.
                plot.viewport.full_x = range;
                plot.viewport.view_x = range;
            }
        }
        Ok(())
    }
}

impl AxisOverrides {
    /// Whether two override values differ in an x setting a stack shares.
    pub fn shared_x_differs(&self, other: &Self) -> bool {
        self.x_range != other.x_range
            || self.x_scale != other.x_scale
            || self.x_display_unit != other.x_display_unit
            || self.x_thousands_separator != other.x_thousands_separator
    }
}

#[cfg(test)]
#[path = "shared_x_stack_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "shared_x_stack_review_tests.rs"]
mod review_tests;
