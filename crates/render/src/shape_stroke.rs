//! Outlines of small filled rectangles, judged at their printed size.
use plotium_figure::Color;

/// Narrowest rectangle, in points on paper, that keeps a background-colored
/// separator: below this the white seam eats most of the bar.
const MIN_SEPARATED_WIDTH_PT: f32 = 3.0;
/// A rectangle narrower than this many outline widths is mostly outline, so it
/// is drawn as a plain fill instead.
const MIN_WIDTH_PER_OUTLINE: f32 = 4.0;
/// A rectangle shorter than this many outline widths would show its top and
/// bottom outlines merged into one heavy rule (a near-zero bar), so it too is
/// drawn as a plain fill.
const MIN_HEIGHT_PER_OUTLINE: f32 = 2.0;

/// The stroke to draw around a projected filled shape. Only axis-aligned
/// rectangles (bars, histogram bins) too small for their outline lose it;
/// every other shape, and every rectangle large enough, keeps `stroke`. A
/// translucent shape (a box plot's box) keeps its outline, which carries it.
/// `scale` converts points to the outline's output units.
pub(crate) fn shape_stroke(
    outline: &[(f32, f32)],
    stroke: Option<(Color, f32)>,
    opacity: f32,
    background: Color,
    scale: f32,
) -> Option<(Color, f32)> {
    let (color, width) = stroke?;
    let Some((w, h)) = rect_size(outline) else {
        return stroke;
    };
    let (w, h) = (w / scale, h / scale);
    let keep = if color == background {
        w >= MIN_SEPARATED_WIDTH_PT
    } else if opacity < 1.0 {
        true
    } else {
        w >= MIN_WIDTH_PER_OUTLINE * width && h >= MIN_HEIGHT_PER_OUTLINE * width
    };
    keep.then_some((color, width))
}

/// Width and height of an axis-aligned rectangle given as four corners in
/// drawing order, or `None` for any other shape.
fn rect_size(outline: &[(f32, f32)]) -> Option<(f32, f32)> {
    let [a, b, c, d] = outline else {
        return None;
    };
    let aligned = |p: (f32, f32), q: (f32, f32)| p.0 == q.0 || p.1 == q.1;
    if !(aligned(*a, *b) && aligned(*b, *c) && aligned(*c, *d) && aligned(*d, *a)) {
        return None;
    }
    let xs = [a.0, b.0, c.0, d.0];
    let ys = [a.1, b.1, c.1, d.1];
    let span = |v: [f32; 4]| {
        v.iter().fold(f32::NEG_INFINITY, |m, x| m.max(*x))
            - v.iter().fold(f32::INFINITY, |m, x| m.min(*x))
    };
    Some((span(xs), span(ys)))
}

#[cfg(test)]
mod tests {
    use super::*;

    const INK: Color = Color::BLACK;
    const WHITE: Color = Color::rgb(255, 255, 255);

    fn rect(w: f32, h: f32) -> [(f32, f32); 4] {
        [
            (10.0, 50.0),
            (10.0 + w, 50.0),
            (10.0 + w, 50.0 - h),
            (10.0, 50.0 - h),
        ]
    }

    #[test]
    fn ordinary_bars_keep_their_outline() {
        let outline = Some((INK, 0.5));
        assert_eq!(
            shape_stroke(&rect(14.0, 40.0), outline, 1.0, WHITE, 1.0),
            outline
        );
        let separator = Some((WHITE, 0.5));
        assert_eq!(
            shape_stroke(&rect(14.0, 40.0), separator, 1.0, WHITE, 1.0),
            separator
        );
    }

    #[test]
    fn narrow_bars_drop_separators_and_outlines() {
        assert_eq!(
            shape_stroke(&rect(2.5, 40.0), Some((WHITE, 0.5)), 1.0, WHITE, 1.0),
            None
        );
        assert_eq!(
            shape_stroke(&rect(1.8, 40.0), Some((INK, 0.5)), 1.0, WHITE, 1.0),
            None
        );
        assert!(shape_stroke(&rect(2.0, 40.0), Some((INK, 0.5)), 1.0, WHITE, 1.0).is_some());
    }

    #[test]
    fn near_zero_bars_drop_the_merged_outline() {
        assert_eq!(
            shape_stroke(&rect(14.0, 0.3), Some((INK, 0.5)), 1.0, WHITE, 1.0),
            None
        );
        assert_eq!(
            shape_stroke(&rect(14.0, 0.0), Some((INK, 0.5)), 1.0, WHITE, 1.0),
            None
        );
        assert!(shape_stroke(&rect(14.0, 1.0), Some((INK, 0.5)), 1.0, WHITE, 1.0).is_some());
    }

    #[test]
    fn sizes_are_judged_in_points_and_other_shapes_are_untouched() {
        // A 6 px wide bar on a 4 px/pt screen is 1.5 pt wide on paper.
        assert_eq!(
            shape_stroke(&rect(6.0, 160.0), Some((INK, 0.5)), 1.0, WHITE, 4.0),
            None
        );
        let triangle = [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0)];
        let stroke = Some((INK, 0.3));
        assert_eq!(shape_stroke(&triangle, stroke, 1.0, WHITE, 1.0), stroke);
        // A translucent box keeps the outline that carries it.
        assert_eq!(
            shape_stroke(&rect(14.0, 0.3), stroke, 0.45, WHITE, 1.0),
            stroke
        );
        let rotated = [(0.0, 1.0), (1.0, 0.0), (2.0, 1.0), (1.0, 2.0)];
        assert_eq!(shape_stroke(&rotated, stroke, 1.0, WHITE, 1.0), stroke);
    }
}
