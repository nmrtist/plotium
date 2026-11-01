//! Degenerate estimate results versus genuinely unavailable estimators.
//!
//! Small and constant fields are measured results; runtime failures are cached.
//! Neither may trigger repeated jobs on every frame.

use super::tests::{grid_dataset, wait_for_app_compute};
use crate::contour_probe;
use crate::state::{ChartSpec, DataBinding, DataDomain, PlotiumApp, StackSpec};
use plotium_figure::{
    ContourBasePolicy, ContourSpec, EstimatorSelection, PositiveFiniteF64, SeriesEncoding,
};

/// A tilted plane: every first difference is identical, so the robust MAD of
/// those differences is exactly zero — the same degenerate case an ideal
/// noiseless synthetic grid produces — while the field itself still spans a
/// range that marching squares can cut.
fn planar_values() -> Vec<f32> {
    (0..4u8)
        .flat_map(|row| (0..4u8).map(move |col| 1.0 + f32::from(row) + f32::from(col)))
        .collect()
}

fn contour_binding(app: &PlotiumApp) -> DataBinding {
    let binding = DataBinding::single(&app.doc.datasets[0]);
    let SeriesEncoding::Contour(contour) = &binding.series[0].encoding else {
        panic!("a true-2D NMR field defaults to a contour encoding");
    };
    assert!(
        matches!(contour.positive.base, ContourBasePolicy::NoiseFloor { .. }),
        "a noise-scale field defaults to a NoiseFloor base"
    );
    binding
}

fn rebuild(app: &mut PlotiumApp, binding: &DataBinding) -> plotium_figure::Figure {
    app.build_binding_figure(
        binding,
        &ChartSpec::default_for(DataDomain::Nmr2d),
        &StackSpec::default(),
        [120.0, 80.0],
    )
}

#[test]
fn a_small_plane_with_a_peak_draws_and_does_not_repeat_measurement() {
    let mut app = PlotiumApp::new_with_settings(crate::settings::Settings::default());
    let mut values = planar_values();
    values[5] = 100.0;
    app.doc.datasets.push(grid_dataset("small", &values));
    let binding = contour_binding(&app);
    contour_probe::reset();
    assert!(rebuild(&mut app, &binding).contours.is_empty());
    wait_for_app_compute(&mut app);
    rebuild(&mut app, &binding);
    wait_for_app_compute(&mut app);
    for _ in 0..3 {
        let figure = rebuild(&mut app, &binding);
        assert!(figure.contours.iter().any(|c| !c.segments.is_empty()));
    }
    assert_eq!(contour_probe::queued_estimates(), 1);
    assert!(!app.session.status.contains("absolute"));
}

#[test]
fn a_constant_field_has_no_crossings_and_does_not_repeat_measurement() {
    let mut app = PlotiumApp::new_with_settings(crate::settings::Settings::default());
    app.doc
        .datasets
        .push(grid_dataset("constant", &vec![7.0; 128 * 128]));
    let binding = contour_binding(&app);
    contour_probe::reset();
    rebuild(&mut app, &binding);
    wait_for_app_compute(&mut app);
    rebuild(&mut app, &binding);
    wait_for_app_compute(&mut app);
    assert!(
        rebuild(&mut app, &binding)
            .contours
            .iter()
            .all(|c| c.segments.is_empty())
    );
    assert_eq!(contour_probe::queued_estimates(), 1);
    assert!(!app.session.status.contains("could not"));
}

#[test]
fn an_unavailable_estimator_reports_an_error_and_is_not_cached_as_degenerate() {
    let mut app = PlotiumApp::new_with_settings(crate::settings::Settings::default());
    app.doc
        .datasets
        .push(grid_dataset("flat", &planar_values()));
    let mut binding = contour_binding(&app);
    let SeriesEncoding::Contour(contour) = &mut binding.series[0].encoding else {
        unreachable!("checked while building the binding");
    };
    *contour = ContourSpec {
        positive: plotium_figure::ContourLevelSpec {
            base: ContourBasePolicy::NoiseFloor {
                multiplier: PositiveFiniteF64::new(5.0).unwrap(),
                peak_fraction: plotium_figure::UnitInterval::new(0.0)
                    .expect("a zero floor is valid"),
                estimator: EstimatorSelection::Frozen {
                    estimator: "retired_estimator".to_owned(),
                    version: 99,
                },
            },
            count: 3,
            ratio: PositiveFiniteF64::new(1.5).unwrap(),
        },
        negative: None,
        style: contour.style.clone(),
    };

    contour_probe::reset();
    let figure = rebuild(&mut app, &binding);
    assert!(figure.contours.is_empty());
    assert_eq!(contour_probe::queued_estimates(), 1);

    wait_for_app_compute(&mut app);
    assert!(
        app.session
            .status
            .contains("Field estimate could not be computed"),
        "a frozen estimator that no longer exists must reach the user: {}",
        app.session.status
    );

    let still_blank = rebuild(&mut app, &binding);
    assert!(still_blank.contours.is_empty());
    assert_eq!(
        contour_probe::queued_estimates(),
        1,
        "an unavailable estimator is cached to prevent per-frame retries"
    );
}
