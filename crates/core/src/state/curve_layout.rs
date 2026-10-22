//! Slot layout of explicit curves: bars, boxes and violins occupy x slots side
//! by side, points and lines overlay them, value labels annotate them, and a
//! transposed plot turns the result on its side. Everything lowers into the
//! figure IR, so renderers need no chart-specific code.
use super::curve_bars::{BarStyle, CAP_FRACTION, numeric_slots_at};
use super::table_charts::marks::{box_marks, violin_marks};
use plotium_figure::{Annotation, Color, ErrorBar, Figure, Polygon, Series, TextAnchor};
use std::collections::HashMap;

/// How an occupant draws the samples in its slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MarkKind {
    /// One rectangle per point, from the baseline to the value.
    Bar,
    /// One box-and-whisker summary of all values sharing an x.
    Box,
    /// One kernel-density violin of all values sharing an x.
    Violin,
}

/// Whether a curve takes a share of each slot or is drawn over them; an
/// overlay carries its replicate spread.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Role {
    Occupant(MarkKind),
    Overlay(f64),
}

/// A curve that takes a share of each x slot it appears in.
pub(super) struct Occupant {
    pub kind: MarkKind,
    pub series: Vec<Series>,
    pub errors: Vec<ErrorBar>,
    pub style: BarStyle,
    pub baseline: f64,
    pub labels: bool,
}

/// A point or line curve drawn over the occupants.
pub(super) struct Overlay {
    pub series: Vec<Series>,
    pub errors: Vec<ErrorBar>,
    /// Spread of replicates sharing an x, as a fraction of the mark width.
    pub spread: f64,
    pub labels: bool,
    /// Offset-stack shift, removed again from labelled values.
    pub baseline: f64,
}

/// Text of value labels.
#[derive(Clone, Copy)]
pub(super) struct LabelStyle {
    pub size: f32,
    pub color: Color,
}

/// Data-space extents of the laid-out marks: `[xmin, xmax, ymin, ymax]`.
pub(super) type Extents = [f64; 4];

/// Occupants split each slot evenly between the curves present in it, and
/// each fills its own width fraction of its share; a curve alone in a slot
/// keeps the whole slot. On a category axis, or when spread, the k-th overlay
/// sits on the k-th occupant's position, so raw points line up with their bar
/// or box; spare overlays, and those whose occupant has no mark at an x, sit on
/// the slot centre. A plain numeric overlay (a trend or fit line) keeps its
/// true x. Category slots are one unit wide, numeric slots follow
/// [`super::curve_bars::slot_widths`].
pub(super) fn lower(
    figure: &mut Figure,
    occupants: Vec<Occupant>,
    overlays: Vec<Overlay>,
    categorical: bool,
    label: LabelStyle,
) -> Result<Option<Extents>, String> {
    let key = |x: f64| (x + 0.0).to_bits();
    let (overlays, plain): (Vec<_>, Vec<_>) = overlays
        .into_iter()
        .partition(|o| categorical || o.spread > 0.0 || o.labels);
    for overlay in plain {
        figure.series.extend(overlay.series);
        figure.error_bars.extend(overlay.errors);
    }
    if occupants.is_empty() && overlays.is_empty() {
        return Ok(None);
    }
    let slots: HashMap<u64, f64> = if categorical {
        HashMap::new()
    } else if occupants.is_empty() {
        numeric_slots_at(overlays.iter().flat_map(|o| &o.series))
    } else {
        numeric_slots_at(occupants.iter().flat_map(|o| &o.series))
    };
    // A position without a slot of its own (an overlay between bars) takes the
    // typical slot rather than one data unit.
    let typical = {
        let mut widths: Vec<f64> = slots.values().copied().collect();
        widths.sort_by(f64::total_cmp);
        widths.get(widths.len() / 2).copied().unwrap_or(1.0)
    };
    let slot_at = |x: f64| slots.get(&key(x)).copied().unwrap_or(typical);
    let mut residents: HashMap<u64, Vec<usize>> = HashMap::new();
    for (index, occupant) in occupants.iter().enumerate() {
        for point in occupant.series.iter().flat_map(|series| &series.points) {
            let curves = residents.entry(key(point[0])).or_default();
            if curves.last() != Some(&index) {
                curves.push(index);
            }
        }
    }
    // Offset from the slot centre and the share of the slot for one curve at x.
    let dodge = |index: usize, x: f64| {
        let slot = slot_at(x);
        let curves = residents.get(&key(x)).map_or(&[][..], Vec::as_slice);
        let count = curves.len().max(1);
        let rank = curves.iter().position(|c| *c == index).unwrap_or(0);
        let share = slot / count as f64;
        ((rank as f64 + 0.5) * share - 0.5 * slot, share)
    };
    let mut extents = Bounds::default();
    for (index, occupant) in occupants.iter().enumerate() {
        match occupant.kind {
            MarkKind::Bar => lower_bar(figure, occupant, |x| dodge(index, x), label, &mut extents)?,
            MarkKind::Box | MarkKind::Violin => {
                lower_distribution(figure, occupant, |x| dodge(index, x), &mut extents)
            }
        }
    }
    for (index, overlay) in overlays.into_iter().enumerate() {
        // Where and how wide the paired mark is at x.
        let paired = categorical || overlay.spread > 0.0;
        let place = |x: f64| match occupants.get(index) {
            Some(occupant)
                if paired
                    && residents
                        .get(&key(x))
                        .is_some_and(|curves| curves.contains(&index)) =>
            {
                let (dx, share) = dodge(index, x);
                (dx, occupant.style.width * share)
            }
            _ => (0.0, 0.8 * slot_at(x)),
        };
        lower_overlay(figure, overlay, place, label, &mut extents)?;
    }
    Ok(extents.get())
}

#[derive(Default)]
struct Bounds(Option<Extents>);

impl Bounds {
    fn include(&mut self, x: f64, y: f64) {
        let e = self.0.get_or_insert([
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
        ]);
        *e = [e[0].min(x), e[1].max(x), e[2].min(y), e[3].max(y)];
    }

    fn get(&self) -> Option<Extents> {
        self.0.filter(|e| e.iter().all(|v| v.is_finite()))
    }
}

fn lower_bar(
    figure: &mut Figure,
    bar: &Occupant,
    dodge: impl Fn(f64) -> (f64, f64),
    label: LabelStyle,
    extents: &mut Bounds,
) -> Result<(), String> {
    // A label sits past its bar's whisker, not under it.
    let reach: HashMap<(u64, u64), (f64, f64)> = bar
        .errors
        .iter()
        .map(|e| {
            let k = ((e.center[0] + 0.0).to_bits(), e.center[1].to_bits());
            (k, (e.negative, e.positive))
        })
        .collect();
    for series in &bar.series {
        for &[x, y] in &series.points {
            let (dx, share) = dodge(x);
            let half = 0.5 * bar.style.width * share;
            let (x0, x1) = (x + dx - half, x + dx + half);
            if ![x0, x1].into_iter().all(f64::is_finite) {
                return Err("Bar presentation overflowed.".into());
            }
            extents.include(x0, bar.baseline);
            extents.include(x1, y);
            let mut rect =
                Polygon::rect(series.name.clone(), x0, x1, bar.baseline, y, series.color);
            rect.origin = series.origin;
            figure
                .polygons
                .push(match bar.style.edge_for(series.color) {
                    Some((color, width)) => rect.with_stroke(color, width),
                    None => rect,
                });
            if bar.labels {
                let (below, above) = reach
                    .get(&((x + 0.0).to_bits(), y.to_bits()))
                    .copied()
                    .unwrap_or((0.0, 0.0));
                let (anchor, end) = if y >= bar.baseline {
                    (TextAnchor::Above, y + above)
                } else {
                    (TextAnchor::Below, y - below)
                };
                figure.annotations.push(value_label(
                    y - bar.baseline,
                    [x + dx, end],
                    anchor,
                    label,
                ));
            }
        }
    }
    for error in &bar.errors {
        let mut error = error.clone();
        // Whiskers are built at their bar's undodged x.
        let (dx, share) = dodge(error.center[0]);
        error.center[0] += dx;
        // Caps scale with the bar rather than a fixed point width, which on a
        // narrow grouped bar reads as a flat bracket.
        error.cap_data_width = Some(CAP_FRACTION * bar.style.width * share);
        // An outlined bar draws its whiskers in the outline's ink; styling has
        // already given each whisker its bar's fill.
        if let Some((color, width)) = bar.style.edge_for(error.color) {
            error.color = color;
            error.width = width;
        }
        // A filled bar would hide the lower whisker drawn beneath it.
        figure.error_bars.push(error.over_data());
    }
    Ok(())
}

fn lower_distribution(
    figure: &mut Figure,
    occupant: &Occupant,
    dodge: impl Fn(f64) -> (f64, f64),
    extents: &mut Bounds,
) {
    // Box and violin outlines follow the bars' outline width (the document's
    // supporting-line width when set); a zero "no outline" falls back to the
    // default width, since the outline is what draws these marks.
    let stroke = occupant
        .style
        .edge
        .map_or(plotium_figure::DEFAULT_BAR_EDGE_WIDTH_PT as f32, |(_, w)| w);
    for series in &occupant.series {
        // All samples at one x form one distribution, in first-seen order.
        let mut groups: Vec<(f64, Vec<f64>)> = Vec::new();
        for &[x, y] in &series.points {
            match groups
                .iter_mut()
                .find(|(gx, _)| (gx + 0.0).to_bits() == (x + 0.0).to_bits())
            {
                Some((_, values)) => values.push(y),
                None => groups.push((x, vec![y])),
            }
        }
        for (x, values) in groups {
            let (dx, share) = dodge(x);
            let (center, half) = (x + dx, 0.5 * occupant.style.width * share);
            let marks = match occupant.kind {
                MarkKind::Violin => {
                    violin_marks(&values, center, half, series.color, &series.name, stroke)
                }
                _ => box_marks(&values, center, half, series.color, &series.name, stroke),
            };
            let Some(marks) = marks else {
                continue;
            };
            extents.include(center - half, marks.y_range[0]);
            extents.include(center + half, marks.y_range[1]);
            figure
                .polygons
                .extend(marks.polygons.into_iter().map(|mut mark| {
                    mark.origin = series.origin;
                    mark
                }));
            // Whiskers and outlines belong to the curve they summarise.
            figure
                .series
                .extend(marks.series.into_iter().map(|mut mark| {
                    mark.origin = series.origin;
                    mark
                }));
        }
    }
}

fn lower_overlay(
    figure: &mut Figure,
    mut overlay: Overlay,
    place: impl Fn(f64) -> (f64, f64),
    label: LabelStyle,
    extents: &mut Bounds,
) -> Result<(), String> {
    // Replicates sharing an x spread evenly across the paired mark's width, in
    // row order, so the result is deterministic.
    let mut counts: HashMap<u64, usize> = HashMap::new();
    for point in overlay.series.iter().flat_map(|s| &s.points) {
        *counts.entry((point[0] + 0.0).to_bits()).or_default() += 1;
    }
    let mut seen: HashMap<u64, usize> = HashMap::new();
    // Placed x of each (x, y) in row order, so replicates with identical
    // values keep their own whiskers.
    let mut moved: HashMap<(u64, u64), std::collections::VecDeque<f64>> = HashMap::new();
    for series in &mut overlay.series {
        for point in &mut series.points {
            let k = (point[0] + 0.0).to_bits();
            let (dx, width) = place(point[0]);
            let n = counts.get(&k).copied().unwrap_or(1);
            let rank = seen.entry(k).or_default();
            let offset = if n > 1 {
                (*rank as f64 / (n - 1) as f64 - 0.5) * overlay.spread * width
            } else {
                0.0
            };
            *rank += 1;
            let x = point[0] + dx + offset;
            if !x.is_finite() {
                return Err("Curve presentation overflowed.".into());
            }
            moved
                .entry((k, point[1].to_bits()))
                .or_default()
                .push_back(x);
            point[0] = x;
            extents.include(x, point[1]);
            if overlay.labels {
                figure.annotations.push(value_label(
                    point[1] - overlay.baseline,
                    *point,
                    TextAnchor::Above,
                    label,
                ));
            }
        }
    }
    for error in &mut overlay.errors {
        let k = (error.center[0] + 0.0).to_bits();
        error.center[0] = moved
            .get_mut(&(k, error.center[1].to_bits()))
            .and_then(|xs| xs.pop_front())
            .unwrap_or_else(|| error.center[0] + place(error.center[0]).0);
    }
    figure.series.extend(overlay.series);
    figure.error_bars.extend(overlay.errors);
    Ok(())
}

fn value_label(value: f64, at: [f64; 2], anchor: TextAnchor, style: LabelStyle) -> Annotation {
    Annotation {
        text: format_value(value),
        at,
        color: style.color,
        size: style.size,
        anchor,
    }
}

/// Three significant figures, trailing zeros trimmed; magnitudes from 10^5 up
/// or below 10^-3 in scientific notation, as the colour-scale labels write them.
pub(super) fn format_value(value: f64) -> String {
    if value == 0.0 {
        return "0".into();
    }
    if !value.is_finite() {
        return value.to_string();
    }
    let trim = |text: String| {
        if text.contains('.') {
            text.trim_end_matches('0').trim_end_matches('.').to_owned()
        } else {
            text
        }
    };
    let magnitude = value.abs().log10().floor() as i32;
    if !(-3..5).contains(&magnitude) {
        let text = format!("{value:.2e}");
        return match text.split_once('e') {
            Some((mantissa, exponent)) => format!("{}e{exponent}", trim(mantissa.to_owned())),
            None => text,
        };
    }
    let decimals = (2 - magnitude).max(0) as usize;
    // Round to three significant figures, including in the integer digits.
    let step = 10f64.powi(magnitude - 2);
    trim(format!("{:.decimals$}", (value / step).round() * step))
}

/// Turns a laid-out figure on its side: data x runs up the vertical axis and
/// values along the horizontal one. Category slots then read top to bottom.
pub(super) fn transpose(figure: &mut Figure) {
    let swap = |p: &mut [f64; 2]| p.swap(0, 1);
    figure
        .series
        .iter_mut()
        .flat_map(|s| &mut s.points)
        .for_each(swap);
    figure
        .polygons
        .iter_mut()
        .flat_map(|p| &mut p.points)
        .for_each(swap);
    for error in &mut figure.error_bars {
        swap(&mut error.center);
        error.horizontal = !error.horizontal;
    }
    for annotation in &mut figure.annotations {
        swap(&mut annotation.at);
        annotation.anchor = match annotation.anchor {
            TextAnchor::Above => TextAnchor::Right,
            TextAnchor::Below => TextAnchor::Left,
            TextAnchor::Right => TextAnchor::Above,
            TextAnchor::Left => TextAnchor::Below,
            TextAnchor::Point => TextAnchor::Point,
        };
    }
    std::mem::swap(&mut figure.x, &mut figure.y);
    if figure.y.categories.is_some() {
        figure.y.reversed = !figure.y.reversed;
    }
}
