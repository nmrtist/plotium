use super::*;
use crate::xps::{self, XpsProcessingRecipe, XpsProcessingStep, XpsStepKind};
use crate::xrd::{self, SavitzkyGolay, SnipBackground, XrdNormalization, XrdProcessing};
use crate::{StepId, StepSource};

const SIGNAL: [f64; 12] = [
    3.0, 4.5, 2.0, 8.0, 40.0, 95.0, 42.0, 9.0, 3.5, 2.5, 6.0, 4.0,
];

fn axis() -> Vec<f64> {
    (0..SIGNAL.len())
        .map(|index| 20.0 + 0.5 * index as f64)
        .collect()
}

#[track_caller]
fn assert_close(actual: &[f64], expected: &[f64]) {
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected) {
        assert!(
            (actual - expected).abs() <= 1e-12 * expected.abs().max(1.0),
            "{actual} != {expected}"
        );
    }
}

fn xps_recipe(kinds: Vec<XpsStepKind>) -> XpsProcessingRecipe {
    XpsProcessingRecipe {
        steps: kinds
            .into_iter()
            .enumerate()
            .map(|(index, kind)| XpsProcessingStep {
                id: StepId::new(index as u64 + 1),
                kind,
                enabled: true,
                source: StepSource::User,
            })
            .collect(),
    }
}

#[test]
fn xrd_pipeline_output_is_stable() {
    let result = xrd::process(
        &axis(),
        &SIGNAL,
        XrdProcessing {
            background: Some(SnipBackground { iterations: 4 }),
            smoothing: Some(SavitzkyGolay {
                window: 5,
                polynomial_order: 2,
            }),
            normalization: XrdNormalization::Area,
        },
    )
    .unwrap();
    assert_close(
        &result.intensity,
        &[
            0.007830858306491521,
            0.0097113899216506,
            -0.02195079012235582,
            0.058240576735892315,
            0.523232748339667,
            0.7914476920581656,
            0.5497557905771219,
            0.08363992326806512,
            -0.02586476033927728,
            0.007133388654315068,
            0.01532853912215927,
            0.01082014526270066,
        ],
    );
    assert_close(
        &result.background.unwrap(),
        &[
            2.9999999999999987,
            2.4621011161012363,
            1.9999999999999996,
            6.561056513656541,
            3.242777315091824,
            2.480996283641935,
            2.540660333590254,
            5.162240774446305,
            3.499999999999999,
            2.5000000000000004,
            3.184170484070541,
            4.000000000000002,
        ],
    );
}

#[test]
fn xps_savitzky_golay_output_is_stable() {
    let recipe = xps_recipe(vec![XpsStepKind::Smooth(SmoothMethod::SavitzkyGolay {
        window: 5,
        poly_order: 2,
    })]);
    let result = xps::process_region(&axis(), &SIGNAL, 0.0, &recipe).unwrap();
    assert_close(
        &result.intensity,
        &[
            5.928571428571431,
            -1.2142857142857117,
            1.5714285714285747,
            9.757142857142867,
            50.971428571428575,
            72.8,
            52.328571428571436,
            11.614285714285723,
            1.5285714285714325,
            3.3571428571428577,
            3.4285714285714293,
            5.142857142857144,
        ],
    );
}

#[test]
fn xps_moving_average_rounds_the_window_up_to_odd() {
    let recipe = xps_recipe(vec![XpsStepKind::Smooth(SmoothMethod::MovingAverage {
        window: 4,
    })]);
    let result = xps::process_region(&axis(), &SIGNAL, 0.0, &recipe).unwrap();
    assert_close(
        &result.intensity,
        &[
            3.1666666666666665,
            4.375,
            11.5,
            29.9,
            37.4,
            38.8,
            37.9,
            30.4,
            12.6,
            5.0,
            4.0,
            4.166666666666667,
        ],
    );
}

#[test]
fn xps_total_area_output_is_stable() {
    let recipe = xps_recipe(vec![XpsStepKind::Normalize(NormalizeMethod::TotalArea)]);
    let result = xps::process_region(&axis(), &SIGNAL, 0.0, &recipe).unwrap();
    assert_close(
        &result.intensity,
        &[
            0.02733485193621868,
            0.04100227790432802,
            0.018223234624145785,
            0.07289293849658314,
            0.36446469248291574,
            0.8656036446469249,
            0.3826879271070615,
            0.08200455580865604,
            0.03189066059225513,
            0.022779043280182234,
            0.05466970387243736,
            0.03644646924829157,
        ],
    );
}

#[test]
fn gaussian_smooth_output_is_stable() {
    assert_close(
        &gaussian_smooth(&SIGNAL, 1.5).unwrap(),
        &[
            3.7659164964752625,
            5.8040366376185695,
            11.779462665717093,
            23.673619302933528,
            37.93296815923521,
            44.857150650271535,
            38.52979776298621,
            24.5019156128515,
            12.540680176840223,
            6.5638321810510725,
            4.720174055870699,
            4.321041888980144,
        ],
    );
}

#[test]
fn fit_boundary_window_reproduces_a_polynomial_at_the_edges() {
    let quadratic: Vec<f64> = (0..9).map(|index| (index * index) as f64).collect();
    let smoothed = savitzky_golay(&quadratic, 5, 2, SavitzkyGolayEdges::FitBoundaryWindow).unwrap();
    assert_close(&smoothed, &quadratic);
}

#[test]
fn repeat_endpoints_flattens_a_polynomial_at_the_edges() {
    let quadratic: Vec<f64> = (0..9).map(|index| (index * index) as f64).collect();
    let smoothed = savitzky_golay(&quadratic, 5, 2, SavitzkyGolayEdges::RepeatEndpoints).unwrap();
    assert_close(&smoothed[2..7], &quadratic[2..7]);
    assert!(smoothed[1] > quadratic[1]);
}

#[test]
fn short_signals_pass_through_smoothing_unchanged() {
    let method = SmoothMethod::SavitzkyGolay {
        window: 9,
        poly_order: 3,
    };
    assert_eq!(smooth(&[1.0, 2.0], method), vec![1.0, 2.0]);
}

fn trace<'a>(x: &'a [f64], y: &'a [f64]) -> Trace<'a> {
    Trace { x, y }
}

#[test]
fn linear_combination_interpolates_onto_the_first_grid() {
    let result = linear_combination(
        trace(&[0.0, 0.5, 1.0], &[1.0, 1.0, 1.0]),
        trace(&[0.0, 1.0], &[0.0, 10.0]),
        -0.5,
    )
    .unwrap();
    assert_close(&result, &[1.0, -1.5, -4.0]);
}

#[test]
fn linear_combination_treats_the_second_trace_as_zero_outside_its_range() {
    let result = linear_combination(
        trace(&[0.0, 1.0, 2.0, 3.0], &[1.0, 1.0, 1.0, 1.0]),
        trace(&[1.0, 2.0], &[5.0, 5.0]),
        -1.0,
    )
    .unwrap();
    assert_close(&result, &[1.0, -4.0, -4.0, 1.0]);
}

#[test]
fn linear_combination_reads_a_descending_axis() {
    let result =
        linear_combination(trace(&[0.5], &[0.0]), trace(&[1.0, 0.0], &[10.0, 0.0]), 1.0).unwrap();
    assert_close(&result, &[5.0]);
    let onto_descending = linear_combination(
        trace(&[1.0, 0.5, 0.0], &[0.0, 0.0, 0.0]),
        trace(&[0.0, 1.0], &[0.0, 10.0]),
        1.0,
    )
    .unwrap();
    assert_close(&onto_descending, &[10.0, 5.0, 0.0]);
}

#[test]
fn average_counts_only_the_traces_covering_each_point() {
    let result = average(&[
        trace(&[0.0, 1.0, 2.0], &[2.0, 4.0, 6.0]),
        trace(&[0.0, 1.0], &[4.0, 8.0]),
    ])
    .unwrap();
    assert_close(&result, &[3.0, 6.0, 6.0]);
}

#[test]
fn traces_must_be_monotonic_and_finite() {
    let good = trace(&[0.0, 1.0], &[1.0, 1.0]);
    assert_eq!(
        linear_combination(good, trace(&[0.0, 0.0], &[1.0, 1.0]), 1.0),
        Err(TraceError::NonMonotonicAxis)
    );
    assert_eq!(
        linear_combination(good, trace(&[0.0, 1.0], &[1.0, f64::NAN]), 1.0),
        Err(TraceError::NonFiniteValue)
    );
    assert_eq!(average(&[]), Err(TraceError::InvalidLength));
}

#[test]
fn end_points_that_differ_by_rounding_still_match() {
    let result = linear_combination(
        trace(&[10.0, 30.000000000000004], &[5.0, 5.0]),
        trace(&[10.0, 30.0], &[2.0, 2.0]),
        -1.0,
    )
    .unwrap();
    assert_close(&result, &[3.0, 3.0]);
}
