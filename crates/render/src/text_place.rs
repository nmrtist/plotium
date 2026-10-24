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

/// Distance between the baselines of a multi-line text, in ems.
pub(crate) const LINE_PITCH: f32 = 1.25;

/// Height of a text block of `lines` lines at `size`.
pub(crate) fn block_height(lines: usize, size: f32) -> f32 {
    size * (1.0 + LINE_PITCH * lines.saturating_sub(1) as f32)
}

/// Where to draw an annotation of `size` points and `height` points tall,
/// anchored at the projected point. `scale` converts points to output units.
pub(crate) fn place(anchor: TextAnchor, (x, y): (f32, f32), height: f32, scale: f32) -> PlacedText {
    let gap = TextAnchor::GAP_PT * scale;
    let (x, y, align, middle) = match anchor {
        TextAnchor::Point => (x, y, TextAlign::Center, false),
        TextAnchor::Above => (x, y - gap, TextAlign::Center, false),
        TextAnchor::Below => (x, y + gap + 0.5 * height * scale, TextAlign::Center, true),
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

/// Each line of `text` at `size` output units placed at `at`: a block sits
/// on its last baseline, or a `middle` block is centred on `at.y`.
pub(crate) fn lines(
    at: PlacedText,
    text: &str,
    size: f32,
) -> impl Iterator<Item = (&str, PlacedText)> {
    let count = text.lines().count();
    let pitch = LINE_PITCH * size;
    let first = if at.middle {
        at.y - 0.5 * pitch * count.saturating_sub(1) as f32
    } else {
        at.y - pitch * count.saturating_sub(1) as f32
    };
    text.lines().enumerate().map(move |(index, line)| {
        (
            line,
            PlacedText {
                y: first + pitch * index as f32,
                ..at
            },
        )
    })
}

/// Box of a placed text `width` wide and `height` high.
pub(crate) fn bounds(at: PlacedText, width: f32, height: f32) -> crate::Rect {
    let left = match at.align {
        TextAlign::Center => at.x - 0.5 * width,
        TextAlign::Start => at.x,
        TextAlign::End => at.x - width,
    };
    // A baseline-placed block rises its whole height above its last baseline.
    let top = if at.middle {
        at.y - 0.5 * height
    } else {
        at.y - height
    };
    crate::Rect::new(left, top, width, height)
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
