//! Output-space geometry for author arrows, shared so every backend draws the
//! same shaft and heads.
use crate::arrow_head;
use plotium_figure::ArrowHead;

type Point = (f32, f32);

/// One arrowhead as `[barb, tip, barb]`: a filled triangle or an open V.
pub(crate) struct ArrowTip {
    pub points: [Point; 3],
    pub filled: bool,
}

pub(crate) struct ArrowShape {
    pub shaft: [Point; 2],
    pub tips: Vec<ArrowTip>,
}

/// Fraction of a filled head's depth the shaft runs into. Stopping inside the
/// triangle rather than at its base hides the shaft's end cap without leaving
/// an anti-aliasing seam.
const SHAFT_INTO_HEAD: f32 = 0.2;
/// Shaft width (pt) up to which heads keep the page-arrow size. Beyond it the
/// head grows in proportion, so its base (about 3.8 × this width) always
/// stays wider than the shaft.
const PAGE_HEAD_MAX_WIDTH: f32 = 3.5;

/// Lay out an arrow from `tail` to `head` in output units, with heads sized like
/// page arrow shapes for a `width` pt shaft (`scale` converts pt to output
/// units). Returns `None` when the ends coincide or are not finite.
pub(crate) fn arrow_shape(
    tail: Point,
    head: Point,
    width: f32,
    scale: f32,
    style: ArrowHead,
) -> Option<ArrowShape> {
    let head_scale = scale * (width / PAGE_HEAD_MAX_WIDTH).max(1.0);
    let length = (head.0 - tail.0).hypot(head.1 - tail.1);
    if !length.is_finite() || length <= 1e-3 {
        return None;
    }
    let tip = |origin: Point, at: Point, filled: bool| {
        let [a, b] = arrow_head(origin, at, head_scale);
        ArrowTip {
            points: [a, at, b],
            filled,
        }
    };
    let shaft_end = |tip: &ArrowTip| {
        let [a, at, b] = tip.points;
        let base = ((a.0 + b.0) * 0.5, (a.1 + b.1) * 0.5);
        let t = 1.0 - SHAFT_INTO_HEAD;
        (at.0 + (base.0 - at.0) * t, at.1 + (base.1 - at.1) * t)
    };
    Some(match style {
        ArrowHead::None => ArrowShape {
            shaft: [tail, head],
            tips: Vec::new(),
        },
        ArrowHead::Open => ArrowShape {
            shaft: [tail, head],
            tips: vec![tip(tail, head, false)],
        },
        ArrowHead::Filled => {
            let end = tip(tail, head, true);
            ArrowShape {
                shaft: [tail, shaft_end(&end)],
                tips: vec![end],
            }
        }
        ArrowHead::Double => {
            let (end, start) = (tip(tail, head, true), tip(head, tail, true));
            ArrowShape {
                shaft: [shaft_end(&start), shaft_end(&end)],
                tips: vec![end, start],
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filled_head_ends_at_the_head_point_and_hides_the_shaft_end() {
        let shape = arrow_shape((0.0, 0.0), (100.0, 0.0), 1.0, 1.0, ArrowHead::Filled)
            .expect("non-degenerate arrow");
        assert_eq!(shape.tips.len(), 1);
        let tip = &shape.tips[0];
        assert!(tip.filled);
        assert_eq!(tip.points[1], (100.0, 0.0));
        let base = tip.points[0].0;
        assert!(base < 100.0);
        assert!(shape.shaft[1].0 > base && shape.shaft[1].0 < 100.0);
    }

    #[test]
    fn double_heads_point_outward() {
        let shape = arrow_shape((0.0, 0.0), (0.0, 40.0), 1.0, 1.0, ArrowHead::Double)
            .expect("non-degenerate arrow");
        assert_eq!(shape.tips.len(), 2);
        assert_eq!(shape.tips[0].points[1], (0.0, 40.0));
        assert_eq!(shape.tips[1].points[1], (0.0, 0.0));
        assert!(shape.tips[0].points[0].1 < 40.0);
        assert!(shape.tips[1].points[0].1 > 0.0);
        assert!(shape.shaft[0].1 > 0.0 && shape.shaft[1].1 < 40.0);
    }

    #[test]
    fn plain_and_open_arrows_keep_the_full_shaft() {
        for style in [ArrowHead::None, ArrowHead::Open] {
            let shape =
                arrow_shape((1.0, 1.0), (5.0, 4.0), 1.0, 2.0, style).expect("non-degenerate arrow");
            assert_eq!(shape.shaft, [(1.0, 1.0), (5.0, 4.0)]);
            assert!(shape.tips.iter().all(|t| !t.filled));
        }
    }

    #[test]
    fn thick_shafts_get_heads_wider_than_the_shaft() {
        for width in [1.0, 3.5, 8.0, 20.0] {
            let shape = arrow_shape((0.0, 0.0), (400.0, 0.0), width, 1.0, ArrowHead::Filled)
                .expect("non-degenerate arrow");
            let [a, _, b] = shape.tips[0].points;
            assert!((a.1 - b.1).abs() > width * 2.0, "{width}");
        }
    }

    #[test]
    fn degenerate_arrows_are_skipped() {
        assert!(arrow_shape((1.0, 1.0), (1.0, 1.0), 1.0, 1.0, ArrowHead::Filled).is_none());
        assert!(arrow_shape((f32::NAN, 1.0), (1.0, 1.0), 1.0, 1.0, ArrowHead::None).is_none());
    }
}
