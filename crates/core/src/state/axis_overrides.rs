use plotium_figure::{
    Axis, AxisBreak, AxisFrame, AxisScale, Figure, GuideLayout, GuidePlacement, GuideVisibility,
    YAxisSide,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AxisRange {
    pub min: f64,
    pub max: f64,
}

impl AxisRange {
    pub fn new(min: f64, max: f64) -> Self {
        if min <= max {
            Self { min, max }
        } else {
            Self { min: max, max: min }
        }
    }

    pub fn from_axis(axis: &Axis) -> Self {
        Self::new(axis.min, axis.max)
    }

    pub fn span(self) -> f64 {
        (self.max - self.min).max(f64::MIN_POSITIVE)
    }

    pub fn contains(self, value: f64) -> bool {
        self.min <= value && value <= self.max
    }

    pub fn is_valid(self) -> bool {
        self.min.is_finite() && self.max.is_finite() && self.min < self.max
    }

    pub fn clamp_to(self, full: Self) -> Self {
        let full_span = full.span();
        let span = self.span().min(full_span);
        if span >= full_span {
            return full;
        }

        let mut min = self.min.max(full.min).min(full.max - span);
        let mut max = min + span;
        if max > full.max {
            max = full.max;
            min = max - span;
        }
        Self { min, max }
    }

    pub fn zoom_around(self, full: Self, anchor: f64, scale: f64) -> Self {
        let span = self.span();
        let full_span = full.span();
        let new_span = (span * scale).clamp(full_span * 1e-6, full_span);
        if new_span >= full_span {
            return full;
        }

        let anchor = anchor.clamp(self.min, self.max);
        let frac = ((anchor - self.min) / span).clamp(0.0, 1.0);
        let min = anchor - frac * new_span;
        let max = min + new_span;
        Self { min, max }.clamp_to(full)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasViewport {
    /// Display-only trace amplification; scientific observations stay unchanged.
    pub display_gain: f64,
    pub full_x: AxisRange,
    pub full_y: AxisRange,
    pub view_x: AxisRange,
    pub view_y: AxisRange,
    pub auto_y: bool,
    /// Right-axis windows, present while the figure has a right axis. Y
    /// navigation moves them by the same fractions as the left windows.
    pub full_y2: Option<AxisRange>,
    pub view_y2: Option<AxisRange>,
    /// Whether automatic y refits the right axis too. The owning plot clears
    /// it while the right axis has a manual range.
    pub auto_y2: bool,
}

impl CanvasViewport {
    pub fn from_figure(fig: &Figure) -> Self {
        let full_x = AxisRange::from_axis(&fig.x);
        let full_y = AxisRange::from_axis(&fig.y);
        let full_y2 = fig.y2.as_ref().map(AxisRange::from_axis);
        Self {
            display_gain: fig.display_gain,
            full_x,
            full_y,
            view_x: full_x,
            view_y: full_y,
            auto_y: true,
            full_y2,
            view_y2: full_y2,
            auto_y2: true,
        }
    }

    pub fn sync_full_from(&mut self, fig: &Figure) {
        self.reset_unsupported_gain(fig);
        let keep_x = self.display_gain != 1.0 && self.full_x == AxisRange::from_axis(&fig.x);
        let keep_y = self.display_gain != 1.0 && self.full_y == AxisRange::from_axis(&fig.y);
        self.full_x = AxisRange::from_axis(&fig.x);
        self.full_y = AxisRange::from_axis(&fig.y);
        // A frozen gain window may include automatic padding or legend room.
        // Rebuilding unchanged data must not trim that room and move baselines.
        if !keep_x {
            self.view_x = self.view_x.clamp_in(self.full_x, fig.x.scale);
        }
        self.sync_full_y2(fig);
        if self.auto_y {
            self.refresh_auto_y(fig);
        } else if !keep_y {
            self.view_y = self.view_y.clamp_in(self.full_y, fig.y.scale);
        }
    }

    pub fn apply_to(&self, fig: &mut Figure) {
        let gain = if fig.supports_display_gain() {
            self.display_gain
        } else {
            1.0
        };
        if let Err(error) = fig.set_display_gain(gain) {
            eprintln!("Could not apply plot display gain: {error}");
        }
        fig.x.min = self.view_x.min;
        fig.x.max = self.view_x.max;
        fig.y.min = self.view_y.min;
        fig.y.max = self.view_y.max;
        if let (Some(axis), Some(view)) = (fig.y2.as_mut(), self.view_y2) {
            axis.min = view.min;
            axis.max = view.max;
        }
    }

    pub(crate) fn reset_unsupported_gain(&mut self, fig: &Figure) {
        if !fig.supports_display_gain() {
            self.display_gain = 1.0;
        }
    }

    pub fn reset_all(&mut self) {
        self.display_gain = 1.0;
        self.view_x = self.full_x;
        self.view_y = self.full_y;
        self.view_y2 = self.full_y2;
        self.auto_y = true;
    }

    pub fn reset_x(&mut self, fig: &Figure) {
        self.view_x = self.full_x;
        if self.auto_y {
            self.refresh_auto_y(fig);
        }
    }

    pub fn reset_y(&mut self, fig: &Figure) {
        self.display_gain = 1.0;
        self.auto_y = true;
        if fig.display_gain != 1.0 {
            let mut original = fig.clone();
            match original.set_display_gain(1.0) {
                Ok(()) => self.refresh_auto_y(&original),
                Err(error) => eprintln!("Could not reset plot display gain: {error}"),
            }
        } else {
            self.refresh_auto_y(fig);
        }
    }

    pub fn zoom_x(&mut self, fig: &Figure, anchor: f64, scale: f64) {
        self.view_x = self
            .view_x
            .zoom_around_in(self.full_x, anchor, scale, fig.x.scale);
        if self.auto_y {
            self.refresh_auto_y(fig);
        }
    }

    pub fn set_linked_x(&mut self, fig: &Figure, range: AxisRange) {
        self.view_x = range.clamp_in(self.full_x, fig.x.scale);
        if self.auto_y {
            self.refresh_auto_y(fig);
        }
    }

    pub fn zoom_y(&mut self, fig: &Figure, anchor: f64, scale: f64) {
        let before = self.view_y;
        self.view_y = self
            .view_y
            .zoom_around_in(self.full_y, anchor, scale, fig.y.scale);
        self.follow_y2(fig, before);
        self.auto_y = false;
    }

    /// Pan by fractions of the visible window, measured in each axis scale.
    pub fn pan(&mut self, fig: &Figure, x_fraction: f64, y_fraction: f64) {
        let before = self.view_y;
        self.view_x = self.view_x.shifted_in(self.full_x, x_fraction, fig.x.scale);
        self.view_y = self.view_y.shifted_in(self.full_y, y_fraction, fig.y.scale);
        self.follow_y2(fig, before);
        self.auto_y = false;
    }

    pub fn select(&mut self, fig: &Figure, x: Option<AxisRange>, y: Option<AxisRange>) {
        if let Some(x) = x {
            self.view_x = x.clamp_in(self.full_x, fig.x.scale);
        }
        if let Some(y) = y {
            let before = self.view_y;
            self.view_y = y.clamp_in(self.full_y, fig.y.scale);
            self.follow_y2(fig, before);
            self.auto_y = false;
        } else if self.auto_y {
            self.refresh_auto_y(fig);
        }
    }

    fn refresh_auto_y(&mut self, fig: &Figure) {
        self.view_y = visible_y_range_on(fig, self.view_x, YAxisSide::Left).unwrap_or(self.full_y);
        self.view_y2 = if self.auto_y2 {
            self.full_y2
                .map(|full| visible_y_range_on(fig, self.view_x, YAxisSide::Right).unwrap_or(full))
        } else {
            self.full_y2
        };
    }
}

/// Why `figure` cannot draw equal data units per length on x and y, or `None`
/// when it can: both axes must be visible, continuous and linear, and a right
/// axis would leave the y scale ambiguous.
pub fn equal_scale_unavailable(figure: &Figure) -> Option<&'static str> {
    if figure.axis_frame == AxisFrame::Hidden {
        Some("Choose a chart with visible axes to edit axis settings.")
    } else if figure.x.categories.is_some() || figure.y.categories.is_some() {
        Some("A categorical axis has no equal x/y scale.")
    } else if figure.x.scale != AxisScale::Linear || figure.y.scale != AxisScale::Linear {
        Some("Equal x/y scale needs linear axes; switch logarithmic axes to linear first.")
    } else if figure.y2.is_some() {
        Some("A plot with a right y axis cannot use equal x/y scale.")
    } else {
        None
    }
}

/// Set an axis window. A table heatmap's window stays on its cells: the range
/// may come from another chart type of the same plot, in another quantity.
fn apply_range(axis: &mut Axis, range: AxisRange, on_cells: bool) {
    if !on_cells {
        axis.min = range.min;
        axis.max = range.max;
        return;
    }
    let (min, max) = (range.min.max(axis.min), range.max.min(axis.max));
    if min < max {
        axis.min = min;
        axis.max = max;
    }
}

/// Per-plot author overrides applied after rebuilding a figure from its data.
/// `None` keeps the corresponding value derived by the chart builder.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AxisOverrides {
    pub x_display_unit: Option<Box<plotium_data::UnitSpec>>,
    pub y_display_unit: Option<Box<plotium_data::UnitSpec>>,
    pub typography: plotium_figure::FigureTypographyOverrides,
    pub x_label: Option<String>,
    pub y_label: Option<String>,
    pub x_range: Option<AxisRange>,
    pub y_range: Option<AxisRange>,
    pub lock_aspect: Option<bool>,
    pub axis_frame: Option<AxisFrame>,
    pub x_scale: Option<AxisScale>,
    pub y_scale: Option<AxisScale>,
    pub x_break: Option<AxisBreak>,
    pub y_break: Option<AxisBreak>,
    pub x_show_tick_labels: Option<bool>,
    pub x_show_label: Option<bool>,
    pub y_show_tick_labels: Option<bool>,
    pub y_show_label: Option<bool>,
    pub x_thousands_separator: Option<bool>,
    pub y_thousands_separator: Option<bool>,
    /// Major tick steps in each axis's display unit.
    pub x_major_step: Option<f64>,
    pub y_major_step: Option<f64>,
    pub guide_visibility: Option<GuideVisibility>,
    pub guide_placement: Option<GuidePlacement>,
    pub guide_layout: Option<GuideLayout>,
    pub guide_title: Option<String>,
    pub legend_position: Option<[f32; 2]>,
    /// Author text anchored in data coordinates, lowered on every apply.
    pub annotations: super::PlotAnnotations,
    /// Right-axis counterparts of the `y_*` overrides. They apply only while
    /// some series is measured on the right axis.
    pub y2_display_unit: Option<Box<plotium_data::UnitSpec>>,
    pub y2_label: Option<String>,
    pub y2_range: Option<AxisRange>,
    pub y2_scale: Option<AxisScale>,
    pub y2_show_tick_labels: Option<bool>,
    pub y2_show_label: Option<bool>,
    pub y2_major_step: Option<f64>,
}

impl AxisOverrides {
    /// Rebind every per-axis override to the other axis, for a plot turned on
    /// its side: titles, ranges, units and scales follow the data they
    /// describe, and annotations keep pointing at the same data.
    pub fn swap_axes(&mut self) {
        std::mem::swap(&mut self.x_display_unit, &mut self.y_display_unit);
        std::mem::swap(&mut self.x_label, &mut self.y_label);
        std::mem::swap(&mut self.x_range, &mut self.y_range);
        std::mem::swap(&mut self.x_scale, &mut self.y_scale);
        std::mem::swap(&mut self.x_show_tick_labels, &mut self.y_show_tick_labels);
        std::mem::swap(&mut self.x_show_label, &mut self.y_show_label);
        std::mem::swap(
            &mut self.x_thousands_separator,
            &mut self.y_thousands_separator,
        );
        std::mem::swap(&mut self.x_major_step, &mut self.y_major_step);
        let annotations = &mut self.annotations;
        for item in &mut annotations.items {
            item.at.swap(0, 1);
        }
        for arrow in &mut annotations.arrows {
            arrow.at.swap(0, 1);
            arrow.to.swap(0, 1);
        }
        for line in &mut annotations.reference_lines {
            line.orientation = match line.orientation {
                plotium_figure::ReferenceOrientation::Vertical => {
                    plotium_figure::ReferenceOrientation::Horizontal
                }
                plotium_figure::ReferenceOrientation::Horizontal => {
                    plotium_figure::ReferenceOrientation::Vertical
                }
            };
        }
    }

    pub fn apply_to(&self, figure: &mut Figure) {
        self.typography.apply_to(&mut figure.typography);
        if let Some(label) = &self.x_label {
            figure.x.label.clone_from(label);
        }
        if let Some(label) = &self.y_label {
            figure.y.label.clone_from(label);
        }
        let heatmap = figure.heatmap.as_ref();
        let x_cells = heatmap.is_some_and(|grid| grid.x_edges.is_some());
        let y_cells = heatmap.is_some_and(|grid| grid.y_edges.is_some());
        if let Some(range) = self.x_range
            && figure.x.categories.is_none()
        {
            apply_range(&mut figure.x, range, x_cells);
        }
        if let Some(range) = self.y_range
            && figure.y.categories.is_none()
        {
            apply_range(&mut figure.y, range, y_cells);
        }
        // A stored lock outlives a rebinding that leaves no equal-scale axes.
        if let Some(lock) = self.lock_aspect
            && (!lock || equal_scale_unavailable(figure).is_none())
        {
            figure.lock_aspect = lock;
        }
        self.apply_right_axis(figure);
        self.apply_scales(figure);
        // A break would distort the equal data scale of a locked aspect.
        let breakable = |axis: &Axis| {
            !figure.lock_aspect && axis.categories.is_none() && figure.heatmap.is_none()
        };
        figure.x.axis_break = self.x_break.filter(|_| breakable(&figure.x));
        // The right axis would keep running straight past a left y break.
        figure.y.axis_break = self
            .y_break
            .filter(|_| breakable(&figure.y) && figure.y2.is_none());
        // Charts without axes (pie, 3D) keep their hidden frame.
        if let Some(frame) = self.axis_frame
            && figure.axis_frame != AxisFrame::Hidden
        {
            figure.axis_frame = frame;
        }
        if let Some(show) = self.x_show_tick_labels {
            figure.x.show_tick_labels = show;
        }
        if let Some(show) = self.x_show_label {
            figure.x.show_label = show;
        }
        if let Some(show) = self.y_show_tick_labels {
            figure.y.show_tick_labels = show;
        }
        if let Some(show) = self.y_show_label {
            figure.y.show_label = show;
        }
        if let Some(group) = self.x_thousands_separator {
            figure.x.thousands_separator = group;
        }
        if let Some(group) = self.y_thousands_separator {
            figure.y.thousands_separator = group;
        }
        figure.x.major_step = self.x_major_step;
        figure.y.major_step = self.y_major_step;
        if let Some(visibility) = self.guide_visibility {
            figure.guide_visibility = visibility;
        }
        if let Some(placement) = self.guide_placement {
            figure.guide_placement = placement;
        }
        if let Some(layout) = self.guide_layout {
            figure.guide_layout = layout;
        }
        if let Some(title) = &self.guide_title {
            figure.guide_title.clone_from(title);
        }
        figure.legend_position = self.legend_position;
        figure.author_annotations = self.annotations.lower();
        figure.reference_lines = self.annotations.lower_reference_lines();
        figure.author_arrows = self.annotations.lower_arrows();
    }

    /// A dragged or explicitly positioned legend is always an inside guide.
    pub fn set_legend_position(&mut self, position: [f32; 2]) {
        self.guide_placement = Some(plotium_figure::GuidePlacement::Inside);
        self.legend_position = Some(position);
    }

    pub fn reset_guide_placement(&mut self) {
        self.guide_placement = None;
        self.legend_position = None;
    }

    pub fn normalized(mut self) -> Self {
        self.x_label = normalize_label(self.x_label);
        self.y_label = normalize_label(self.y_label);
        self.x_range = self.x_range.filter(|range| range.is_valid());
        self.y_range = self.y_range.filter(|range| range.is_valid());
        self.y2_label = normalize_label(self.y2_label);
        self.y2_range = self.y2_range.filter(|range| range.is_valid());
        self.axis_frame = self.axis_frame.filter(|frame| *frame != AxisFrame::Hidden);
        self.x_break = self.x_break.filter(axis_break_is_valid);
        self.y_break = self.y_break.filter(axis_break_is_valid);
        self.guide_title = normalize_label(self.guide_title);
        self.legend_position = self.legend_position.and_then(|[x, y]| {
            (x.is_finite() && y.is_finite()).then(|| [x.clamp(0.0, 1.0), y.clamp(0.0, 1.0)])
        });
        self
    }
}

pub fn axis_break_is_valid(brk: &AxisBreak) -> bool {
    brk.from.is_finite()
        && brk.to.is_finite()
        && brk.from != brk.to
        && brk
            .lower_fraction
            .is_none_or(|f| f.is_finite() && f > 0.0 && f < 1.0)
}

fn normalize_label(label: Option<String>) -> Option<String> {
    label.filter(|text| !text.trim().is_empty())
}

pub fn visible_y_range(fig: &Figure, x: AxisRange) -> Option<AxisRange> {
    visible_y_range_on(fig, x, YAxisSide::Left)
}

/// Automatic window of the y axis on `side` over the data inside the `x`
/// window. Filled shapes (bars, boxes) are always measured on the left axis.
pub fn visible_y_range_on(fig: &Figure, x: AxisRange, side: YAxisSide) -> Option<AxisRange> {
    if fig.y_axis(side).scale == plotium_figure::AxisScale::Log10 {
        return Some(super::log_auto_range(super::log_axes::axis_values(
            fig,
            false,
            Some(x),
            side,
        )));
    }
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;

    for series in fig.series.iter().filter(|s| s.appearance.y_axis == side) {
        for &[px, py] in &series.points {
            if x.contains(px) && py.is_finite() {
                lo = lo.min(py);
                hi = hi.max(py);
            }
        }
    }

    // Presentation edits and X navigation recompute automatic Y limits. Include
    // visible uncertainty endpoints, not just centers, or rebuilt error bars clip.
    for error in fig.error_bars.iter().filter(|e| e.y_axis == side) {
        // A horizontal whisker spans x; only its centre has a y extent.
        let (left, right) = if error.horizontal {
            (
                error.center[0] - error.negative,
                error.center[0] + error.positive,
            )
        } else {
            (error.center[0], error.center[0])
        };
        if right >= x.min && left <= x.max {
            let (lower, upper) = if error.horizontal {
                (error.center[1], error.center[1])
            } else {
                (
                    error.center[1] - error.negative,
                    error.center[1] + error.positive,
                )
            };
            if lower.is_finite() && upper.is_finite() {
                lo = lo.min(lower);
                hi = hi.max(upper);
            }
        }
    }

    // Filled shapes (bars, boxes) count wholly once they reach into the window,
    // and one resting on y = 0 keeps that baseline as the unpadded floor.
    // Aspect-locked drawings (pies, ternary maps) keep their builder's window.
    let mut zero_floor = false;
    let left = side == YAxisSide::Left;
    for polygon in fig.polygons.iter().filter(|_| left && !fig.lock_aspect) {
        let (x0, x1) = polygon
            .points
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), p| {
                (a.min(p[0]), b.max(p[0]))
            });
        if x1 < x.min || x0 > x.max {
            continue;
        }
        for &[_, py] in &polygon.points {
            if py.is_finite() {
                lo = lo.min(py);
                hi = hi.max(py);
                zero_floor |= py == 0.0;
            }
        }
    }

    if !lo.is_finite() || !hi.is_finite() {
        return None;
    }

    // A flat curve needs visible padding at its coordinate scale. MIN_POSITIVE
    // rounds away at nonzero Y and leaves a practically zero range at Y = 0.
    let span = if hi > lo { hi - lo } else { lo.abs().max(1.0) };
    let floor = if zero_floor && lo == 0.0 {
        0.0
    } else {
        lo - 0.05 * span
    };
    let range = AxisRange::new(floor, hi + 0.08 * span);
    range.is_valid().then_some(range)
}

#[cfg(test)]
mod tests {
    use super::*;
    use plotium_figure::{Axis, Figure};

    #[test]
    fn empty_labels_and_invalid_ranges_normalize_to_auto() {
        let overrides = AxisOverrides {
            x_label: Some("  ".to_owned()),
            y_label: Some("Signal".to_owned()),
            x_range: Some(AxisRange { min: 1.0, max: 1.0 }),
            y_range: Some(AxisRange {
                min: 5.0,
                max: -5.0,
            }),
            ..AxisOverrides::default()
        }
        .normalized();
        assert_eq!(overrides.x_label, None);
        assert_eq!(overrides.y_label.as_deref(), Some("Signal"));
        assert_eq!(overrides.x_range, None);
        assert_eq!(overrides.y_range, None);
    }

    #[test]
    fn a_stored_equal_scale_lock_skips_a_categorical_rebuild() {
        let mut x = Axis::new("group", 0.0, 2.0);
        x.categories = Some(vec!["a".to_owned(), "b".to_owned()]);
        let mut figure = Figure::new("", x, Axis::new("y", 0.0, 1.0));
        let lock = AxisOverrides {
            lock_aspect: Some(true),
            ..AxisOverrides::default()
        };
        lock.apply_to(&mut figure);
        assert!(!figure.lock_aspect);

        figure.x.categories = None;
        lock.apply_to(&mut figure);
        assert!(figure.lock_aspect);
    }

    #[test]
    fn overrides_apply_without_changing_unspecified_axes() {
        let mut figure = Figure::new(
            "",
            Axis::new("automatic x", 0.0, 10.0),
            Axis::new("automatic y", -1.0, 1.0),
        );
        AxisOverrides {
            x_label: Some("Time".to_owned()),
            y_range: Some(AxisRange::new(-2.0, 2.0)),
            lock_aspect: Some(true),
            ..AxisOverrides::default()
        }
        .apply_to(&mut figure);
        assert_eq!(figure.x.label, "Time");
        assert_eq!(figure.y.label, "automatic y");
        assert_eq!(AxisRange::from_axis(&figure.x), AxisRange::new(0.0, 10.0));
        assert_eq!(AxisRange::from_axis(&figure.y), AxisRange::new(-2.0, 2.0));
        assert!(figure.lock_aspect);
    }

    #[test]
    fn numeric_ranges_do_not_replace_categorical_axis_windows() {
        let mut figure = Figure::new("", Axis::new("x", -0.5, 2.5), Axis::new("y", 0.0, 1.0));
        figure.x.categories = Some(vec!["A".to_owned(), "B".to_owned(), "C".to_owned()]);
        AxisOverrides {
            x_range: Some(AxisRange::new(1.0, 8.0)),
            ..AxisOverrides::default()
        }
        .apply_to(&mut figure);

        assert_eq!(AxisRange::from_axis(&figure.x), AxisRange::new(-0.5, 2.5));
    }

    #[test]
    fn frame_override_boxes_open_axes_but_never_reveals_hidden_ones() {
        let boxed = AxisOverrides {
            axis_frame: Some(AxisFrame::Box),
            ..AxisOverrides::default()
        };
        let mut figure = Figure::new("", Axis::new("x", 0.0, 1.0), Axis::new("y", 0.0, 1.0));
        boxed.apply_to(&mut figure);
        assert_eq!(figure.axis_frame, AxisFrame::Box);

        figure.axis_frame = AxisFrame::Hidden;
        boxed.apply_to(&mut figure);
        assert_eq!(figure.axis_frame, AxisFrame::Hidden);

        let hidden = AxisOverrides {
            axis_frame: Some(AxisFrame::Hidden),
            ..AxisOverrides::default()
        }
        .normalized();
        assert_eq!(hidden.axis_frame, None);
    }

    #[test]
    fn legend_position_normalizes_to_the_plot_area() {
        let clamped = AxisOverrides {
            legend_position: Some([-0.5, 1.5]),
            ..AxisOverrides::default()
        }
        .normalized();
        assert_eq!(clamped.legend_position, Some([0.0, 1.0]));

        let invalid = AxisOverrides {
            legend_position: Some([f32::NAN, 0.5]),
            ..AxisOverrides::default()
        }
        .normalized();
        assert_eq!(invalid.legend_position, None);
    }
}
