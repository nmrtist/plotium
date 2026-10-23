//! Placement of anchored text annotations, shared by every backend.
use plotium_figure::TextAnchor;

/// Horizontal alignment of a placed annotation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TextAlign {
    Center,
    Start,
    End,
}

/// Output position of an annotation's text: `middle` texts are centred
/// vertically on `y`; the others sit on their baseline there.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PlacedText {
    pub x: f32,
    pub y: f32,
    pub align: TextAlign,
    pub middle: bool,
}

/// Where to draw an annotation of `size` points anchored at the projected
/// point. `scale` converts points to output units.
pub(crate) fn place(anchor: TextAnchor, (x, y): (f32, f32), size: f32, scale: f32) -> PlacedText {
    let gap = TextAnchor::GAP_PT * scale;
    let (x, y, align, middle) = match anchor {
        TextAnchor::Point => (x, y, TextAlign::Center, false),
        TextAnchor::Above => (x, y - gap, TextAlign::Center, false),
        TextAnchor::Below => (x, y + gap + 0.5 * size * scale, TextAlign::Center, true),
        TextAnchor::Left => (x - gap, y, TextAlign::End, true),
        TextAnchor::Right => (x + gap, y, TextAlign::Start, true),
    };
    PlacedText {
        x,
        y,
        align,
        middle,
    }
}

/// Box of a placed text `width` wide and one `size` high.
pub(crate) fn bounds(at: PlacedText, width: f32, size: f32) -> crate::Rect {
    let left = match at.align {
        TextAlign::Center => at.x - 0.5 * width,
        TextAlign::Start => at.x,
        TextAlign::End => at.x - width,
    };
    // A baseline-placed text rises one font size above its baseline.
    let top = if at.middle {
        at.y - 0.5 * size
    } else {
        at.y - size
    };
    crate::Rect::new(left, top, width, size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_keep_a_gap_on_their_side_and_points_stay_put() {
        assert_eq!(
            place(TextAnchor::Point, (10.0, 20.0), 7.0, 1.0),
            PlacedText {
                x: 10.0,
                y: 20.0,
                align: TextAlign::Center,
                middle: false
            }
        );
        assert_eq!(place(TextAnchor::Above, (10.0, 20.0), 7.0, 1.0).y, 17.0);
        let below = place(TextAnchor::Below, (10.0, 20.0), 7.0, 2.0);
        assert_eq!((below.y, below.middle), (20.0 + 6.0 + 7.0, true));
        assert_eq!(
            place(TextAnchor::Left, (10.0, 20.0), 7.0, 1.0).align,
            TextAlign::End
        );
        assert_eq!(place(TextAnchor::Right, (10.0, 20.0), 7.0, 1.0).x, 13.0);
    }
}
