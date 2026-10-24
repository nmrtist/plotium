//! Snapping by plot area. Frames of equal size can hold plot areas that do not
//! line up, because their tick labels differ in width; a dragged frame also
//! pulls its own plot-area edges onto the plot-area edges of other plots.
use super::{MovableEdges, SnapGuide, SnapTargets};
use crate::state::ObjectFrame;

/// Distances from a frame's edges in to its plot area, in pt.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PlotInsets {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl PlotInsets {
    /// How far `plot` sits inside `frame`.
    pub fn of(frame: ObjectFrame, plot: ObjectFrame) -> Self {
        Self {
            left: plot.x - frame.x,
            top: plot.y - frame.y,
            right: frame.x + frame.width - (plot.x + plot.width),
            bottom: frame.y + frame.height - (plot.y + plot.height),
        }
    }
}

/// Plot-area edges a dragged frame's own plot area snaps to.
#[derive(Clone, Debug, Default)]
pub struct PlotEdges {
    pub xs: Vec<f32>,
    pub ys: Vec<f32>,
}

impl PlotEdges {
    pub fn push(&mut self, plot: ObjectFrame) {
        self.xs.extend([plot.x, plot.x + plot.width]);
        self.ys.extend([plot.y, plot.y + plot.height]);
    }
}

/// A dragged frame's plot insets and the plot edges they snap to.
pub type PlotSnap<'a> = Option<(PlotInsets, &'a PlotEdges)>;

/// The nearest pull within `threshold` of any edge onto a target of its list,
/// as `(delta_to_apply, snapped_target_pos)`. Within the first (outer frame)
/// pair the last of equal pulls wins; a plot-area pull must be strictly
/// nearer, so the outer frame keeps priority.
fn nearest(pairs: &[(&[f32], &[f32])], threshold: f32) -> Option<(f32, f32)> {
    let mut best_dist = threshold;
    let mut result = None;
    for (index, (edges, targets)) in pairs.iter().enumerate() {
        for &e in *edges {
            for &t in *targets {
                let dist = (e - t).abs();
                if dist < best_dist || (index == 0 && dist == best_dist) {
                    best_dist = dist;
                    result = Some((t - e, t));
                }
            }
        }
    }
    result
}

/// Snap a moving frame by translating it so its nearest edge or centre lands
/// on a target line, or its plot-area edge on another plot's, independently
/// per axis. Returns the corrected frame and the guide lines to draw.
pub fn snap_move_with_plot(
    frame: ObjectFrame,
    targets: &SnapTargets,
    plot: PlotSnap,
    threshold: f32,
) -> (ObjectFrame, Vec<SnapGuide>) {
    let (left, top) = (frame.x, frame.y);
    let (right, bottom) = (left + frame.width, top + frame.height);
    let outer_x = [left, left + frame.width * 0.5, right];
    let outer_y = [top, top + frame.height * 0.5, bottom];
    let (plot_x, plot_y, edges) = match plot {
        Some((insets, edges)) => (
            vec![left + insets.left, right - insets.right],
            vec![top + insets.top, bottom - insets.bottom],
            Some(edges),
        ),
        None => (Vec::new(), Vec::new(), None),
    };
    let none: &[f32] = &[];
    let (plot_xs, plot_ys) = edges.map_or((none, none), |e| (&e.xs[..], &e.ys[..]));
    let mut out = frame;
    let mut guides = Vec::new();
    if let Some((dx, pos)) = nearest(&[(&outer_x, &targets.xs), (&plot_x, plot_xs)], threshold) {
        out.x += dx;
        guides.push(SnapGuide {
            vertical: true,
            pos,
        });
    }
    if let Some((dy, pos)) = nearest(&[(&outer_y, &targets.ys), (&plot_y, plot_ys)], threshold) {
        out.y += dy;
        guides.push(SnapGuide {
            vertical: false,
            pos,
        });
    }
    (out, guides)
}

/// Snap a resizing frame by pulling only its moving edges (or the plot-area
/// edges inside them) onto targets, keeping the anchored edges fixed. A snap
/// that would shrink the frame below `min_size` in that axis is skipped.
pub fn snap_resize_with_plot(
    frame: ObjectFrame,
    edges: MovableEdges,
    targets: &SnapTargets,
    plot: PlotSnap,
    threshold: f32,
    min_size: f32,
) -> (ObjectFrame, Vec<SnapGuide>) {
    let insets = plot.map(|(insets, _)| insets).unwrap_or_default();
    let none: &[f32] = &[];
    let (plot_xs, plot_ys) = plot.map_or((none, none), |(_, e)| (&e.xs[..], &e.ys[..]));
    let has_plot = plot.is_some();
    let mut lo = [frame.x, frame.y];
    let mut hi = [frame.x + frame.width, frame.y + frame.height];
    let mut guides = Vec::new();
    for (axis, moves_lo, moves_hi, inset_lo, inset_hi, outer_targets, plot_targets) in [
        (
            0,
            edges.left,
            edges.right,
            insets.left,
            insets.right,
            &targets.xs[..],
            plot_xs,
        ),
        (
            1,
            edges.top,
            edges.bottom,
            insets.top,
            insets.bottom,
            &targets.ys[..],
            plot_ys,
        ),
    ] {
        // Corner handles move at most one edge per axis.
        let edge = if moves_lo {
            lo[axis]
        } else if moves_hi {
            hi[axis]
        } else {
            continue;
        };
        let inner = if moves_lo {
            edge + inset_lo
        } else {
            edge - inset_hi
        };
        let inner: &[f32] = if has_plot { &[inner] } else { &[] };
        let Some((delta, pos)) = nearest(
            &[(&[edge], outer_targets), (inner, plot_targets)],
            threshold,
        ) else {
            continue;
        };
        let (new_lo, new_hi) = if moves_lo {
            (lo[axis] + delta, hi[axis])
        } else {
            (lo[axis], hi[axis] + delta)
        };
        if new_hi - new_lo >= min_size {
            lo[axis] = new_lo;
            hi[axis] = new_hi;
            guides.push(SnapGuide {
                vertical: axis == 0,
                pos,
            });
        }
    }
    (
        ObjectFrame::new(lo[0], lo[1], hi[0] - lo[0], hi[1] - lo[1]),
        guides,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edges_at(xs: &[f32]) -> PlotEdges {
        PlotEdges {
            xs: xs.to_vec(),
            ys: Vec::new(),
        }
    }

    #[test]
    fn a_moving_frame_lines_its_plot_area_up_with_another_plot() {
        // A sibling frame starts at x = 0 with its plot area at 30; this frame's
        // tick labels are narrower, so its plot area sits 25 pt in.
        let mut targets = SnapTargets::default();
        targets.xs.push(0.0);
        let plots = edges_at(&[30.0, 200.0]);
        let insets = PlotInsets {
            left: 25.0,
            ..Default::default()
        };
        let frame = ObjectFrame::new(4.0, 300.0, 170.0, 100.0);
        let (snapped, guides) = snap_move_with_plot(frame, &targets, Some((insets, &plots)), 6.0);
        assert_eq!(snapped.x, 5.0);
        assert_eq!(
            guides,
            vec![SnapGuide {
                vertical: true,
                pos: 30.0
            }]
        );
        // Nearer the frame edge, the frame edge wins.
        let frame = ObjectFrame::new(1.0, 300.0, 170.0, 100.0);
        let (snapped, _) = snap_move_with_plot(frame, &targets, Some((insets, &plots)), 6.0);
        assert_eq!(snapped.x, 0.0);
    }

    #[test]
    fn a_resized_edge_snaps_its_plot_area_and_keeps_the_anchor() {
        let targets = SnapTargets::default();
        let plots = edges_at(&[200.0]);
        let insets = PlotInsets {
            right: 8.0,
            ..Default::default()
        };
        let frame = ObjectFrame::new(10.0, 0.0, 195.0, 80.0);
        let edges = MovableEdges {
            right: true,
            ..Default::default()
        };
        let (snapped, guides) =
            snap_resize_with_plot(frame, edges, &targets, Some((insets, &plots)), 6.0, 24.0);
        assert_eq!(snapped, ObjectFrame::new(10.0, 0.0, 198.0, 80.0));
        assert_eq!(guides.len(), 1);
    }
}
