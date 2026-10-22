//! Min/max envelope pooling for dense line series, shared by the screen
//! renderer and preview SVG export so both thin a trace the same way.
use std::borrow::Cow;

pub(crate) struct LinePoints<'a> {
    pub points: Cow<'a, [[f64; 2]]>,
    /// Read by the screen renderer's statistics only.
    #[cfg_attr(not(feature = "screen"), allow(dead_code))]
    pub source_points_visited: usize,
    pub pooled: bool,
}

/// Clip to the viewport, then pool dense lines into min/max envelope buckets.
pub(crate) fn line_points(
    points: &[[f64; 2]],
    x_min: f64,
    x_max: f64,
    columns: usize,
) -> LinePoints<'_> {
    let visible = visible_line_points(points, x_min, x_max);
    let source_points_visited = visible.len();
    if visible.len() <= columns.saturating_mul(2) {
        return LinePoints {
            points: Cow::Borrowed(visible),
            source_points_visited,
            pooled: false,
        };
    }
    let bucket_size = visible.len().div_ceil(columns.max(1));
    pooled_line(visible, sample_buckets(visible, bucket_size), false)
}

/// [`line_points`] for a plot with a log axis. On log x the buckets
/// are output columns rather than equal sample counts, so linearly sampled
/// data keeps its detail in the low decades; on log y the envelope ignores
/// samples without a position, which would otherwise break the whole bucket.
pub(crate) fn scaled_line_points<'a>(
    points: &'a [[f64; 2]],
    x_axis: &plotium_figure::Axis,
    y_log: bool,
    columns: usize,
) -> LinePoints<'a> {
    let x_log = !x_axis.effective_scale().is_linear();
    let (x_min, x_max) = (x_axis.min.min(x_axis.max), x_axis.min.max(x_axis.max));
    if !x_log && !y_log {
        return line_points(points, x_min, x_max, columns);
    }
    let visible = visible_line_points(points, x_min, x_max);
    if visible.len() <= columns.saturating_mul(2) {
        return LinePoints {
            points: Cow::Borrowed(visible),
            source_points_visited: visible.len(),
            pooled: false,
        };
    }
    let columns = columns.max(1);
    if x_log {
        let column = |point: &[f64; 2]| {
            let n = x_axis.normalize(point[0]);
            n.is_finite()
                .then(|| (n * columns as f64).floor().clamp(0.0, columns as f64) as usize)
        };
        pooled_line(
            visible,
            visible.chunk_by(|a, b| {
                is_finite(a) && is_finite(b) && column(a).is_some() && column(a) == column(b)
            }),
            y_log,
        )
    } else {
        let bucket_size = visible.len().div_ceil(columns);
        pooled_line(visible, sample_buckets(visible, bucket_size), y_log)
    }
}

fn is_finite(point: &[f64; 2]) -> bool {
    point[0].is_finite() && point[1].is_finite()
}

/// Equal-count buckets that never span a gap: each run of finite samples is
/// chunked on its own, and each run of gap samples is one bucket.
fn sample_buckets(visible: &[[f64; 2]], bucket_size: usize) -> impl Iterator<Item = &[[f64; 2]]> {
    visible
        .chunk_by(|a, b| is_finite(a) == is_finite(b))
        .flat_map(move |run| {
            run.chunks(if is_finite(&run[0]) {
                bucket_size
            } else {
                run.len()
            })
        })
}

/// Envelope of each bucket, in source order, framed by the first and last
/// visible samples. A bucket without a finite (with `positive_only`, positive)
/// sample keeps its first one, which then breaks the stroke as the source would.
fn pooled_line<'a, 'b>(
    visible: &'a [[f64; 2]],
    buckets: impl Iterator<Item = &'b [[f64; 2]]>,
    positive_only: bool,
) -> LinePoints<'a> {
    let source_points_visited = visible.len();
    let mut pooled = Vec::new();
    pooled.push(visible[0]);
    for bucket in buckets {
        let candidate = |index: &usize| {
            is_finite(&bucket[*index]) && (!positive_only || bucket[*index][1] > 0.0)
        };
        let Some(first) = (0..bucket.len()).find(candidate) else {
            pooled.push(bucket[0]);
            continue;
        };
        let mut min_index = first;
        let mut max_index = first;
        for index in (first + 1..bucket.len()).filter(candidate) {
            if bucket[index][1] < bucket[min_index][1] {
                min_index = index;
            }
            if bucket[index][1] > bucket[max_index][1] {
                max_index = index;
            }
        }
        if min_index <= max_index {
            pooled.push(bucket[min_index]);
            if max_index != min_index {
                pooled.push(bucket[max_index]);
            }
        } else {
            pooled.push(bucket[max_index]);
            pooled.push(bucket[min_index]);
        }
    }
    if let Some(last) = visible.last()
        && pooled.last() != Some(last)
    {
        pooled.push(*last);
    }
    LinePoints {
        points: Cow::Owned(pooled),
        source_points_visited,
        pooled: true,
    }
}

fn visible_line_points(points: &[[f64; 2]], x_min: f64, x_max: f64) -> &[[f64; 2]] {
    // Keep one neighbour on each side for continuity. Handles ascending traces
    // (time) and descending ones (NMR ppm); a flat or non-monotonic series keeps
    // its whole extent, which is safe but less selective.
    let monotonic = points.windows(2).all(|pair| pair[0][0] <= pair[1][0])
        || points.windows(2).all(|pair| pair[0][0] >= pair[1][0]);
    if !monotonic {
        return points;
    }
    let first_x = points.first().map(|p| p[0]);
    let last_x = points.last().map(|p| p[0]);
    let (start, end) = match (first_x, last_x) {
        (Some(first), Some(last)) if first < last => {
            let start = points
                .partition_point(|point| point[0] < x_min)
                .saturating_sub(1);
            let end = points
                .partition_point(|point| point[0] <= x_max)
                .saturating_add(1)
                .min(points.len());
            (start.min(end), end)
        }
        (Some(first), Some(last)) if first > last => {
            let start = points
                .partition_point(|point| point[0] > x_max)
                .saturating_sub(1);
            let end = points
                .partition_point(|point| point[0] >= x_min)
                .saturating_add(1)
                .min(points.len());
            (start.min(end), end)
        }
        _ => (0, points.len()),
    };
    &points[start..end]
}
