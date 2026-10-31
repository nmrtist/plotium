//! Figure model: a pure-data description of what to draw, in data-space
//! coordinates. Both renderers (egui screen, SVG export) consume this model.

mod display_gain;
pub use display_gain::{MAX_DISPLAY_GAIN, MIN_DISPLAY_GAIN};

mod axis_break;
mod axis_scale;
pub use axis_break::{AXIS_BREAK_GAP, AxisBreak};
mod color_shade;
mod colormap;
mod display;
pub use axis_scale::AxisScale;
mod encoding;
mod error_bar;
mod text_anchor;
pub use error_bar::ErrorBar;
pub use text_anchor::TextAnchor;
mod polygon;
pub use polygon::Polygon;
mod reference_line;
mod typeface;
pub use display::*;
pub use reference_line::{
    ReferenceLabelPosition, ReferenceLabelSide, ReferenceLine, ReferenceOrientation,
};
pub use typeface::{FontWeight, Typeface};

pub use colormap::ColormapId;
pub use encoding::{
    ColorSource, ContourBasePolicy, ContourLevelSpec, ContourSpec, ContourStyle,
    DEFAULT_BAR_EDGE_WIDTH_PT, DEFAULT_BAR_WIDTH, DEFAULT_DATA_LINE_WIDTH_PT, EstimatorSelection,
    FiniteF64, HeatmapSpec, ImageInterpolation, ImageSpec, LineEncoding, PositiveFiniteF32,
    PositiveFiniteF64, SeriesEncoding, UnitInterval,
};

/// An RGB color, 0–255 per channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub const BLACK: Color = Color::rgb(0, 0, 0);
    pub const AXIS: Color = Color::BLACK;
    pub const TRACE: Color = Color::rgb(0x0f, 0x4d, 0x92);
    pub const GRID: Color = Color::rgb(220, 222, 228);

    pub fn to_hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

/// Point sizes of a figure's axis furniture text and strokes. Absolute typographic points
/// (1/72"), independent of the plot's frame size — the journal convention — so
/// resizing a panel never changes its type size. Defaults sit mid-band for
/// journal figures (most ask for 5–7 pt minimum at final size) and pass the
/// export precheck's 7 pt floor.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FigureTypography {
    /// Tick value labels.
    pub tick_pt: f32,
    /// Axis titles (the x/y labels).
    pub label_pt: f32,
    /// Figure title above the plot.
    pub title_pt: f32,
    /// Legend labels.
    pub legend_pt: f32,
    /// Legend-label text color.
    pub legend_color: Color,
    /// Stroke width of the axis frame and tick marks.
    #[serde(default = "default_axis_line_pt")]
    pub axis_line_pt: f32,
    /// Length of the tick marks.
    #[serde(default = "default_tick_length_pt")]
    pub tick_length_pt: f32,
    /// Whether tick marks point away from or into the plot area.
    #[serde(default)]
    pub tick_direction: TickDirection,
    /// Unlabelled minor ticks between neighbouring major ticks; zero hides them.
    #[serde(default)]
    pub minor_ticks: u8,
    /// Typeface of all text drawn inside a plot.
    #[serde(default)]
    pub typeface: Typeface,
    /// Weight of all text drawn inside a plot.
    #[serde(default)]
    pub font_weight: FontWeight,
    /// Default stroke of supporting lines that are not data: bar outlines and
    /// new reference lines and arrows. `None` keeps each one's own default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aux_line_pt: Option<f32>,
}

/// Which side of the axis line the tick marks point to.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TickDirection {
    #[default]
    Out,
    In,
}

impl FigureTypography {
    /// Room the ticks take outside the plot area, before tick labels start.
    pub fn tick_outset(&self) -> f32 {
        match self.tick_direction {
            TickDirection::Out => self.tick_length_pt,
            TickDirection::In => 0.0,
        }
    }

    /// Signed tick extent measured away from the plot area: negative ticks
    /// point into the data.
    pub fn tick_reach(&self) -> f32 {
        match self.tick_direction {
            TickDirection::Out => self.tick_length_pt,
            TickDirection::In => -self.tick_length_pt,
        }
    }

    /// Minor ticks are half as long as major ticks and point the same way.
    pub fn minor_tick_reach(&self) -> f32 {
        self.tick_reach() * 0.5
    }
}

pub(crate) fn is_left(side: &YAxisSide) -> bool {
    *side == YAxisSide::Left
}

fn default_axis_line_pt() -> f32 {
    1.0
}

fn default_tick_length_pt() -> f32 {
    3.0
}

/// Whether explanatory guides follow the chart's semantic defaults or an
/// explicit author choice. Guides currently include categorical legends and
/// continuous colour scales.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuideVisibility {
    #[default]
    Auto,
    Show,
    Hide,
}

/// Placement shared by categorical legends and continuous colour scales.
/// A dragged categorical legend additionally records `legend_position` and
/// therefore becomes a custom inside placement until reset.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuidePlacement {
    #[default]
    Auto,
    Inside,
    OutsideRight,
    OutsideBottom,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuideLayout {
    #[default]
    Auto,
    Vertical,
    Horizontal,
}

impl Default for FigureTypography {
    fn default() -> Self {
        Self {
            tick_pt: 7.0,
            label_pt: 8.0,
            title_pt: 8.0,
            legend_pt: 7.0,
            legend_color: Color::AXIS,
            axis_line_pt: default_axis_line_pt(),
            tick_length_pt: default_tick_length_pt(),
            tick_direction: TickDirection::Out,
            minor_ticks: 0,
            typeface: Typeface::Arial,
            font_weight: FontWeight::Regular,
            aux_line_pt: None,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Axis {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display: Option<AxisDisplay>,
    pub label: String,
    #[serde(default = "default_true")]
    pub show_tick_labels: bool,
    #[serde(default = "default_true")]
    pub show_label: bool,
    /// Group integer digits of numeric tick labels with commas ("1,000"). Large
    /// values are then written in full instead of with a ×10ⁿ multiplier.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub thousands_separator: bool,
    /// Distance between major ticks in the display unit; `None` chooses one.
    /// Only linear numeric axes without a break honour it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub major_step: Option<f64>,
    pub min: f64,
    pub max: f64,
    /// If true, larger values draw toward the lower screen coordinate (left for
    /// x, the NMR ppm convention; top for y).
    pub reversed: bool,
    /// Ordinal mode: `Some(names)` places one category per integer position
    /// (0, 1, …) and renderers label those positions with the names instead of
    /// generating numeric ticks. `min`/`max` still define the visible window,
    /// conventionally `[-0.5, n - 0.5]` so slots get equal half-unit padding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub categories: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "AxisScale::is_linear")]
    pub scale: AxisScale,
    /// Values hidden by a broken axis; ignored unless the axis is linear,
    /// numeric and its window contains the whole break.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub axis_break: Option<AxisBreak>,
}

impl Axis {
    pub fn new(label: impl Into<String>, min: f64, max: f64) -> Self {
        Self {
            display: None,
            label: label.into(),
            show_tick_labels: true,
            show_label: true,
            thousands_separator: false,
            major_step: None,
            min,
            max,
            reversed: false,
            categories: None,
            scale: AxisScale::Linear,
            axis_break: None,
        }
    }

    /// An ordinal axis with one slot per category name, windowed to give every
    /// slot equal padding.
    pub fn categorical(label: impl Into<String>, names: Vec<String>) -> Self {
        let n = names.len().max(1) as f64;
        Self {
            display: None,
            label: label.into(),
            show_tick_labels: true,
            show_label: true,
            thousands_separator: false,
            major_step: None,
            min: -0.5,
            max: n - 0.5,
            reversed: false,
            categories: Some(names),
            scale: AxisScale::Linear,
            axis_break: None,
        }
    }

    pub fn reversed(mut self, yes: bool) -> Self {
        self.reversed = yes;
        self
    }

    /// Axis span, forced non-zero to avoid divide-by-zero in renderers.
    pub fn span(&self) -> f64 {
        let s = self.max - self.min;
        if s == 0.0 { 1.0 } else { s }
    }

    /// The scale positions are drawn with. A log axis whose window is not
    /// strictly positive has no finite log positions and draws linearly; range
    /// edits reject such windows before they reach a figure.
    pub fn effective_scale(&self) -> AxisScale {
        if self.scale.supports_range(self.min, self.max) {
            self.scale
        } else {
            AxisScale::Linear
        }
    }

    /// Map a value to a normalized `[0,1]` position, honoring `reversed`, the
    /// axis scale and an active break. Values without a position on a log axis
    /// come back non-finite.
    pub fn normalize(&self, v: f64) -> f64 {
        let t = match self.effective_scale() {
            AxisScale::Linear => self.broken((v - self.min) / self.span()),
            scale => {
                let (lo, span) = self.transformed_window(scale);
                (scale.forward(v) - lo) / span
            }
        };
        if self.reversed { 1.0 - t } else { t }
    }

    /// The value at a normalized position; the inverse of [`Axis::normalize`].
    pub fn value_at(&self, n: f64) -> f64 {
        let t = if self.reversed { 1.0 - n } else { n };
        match self.effective_scale() {
            AxisScale::Linear => self.min + self.unbroken(t) * self.span(),
            scale => {
                let (lo, span) = self.transformed_window(scale);
                scale.inverse(lo + t * span)
            }
        }
    }

    fn transformed_window(&self, scale: AxisScale) -> (f64, f64) {
        let lo = scale.forward(self.min);
        let span = scale.forward(self.max) - lo;
        (lo, if span == 0.0 { 1.0 } else { span })
    }
}

const fn default_true() -> bool {
    true
}

/// Runtime provenance copied from the owning native binding, never allocated here.
/// It is meaningful only together with the owning plot's stable target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SeriesOrigin(pub u64);

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Series {
    /// Original display coordinates while gain is active; shared by figure clones.
    #[serde(skip)]
    pub(crate) unamplified_y: Option<std::sync::Arc<Vec<f64>>>,
    /// Zero-signal position after stacking, independent of display gain.
    #[serde(default)]
    pub display_baseline: f64,
    #[serde(skip)]
    pub origin: Option<SeriesOrigin>,
    #[serde(default)]
    pub appearance: CurveAppearance,
    pub name: String,
    pub points: Vec<[f64; 2]>,
    pub color: Color,
    pub width: f32,
    #[serde(default)]
    pub kind: SeriesKind,
}

/// A stored 1D NMR integral description. Renderers derive the cumulative trace
/// from `Figure::series[source_series]`, keeping high-resolution spectrum points
/// in one place in project snapshots.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct IntegralCurve {
    pub start_ppm: f64,
    pub end_ppm: f64,
    pub normalized_area: f64,
    pub label: String,
    pub color: Color,
    pub width: f32,
    pub source_series: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SeriesKind {
    #[default]
    Line,
    Points,
    /// Independent vertical stems from the zero baseline to each point.
    Stick,
    /// Filled bars from the zero baseline. An encoding kind only: plot builders
    /// lower bars into [`Polygon`] rectangles and [`ErrorBar`] whiskers, so a
    /// rendered figure never carries a `Bar` series.
    Bar,
    /// Box-and-whisker summaries of raw samples per x slot; lowered like `Bar`.
    Box,
    /// Kernel-density violins of raw samples per x slot; lowered like `Bar`.
    Violin,
}

impl Series {
    pub fn line(name: impl Into<String>, points: Vec<[f64; 2]>) -> Self {
        Self {
            unamplified_y: None,
            display_baseline: 0.0,
            origin: None,
            appearance: CurveAppearance::default(),
            name: name.into(),
            points,
            color: Color::TRACE,
            width: DEFAULT_DATA_LINE_WIDTH_PT,
            kind: SeriesKind::Line,
        }
    }

    pub fn points(name: impl Into<String>, points: Vec<[f64; 2]>) -> Self {
        Self {
            unamplified_y: None,
            display_baseline: 0.0,
            origin: None,
            appearance: CurveAppearance::default(),
            name: name.into(),
            points,
            color: Color::TRACE,
            width: 3.0,
            kind: SeriesKind::Points,
        }
    }

    pub fn sticks(name: impl Into<String>, points: Vec<[f64; 2]>) -> Self {
        Self {
            unamplified_y: None,
            display_baseline: 0.0,
            origin: None,
            appearance: CurveAppearance::default(),
            name: name.into(),
            points,
            color: Color::TRACE,
            width: DEFAULT_DATA_LINE_WIDTH_PT,
            kind: SeriesKind::Stick,
        }
    }

    pub fn colored(mut self, color: Color) -> Self {
        self.color = color;
        self
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Annotation {
    pub text: String,
    pub at: [f64; 2],
    /// Zero-signal baseline of the trace that owns this label.
    #[serde(default)]
    pub display_baseline: f64,
    pub color: Color,
    pub size: f32,
    #[serde(default, skip_serializing_if = "TextAnchor::is_point")]
    pub anchor: TextAnchor,
    /// Pinned offset of an author label from its projected anchor, in pt with
    /// x right and y up; `None` lets the renderer place it clear of data.
    /// Builder labels always sit at their anchor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset_pt: Option<[f32; 2]>,
}

/// Arrowhead placement and fill for an [`ArrowAnnotation`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArrowHead {
    /// A plain line segment.
    None,
    /// An unfilled V at the head point.
    Open,
    /// A filled triangle at the head point.
    Filled,
    /// Filled triangles at both ends.
    Double,
}

/// A straight arrow between two data-coordinate points, drawn inside the plot
/// area like text annotations.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ArrowAnnotation {
    pub from: [f64; 2],
    pub to: [f64; 2],
    pub color: Color,
    /// Shaft stroke width in pt; heads widen for thick shafts.
    pub width: f32,
    pub head: ArrowHead,
}

/// A semantic x-range that spans the data area. Unlike editor selection chrome,
/// range annotations are figure content and are exported by every renderer.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RangeAnnotation {
    /// Stable source-region identity for direct manipulation.
    pub source_id: u64,
    pub x0: f64,
    pub x1: f64,
    pub label: String,
    /// Manually placed label center as fractions of the plot rectangle.
    pub label_position: Option<[f32; 2]>,
    pub color: Color,
    pub fill_opacity: f32,
    pub width: f32,
}

/// A 2D contour overlay as pre-computed data-space line segments (each
/// `[[x0,y0],[x1,y1]]`). The heavy marching-squares pass runs once when the
/// figure is built, so both renderers only project and stroke.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Contour {
    pub segments: Vec<[[f64; 2]; 2]>,
    pub color: Color,
    pub width: f32,
}

/// A 1D trace pinned alongside a 2D contour's axis (the standard NMR marginal
/// projection). `points` are `[ppm, intensity]` on the contour's corresponding
/// axis — the top track shares the x (F2) mapping, the left track the y (F1)
/// mapping. Intensity is raw; the renderer autoscales it within the reserved
/// band, so an attached trace wider than the contour window still fills it.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AxisTrace {
    pub points: Vec<[f64; 2]>,
    pub color: Color,
    pub width: f32,
}

/// A dense value grid rendered as filled cells through a colormap. Unlike
/// [`Contour`] — which keeps only extracted line segments — the raw matrix is
/// retained so renderers can fill every cell.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HeatmapGrid {
    pub rows: usize,
    pub cols: usize,
    /// Row-major `rows × cols` values (the [`crate::Contour`]/marching-squares
    /// convention). Non-finite cells are skipped (drawn as background).
    pub values: Vec<f32>,
    /// Data-space x of the leftmost and rightmost cell edges; columns divide
    /// this span evenly.
    pub x_bounds: [f64; 2],
    /// Data-space y of the row-0 edge and the last-row edge; rows divide this
    /// span evenly.
    pub y_bounds: [f64; 2],
    /// `cols + 1` x cell boundaries for unevenly spaced columns, replacing the
    /// even division of `x_bounds`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x_edges: Option<Vec<f64>>,
    /// `rows + 1` y cell boundaries for unevenly spaced rows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y_edges: Option<Vec<f64>>,
    pub colormap: ColormapId,
    /// Value range mapped to the colormap ends; cells clamp outside it.
    pub value_range: [f32; 2],
}

impl HeatmapGrid {
    /// The normalized `[0, 1]` colormap position of one cell, or `None` for a
    /// missing / non-finite cell.
    pub fn normalized(&self, row: usize, col: usize) -> Option<f32> {
        let v = *self.values.get(row * self.cols + col)?;
        if !v.is_finite() {
            return None;
        }
        let [lo, hi] = self.value_range;
        let span = hi - lo;
        if span.abs() < f32::EPSILON {
            return Some(0.5);
        }
        Some(((v - lo) / span).clamp(0.0, 1.0))
    }

    /// Data-space rectangle `[x0, x1, y0, y1]` covered by one cell.
    pub fn cell_bounds(&self, row: usize, col: usize) -> [f64; 4] {
        let [x0, x1] = cell_span(self.x_edges.as_deref(), self.x_bounds, self.cols, col);
        let [y0, y1] = cell_span(self.y_edges.as_deref(), self.y_bounds, self.rows, row);
        [x0, x1, y0, y1]
    }
}

fn cell_span(edges: Option<&[f64]>, bounds: [f64; 2], count: usize, index: usize) -> [f64; 2] {
    if let Some(&[start, end]) = edges.and_then(|edges| edges.get(index..index + 2)) {
        return [start, end];
    }
    let step = (bounds[1] - bounds[0]) / count.max(1) as f64;
    let start = bounds[0] + index as f64 * step;
    [start, start + step]
}

/// Border treatment for the data area. Open axes suit 1D traces and ordinary
/// quantitative charts; a box defines the bounded field of a 2D contour while
/// ticks and labels remain on the left and bottom only. Hidden suppresses the
/// frame, ticks, and axis labels entirely (pie charts, 3D projections).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AxisFrame {
    #[default]
    Open,
    Box,
    Hidden,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Figure {
    /// Display amplification already applied to this materialized geometry.
    #[serde(default = "display_gain::unit_gain")]
    pub display_gain: f64,
    pub title: String,
    pub x: Axis,
    pub y: Axis,
    /// The right y axis. Present exactly when some series is measured on it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y2: Option<Axis>,
    pub series: Vec<Series>,
    /// Persistent descriptions of result-bearing 1D NMR integral curves.
    #[serde(default)]
    pub integral_curves: Vec<IntegralCurve>,
    /// Filled polygons, painted after the heatmap and before contours/series so
    /// bodies (bars, boxes, violins, wedges) sit under outlines and markers.
    #[serde(default)]
    pub polygons: Vec<Polygon>,
    /// Backmost layer: a dense colormapped cell grid.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heatmap: Option<HeatmapGrid>,
    /// Vertical uncertainty whiskers, painted behind the corresponding series.
    #[serde(default)]
    pub error_bars: Vec<ErrorBar>,
    pub annotations: Vec<Annotation>,
    /// Author-placed text anchored in data coordinates. Plot author overrides
    /// own and replace this list on every apply, so it stays apart from the
    /// builder-derived `annotations` and re-applying overrides is idempotent.
    #[serde(default)]
    pub author_annotations: Vec<Annotation>,
    /// Author-placed reference lines, owned and replaced by plot overrides
    /// exactly like `author_annotations`.
    #[serde(default)]
    pub reference_lines: Vec<ReferenceLine>,
    /// Author-placed arrows, owned by plot overrides like `author_annotations`.
    #[serde(default)]
    pub author_arrows: Vec<ArrowAnnotation>,
    pub range_annotations: Vec<RangeAnnotation>,
    /// Zero, one, or many contour overlays painted on the shared axes (e.g. one
    /// per dataset in a 2D color-overlay stack), each with its own colour/width.
    #[serde(default)]
    pub contours: Vec<Contour>,
    /// Marginal 1D projection along the top (F2/x) axis of a 2D contour, drawn in
    /// a reserved band above the plot. `None` = no top projection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_projection: Option<AxisTrace>,
    /// Marginal 1D projection along the left (F1/y) axis, drawn in a reserved band
    /// to the left of the plot. `None` = no left projection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub left_projection: Option<AxisTrace>,
    /// Preferred output size in logical units (px for screen, pt for export).
    pub width: f32,
    pub height: f32,
    pub background: Color,
    pub show_grid: bool,
    /// Authored visibility for every explanatory guide belonging to this plot.
    #[serde(default)]
    pub guide_visibility: GuideVisibility,
    /// Authored guide placement. Auto uses the guide kind's publication default:
    /// categorical legends inside, continuous colour scales outside right.
    #[serde(default)]
    pub guide_placement: GuidePlacement,
    /// Direction used by categorical legend entries. Auto chooses a compact
    /// vertical list; continuous colour scales follow their placement.
    #[serde(default)]
    pub guide_layout: GuideLayout,
    /// Optional explanatory title above a categorical legend.
    #[serde(default)]
    pub guide_title: String,
    /// Manually placed legend origin within its available plot area.
    pub legend_position: Option<[f32; 2]>,
    /// The figure's logical child series own their colours. A parent field
    /// binding may still apply scale and width, but must not collapse a
    /// categorical or repeated-series palette to one colour.
    pub series_colors_are_semantic: bool,
    /// Render the data area with equal data-units-per-pixel on both axes,
    /// letterboxed within the frame. Set for homonuclear 2D (square COSY/NOESY).
    #[serde(default)]
    pub lock_aspect: bool,
    #[serde(default)]
    pub axis_frame: AxisFrame,
    /// Text sizes and legend text color; see [`FigureTypography`].
    #[serde(default)]
    pub typography: FigureTypography,
}

impl Figure {
    pub fn new(title: impl Into<String>, x: Axis, y: Axis) -> Self {
        Self {
            display_gain: 1.0,
            title: title.into(),
            x,
            y,
            y2: None,
            series: Vec::new(),
            integral_curves: Vec::new(),
            polygons: Vec::new(),
            heatmap: None,
            error_bars: Vec::new(),
            annotations: Vec::new(),
            author_annotations: Vec::new(),
            reference_lines: Vec::new(),
            author_arrows: Vec::new(),
            range_annotations: Vec::new(),
            contours: Vec::new(),
            top_projection: None,
            left_projection: None,
            width: 900.0,
            height: 520.0,
            background: Color::rgb(255, 255, 255),
            show_grid: false,
            guide_visibility: GuideVisibility::Auto,
            guide_placement: GuidePlacement::Auto,
            guide_layout: GuideLayout::Auto,
            guide_title: String::new(),
            legend_position: None,
            series_colors_are_semantic: false,
            lock_aspect: false,
            axis_frame: AxisFrame::Open,
            typography: FigureTypography::default(),
        }
    }

    pub fn with_series(mut self, s: Series) -> Self {
        self.series.push(s);
        self
    }

    pub fn with_polygon(mut self, p: Polygon) -> Self {
        self.polygons.push(p);
        self
    }

    pub fn with_error_bar(mut self, error_bar: ErrorBar) -> Self {
        self.error_bars.push(error_bar);
        self
    }

    pub fn with_annotation(mut self, a: Annotation) -> Self {
        self.annotations.push(a);
        self
    }

    /// The vertical axis a series on `side` is measured against. A figure
    /// without a right axis measures every series on the left one.
    pub fn y_axis(&self, side: YAxisSide) -> &Axis {
        match (side, &self.y2) {
            (YAxisSide::Right, Some(axis)) => axis,
            _ => &self.y,
        }
    }

    /// Whether any series is measured on the right axis.
    pub fn uses_right_axis(&self) -> bool {
        self.series.iter().any(|s| s.appearance.y_axis.is_right())
    }

    /// Every data-anchored text label: builder-derived first, then authored.
    pub fn text_annotations(&self) -> impl Iterator<Item = &Annotation> {
        self.annotations.iter().chain(&self.author_annotations)
    }

    pub fn with_contour(mut self, c: Contour) -> Self {
        self.contours.push(c);
        self
    }

    pub fn with_axis_frame(mut self, frame: AxisFrame) -> Self {
        self.axis_frame = frame;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::{Axis, FigureTypography, TickDirection};

    #[test]
    fn axis_text_visibility_defaults_to_visible() {
        let axis = Axis::new("x", 0.0, 1.0);
        assert!(axis.show_tick_labels);
        assert!(axis.show_label);
    }

    #[test]
    fn missing_axis_visibility_fields_deserialize_as_visible() {
        let axis: Axis =
            serde_json::from_str(r#"{"label":"x","min":0.0,"max":1.0,"reversed":false}"#).unwrap();
        assert!(axis.show_tick_labels);
        assert!(axis.show_label);
    }

    #[test]
    fn typography_saved_before_axis_strokes_keeps_the_previous_look() {
        let typography: FigureTypography = serde_json::from_str(
            r#"{"tick_pt":7.0,"label_pt":8.0,"title_pt":8.0,"legend_pt":7.0,
                "legend_color":{"r":26,"g":26,"b":26}}"#,
        )
        .unwrap();
        assert_eq!(typography.axis_line_pt, 1.0);
        assert_eq!(typography.tick_length_pt, 3.0);
        assert_eq!(typography.tick_direction, TickDirection::Out);
        assert_eq!(typography.minor_ticks, 0);
        assert_eq!(typography.typeface, super::Typeface::Arial);
        assert_eq!(typography.font_weight, super::FontWeight::Regular);
    }
}
