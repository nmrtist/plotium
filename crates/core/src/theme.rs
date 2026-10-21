//! Document-level style themes: a named bundle of background, text colour, base
//! font size, axis furniture and a trace palette, applied to a canvas in one
//! undoable step. Built-in themes ship with Plotium; user styles are the same
//! bundle captured from a canvas and saved to the configuration directory.

use crate::actions::Action;
use crate::state::{
    ObjectId, ObjectStyle, PlotiumApp, ShapeKind, ShapeObject, StyleLibrary, TextBox,
};
use plotium_figure::{
    Color, FigureTypography, FontWeight, PositiveFiniteF32, TickDirection, Typeface,
};
use serde::{Deserialize, Serialize};

mod user_styles;
pub use user_styles::{
    STYLE_EXTENSION, StyleError, UserStyleInfo, delete_user_style, list_user_styles,
    load_user_style, resolve_theme, save_user_style, styles_dir, user_style_exists,
    validate_style_name,
};

/// A named bundle of document-level style defaults. `trace_palette[0]` is the
/// primary trace colour; further entries colour overlaid series in order.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Theme {
    /// Built-in themes use a fixed lowercase id; a user style is identified by
    /// its file name, so neither is stored in the style file.
    #[serde(skip)]
    pub id: String,
    #[serde(skip)]
    pub name: String,
    pub background: Color,
    pub text_color: Color,
    pub base_font_pt: f32,
    /// Axis-furniture text sizes and strokes for every plot in the document.
    pub figure_typography: FigureTypography,
    pub trace_palette: Vec<Color>,
    /// Width of every line series; none keeps each series' own width.
    pub line_width_pt: Option<f32>,
    /// Marker diameter for every curve that draws markers; none keeps each
    /// series' own size.
    pub marker_size_pt: Option<f32>,
    /// Remove marker outlines so markers are solid in their fill colour.
    #[serde(default)]
    pub clear_marker_edges: bool,
    /// Marker fill and outline paired index-for-index with `trace_palette`.
    /// Empty, or no paint at a position, keeps each curve's own marker paint.
    #[serde(default)]
    pub marker_palette: Vec<Option<MarkerPaint>>,
}

/// The closed-marker paint of one curve, as captured from a canvas.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MarkerPaint {
    /// No fill fills with the curve colour.
    pub fill: Option<Color>,
    /// No edge colour outlines in the curve colour.
    pub edge_color: Option<Color>,
    /// No edge width draws no outline unless the marker is hollow.
    pub edge_width_pt: Option<PositiveFiniteF32>,
    /// Closed markers draw only their outline.
    pub hollow: bool,
}

impl MarkerPaint {
    fn of(appearance: &plotium_figure::CurveAppearance) -> Self {
        Self {
            fill: appearance.marker_fill,
            edge_color: appearance.marker_edge_color,
            edge_width_pt: appearance.marker_edge_width,
            hollow: appearance.marker_hollow,
        }
    }

    fn paint(self, appearance: &mut plotium_figure::CurveAppearance) {
        appearance.marker_fill = self.fill;
        appearance.marker_edge_color = self.edge_color;
        appearance.marker_edge_width = self.edge_width_pt;
        appearance.marker_hollow = self.hollow;
    }
}

impl Theme {
    /// Built-in theme ids in menu order. A user style may not take one of
    /// these names, so a theme id stays unambiguous.
    pub const BUILTIN_IDS: [&'static str; 4] =
        ["publication", "nature", "presentation_dark", "vibrant"];

    /// The built-in themes in menu order.
    pub fn all() -> Vec<Theme> {
        Self::BUILTIN_IDS
            .iter()
            .filter_map(|id| Self::by_id(id))
            .collect()
    }

    /// Resolve a built-in theme id.
    pub fn by_id(id: &str) -> Option<Theme> {
        match id {
            "publication" => Some(Self::publication()),
            "nature" => Some(Self::nature()),
            "presentation_dark" => Some(Self::presentation_dark()),
            "vibrant" => Some(Self::vibrant()),
            _ => None,
        }
    }

    fn trace_color(&self, i: usize) -> Color {
        if self.trace_palette.is_empty() {
            Color::TRACE
        } else {
            self.trace_palette[i % self.trace_palette.len()]
        }
    }

    fn marker_paint(&self, i: usize) -> Option<MarkerPaint> {
        if self.marker_palette.is_empty() {
            None
        } else {
            self.marker_palette[i % self.marker_palette.len()]
        }
    }

    fn negative_contour_color(&self, i: usize) -> Color {
        let candidate = self.trace_color(i + 1);
        if candidate == self.trace_color(i) {
            Color::rgb(0xd1, 0x24, 0x2a)
        } else {
            candidate
        }
    }

    fn style_library(&self) -> StyleLibrary {
        let mut text = TextBox::label(String::new());
        text.color = self.text_color;
        text.font_size = self.base_font_pt;
        let mut panel_label = TextBox::panel_label(String::new());
        panel_label.color = self.text_color;
        let mut shape = ShapeObject::new(ShapeKind::Rect);
        shape.stroke = self.text_color;
        StyleLibrary {
            text,
            panel_label,
            shape,
            figure_typography: self.figure_typography,
        }
    }

    fn restyle_object(&self, style: &mut ObjectStyle) {
        match style {
            ObjectStyle::Text(t) => t.color = self.text_color,
            ObjectStyle::Shape(s) => s.stroke = self.text_color,
        }
    }

    fn publication() -> Theme {
        Theme {
            id: "publication".to_owned(),
            name: "Publication".to_owned(),
            background: Color::rgb(255, 255, 255),
            text_color: Color::rgb(0, 0, 0),
            base_font_pt: 12.0,
            figure_typography: FigureTypography::default(),
            trace_palette: vec![
                Color::rgb(0x0f, 0x4d, 0x92),
                Color::rgb(0xb6, 0x43, 0x42),
                Color::rgb(0x2e, 0x9e, 0x44),
                Color::rgb(0x42, 0x94, 0x9e),
                Color::rgb(0x4d, 0x4d, 0x4d),
            ],
            line_width_pt: None,
            marker_size_pt: None,
            clear_marker_edges: false,
            marker_palette: Vec::new(),
        }
    }

    /// Nature Portfolio final-artwork guidelines: 7 pt text (inside
    /// the 5–7 pt range and at the export precheck's 7 pt floor), thin 0.6 pt
    /// axes with outward 3 pt ticks, 0.75 pt line series, 3 pt markers without
    /// an outline, and the colour-blind-safe Okabe–Ito palette.
    fn nature() -> Theme {
        let black = Color::rgb(0, 0, 0);
        Theme {
            id: "nature".to_owned(),
            name: "Nature".to_owned(),
            background: Color::rgb(255, 255, 255),
            text_color: black,
            base_font_pt: 7.0,
            figure_typography: FigureTypography {
                tick_pt: 7.0,
                label_pt: 7.0,
                title_pt: 7.0,
                legend_pt: 7.0,
                legend_color: black,
                axis_line_pt: 0.6,
                tick_length_pt: 3.0,
                tick_direction: TickDirection::Out,
                minor_ticks: 0,
                typeface: Typeface::Arial,
                font_weight: FontWeight::Regular,
            },
            // Okabe–Ito, yellow last because it has the weakest contrast on
            // white.
            trace_palette: vec![
                black,
                Color::rgb(0x00, 0x72, 0xb2),
                Color::rgb(0xd5, 0x5e, 0x00),
                Color::rgb(0x00, 0x9e, 0x73),
                Color::rgb(0xe6, 0x9f, 0x00),
                Color::rgb(0x56, 0xb4, 0xe9),
                Color::rgb(0xcc, 0x79, 0xa7),
                Color::rgb(0xf0, 0xe4, 0x42),
            ],
            line_width_pt: Some(0.75),
            marker_size_pt: Some(3.0),
            clear_marker_edges: true,
            marker_palette: Vec::new(),
        }
    }

    fn presentation_dark() -> Theme {
        Theme {
            id: "presentation_dark".to_owned(),
            name: "Presentation Dark".to_owned(),
            background: Color::rgb(0x14, 0x18, 0x20),
            text_color: Color::rgb(0xf0, 0xf2, 0xf5),
            base_font_pt: 16.0,
            // Slides are read from meters away; scale the axis type up with
            // the base font.
            figure_typography: FigureTypography {
                tick_pt: 10.0,
                label_pt: 12.0,
                title_pt: 12.0,
                legend_pt: 9.0,
                // All renderers put legends on a translucent white backing.
                legend_color: Color::AXIS,
                ..FigureTypography::default()
            },
            trace_palette: vec![
                Color::rgb(0x4d, 0xa6, 0xff),
                Color::rgb(0xff, 0x8a, 0x3d),
                Color::rgb(0x3d, 0xd6, 0x8c),
                Color::rgb(0xff, 0x6b, 0x9d),
                Color::rgb(0xff, 0xd1, 0x66),
            ],
            line_width_pt: None,
            marker_size_pt: None,
            clear_marker_edges: false,
            marker_palette: Vec::new(),
        }
    }

    fn vibrant() -> Theme {
        Theme {
            id: "vibrant".to_owned(),
            name: "Vibrant".to_owned(),
            background: Color::rgb(255, 255, 255),
            text_color: Color::rgb(0x1a, 0x1a, 0x1a),
            base_font_pt: 13.0,
            figure_typography: FigureTypography {
                tick_pt: 8.0,
                label_pt: 9.0,
                title_pt: 9.0,
                legend_pt: 7.0,
                legend_color: Color::rgb(0x1a, 0x1a, 0x1a),
                ..FigureTypography::default()
            },
            trace_palette: vec![
                Color::rgb(0xe6, 0x00, 0x49),
                Color::rgb(0x00, 0x99, 0xc6),
                Color::rgb(0xf5, 0xa6, 0x23),
                Color::rgb(0x7b, 0x2f, 0xf2),
                Color::rgb(0x00, 0xb8, 0x74),
            ],
            line_width_pt: None,
            marker_size_pt: None,
            clear_marker_edges: false,
            marker_palette: Vec::new(),
        }
    }
}

/// Document-level style state captured for a theme apply, so the change can be
/// reverted as a single undo step.
#[derive(Clone, PartialEq)]
pub struct ThemeSnapshot {
    pub background: Color,
    pub style_library: StyleLibrary,
    pub object_styles: Vec<(ObjectId, ObjectStyle)>,
    pub series_encodings: Vec<(ObjectId, Vec<plotium_figure::SeriesEncoding>)>,
}

impl PlotiumApp {
    fn capture_theme_snapshot(&self, ci: usize) -> ThemeSnapshot {
        let canvas = &self.doc.canvases[ci];
        let mut object_styles = Vec::new();
        let mut series_encodings = Vec::new();
        for object in &canvas.objects {
            if let Some(style) = object.style() {
                object_styles.push((object.id, style));
            } else if let Some(plot) = object.plot() {
                series_encodings.push((
                    object.id,
                    plot.binding
                        .series
                        .iter()
                        .map(|series| series.encoding.clone())
                        .collect(),
                ));
            }
        }
        ThemeSnapshot {
            background: canvas.background,
            style_library: self.doc.style_library.clone(),
            object_styles,
            series_encodings,
        }
    }

    fn themed_snapshot(&self, ci: usize, theme: &Theme) -> ThemeSnapshot {
        let mut snap = self.capture_theme_snapshot(ci);
        snap.background = theme.background;
        snap.style_library = theme.style_library();
        for (_, style) in &mut snap.object_styles {
            theme.restyle_object(style);
        }
        for (_, encodings) in &mut snap.series_encodings {
            for (i, encoding) in encodings.iter_mut().enumerate() {
                match encoding {
                    plotium_figure::SeriesEncoding::Line(line) => {
                        line.color = plotium_figure::ColorSource::Explicit(theme.trace_color(i));
                        // Sizing a curve without markers would only surprise
                        // the user when they later turn markers on.
                        if line.appearance.marker(line.kind) != plotium_figure::Marker::None
                            && let Some(size) = theme
                                .marker_size_pt
                                .and_then(plotium_figure::PositiveFiniteF32::new)
                        {
                            line.appearance.marker_size = Some(size);
                        }
                        if theme.clear_marker_edges {
                            line.appearance.marker_edge_width = None;
                        }
                        if let Some(paint) = theme.marker_paint(i) {
                            paint.paint(&mut line.appearance);
                        }
                        if line.kind == plotium_figure::SeriesKind::Line
                            && let Some(width) = theme
                                .line_width_pt
                                .and_then(plotium_figure::PositiveFiniteF32::new)
                        {
                            line.width = width;
                        }
                    }
                    plotium_figure::SeriesEncoding::Contour(contour) => {
                        contour.style.positive_color =
                            plotium_figure::ColorSource::Explicit(theme.trace_color(i));
                        contour.style.negative_color =
                            plotium_figure::ColorSource::Explicit(theme.negative_contour_color(i));
                    }
                    plotium_figure::SeriesEncoding::Heatmap(_)
                    | plotium_figure::SeriesEncoding::Image(_) => {}
                }
            }
        }
        snap
    }

    pub fn apply_theme_snapshot(&mut self, canvas: usize, snap: &ThemeSnapshot) {
        if let Some(c) = self.doc.canvases.get_mut(canvas) {
            c.background = snap.background;
            for (id, style) in &snap.object_styles {
                if let Some(o) = c.object_mut(*id) {
                    o.set_style(style);
                }
            }
            for (id, encodings) in &snap.series_encodings {
                if let Some(plot) = c.object_mut(*id).and_then(|o| o.plot_mut()) {
                    for (series, encoding) in plot.binding.series.iter_mut().zip(encodings) {
                        series.encoding = encoding.clone();
                    }
                }
            }
        }
        self.doc.style_library = snap.style_library.clone();
        // The style library is document-level and figures stamp its typography
        // at build time, so every canvas must rebuild — not just the themed one
        // — or the others would keep stale type until their next rebuild.
        for ci in 0..self.doc.canvases.len() {
            self.rebuild_canvas(ci);
        }
    }

    /// Set the document's figure typography and rebuild every plot so the new
    /// sizes are visible at once. Shared by the action's apply and revert.
    pub fn set_figure_typography_value(&mut self, typography: FigureTypography) {
        self.doc.style_library.figure_typography = typography;
        for ci in 0..self.doc.canvases.len() {
            self.rebuild_canvas(ci);
        }
    }

    pub fn apply_theme(&mut self, theme: &Theme) {
        let Some(ci) = self.session.active_canvas else {
            self.session.status = "Open a canvas before applying a theme.".to_owned();
            return;
        };
        let before = self.capture_theme_snapshot(ci);
        let after = self.themed_snapshot(ci, theme);
        self.execute_action(Action::apply_theme(ci, before, after));
        self.session.status = format!("Applied the {} theme.", theme.name);
    }

    /// Capture a canvas's current look as a theme named `name`: its page
    /// background, the document text style and figure typography, and for each
    /// series position the curve colour and marker paint of the first plot
    /// with a curve there, matching how a theme colours series by position.
    /// Line width and marker size come from the first curve that draws a line
    /// or markers. A canvas without curves keeps the Publication palette.
    pub fn theme_from_canvas(&self, canvas: usize, name: &str) -> Option<Theme> {
        use plotium_figure::{Marker, SeriesEncoding, SeriesKind};
        let canvas = self.doc.canvases.get(canvas)?;
        let library = &self.doc.style_library;
        let mut slots: Vec<Option<(Color, Option<MarkerPaint>)>> = Vec::new();
        let mut line_width = None;
        let mut marker_size = None;
        for plot in canvas.objects.iter().filter_map(|object| object.plot()) {
            for (i, series) in plot.binding.series.iter().enumerate() {
                let slot = match &series.encoding {
                    SeriesEncoding::Line(line) => {
                        if line.kind == SeriesKind::Line {
                            line_width.get_or_insert(line.width.get());
                        }
                        if line.appearance.marker(line.kind) != Marker::None
                            && let Some(size) = line.appearance.marker_size
                        {
                            marker_size.get_or_insert(size.get());
                        }
                        (
                            line.color.resolve(),
                            Some(MarkerPaint::of(&line.appearance)),
                        )
                    }
                    SeriesEncoding::Contour(contour) => {
                        (contour.style.positive_color.resolve(), None)
                    }
                    SeriesEncoding::Heatmap(_) | SeriesEncoding::Image(_) => continue,
                };
                if slots.len() <= i {
                    slots.resize(i + 1, None);
                }
                slots[i].get_or_insert(slot);
            }
        }
        let fallback = Theme::publication();
        let (trace_palette, marker_palette) = if slots.is_empty() {
            (fallback.trace_palette.clone(), Vec::new())
        } else {
            // A position only images or heatmaps occupy has no curve colour;
            // it keeps the Publication colour so later positions stay aligned.
            slots
                .iter()
                .enumerate()
                .map(|(i, slot)| slot.unwrap_or((fallback.trace_color(i), None)))
                .unzip()
        };
        Some(Theme {
            id: name.to_owned(),
            name: name.to_owned(),
            background: canvas.background,
            text_color: library.text.color,
            base_font_pt: library.text.font_size,
            figure_typography: library.figure_typography,
            trace_palette,
            line_width_pt: line_width,
            marker_size_pt: marker_size,
            clear_marker_edges: false,
            marker_palette,
        })
    }

    /// Prepare the semantic theme edit without committing it. Automation uses
    /// this to combine several canvases into one validated undo transaction.
    pub fn theme_action(&self, canvas: usize, theme: &Theme) -> Option<Action> {
        self.doc.canvases.get(canvas)?;
        Some(Action::apply_theme(
            canvas,
            self.capture_theme_snapshot(canvas),
            self.themed_snapshot(canvas, theme),
        ))
    }
}
