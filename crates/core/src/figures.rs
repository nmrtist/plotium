//! Figure builders: turning processed 1D/2D and pseudo-2D data into renderable [`Figure`]s.

use plotium_analysis::diffusion::DiffusionMap;
use plotium_analysis::ilt::IltResult;
use plotium_analysis::robust::deplaned_location_scale;
use plotium_figure::{
    Annotation, Axis, AxisFrame, Color, Contour, ContourBasePolicy, ContourLevelSpec, ContourSpec,
    Figure, Series,
};
use plotium_io::nmr_view::NmrSource;
use plotium_processing::{Preset2D, Processed1D, Spectrum, Spectrum2D, StackSpectrum, TimeTrace};

use crate::state::{
    FieldSummary, FiniteF64, ResolvedPeak, default_contour_spec, scalar_grid_capabilities,
};

pub fn build_figure(data: &NmrSource, spec: &Spectrum, peaks: &[ResolvedPeak]) -> Figure {
    let (ppm_lo, ppm_hi) = spec.ppm_bounds();
    let (i_lo, i_hi) = spec.intensity_bounds();
    let range = (i_hi - i_lo).max(f64::MIN_POSITIVE);
    // Pad the intensity range, with extra headroom on top for peak labels.
    let y = Axis::new("Intensity (a.u.)", i_lo - 0.05 * range, i_hi + 0.08 * range);
    // NMR convention: chemical shift increases to the left.
    let x = Axis::new(
        if spec.unit == nmr::axis::AxisUnit::Ppm {
            axis_label(data.nucleus())
        } else {
            "Frequency (Hz)".into()
        },
        ppm_lo,
        ppm_hi,
    )
    .reversed(spec.unit == nmr::axis::AxisUnit::Ppm);

    let fig = Figure::new(
        format!("{} spectrum — {}", data.nucleus(), data.source()),
        x,
        y,
    )
    .with_series(Series::line("real", spec.real_points()).colored(Color::TRACE));

    apply_peak_labels(fig, peaks)
}

pub fn build_time_figure(data: &NmrSource, trace: &TimeTrace) -> Figure {
    let (time_lo, time_hi) = trace.time_bounds();
    let mut intensity = trace.values.iter().map(|value| value.re);
    let first = intensity.next().unwrap_or(0.0);
    let (minimum, maximum) = intensity.fold((first, first), |(minimum, maximum), value| {
        (minimum.min(value), maximum.max(value))
    });
    let range = (maximum - minimum).max(f64::MIN_POSITIVE);
    let x = Axis::new("Acquisition time (s)", time_lo, time_hi);
    let y = Axis::new(
        "Signal (a.u.)",
        minimum - 0.05 * range,
        maximum + 0.05 * range,
    );
    Figure::new(format!("{} FID — {}", data.nucleus(), data.source()), x, y)
        .with_series(Series::line("real", trace.real_points()).colored(Color::TRACE))
}

pub fn build_processed_1d_figure(
    data: &NmrSource,
    processed: &Processed1D,
    peaks: &[ResolvedPeak],
) -> Figure {
    match processed {
        Processed1D::Time(trace) => build_time_figure(data, trace),
        Processed1D::Frequency(spectrum) => build_figure(data, spectrum, peaks),
    }
}

pub fn apply_peak_labels(mut fig: Figure, peaks: &[ResolvedPeak]) -> Figure {
    for peak in peaks {
        fig = fig.with_annotation(Annotation {
            display_baseline: 0.0,
            text: peak.label.clone(),
            at: [peak.x, peak.y],
            color: Color::rgb(0x8a, 0x1c, 0x1c),
            size: 12.0,
            anchor: plotium_figure::TextAnchor::Point,
            offset_pt: None,
        });
    }
    fig
}

/// Build the non-geometric shell of a processed true-2D figure. Contour
/// geometry is supplied later by the versioned field cache; this convenience
/// builder intentionally never runs marching squares on the caller's thread.
pub fn build_figure_2d(spec: &Spectrum2D, preset: Preset2D) -> Figure {
    let (f2_lo, f2_hi) = spec.f2_bounds();
    let (f1_lo, f1_hi) = spec.f1_bounds();
    let x = nmr_axis(&spec.direct, f2_lo, f2_hi);
    let y = nmr_axis(&spec.indirect, f1_lo, f1_hi);

    let mut fig = Figure::new(format!("{} — {}", preset.label(), spec.source), x, y)
        .with_axis_frame(AxisFrame::Box);
    fig.lock_aspect = equal_scale_for_nmr_2d(spec);
    fig
}

/// Whether an imported true-2D spectrum has commensurate frequency axes whose
/// full ranges remain useful when rendered with equal data units per pixel.
pub(crate) fn equal_scale_for_nmr_2d(spec: &Spectrum2D) -> bool {
    if spec.f2_domain != plotium_io::Domain::Frequency
        || spec.f1_domain != plotium_io::Domain::Frequency
        || spec.direct.nucleus.is_empty()
        || spec.direct.nucleus != spec.indirect.nucleus
        || spec.direct.unit != spec.indirect.unit
    {
        return false;
    }
    let (f2_lo, f2_hi) = spec.f2_bounds();
    let (f1_lo, f1_hi) = spec.f1_bounds();
    let f2_span = (f2_hi - f2_lo).abs();
    let f1_span = (f1_hi - f1_lo).abs();
    let narrow = f2_span.min(f1_span);
    let wide = f2_span.max(f1_span);
    narrow.is_finite() && wide.is_finite() && narrow > 0.0 && wide / narrow <= 2.0
}

/// Resolve levels for the existing DOSY/ILT analysis-map workers. Ordinary
/// `FieldPayload::ScalarGrid2D` contours use the versioned field resolver and
/// never reach this legacy analysis-only helper on the UI thread.
fn contour_levels(
    values: &[f32],
    rows: usize,
    cols: usize,
    level: &ContourLevelSpec,
    negative: bool,
) -> Vec<f64> {
    let Some((minimum, maximum)) = finite_range(values) else {
        return Vec::new();
    };
    let peak = if negative {
        -minimum.min(0.0)
    } else {
        maximum.max(0.0)
    };
    if peak <= 0.0 {
        return Vec::new();
    }
    let base = match &level.base {
        ContourBasePolicy::Absolute(value) => value.get(),
        ContourBasePolicy::NoiseFloor {
            multiplier,
            peak_fraction,
            ..
        } => {
            let provenance = crate::state::EstimateProvenance {
                estimator: plotium_analysis::robust::NMR_BACKGROUND_ID.to_owned(),
                version: plotium_analysis::robust::NMR_BACKGROUND_VERSION,
            };
            let noise = match crate::state::diagnose_noise(values, rows, cols, provenance) {
                Ok(crate::state::EstimateResult::Noise(noise)) => noise,
                Ok(crate::state::EstimateResult::Unavailable(reason)) => {
                    // Shipping analysis maps use the bounded anchor. Keep a
                    // diagnostic if a direct caller requests an unavailable one.
                    eprintln!("Plotium analysis-map contour unavailable: {reason}");
                    return Vec::new();
                }
                Err(error) => {
                    eprintln!("Plotium analysis-map noise diagnostic failed: {error}");
                    return Vec::new();
                }
                Ok(crate::state::EstimateResult::LocationScale(_)) => {
                    unreachable!("noise diagnostics never return a background estimate")
                }
            };
            let (Some(min), Some(max)) = (FiniteF64::new(minimum), FiniteF64::new(maximum)) else {
                return Vec::new();
            };
            crate::state::noise_contour_base(
                &noise,
                multiplier.get(),
                *peak_fraction,
                FieldSummary { min, max },
            )
        }
        ContourBasePolicy::BackgroundScale { multiplier, .. } => {
            let (location, scale) = deplaned_location_scale(values, rows, cols);
            (location + multiplier.get() * scale).abs()
        }
        ContourBasePolicy::FractionOfRange(fraction) => {
            minimum + fraction.get() * (maximum - minimum)
        }
    };
    // The ladder — including which policies may be rewritten when their base is
    // unusable — is shared with the versioned field resolver, and works purely
    // in positive magnitudes; this half applies its own sign afterwards. These
    // analysis maps are `FractionOfRange` (see `bounded_scalar_contour_spec`),
    // so they never carry an explicit threshold for the ladder to report on.
    let levels = crate::contour_ladder::contour_level_ladder(base, peak, level).levels;
    if negative {
        levels.into_iter().map(|value| -value).collect()
    } else {
        levels
    }
}

fn finite_range(values: &[f32]) -> Option<(f64, f64)> {
    let mut finite = values.iter().copied().filter(|value| value.is_finite());
    let first = f64::from(finite.next()?);
    Some(finite.fold((first, first), |(minimum, maximum), value| {
        let value = f64::from(value);
        (minimum.min(value), maximum.max(value))
    }))
}

/// Build a waterfall figure from a pseudo-2D stack: the direct-dimension
/// spectrum of each increment, offset vertically. Increments are strided so at
/// most `MAX_STACK_TRACES` are drawn.
pub fn build_stack_figure(stack: &StackSpectrum) -> Figure {
    const MAX_STACK_TRACES: usize = 48;
    let (lo, hi) = stack.ppm_bounds();
    let n = stack.increments();
    let peak = stack.max_magnitude().max(f64::MIN_POSITIVE);
    let dy = peak * 0.12;
    let y_top = peak + n as f64 * dy;

    let x = match stack.direct_domain {
        plotium_io::Domain::Time => Axis::new("Direct acquisition time (s)", lo, hi),
        plotium_io::Domain::Frequency => nmr_axis(&stack.direct, lo, hi),
    };
    // The stack is phased to absorptive, so traces carry the signed real part:
    // short-τ relaxation increments dip below their baseline (inverted peaks).
    let y = Axis::new("Increment (offset)", -1.1 * peak, y_top * 1.02);
    let mut fig = Figure::new(format!("Pseudo-2D stack — {}", stack.source), x, y);

    let step = (n / MAX_STACK_TRACES).max(1);
    for i in (0..n).step_by(step) {
        let offset = i as f64 * dy;
        let pts: Vec<[f64; 2]> = stack
            .ppm
            .iter()
            .zip(&stack.traces[i])
            .map(|(&p, c)| [p, c.re + offset])
            .collect();
        let mut series = Series::line(format!("{i}"), pts).colored(Color::TRACE);
        series.display_baseline = offset;
        fig = fig.with_series(series);
    }
    fig
}

pub(crate) fn nmr_axis(meta: &plotium_processing::AxisMeta, lo: f64, hi: f64) -> Axis {
    use nmr::axis::AxisUnit;
    let label = match meta.unit {
        Some(AxisUnit::Ppm) => axis_label(&meta.nucleus),
        Some(AxisUnit::Hertz) => format!("{} frequency (Hz)", format_nucleus(&meta.nucleus)),
        Some(AxisUnit::Second) => "Acquisition time (s)".into(),
        _ => "Coordinate".into(),
    };
    Axis::new(label, lo, hi).reversed(meta.unit == Some(AxisUnit::Ppm))
}

pub(crate) fn axis_label(nucleus: &str) -> String {
    format!("{} chemical shift (ppm)", format_nucleus(nucleus))
}

pub(crate) fn format_nucleus(nucleus: &str) -> String {
    let mut formatted = String::new();
    let mut chars = nucleus.chars().peekable();
    while chars.peek().is_some_and(char::is_ascii_digit) {
        let digit = chars.next().expect("peeked digit must exist");
        formatted.push(match digit {
            '0' => '⁰',
            '1' => '¹',
            '2' => '²',
            '3' => '³',
            '4' => '⁴',
            '5' => '⁵',
            '6' => '⁶',
            '7' => '⁷',
            '8' => '⁸',
            '9' => '⁹',
            _ => digit,
        });
    }
    formatted.extend(chars);
    formatted
}

/// Build a DOSY contour figure from a per-column diffusion map: x = chemical
/// shift (reversed), y = log₁₀(D). Fitted columns deposit a Gaussian blob on an
/// intensity grid that is then contoured.
pub fn build_dosy_figure(map: &DiffusionMap, nucleus: &str, source: &str) -> Figure {
    build_dosy_figure_cancellable(map, nucleus, source, &|| false)
        .expect("non-cancelling DOSY figure")
}

pub fn build_dosy_figure_cancellable(
    map: &DiffusionMap,
    nucleus: &str,
    source: &str,
    cancelled: &impl Fn() -> bool,
) -> Option<Figure> {
    const NX: usize = 512;
    const NY: usize = 300;
    let fitted: Vec<(f64, f64, f64)> = map
        .ppm
        .iter()
        .zip(&map.d)
        .zip(&map.amp)
        .filter_map(|((&p, &d), &a)| (d.is_finite() && d > 0.0).then_some((p, d.log10(), a)))
        .collect();

    let (ppm_lo, ppm_hi) = map
        .ppm
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), &p| {
            (lo.min(p), hi.max(p))
        });
    let x = Axis::new(axis_label(nucleus), ppm_lo, ppm_hi).reversed(true);

    if fitted.is_empty() {
        let y = Axis::new("log₁₀(D / (m²/s))", -10.5, -8.5);
        return Some(Figure::new(format!("DOSY — {source}"), x, y).with_axis_frame(AxisFrame::Box));
    }
    let (mut logd_lo, mut logd_hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for &(_, ld, _) in &fitted {
        logd_lo = logd_lo.min(ld);
        logd_hi = logd_hi.max(ld);
    }
    // Pad the D axis by half a decade each side.
    logd_lo -= 0.5;
    logd_hi += 0.5;
    let y = Axis::new("log₁₀(D / (m²/s))", logd_lo, logd_hi).reversed(true);

    // Accumulate Gaussian blobs onto the grid (row-major, NY rows × NX cols).
    let mut grid = vec![0.0f32; NX * NY];
    let sx = (ppm_hi - ppm_lo).max(f64::MIN_POSITIVE);
    let sy = (logd_hi - logd_lo).max(f64::MIN_POSITIVE);
    let sig_x = 1.5f64; // px in ppm direction
    let sig_y = 3.0f64; // px in log-D direction
    for &(ppm, logd, amp) in &fitted {
        if cancelled() {
            return None;
        }
        let cx = ((ppm - ppm_lo) / sx * (NX - 1) as f64).round() as isize;
        let cy = ((logd - logd_lo) / sy * (NY - 1) as f64).round() as isize;
        let rx = (sig_x * 3.0) as isize;
        let ry = (sig_y * 3.0) as isize;
        for dy in -ry..=ry {
            let yy = cy + dy;
            if yy < 0 || yy >= NY as isize {
                continue;
            }
            for dx in -rx..=rx {
                let xx = cx + dx;
                if xx < 0 || xx >= NX as isize {
                    continue;
                }
                let g = (-(dx as f64).powi(2) / (2.0 * sig_x * sig_x)
                    - (dy as f64).powi(2) / (2.0 * sig_y * sig_y))
                    .exp();
                grid[yy as usize * NX + xx as usize] += (amp * g) as f32;
            }
        }
    }
    let mut fig = Figure::new(format!("DOSY — {source}"), x, y).with_axis_frame(AxisFrame::Box);
    let contour = bounded_scalar_contour_spec();
    let levels = contour_levels(&grid, NY, NX, &contour.positive, false);
    if !levels.is_empty() {
        // Grid rows map onto [logd_lo, logd_hi], cols onto [ppm_lo, ppm_hi].
        #[cfg(test)]
        crate::contour_probe::record_marching_squares();
        let segments = plotium_render::contour::segments_cancellable(
            &grid, NY, NX, ppm_lo, ppm_hi, logd_lo, logd_hi, &levels, cancelled,
        )?;
        fig = fig.with_contour(Contour {
            origin: None,
            segments,
            color: contour.style.positive_color.resolve(),
            width: contour.style.width.get(),
        });
    }
    Some(fig)
}

/// Build a DOSY contour figure from a full ILT/CONTIN inversion: x = chemical
/// shift (reversed), y = log₁₀(D). `amp[c]` is column `c`'s D distribution over
/// the shared, log-spaced `d_grid`, so its rows map linearly onto log₁₀(D) and
/// its columns onto the ppm axis — contoured directly without re-binning.
pub fn build_ilt_figure(result: &IltResult, nucleus: &str, source: &str) -> Figure {
    build_ilt_figure_cancellable(result, nucleus, source, &|| false)
        .expect("non-cancelling ILT figure")
}

pub fn build_ilt_figure_cancellable(
    result: &IltResult,
    nucleus: &str,
    source: &str,
    cancelled: &impl Fn() -> bool,
) -> Option<Figure> {
    let nx = result.ppm.len();
    let ny = result.d_grid.len();
    let (ppm_lo, ppm_hi) = result
        .ppm
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), &p| {
            (lo.min(p), hi.max(p))
        });
    let x = Axis::new(axis_label(nucleus), ppm_lo, ppm_hi).reversed(true);

    let logd: Vec<f64> = result
        .d_grid
        .iter()
        .map(|&d| d.max(f64::MIN_POSITIVE).log10())
        .collect();
    if nx < 2 || ny < 2 {
        let y = Axis::new("log₁₀(D / (m²/s))", -10.5, -8.5);
        return Some(
            Figure::new(format!("DOSY (ILT) — {source}"), x, y).with_axis_frame(AxisFrame::Box),
        );
    }
    let (logd_lo, logd_hi) = (logd[0], logd[ny - 1]);
    let y = Axis::new(
        "log₁₀(D / (m²/s))",
        logd_lo.min(logd_hi),
        logd_lo.max(logd_hi),
    )
    .reversed(true);

    // Row-major NY×NX grid: row = D index, col = ppm column.
    let mut grid = vec![0.0f32; nx * ny];
    for (c, col) in result.amp.iter().enumerate().take(nx) {
        if cancelled() {
            return None;
        }
        for (r, &a) in col.iter().enumerate().take(ny) {
            grid[r * nx + c] = a as f32;
        }
    }
    let mut fig =
        Figure::new(format!("DOSY (ILT) — {source}"), x, y).with_axis_frame(AxisFrame::Box);
    let contour = bounded_scalar_contour_spec();
    let levels = contour_levels(&grid, ny, nx, &contour.positive, false);
    if !levels.is_empty() {
        #[cfg(test)]
        crate::contour_probe::record_marching_squares();
        let segments = plotium_render::contour::segments_cancellable(
            &grid,
            ny,
            nx,
            result.ppm[0],
            result.ppm[nx - 1],
            logd_lo,
            logd_hi,
            &levels,
            cancelled,
        )?;
        fig = fig.with_contour(Contour {
            origin: None,
            segments,
            color: contour.style.positive_color.resolve(),
            width: contour.style.width.get(),
        });
    }
    Some(fig)
}

fn bounded_scalar_contour_spec() -> ContourSpec {
    let capabilities = scalar_grid_capabilities(true, &[crate::automation::CAP_FIELD_BOUNDED]);
    // `Bounded` anchors the base to the value range, so no peak is consulted.
    default_contour_spec(&capabilities, crate::state::NO_PEAK)
}

#[cfg(test)]
#[path = "figures_tests.rs"]
mod tests;
