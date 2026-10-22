//! Box and violin geometry of one sample at one x slot, shared by the table
//! distribution charts and explicit box/violin curves.
use super::{WHITE, darkened};
use plotium_analysis::statistics::{describe, gaussian_kde};
use plotium_figure::{Color, Polygon, Series};

/// Shapes and strokes of one summarised sample, with its y extent.
pub(crate) struct Marks {
    pub polygons: Vec<Polygon>,
    pub series: Vec<Series>,
    pub y_range: [f64; 2],
}

impl Marks {
    fn new() -> Self {
        Self {
            polygons: Vec::new(),
            series: Vec::new(),
            y_range: [f64::INFINITY, f64::NEG_INFINITY],
        }
    }

    fn include(&mut self, y: f64) {
        self.y_range = [self.y_range[0].min(y), self.y_range[1].max(y)];
    }
}

/// A Tukey box plot centred on `xc` with half-width `half`: the IQR box, the
/// median, whiskers to the most extreme observations inside 1.5 × IQR, and
/// the outliers beyond as points. `name` labels the box for the legend; the
/// box outline and whiskers are `stroke` wide and the median half again.
pub(crate) fn box_marks(
    values: &[f64],
    xc: f64,
    half: f64,
    color: Color,
    name: &str,
    stroke: f32,
) -> Option<Marks> {
    let stats = describe(values).ok()?;
    let mut marks = Marks::new();
    marks.include(stats.minimum);
    marks.include(stats.maximum);
    let (q1, q3) = (stats.first_quartile, stats.third_quartile);
    let fence = 1.5 * stats.interquartile_range;
    // Tukey whiskers: the most extreme observations inside the fences.
    let whisker_lo = values
        .iter()
        .copied()
        .filter(|&v| v >= q1 - fence)
        .fold(f64::INFINITY, f64::min);
    let whisker_hi = values
        .iter()
        .copied()
        .filter(|&v| v <= q3 + fence)
        .fold(f64::NEG_INFINITY, f64::max);
    marks.polygons.push(
        Polygon::rect(name, xc - half, xc + half, q1, q3, color)
            .with_opacity(0.45)
            .with_stroke(color, stroke),
    );
    let mut line = |points: Vec<[f64; 2]>, width: f32| {
        let mut s = Series::line(String::new(), points).colored(color);
        s.width = width;
        marks.series.push(s);
    };
    let cap = 0.4 * half;
    line(
        vec![[xc - half, stats.median], [xc + half, stats.median]],
        1.5 * stroke,
    );
    line(vec![[xc, q3], [xc, whisker_hi]], stroke);
    line(vec![[xc, q1], [xc, whisker_lo]], stroke);
    line(vec![[xc - cap, whisker_hi], [xc + cap, whisker_hi]], stroke);
    line(vec![[xc - cap, whisker_lo], [xc + cap, whisker_lo]], stroke);
    let outliers: Vec<[f64; 2]> = values
        .iter()
        .filter(|&&v| v < q1 - fence || v > q3 + fence)
        .map(|&v| [xc, v])
        .collect();
    if !outliers.is_empty() {
        let mut s = Series::points(String::new(), outliers).colored(color);
        s.width = 2.0;
        marks.series.push(s);
    }
    Some(marks)
}

/// A violin centred on `xc` whose widest point is `half` from the centre: the
/// Gaussian KDE outline, a thin quartile bar and a white median dot. Too few or
/// identical observations have no density and fall back to the raw points.
/// The outline is `stroke` wide.
pub(crate) fn violin_marks(
    values: &[f64],
    xc: f64,
    half: f64,
    color: Color,
    name: &str,
    stroke: f32,
) -> Option<Marks> {
    let mut marks = Marks::new();
    let Ok(kde) = gaussian_kde(values, 120) else {
        if values.is_empty() {
            return None;
        }
        let pts: Vec<[f64; 2]> = values.iter().map(|&v| [xc, v]).collect();
        pts.iter().for_each(|p| marks.include(p[1]));
        let mut s = Series::points(String::new(), pts).colored(color);
        s.width = 2.0;
        marks.series.push(s);
        return Some(marks);
    };
    let max_density = kde.densities.iter().copied().fold(0.0f64, f64::max);
    if max_density <= 0.0 {
        return None;
    }
    marks.include(kde.xs[0]);
    marks.include(kde.xs[kde.xs.len() - 1]);
    let half_widths: Vec<f64> = kde
        .densities
        .iter()
        .map(|d| half * d / max_density)
        .collect();
    // The violin outline is concave, so emit one convex trapezoid strip per
    // KDE grid interval (the renderer's convex-polygon invariant). Only the
    // first strip carries the name, so the legend shows one entry.
    for i in 0..kde.xs.len() - 1 {
        let (y0, y1) = (kde.xs[i], kde.xs[i + 1]);
        let (w0, w1) = (half_widths[i], half_widths[i + 1]);
        marks.polygons.push(
            Polygon::new(
                if i == 0 { name } else { "" },
                vec![[xc - w0, y0], [xc + w0, y0], [xc + w1, y1], [xc - w1, y1]],
                color,
            )
            .with_opacity(0.5),
        );
    }
    let mut outline: Vec<[f64; 2]> = kde
        .xs
        .iter()
        .zip(&half_widths)
        .map(|(&y, &w)| [xc - w, y])
        .collect();
    outline.extend(
        kde.xs
            .iter()
            .zip(&half_widths)
            .rev()
            .map(|(&y, &w)| [xc + w, y]),
    );
    outline.push(outline[0]);
    let mut outline = Series::line(String::new(), outline).colored(color);
    outline.width = stroke;
    marks.series.push(outline);
    // Inner quartile bar with a contrasting median marker.
    if let Ok(stats) = describe(values) {
        let bar = half * (0.03 / 0.42);
        marks.polygons.push(Polygon::rect(
            String::new(),
            xc - bar,
            xc + bar,
            stats.first_quartile,
            stats.third_quartile,
            darkened(color, 0.65),
        ));
        let mut median = Series::points(String::new(), vec![[xc, stats.median]]);
        median = median.colored(WHITE);
        median.width = 2.0;
        marks.series.push(median);
    }
    Some(marks)
}
