use super::{
    AxisOverrides, AxisProjections, CanvasViewport, ChartSpec, DataBinding, DatasetId, DerivedAxes,
    SeriesId, StackSpec,
};
use plotium_figure::{Figure, FigureTypography};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_FIGURE_GEOMETRY_GENERATION: AtomicU64 = AtomicU64::new(1);

fn next_figure_geometry_generation() -> u64 {
    NEXT_FIGURE_GEOMETRY_GENERATION
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
            value.checked_add(1)
        })
        .expect("figure geometry generation overflow")
}

#[derive(Clone)]
pub struct PlotObject {
    /// Dataset whose current display/channel choice this default view follows.
    /// Independent plots carry `None` and keep their item-addressed sources.
    pub display_owner: Option<DatasetId>,
    /// Persistent high-water mark for owner-local series identities. This is
    /// deliberately outside `binding`, which actions may replace wholesale.
    pub next_series_id: SeriesId,
    pub binding: DataBinding,
    /// The selected chart type (registry id) + its context, driving figure
    /// rebuilds through `state::charts`. Defaults to the dataset domain's default.
    pub chart: ChartSpec,
    /// The multi-series stacking layout. Default = superimposed overlay.
    pub stack: StackSpec,
    /// Marginal 1D axis projections for a 2D contour (empty for other plots).
    pub projections: AxisProjections,
    pub axis_overrides: AxisOverrides,
    /// Axis presentation emitted by the latest figure build, before author
    /// overrides are applied. Derived property defaults read this artifact.
    derived_axes: DerivedAxes,
    figure: Figure,
    /// Runtime-only identity for screen geometry caches. Clones retain it because
    /// their contour buffers have identical contents; figure rebuilds replace it.
    figure_geometry_generation: u64,
    pub viewport: CanvasViewport,
    /// Runtime x direction imposed by a shared-x stack (its bottom plot's), never
    /// persisted. `None` draws the direction the figure was built with.
    stack_x_reversed: Option<bool>,
    /// The x direction of the latest built figure, restored on leaving a stack.
    natural_x_reversed: bool,
    /// Whether the latest built figure was turned on its side; a change swaps
    /// the axes, so the old view no longer applies. Runtime only.
    built_transposed: bool,
}

impl PlotObject {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        display_owner: Option<DatasetId>,
        next_series_id: SeriesId,
        binding: DataBinding,
        chart: ChartSpec,
        stack: StackSpec,
        projections: AxisProjections,
        axis_overrides: AxisOverrides,
        figure: Figure,
        viewport: CanvasViewport,
    ) -> Self {
        let derived_axes = DerivedAxes::from_figure(&figure);
        let natural_x_reversed = figure.x.reversed;
        let mut plot = Self {
            display_owner,
            next_series_id,
            binding,
            chart,
            stack,
            projections,
            axis_overrides,
            derived_axes,
            figure,
            figure_geometry_generation: next_figure_geometry_generation(),
            viewport,
            stack_x_reversed: None,
            natural_x_reversed,
            built_transposed: false,
        };
        plot.viewport.auto_y2 = !plot.has_manual_y2_range();
        plot.built_transposed = plot
            .binding
            .coordinates
            .as_ref()
            .is_some_and(|c| c.transposed);
        plot.make_legend_room();
        plot
    }

    /// Restore a materialized figure that already contains author overrides and
    /// viewport state while retaining the separately rebuilt automatic axes.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_materialized_figure(
        display_owner: Option<DatasetId>,
        next_series_id: SeriesId,
        binding: DataBinding,
        chart: ChartSpec,
        stack: StackSpec,
        projections: AxisProjections,
        axis_overrides: AxisOverrides,
        derived_axes: DerivedAxes,
        figure: Figure,
        viewport: CanvasViewport,
    ) -> Self {
        let natural_x_reversed = figure.x.reversed;
        let mut plot = Self {
            display_owner,
            next_series_id,
            binding,
            chart,
            stack,
            projections,
            axis_overrides,
            derived_axes,
            figure,
            figure_geometry_generation: next_figure_geometry_generation(),
            viewport,
            stack_x_reversed: None,
            natural_x_reversed,
            built_transposed: false,
        };
        plot.built_transposed = plot
            .binding
            .coordinates
            .as_ref()
            .is_some_and(|c| c.transposed);
        super::axis_display::apply_display(
            &mut plot.figure,
            plot.binding.coordinates.as_ref(),
            &plot.derived_axes,
            &plot.axis_overrides,
        );
        plot.axis_overrides
            .typography
            .apply_to(&mut plot.figure.typography);
        plot.viewport.auto_y2 = !plot.has_manual_y2_range();
        plot.make_legend_room();
        plot
    }

    pub fn figure(&self) -> &Figure {
        &self.figure
    }

    /// The x direction this plot draws outside a shared-x stack.
    pub fn natural_x_reversed(&self) -> bool {
        self.natural_x_reversed
    }

    /// Follow a shared-x stack's x direction, or return to the built one.
    pub(crate) fn set_stack_x_reversed(&mut self, reversed: Option<bool>) {
        if self.stack_x_reversed == reversed {
            return;
        }
        self.stack_x_reversed = reversed;
        self.figure.x.reversed = reversed.unwrap_or(self.natural_x_reversed);
        self.figure_geometry_generation = next_figure_geometry_generation();
    }

    pub fn figure_geometry_generation(&self) -> u64 {
        self.figure_geometry_generation
    }

    pub fn derived_axes(&self) -> &DerivedAxes {
        &self.derived_axes
    }

    pub(crate) fn automatic_axis_label(&self, x: bool) -> String {
        let coordinates = self.binding.coordinates.as_ref();
        super::axis_display::display_label(
            if x {
                &self.derived_axes.x_label
            } else {
                &self.derived_axes.y_label
            },
            coordinates.and_then(|c| if x { c.horizontal() } else { c.vertical() }),
            if x {
                self.axis_overrides.x_display_unit.as_deref()
            } else {
                self.axis_overrides.y_display_unit.as_deref()
            },
        )
    }

    /// The right-axis title the plot shows without an authored one.
    pub(crate) fn automatic_right_axis_label(&self) -> String {
        let coordinates = self.binding.coordinates.as_ref();
        super::axis_display::display_label(
            &self.derived_axes.y2_label,
            coordinates.and_then(|c| c.y2.as_ref()),
            self.axis_overrides.y2_display_unit.as_deref(),
        )
    }

    pub fn allocate_series_id(&mut self) -> SeriesId {
        let id = self.next_series_id;
        self.next_series_id = id.checked_advance(1);
        id
    }

    /// Assign identities to a newly materialized binding in order. Callers that
    /// restore persisted bindings must preserve their ids and use
    /// `repair_series_allocator` instead.
    pub fn mint_series_ids(&mut self) {
        let start = self.next_series_id;
        for (offset, series) in self.binding.series.iter_mut().enumerate() {
            series.id = start.checked_advance(offset as u64);
        }
        self.next_series_id = start.checked_advance(self.binding.series.len() as u64);
    }

    /// Raise the allocator above every id the (possibly persisted) binding
    /// already carries, so the next `allocate_series_id` cannot alias one.
    ///
    /// Returns `None` when the binding's highest id is `u64::MAX` and no
    /// successor exists: the file is unusable rather than merely inconsistent,
    /// and the caller must reject it. Defaulting to zero here would skip the
    /// repair entirely and hand out a duplicate on the very next allocation.
    #[must_use]
    pub fn repair_series_allocator(&mut self) -> Option<()> {
        let Some(highest) = self.binding.series.iter().map(|series| series.id).max() else {
            return Some(());
        };
        let required = highest.try_advance(1)?;
        self.next_series_id = self.next_series_id.max(required);
        Some(())
    }

    pub fn primary_dataset(&self) -> Option<DatasetId> {
        self.binding.primary_dataset()
    }

    fn commit_rebuilt_figure(
        &mut self,
        mut figure: Figure,
        prepare: impl FnOnce(&mut Self, &mut Figure),
    ) {
        self.derived_axes = DerivedAxes::from_figure(&figure);
        self.natural_x_reversed = figure.x.reversed;
        self.built_transposed = self
            .binding
            .coordinates
            .as_ref()
            .is_some_and(|c| c.transposed);
        super::axis_display::apply_display(
            &mut figure,
            self.binding.coordinates.as_ref(),
            &self.derived_axes,
            &self.axis_overrides,
        );
        self.viewport.auto_y2 = !self.has_manual_y2_range();
        prepare(self, &mut figure);
        if let Some(reversed) = self.stack_x_reversed {
            figure.x.reversed = reversed;
        }
        self.figure = figure;
        self.make_legend_room();
        self.figure_geometry_generation = next_figure_geometry_generation();
    }

    /// Adopt a rebuilt figure whose chart semantics may have changed.
    pub(crate) fn adopt_rebuilt_figure(&mut self, figure: Figure) {
        self.commit_rebuilt_figure(figure, |plot, figure| {
            plot.axis_overrides.apply_to(figure);
            plot.viewport = CanvasViewport::from_figure(figure);
            plot.viewport.auto_y2 = !plot.has_manual_y2_range();
            if plot.has_manual_y_range(figure) {
                plot.viewport.auto_y = false;
            }
            plot.viewport.apply_to(figure);
        });
    }

    /// Rebuild → overrides → viewport sync/apply. Effective range overrides
    /// replace the full data bounds; zoom and pan remain constrained within them.
    pub(crate) fn preserve_viewport_on_rebuild(&mut self, figure: Figure) {
        let transposed = self
            .binding
            .coordinates
            .as_ref()
            .is_some_and(|c| c.transposed);
        if transposed != self.built_transposed {
            self.built_transposed = transposed;
            return self.reset_viewport_on_rebuild(figure);
        }
        self.commit_rebuilt_figure(figure, |plot, figure| {
            plot.axis_overrides.apply_to(figure);
            if plot.has_manual_y_range(figure) {
                plot.viewport.auto_y = false;
            }
            plot.viewport.sync_full_from(figure);
            plot.viewport.apply_to(figure);
        });
    }

    /// Rebuild a plot whose chart semantics changed, starting its viewport at
    /// the effective overridden ranges rather than retaining an incompatible view.
    pub(crate) fn reset_viewport_on_rebuild(&mut self, figure: Figure) {
        self.adopt_rebuilt_figure(figure);
    }

    pub(crate) fn rebuild_for_axis_overrides(
        &mut self,
        figure: Figure,
        x_range_changed: bool,
        y_range_changed: bool,
        y2_range_changed: bool,
    ) {
        self.commit_rebuilt_figure(figure, |plot, figure| {
            plot.axis_overrides.apply_to(figure);
            let effective_y_range = plot.has_manual_y_range(figure);
            if y_range_changed {
                plot.viewport.auto_y = !effective_y_range;
            } else if effective_y_range {
                plot.viewport.auto_y = false;
            }
            plot.viewport.sync_full_from(figure);
            if x_range_changed {
                plot.viewport.reset_x(figure);
            }
            if y_range_changed {
                if effective_y_range {
                    plot.viewport.view_y = plot.viewport.full_y;
                    plot.viewport.auto_y = false;
                } else {
                    plot.viewport.reset_y(figure);
                }
            }
            if y2_range_changed {
                plot.viewport.view_y2 = plot.viewport.full_y2;
            }
            plot.viewport.apply_to(figure);
        });
    }

    pub fn apply_viewport(&mut self) {
        self.viewport.apply_to(&mut self.figure);
        self.make_legend_room();
    }

    /// Widen automatic axes when the automatic legend has no free slot. An
    /// axis qualifies only without a manual range and while it shows its full
    /// extent, so zoomed and author-set windows are never changed. The growth
    /// lives only in the figure and is recomputed from the viewport each time.
    fn make_legend_room(&mut self) {
        let viewport = &self.viewport;
        let full_x = viewport.view_x == viewport.full_x;
        let grow_x = self.axis_overrides.x_range.is_none() && full_x;
        let grow_y = self.axis_overrides.y_range.is_none()
            && full_x
            && (viewport.view_y == viewport.full_y || viewport.auto_y);
        let grow_y2 = self.axis_overrides.y2_range.is_none()
            && full_x
            && (viewport.view_y2 == viewport.full_y2 || viewport.auto_y);
        let size = [self.figure.width, self.figure.height];
        plotium_render::make_legend_room(&mut self.figure, size, grow_x, grow_y, grow_y2);
    }

    /// Zoom the requested axes in place without cloning the materialized figure.
    /// Anchors are data coordinates; `None` leaves that axis unchanged.
    pub fn zoom_viewport_around(
        &mut self,
        x_anchor: Option<f64>,
        y_anchor: Option<f64>,
        scale: f64,
    ) {
        if let Some(anchor) = x_anchor {
            self.viewport.zoom_x(&self.figure, anchor, scale);
        }
        if let Some(anchor) = y_anchor {
            self.viewport.zoom_y(&self.figure, anchor, scale);
        }
        self.apply_viewport();
    }

    /// Pan by fractions of the drawn window, which can exceed the viewport by
    /// legend room; measuring against it keeps the data under the pointer.
    pub fn pan_viewport(&mut self, x_fraction: f64, y_fraction: f64) {
        let drawn = |axis: &plotium_figure::Axis| match axis.effective_scale() {
            plotium_figure::AxisScale::Linear => axis.span(),
            scale => scale.forward(axis.max) - scale.forward(axis.min),
        };
        let x = x_fraction * drawn(&self.figure.x);
        let y = y_fraction * drawn(&self.figure.y);
        self.viewport.pan(&self.figure, x, y);
        self.apply_viewport();
    }

    pub(crate) fn apply_axis_overrides(&mut self) {
        super::axis_display::apply_display(
            &mut self.figure,
            self.binding.coordinates.as_ref(),
            &self.derived_axes,
            &self.axis_overrides,
        );
        self.axis_overrides.apply_to(&mut self.figure);
    }

    /// Replace authored annotations without a rebuild: anchors are data
    /// coordinates, so only the lowered labels and legend room change.
    pub(crate) fn set_annotations(&mut self, annotations: super::PlotAnnotations) {
        self.figure.author_annotations = annotations.lower();
        self.figure.reference_lines = annotations.lower_reference_lines();
        self.figure.author_arrows = annotations.lower_arrows();
        self.axis_overrides.annotations = annotations;
        self.apply_viewport();
    }

    pub(crate) fn set_figure_typography(&mut self, typography: FigureTypography) {
        self.figure.typography = typography;
        self.axis_overrides
            .typography
            .apply_to(&mut self.figure.typography);
        self.apply_viewport();
    }

    pub(crate) fn set_integral_curves(
        &mut self,
        curves: &[plotium_figure::IntegralCurve],
        visible: bool,
    ) {
        if visible {
            self.figure.integral_curves.clear();
            self.figure.integral_curves.extend_from_slice(curves);
        } else {
            self.figure.integral_curves.clear();
        }
    }

    #[cfg(test)]
    pub(crate) fn set_axis_frame(&mut self, axis_frame: plotium_figure::AxisFrame) {
        self.figure.axis_frame = axis_frame;
    }

    pub(crate) fn normalize_viewport(&self, viewport: &mut CanvasViewport) {
        if self.has_manual_y_range(&self.figure) && viewport.auto_y {
            viewport.view_y = viewport.full_y;
            viewport.auto_y = false;
        }
        viewport.auto_y2 = !self.has_manual_y2_range();
    }

    fn has_manual_y2_range(&self) -> bool {
        self.axis_overrides.y2_range.is_some()
    }

    fn has_manual_y_range(&self, figure: &Figure) -> bool {
        self.axis_overrides.y_range.is_some() && figure.y.categories.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AxisRange;
    use plotium_figure::{Axis, Color, Contour, Series};

    #[test]
    fn in_place_zoom_preserves_figure_buffers() {
        let mut figure = Figure::new("", Axis::new("x", 0.0, 10.0), Axis::new("y", 0.0, 10.0));
        figure.series.push(Series::line(
            "trace",
            (0..32)
                .map(|index| [index as f64 / 3.1, index as f64])
                .collect(),
        ));
        figure.contours.push(Contour {
            segments: vec![[[0.0, 0.0], [1.0, 1.0]], [[9.0, 1.0], [10.0, 2.0]]],
            color: Color::BLACK,
            width: 1.0,
        });
        let viewport = CanvasViewport::from_figure(&figure);
        let mut plot = PlotObject::new(
            None,
            SeriesId::new(1),
            DataBinding {
                coordinates: None,
                series: Vec::new(),
            },
            ChartSpec::default(),
            StackSpec::default(),
            AxisProjections::default(),
            AxisOverrides::default(),
            figure,
            viewport,
        );
        let line_buffer = plot.figure().series[0].points.as_ptr();
        let contour_buffer = plot.figure().contours[0].segments.as_ptr();

        plot.zoom_viewport_around(Some(5.0), Some(5.0), 0.8);
        plot.zoom_viewport_around(Some(4.0), None, 0.8);

        assert_eq!(plot.figure().series[0].points.as_ptr(), line_buffer);
        assert_eq!(plot.figure().contours[0].segments.as_ptr(), contour_buffer);
    }

    fn crowded_plot(overrides: AxisOverrides) -> PlotObject {
        let mut figure = Figure::new("", Axis::new("x", 0.0, 10.0), Axis::new("y", 0.0, 10.0));
        figure.width = 320.0;
        figure.height = 220.0;
        figure.guide_visibility = plotium_figure::GuideVisibility::Show;
        // Horizontal lines fill every inside legend anchor.
        figure.series = (0..=20)
            .map(|i| {
                let y = f64::from(i) * 0.5;
                let name = if i < 2 {
                    format!("S{i}")
                } else {
                    String::new()
                };
                Series::line(name, vec![[0.0, y], [10.0, y]])
            })
            .collect();
        overrides.apply_to(&mut figure);
        let mut viewport = CanvasViewport::from_figure(&figure);
        viewport.auto_y = overrides.y_range.is_none();
        PlotObject::new(
            None,
            SeriesId::new(1),
            DataBinding {
                coordinates: None,
                series: Vec::new(),
            },
            ChartSpec::default(),
            StackSpec::default(),
            AxisProjections::default(),
            overrides,
            figure,
            viewport,
        )
    }

    fn windows(plot: &PlotObject) -> [AxisRange; 2] {
        [
            AxisRange::from_axis(&plot.figure().x),
            AxisRange::from_axis(&plot.figure().y),
        ]
    }

    #[test]
    fn automatic_axes_grow_for_a_crowded_legend_and_recover_without_one() {
        let mut plot = crowded_plot(AxisOverrides::default());
        let [x, y] = windows(&plot);
        assert_ne!(
            [x, y],
            [AxisRange::new(0.0, 10.0), AxisRange::new(0.0, 10.0)]
        );
        assert!(x.min <= 0.0 && x.max >= 10.0 && y.min <= 0.0 && y.max >= 10.0);
        // The viewport keeps the data-derived window; only the figure grows.
        assert_eq!(plot.viewport.full_x, AxisRange::new(0.0, 10.0));

        plot.figure.guide_placement = plotium_figure::GuidePlacement::Inside;
        plot.apply_viewport();
        assert_eq!(
            windows(&plot),
            [AxisRange::new(0.0, 10.0), AxisRange::new(0.0, 10.0)]
        );
    }

    #[test]
    fn manual_ranges_and_zoomed_views_never_grow() {
        let manual = AxisRange::new(0.0, 10.0);
        let plot = crowded_plot(AxisOverrides {
            x_range: Some(manual),
            y_range: Some(manual),
            ..AxisOverrides::default()
        });
        assert_eq!(windows(&plot), [manual, manual]);

        let mut plot = crowded_plot(AxisOverrides {
            y_range: Some(manual),
            ..AxisOverrides::default()
        });
        assert_eq!(windows(&plot)[1], manual);
        assert_ne!(windows(&plot)[0], manual);

        let mut zoomed = crowded_plot(AxisOverrides::default());
        zoomed.zoom_viewport_around(Some(5.0), Some(5.0), 0.8);
        assert_eq!(
            windows(&zoomed),
            [zoomed.viewport.view_x, zoomed.viewport.view_y]
        );
        plot.zoom_viewport_around(Some(5.0), None, 0.8);
        assert_eq!(windows(&plot), [plot.viewport.view_x, manual]);

        // A pan leaves y at its full extent; a zoomed x still keeps y fixed.
        let mut panned = crowded_plot(AxisOverrides::default());
        panned.viewport.auto_y = false;
        panned.zoom_viewport_around(Some(5.0), None, 0.8);
        assert_eq!(
            windows(&panned),
            [panned.viewport.view_x, panned.viewport.full_y]
        );
    }
}
