//! A single break in a linear axis: the values between `from` and `to` are
//! left out and the two remaining pieces sit either side of a small gap.

use crate::{Axis, AxisScale};

/// Fraction of the axis length left empty between the two pieces.
pub const AXIS_BREAK_GAP: f64 = 0.03;

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AxisBreak {
    /// Data values hidden by the break; stored in either order.
    pub from: f64,
    pub to: f64,
    /// Share of the drawn axis length given to the piece holding the lower
    /// values. `None` gives both pieces the same data-units-per-length.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lower_fraction: Option<f64>,
}

impl AxisBreak {
    pub fn new(from: f64, to: f64) -> Self {
        Self {
            from,
            to,
            lower_fraction: None,
        }
    }

    /// The hidden interval as `(low, high)`.
    pub fn bounds(&self) -> (f64, f64) {
        (self.from.min(self.to), self.from.max(self.to))
    }
}

/// A break resolved against one axis window, in the axis's raw linear
/// parameter `t = (v - min) / (max - min)` before `reversed` is applied.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct BreakMap {
    /// Raw `t` where the hidden interval starts and ends, `p < q`.
    p: f64,
    q: f64,
    /// Drawn length of the `[0, p]` piece; the gap follows it.
    first_len: f64,
}

impl BreakMap {
    /// `None` unless the hidden interval lies strictly inside the window and
    /// leaves both pieces a non-empty share of it.
    pub(crate) fn resolve(brk: &AxisBreak, min: f64, max: f64) -> Option<Self> {
        let span = max - min;
        let (low, high) = brk.bounds();
        if !span.is_finite() || span == 0.0 || !low.is_finite() || !high.is_finite() {
            return None;
        }
        let (ta, tb) = ((low - min) / span, (high - min) / span);
        let (p, q) = (ta.min(tb), ta.max(tb));
        if p <= 0.0 || q >= 1.0 || q <= p {
            return None;
        }
        let drawn = 1.0 - AXIS_BREAK_GAP;
        let natural = p / (p + 1.0 - q);
        // The `[0, p]` piece holds the lower values only when min < max.
        let first_share = match brk.lower_fraction {
            Some(f) if f.is_finite() && f > 0.0 && f < 1.0 => {
                if span > 0.0 {
                    f
                } else {
                    1.0 - f
                }
            }
            _ => natural,
        };
        Some(Self {
            p,
            q,
            first_len: drawn * first_share,
        })
    }

    pub(crate) fn forward(&self, t: f64) -> f64 {
        let second_start = self.first_len + AXIS_BREAK_GAP;
        if t <= self.p {
            t / self.p * self.first_len
        } else if t >= self.q {
            second_start + (t - self.q) / (1.0 - self.q) * (1.0 - second_start)
        } else {
            self.first_len + (t - self.p) / (self.q - self.p) * AXIS_BREAK_GAP
        }
    }

    pub(crate) fn inverse(&self, n: f64) -> f64 {
        let second_start = self.first_len + AXIS_BREAK_GAP;
        if n <= self.first_len {
            n / self.first_len * self.p
        } else if n >= second_start {
            self.q + (n - second_start) / (1.0 - second_start) * (1.0 - self.q)
        } else {
            self.p + (n - self.first_len) / AXIS_BREAK_GAP * (self.q - self.p)
        }
    }

    /// The gap as raw normalized positions `(start, end)`.
    pub(crate) fn gap(&self) -> (f64, f64) {
        (self.first_len, self.first_len + AXIS_BREAK_GAP)
    }
}

impl Axis {
    /// The break drawn on this axis, if its settings apply to the window.
    pub fn active_break(&self) -> Option<AxisBreak> {
        self.break_map().and(self.axis_break)
    }

    /// The gap of an active break as normalized positions `(start, end)`,
    /// `start < end`, after `reversed` is applied.
    pub fn break_gap(&self) -> Option<(f64, f64)> {
        let (a, b) = self.break_map()?.gap();
        Some(if self.reversed {
            (1.0 - b, 1.0 - a)
        } else {
            (a, b)
        })
    }

    /// The two visible value windows of an active break, each `(low, high)`,
    /// lower values first.
    pub fn break_pieces(&self) -> Option<[(f64, f64); 2]> {
        let (low, high) = self.active_break()?.bounds();
        let (lo, hi) = (self.min.min(self.max), self.min.max(self.max));
        Some([(lo, low), (high, hi)])
    }

    /// True when an active break hides `v`.
    pub fn hides(&self, v: f64) -> bool {
        self.active_break().is_some_and(|brk| {
            let (low, high) = brk.bounds();
            v > low && v < high
        })
    }

    /// Raw linear position `t` → drawn position, honoring an active break.
    pub(crate) fn broken(&self, t: f64) -> f64 {
        self.break_map().map_or(t, |map| map.forward(t))
    }

    /// Drawn position → raw linear position `t`; inverse of [`Axis::broken`].
    pub(crate) fn unbroken(&self, n: f64) -> f64 {
        self.break_map().map_or(n, |map| map.inverse(n))
    }

    pub(crate) fn break_map(&self) -> Option<BreakMap> {
        let brk = self.axis_break.as_ref()?;
        if self.categories.is_some() || self.effective_scale() != AxisScale::Linear {
            return None;
        }
        BreakMap::resolve(brk, self.min, self.max)
    }
}
