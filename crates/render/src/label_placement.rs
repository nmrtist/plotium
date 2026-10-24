//! Placement of author labels clear of plotted data. Reference-line labels are
//! placed first, then author text annotations; each avoids series, bars, error
//! bars, arrows, the legend, value labels and the labels placed before it.
//! Every backend, diagnostics and canvas hit testing read one [`LabelLayout`].
use crate::legend_placement::{clip, for_each_data_segment, grow, project};
use crate::text_place::{PlacedText, TextAlign};
use crate::{Rect, TextAnchor};
use plotium_figure::{
    Annotation, Figure, ReferenceLabelPosition, ReferenceLabelSide, ReferenceLine,
    ReferenceOrientation, YAxisSide,
};

/// Gap between a reference line and its label, in pt.
const REFERENCE_GAP_PT: f32 = 2.0;
/// How far an automatic text label may move, in steps of half a text height.
const TEXT_REACH_STEPS: i32 = 6;
/// Clearance a label keeps from data, in pt.
const CLEARANCE_PT: f32 = 1.0;

type Point = (f32, f32);

/// Where each author label is drawn, in output units.
#[derive(Clone, Debug, Default)]
pub struct LabelLayout {
    /// One entry per `Figure::reference_lines` item; `None` when the line is
    /// outside the visible range or has no label.
    pub reference: Vec<Option<ReferenceLabelBox>>,
    /// One entry per `Figure::author_annotations` item; `None` when its anchor
    /// has no finite position.
    pub texts: Vec<Option<TextLabelBox>>,
}

#[derive(Clone, Copy, Debug)]
pub struct ReferenceLabelBox {
    pub position: ReferenceLabelPosition,
    /// Output x of a vertical line or y of a horizontal one.
    pub line: f32,
    /// The text is aligned on `x` by `anchor`; `top` is its top edge.
    pub x: f32,
    pub top: f32,
    pub anchor: TextAnchor,
    /// Font size in output units.
    pub size: f32,
    pub bounds: Rect,
}

#[derive(Clone, Copy, Debug)]
pub struct TextLabelBox {
    /// Centre of the text baseline.
    pub x: f32,
    pub y: f32,
    /// Offset from the projected anchor in pt, x right and y up.
    pub offset_pt: [f32; 2],
    pub bounds: Rect,
}

/// Output x of a vertical line or y of a horizontal one; `None` when the
/// value lies outside the visible plot.
pub(crate) fn line_position(fig: &Figure, plot: Rect, line: &ReferenceLine) -> Option<f32> {
    if !line.value.is_finite() {
        return None;
    }
    match line.orientation {
        ReferenceOrientation::Vertical => {
            let x = project(fig, plot, [line.value, fig.y.min], YAxisSide::Left).0;
            (x.is_finite() && x >= plot.left && x <= plot.right()).then_some(x)
        }
        ReferenceOrientation::Horizontal => {
            let y = project(fig, plot, [fig.x.min, line.value], YAxisSide::Left).1;
            (y.is_finite() && y >= plot.top && y <= plot.bottom()).then_some(y)
        }
    }
}

/// Place every reference-line label and author text annotation of `fig`.
/// `scale` converts points to output units. Placement runs in points and is
/// scaled afterwards, so zooming, which also changes how dense lines are
/// pooled, cannot change the choice.
pub fn label_layout(fig: &Figure, plot: Rect, scale: f32) -> LabelLayout {
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    let layout = layout_pt(fig, scale_rect(plot, 1.0 / scale));
    LabelLayout {
        reference: layout
            .reference
            .into_iter()
            .map(|label| {
                label.map(|l| ReferenceLabelBox {
                    line: l.line * scale,
                    x: l.x * scale,
                    top: l.top * scale,
                    size: l.size * scale,
                    bounds: scale_rect(l.bounds, scale),
                    ..l
                })
            })
            .collect(),
        texts: layout
            .texts
            .into_iter()
            .map(|label| {
                label.map(|l| TextLabelBox {
                    x: l.x * scale,
                    y: l.y * scale,
                    bounds: scale_rect(l.bounds, scale),
                    ..l
                })
            })
            .collect(),
    }
}

fn scale_rect(r: Rect, by: f32) -> Rect {
    Rect::new(r.left * by, r.top * by, r.width * by, r.height * by)
}

/// A text label's candidates, preferred first; a pinned text has one.
struct TextCandidates {
    boxes: Vec<TextLabelBox>,
    anchor: Point,
    /// Arrow ends this close to the anchor point at the label.
    own_end: f32,
}

/// The box around every candidate of a label that has a choice to make.
fn search_region(bounds: impl Iterator<Item = Rect>) -> Option<Rect> {
    let mut bounds = bounds.peekable();
    let first = bounds.next()?;
    bounds.peek()?;
    Some(grow(bounds.fold(first, union), CLEARANCE_PT))
}

/// [`label_layout`] in points.
fn layout_pt(fig: &Figure, plot: Rect) -> LabelLayout {
    let references: Vec<_> = fig
        .reference_lines
        .iter()
        .map(|line| reference_candidates(fig, plot, line))
        .collect();
    let texts: Vec<_> = fig
        .author_annotations
        .iter()
        .map(|a| text_candidates(fig, plot, a))
        .collect();
    // Only obstacles near a label with a choice to make are collected.
    let regions: Vec<Rect> = references
        .iter()
        .flatten()
        .filter_map(|boxes| search_region(boxes.iter().map(|b| b.bounds)))
        .chain(
            texts
                .iter()
                .flatten()
                .filter_map(|t| search_region(t.boxes.iter().map(|b| b.bounds))),
        )
        .collect();
    let mut obstacles = Obstacles::collect(fig, plot, &regions);
    let reference = references
        .into_iter()
        .map(|boxes| {
            let boxes = boxes?;
            let region = search_region(boxes.iter().map(|b| b.bounds));
            let near = obstacles.near(region, &obstacles.arrows);
            let placed = best(&boxes, |b| near.score(b.bounds, plot));
            obstacles.boxes.extend(placed.map(|b| b.bounds));
            placed
        })
        .collect();
    let texts = texts
        .into_iter()
        .map(|candidates| {
            let TextCandidates {
                boxes,
                anchor,
                own_end,
            } = candidates?;
            let arrows: Vec<_> = obstacles
                .arrows
                .iter()
                .filter_map(|&(from, to)| trim_arrow(from, to, anchor, own_end))
                .collect();
            let region = search_region(boxes.iter().map(|b| b.bounds));
            let near = obstacles.near(region, &arrows);
            let placed = best(&boxes, |b| near.score(b.bounds, plot));
            obstacles.boxes.extend(placed.map(|b| b.bounds));
            placed
        })
        .collect();
    LabelLayout { reference, texts }
}

/// Every text annotation with where it is drawn: builder labels at their
/// anchor, author labels where `labels` placed them.
pub(crate) fn drawn_texts<'a>(
    fig: &'a Figure,
    plot: Rect,
    labels: &'a LabelLayout,
    scale: f32,
) -> impl Iterator<Item = (&'a Annotation, PlacedText)> + 'a {
    let builder = fig.annotations.iter().filter_map(move |a| {
        let p = project(fig, plot, a.at, YAxisSide::Left);
        let (_, height) = text_extent(&a.text, a.size);
        crate::is_finite_point(p).then(|| (a, crate::text_place::place(a.anchor, p, height, scale)))
    });
    let author = fig
        .author_annotations
        .iter()
        .zip(&labels.texts)
        .filter_map(|(a, label)| {
            let label = label.as_ref()?;
            Some((
                a,
                PlacedText {
                    x: label.x,
                    y: label.y,
                    align: TextAlign::Center,
                    middle: false,
                },
            ))
        });
    builder.chain(author)
}

/// Estimated width and height of `text` at `size` output units.
pub(crate) fn text_extent(text: &str, size: f32) -> (f32, f32) {
    let width = text
        .lines()
        .map(|line| crate::ticks::estimated_text_width(line, size))
        .fold(0.0, f32::max);
    (
        width,
        crate::text_place::block_height(text.lines().count(), size),
    )
}

fn reference_candidates(
    fig: &Figure,
    plot: Rect,
    line: &ReferenceLine,
) -> Option<Vec<ReferenceLabelBox>> {
    if line.label.trim().is_empty() {
        return None;
    }
    let at = line_position(fig, plot, line)?;
    let size = line.label_size.unwrap_or(fig.typography.tick_pt);
    let (width, _) = text_extent(&line.label, size);
    let label = |position| reference_box(plot, line.orientation, at, position, size, width);
    if let Some(position) = line.label_position.filter(|p| p.along.is_finite()) {
        return Some(vec![label(position)]);
    }
    use ReferenceLabelSide::{After, Before};
    let candidates: Vec<ReferenceLabelPosition> = match line.orientation {
        ReferenceOrientation::Vertical => {
            // The roomier side first.
            let (near, far) = if at > plot.left + plot.width * 0.5 {
                (Before, After)
            } else {
                (After, Before)
            };
            [1.0, 0.5, 0.0]
                .into_iter()
                .flat_map(|along| [near, far].map(|side| ReferenceLabelPosition { along, side }))
                .collect()
        }
        ReferenceOrientation::Horizontal => [1.0, 0.0, 0.5]
            .into_iter()
            .flat_map(|along| [Before, After].map(|side| ReferenceLabelPosition { along, side }))
            .collect(),
    };
    Some(candidates.into_iter().map(label).collect())
}

fn reference_box(
    plot: Rect,
    orientation: ReferenceOrientation,
    at: f32,
    position: ReferenceLabelPosition,
    size: f32,
    width: f32,
) -> ReferenceLabelBox {
    let gap = REFERENCE_GAP_PT;
    let before = position.side == ReferenceLabelSide::Before;
    let (x, top, anchor, left) = match orientation {
        ReferenceOrientation::Vertical => {
            let centre = plot.bottom() - position.along * plot.height;
            let low = plot.top + gap;
            let high = (plot.bottom() - gap - size).max(low);
            let top = (centre - 0.5 * size).min(high).max(low);
            if before {
                (at - gap, top, TextAnchor::Right, at - gap - width)
            } else {
                (at + gap, top, TextAnchor::Left, at + gap)
            }
        }
        ReferenceOrientation::Horizontal => {
            let top = if before { at - gap - size } else { at + gap };
            let low = plot.left + gap;
            let high = plot.right() - gap - width;
            let left = (plot.left + position.along * plot.width - 0.5 * width)
                .min(high)
                .max(low);
            if high <= low || left >= high {
                // An overlong label keeps the right end.
                let x = plot.right() - gap;
                (x, top, TextAnchor::Right, x - width)
            } else if left <= low {
                (low, top, TextAnchor::Left, low)
            } else {
                (left + 0.5 * width, top, TextAnchor::Center, left)
            }
        }
    };
    ReferenceLabelBox {
        position,
        line: at,
        x,
        top,
        anchor,
        size,
        bounds: Rect::new(left, top, width, size),
    }
}

fn text_candidates(fig: &Figure, plot: Rect, annotation: &Annotation) -> Option<TextCandidates> {
    let anchor = project(fig, plot, annotation.at, YAxisSide::Left);
    if !crate::is_finite_point(anchor) {
        return None;
    }
    let (width, height) = text_extent(&annotation.text, annotation.size);
    let label = |offset_pt: [f32; 2]| {
        let (x, y) = (anchor.0 + offset_pt[0], anchor.1 - offset_pt[1]);
        TextLabelBox {
            x,
            y,
            offset_pt,
            bounds: Rect::new(x - 0.5 * width, y - height, width, height),
        }
    };
    let own_end = (0.5 * width).hypot(height) + plotium_figure::TextAnchor::GAP_PT;
    if let Some(offset) = annotation
        .offset_pt
        .filter(|o| o.iter().all(|v| v.is_finite()))
    {
        return Some(TextCandidates {
            boxes: vec![label(offset)],
            anchor,
            own_end,
        });
    }
    // Candidates on a half-text-height grid within three text heights,
    // nearest first; equally near ones prefer up, then right.
    let step = 0.5 * annotation.size;
    let mut steps: Vec<(i32, i32)> = (-TEXT_REACH_STEPS..=TEXT_REACH_STEPS)
        .flat_map(|i| (-TEXT_REACH_STEPS..=TEXT_REACH_STEPS).map(move |j| (i, j)))
        .filter(|(i, j)| i * i + j * j <= TEXT_REACH_STEPS * TEXT_REACH_STEPS)
        .collect();
    steps.sort_by_key(|&(i, j)| (i * i + j * j, -j, -i));
    Some(TextCandidates {
        boxes: steps
            .into_iter()
            .map(|(i, j)| label([i as f32 * step, j as f32 * step]))
            .collect(),
        anchor,
        own_end,
    })
}

/// The first clear candidate, else the least covered one (earliest on ties).
fn best<T: Copy>(candidates: &[T], score: impl Fn(&T) -> Score) -> Option<T> {
    let mut best: Option<(Score, T)> = None;
    for candidate in candidates {
        let s = score(candidate);
        if s == Score::default() {
            return Some(*candidate);
        }
        if best.is_none_or(|(b, _)| s < b) {
            best = Some((s, *candidate));
        }
    }
    best.map(|(_, candidate)| candidate)
}

/// The part of arrow `from`–`to` farther than `radius` from `anchor`.
fn trim_arrow(from: Point, to: Point, anchor: Point, radius: f32) -> Option<(Point, Point)> {
    let distance = |p: Point| (p.0 - anchor.0).hypot(p.1 - anchor.1);
    let length = (to.0 - from.0).hypot(to.1 - from.1);
    let lerp = |t: f32| (from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t);
    let cut = if length > 0.0 {
        (radius / length).min(1.0)
    } else {
        1.0
    };
    match (distance(from) <= radius, distance(to) <= radius) {
        (true, true) => None,
        (true, false) => Some((lerp(cut), to)),
        (false, true) => Some((from, lerp(1.0 - cut))),
        (false, false) => Some((from, to)),
    }
}

/// How badly a label box is covered; the derived order ranks leaving the
/// plot above overlapping another label above touching data.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
struct Score {
    outside: bool,
    boxes: usize,
    hits: usize,
}

#[derive(Default)]
struct Obstacles {
    segments: Vec<(Point, Point)>,
    polygons: Vec<Vec<Point>>,
    arrows: Vec<(Point, Point)>,
    /// Legend, colour scale, value labels and labels already placed.
    boxes: Vec<Rect>,
}

impl Obstacles {
    /// The obstacles that can touch any of `regions`, in points.
    fn collect(fig: &Figure, plot: Rect, regions: &[Rect]) -> Self {
        if regions.is_empty() {
            return Self::default();
        }
        let wanted = |a: Point, b: Point| regions.iter().any(|r| clip(a, b, *r).is_some());
        let mut segments = Vec::new();
        for_each_data_segment(fig, plot, |a, b| {
            if wanted(a, b) {
                segments.push((a, b));
            }
        });
        let placed = |p: [f64; 2]| {
            Some(project(fig, plot, p, YAxisSide::Left)).filter(|q| crate::is_finite_point(*q))
        };
        let polygons = fig
            .polygons
            .iter()
            .filter(|p| p.opacity > 0.0 && p.points.len() >= 3)
            .filter_map(|p| {
                p.points
                    .iter()
                    .map(|q| placed(*q))
                    .collect::<Option<Vec<_>>>()
            })
            .filter(|p| regions.iter().any(|r| overlaps(bounding_box(p), *r)))
            .collect();
        let arrows = fig
            .author_arrows
            .iter()
            .filter_map(|a| Some((placed(a.from)?, placed(a.to)?)))
            .collect();
        let mut boxes: Vec<Rect> = [
            crate::legend_rect(fig, plot, 1.0),
            crate::color_scale_rect(fig, plot, 1.0),
        ]
        .into_iter()
        .flatten()
        .collect();
        boxes.extend(
            fig.annotations
                .iter()
                .filter(|a| !a.text.trim().is_empty())
                .filter_map(|a| {
                    let p = placed(a.at)?;
                    let (width, height) = text_extent(&a.text, a.size);
                    let at = crate::text_place::place(a.anchor, p, height, 1.0);
                    Some(crate::text_place::bounds(at, width, height))
                }),
        );
        Self {
            segments,
            polygons,
            arrows,
            boxes,
        }
    }

    /// The obstacles that can touch `region`, with `arrows` in place of the
    /// figure's own; none without a region to search.
    fn near(&self, region: Option<Rect>, arrows: &[(Point, Point)]) -> Self {
        let Some(region) = region else {
            return Self::default();
        };
        let touches = |&&(a, b): &&(Point, Point)| clip(a, b, region).is_some();
        Self {
            segments: self.segments.iter().filter(touches).copied().collect(),
            polygons: self
                .polygons
                .iter()
                .filter(|p| overlaps(bounding_box(p), region))
                .cloned()
                .collect(),
            arrows: arrows.iter().filter(touches).copied().collect(),
            boxes: self
                .boxes
                .iter()
                .filter(|b| overlaps(**b, region))
                .copied()
                .collect(),
        }
    }

    fn score(&self, label: Rect, plot: Rect) -> Score {
        let zone = grow(label, CLEARANCE_PT);
        let tolerance = 1e-3;
        Score {
            outside: label.left < plot.left - tolerance
                || label.top < plot.top - tolerance
                || label.right() > plot.right() + tolerance
                || label.bottom() > plot.bottom() + tolerance,
            boxes: self.boxes.iter().filter(|b| overlaps(zone, **b)).count(),
            hits: self
                .segments
                .iter()
                .chain(&self.arrows)
                .filter(|(a, b)| clip(*a, *b, zone).is_some())
                .count()
                + self
                    .polygons
                    .iter()
                    .filter(|p| polygon_hits(p, zone))
                    .count(),
        }
    }
}

fn overlaps(a: Rect, b: Rect) -> bool {
    a.left < b.right() && b.left < a.right() && a.top < b.bottom() && b.top < a.bottom()
}

fn union(a: Rect, b: Rect) -> Rect {
    let left = a.left.min(b.left);
    let top = a.top.min(b.top);
    Rect::new(
        left,
        top,
        a.right().max(b.right()) - left,
        a.bottom().max(b.bottom()) - top,
    )
}

fn bounding_box(points: &[Point]) -> Rect {
    let (mut left, mut top) = (f32::INFINITY, f32::INFINITY);
    let (mut right, mut bottom) = (f32::NEG_INFINITY, f32::NEG_INFINITY);
    for &(x, y) in points {
        left = left.min(x);
        top = top.min(y);
        right = right.max(x);
        bottom = bottom.max(y);
    }
    Rect::new(left, top, right - left, bottom - top)
}

/// Whether a convex polygon touches `zone`: an edge crosses it, or the zone
/// lies inside the polygon.
fn polygon_hits(points: &[Point], zone: Rect) -> bool {
    let edges = points.iter().zip(points.iter().cycle().skip(1));
    if edges.clone().any(|(a, b)| clip(*a, *b, zone).is_some()) {
        return true;
    }
    let centre = (zone.left + 0.5 * zone.width, zone.top + 0.5 * zone.height);
    let sides: Vec<f32> = edges
        .map(|(a, b)| (b.0 - a.0) * (centre.1 - a.1) - (b.1 - a.1) * (centre.0 - a.0))
        .collect();
    sides.iter().all(|s| *s >= 0.0) || sides.iter().all(|s| *s <= 0.0)
}

#[cfg(test)]
#[path = "label_placement_tests.rs"]
mod tests;
