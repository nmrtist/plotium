//! End-to-end display-policy evidence on known signed peaks, not just scales.
use super::*;
use plotium_figure::UnitInterval;

const SIDE: usize = 256;
struct Peak {
    row: usize,
    col: usize,
    amplitude: f64,
}

fn white(seed: u64) -> Vec<f32> {
    let mut state = seed;
    let mut uniform = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state as f64 / u64::MAX as f64).max(f64::MIN_POSITIVE)
    };
    (0..SIDE * SIDE)
        .map(|_| {
            ((-2.0 * uniform().ln()).sqrt() * (std::f64::consts::TAU * uniform()).cos()) as f32
        })
        .collect()
}

fn background(seed: u64, correlated: bool) -> Vec<f32> {
    let raw = white(seed);
    if !correlated {
        return raw;
    }
    let mut out = vec![0.0; raw.len()];
    for row in 0..SIDE {
        for col in 0..SIDE {
            out[row * SIDE + col] = (0..4)
                .flat_map(|dr| (0..4).map(move |dc| (dr, dc)))
                .map(|(dr, dc)| raw[((row + dr) % SIDE) * SIDE + (col + dc) % SIDE])
                .sum::<f32>()
                / 4.0;
        }
    }
    out
}

fn add_peak(values: &mut [f32], peak: &Peak) {
    // Compact support for an effectively Gaussian line, including a broad
    // strong peak whose tails cannot be approximated by a single hot pixel.
    let width: f64 = if peak.amplitude.abs() > 50.0 {
        2.0
    } else {
        1.2
    };
    for row in peak.row - 12..=peak.row + 12 {
        for col in peak.col - 12..=peak.col + 12 {
            let radius2 =
                (row as f64 - peak.row as f64).powi(2) + (col as f64 - peak.col as f64).powi(2);
            values[row * SIDE + col] +=
                (peak.amplitude * (-radius2 / (2.0 * width.powi(2))).exp()) as f32;
        }
    }
}

fn crosses(values: &[f32], row: usize, col: usize, threshold: f64) -> bool {
    let corners = [
        values[row * SIDE + col],
        values[row * SIDE + col + 1],
        values[(row + 1) * SIDE + col],
        values[(row + 1) * SIDE + col + 1],
    ];
    let low = corners.into_iter().fold(f32::INFINITY, f32::min) as f64;
    let high = corners.into_iter().fold(f32::NEG_INFINITY, f32::max) as f64;
    low < threshold && high >= threshold
}

fn recalled(values: &[f32], peaks: &[Peak], threshold: f64) -> usize {
    peaks
        .iter()
        .filter(|peak| {
            let sign = peak.amplitude.signum();
            let above = |row, col| f64::from(values[row * SIDE + col]) * sign >= threshold;
            if !above(peak.row, peak.col) {
                return false;
            }
            // A real closed contour must enclose the known peak centre. Reject a
            // component escaping this ROI, rather than counting arbitrary crossings.
            let mut seen = [[false; 11]; 11];
            let mut queue = vec![(peak.row, peak.col)];
            seen[5][5] = true;
            while let Some((row, col)) = queue.pop() {
                if row.abs_diff(peak.row) == 5 || col.abs_diff(peak.col) == 5 {
                    return false;
                }
                for (r, c) in [
                    (row - 1, col),
                    (row + 1, col),
                    (row, col - 1),
                    (row, col + 1),
                ] {
                    let ri = r + 5 - peak.row;
                    let ci = c + 5 - peak.col;
                    if !seen[ri][ci] && above(r, c) {
                        seen[ri][ci] = true;
                        queue.push((r, c));
                    }
                }
            }
            true
        })
        .count()
}

fn background_crossings(values: &[f32], peaks: &[Peak], threshold: f64) -> usize {
    let mut count = 0;
    for row in 0..SIDE - 1 {
        for col in 0..SIDE - 1 {
            if peaks
                .iter()
                .any(|p| row.abs_diff(p.row) <= 13 && col.abs_diff(p.col) <= 13)
            {
                continue;
            }
            count += usize::from(crosses(values, row, col, threshold));
            count += usize::from(crosses(values, row, col, -threshold));
        }
    }
    count
}

#[test]
fn signed_peak_matrix_reports_recall_and_background_crossings() {
    let mut total_old = 0;
    let mut total_new = 0;
    let mut targets = 0;
    let mut old_background = 0;
    let mut new_background = 0;
    let mut measurable = 0;
    let mut control_matches = 0;
    let mut unfloored_recall = 0;
    let mut unfloored_crossings = 0;
    for seed in [0x9372_4bd3, 0xb392_1583, 0xa14c_112f] {
        for correlated in [false, true] {
            for dynamic_range in [1e2, 1e4, 1e5] {
                for count in [4, 16] {
                    for snr in [5.0, 8.0, 12.0] {
                        let control = background(seed, correlated);
                        let mut values = control.clone();
                        let strong = Peak {
                            row: 220,
                            col: 220,
                            amplitude: dynamic_range,
                        };
                        let weak: Vec<_> = (0..count)
                            .map(|i| Peak {
                                row: 24 + (i / 4) * 48,
                                col: 24 + (i % 4) * 48,
                                amplitude: if i % 2 == 0 { snr } else { -snr },
                            })
                            .collect();
                        add_peak(&mut values, &strong);
                        for peak in &weak {
                            add_peak(&mut values, peak);
                        }
                        let maximum = values
                            .iter()
                            .map(|v| f64::from(v.abs()))
                            .fold(0.0, f64::max);
                        let old = 5.0
                            * plotium_analysis::robust::robust_difference_mad(&values, SIDE, SIDE)
                                .max(1e-4 * maximum);
                        let result = diagnose_noise(
                            &values,
                            SIDE,
                            SIDE,
                            EstimateProvenance {
                                estimator: plotium_analysis::robust::NMR_BACKGROUND_ID.into(),
                                version: 1,
                            },
                        )
                        .unwrap();
                        let EstimateResult::Noise(noise) = result else {
                            eprintln!(
                                "matrix unavailable: correlated={correlated} range={dynamic_range:e} peaks={count} snr={snr}"
                            );
                            continue;
                        };
                        measurable += 1;
                        let summary = FieldSummary {
                            min: FiniteF64::new(-maximum).unwrap(),
                            max: FiniteF64::new(maximum).unwrap(),
                        };
                        let new = noise_contour_base(
                            &noise,
                            5.0,
                            UnitInterval::new(0.0).unwrap(),
                            summary,
                        );
                        let unfloored = 5.0
                            * plotium_analysis::robust::robust_difference_mad(&values, SIDE, SIDE);
                        unfloored_recall += recalled(&values, &weak, unfloored);
                        control_matches += recalled(&control, &weak, new);
                        let old_recall = recalled(&values, &weak, old);
                        let new_recall = recalled(&values, &weak, new);
                        let mut excluded = weak;
                        excluded.push(strong);
                        let old_crossings = background_crossings(&values, &excluded, old);
                        let new_crossings = background_crossings(&values, &excluded, new);
                        unfloored_crossings += background_crossings(&values, &excluded, unfloored);
                        if seed == 0x9372_4bd3 && snr == 8.0 && count == 16 {
                            eprintln!(
                                "matrix corr={correlated} range={dynamic_range:e} peaks={count} snr={snr}: T {old:.2}->{new:.2}; recall {old_recall}->{new_recall}/{count}; background cells {old_crossings}->{new_crossings}"
                            );
                        }
                        targets += count;
                        total_old += old_recall;
                        total_new += new_recall;
                        old_background += old_crossings;
                        new_background += new_crossings;
                        // These supported backgrounds must be estimated without an
                        // extreme parent peak imposing a global fixed-fraction floor.
                        if dynamic_range >= 1e4 && snr == 12.0 {
                            assert!(
                                new_recall >= count * 3 / 4,
                                "12-sigma recall={new_recall}/{count}, T={new}"
                            );
                        }
                    }
                }
            }
        }
    }
    eprintln!(
        "ablations: unfloored diff recall={unfloored_recall}, background={unfloored_crossings}; no-peak matched controls={control_matches}"
    );
    assert_eq!(control_matches, 0);
    eprintln!(
        "matrix aggregate: measurable={measurable}/108; recall {total_old}->{total_new}/{targets}; background crossing cells {old_background}->{new_background}"
    );
    assert_eq!(measurable, 108);
    assert!(total_new > total_old);
    assert!(new_background * 10 < old_background);
}

#[test]
fn localized_stripes_expose_the_global_threshold_limit() {
    let weak: Vec<_> = (0..4)
        .map(|i| Peak {
            row: 24,
            col: 24 + i * 48,
            amplitude: if i % 2 == 0 { 8.0 } else { -8.0 },
        })
        .collect();
    for stripe_rows in [1, 32] {
        let mut values = background(0x9372_4bd3, false);
        for p in &weak {
            add_peak(&mut values, p);
        }
        for row in 192..192 + stripe_rows {
            for col in 32..96 {
                values[row * SIDE + col] += 30.0;
            }
        }
        let report = plotium_analysis::nmr_background::diagnose(&values, SIDE, SIDE).unwrap();
        let result = diagnose_noise(
            &values,
            SIDE,
            SIDE,
            EstimateProvenance {
                estimator: plotium_analysis::robust::NMR_BACKGROUND_ID.into(),
                version: 1,
            },
        )
        .unwrap();
        let EstimateResult::Noise(noise) = result else {
            panic!("distributed clean background remains available")
        };
        let level = noise_contour_base(
            &noise,
            5.0,
            UnitInterval::new(0.0).unwrap(),
            FieldSummary {
                min: FiniteF64::new(-40.0).unwrap(),
                max: FiniteF64::new(40.0).unwrap(),
            },
        );
        let crossings = background_crossings(&values, &weak, level);
        let recall = recalled(&values, &weak, level);
        eprintln!(
            "stripe rows={stripe_rows}: coverage={:.3}, threshold={level:.2}, weak recall={recall}/4, outside-peak crossing cells={crossings}",
            report.coverage
        );
        // A small contaminated region cannot be classified as false signal by
        // global background statistics alone. Preserve this limitation openly.
        assert!(crossings > 20);
    }
}
