//! Shared contour property fixtures and their smoke test.

use super::*;

/// Distributed Gaussian background with signed peaks for scientific estimates.
fn default_plane() -> Vec<f64> {
    let mut state = 123456789_u64;
    let mut uniform = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state as f64 / u64::MAX as f64).max(f64::MIN_POSITIVE)
    };
    let mut values = (0..128 * 128)
        .map(|_| (-2.0 * uniform().ln()).sqrt() * (std::f64::consts::TAU * uniform()).cos())
        .collect::<Vec<_>>();
    values[3000] = 50.0;
    values[9000] = -40.0;
    values
}

fn nmr2d_with(source: &str, values: &[f64]) -> plotium_io::NmrData2D {
    let dimension = |nucleus: &str| plotium_io::Dim {
        spectral_width_hz: 4_000.0,
        observe_freq_mhz: 400.0,
        carrier_ppm: 0.0,
        nucleus: nucleus.to_owned(),
        group_delay: 0.0,
    };
    plotium_io::NmrData2D {
        data: values
            .iter()
            .map(|value| num_complex::Complex64::new(*value, 0.5))
            .collect(),
        rows: (values.len() as f64).sqrt() as usize,
        cols: (values.len() as f64).sqrt() as usize,
        domain: plotium_io::Domain::Frequency,
        direct: dimension("1H"),
        indirect: dimension("13C"),
        quad: plotium_io::QuadMode::Complex,
        indirect_conjugate: false,
        experiment: None,
        pseudo_axis: None,
        diffusion: None,
        nus: None,
        source: source.to_owned(),
    }
}

pub(super) fn nmr1d_with(source: &str) -> plotium_io::NmrData {
    plotium_io::NmrData {
        points: (0..32)
            .map(|value| num_complex::Complex64::new(f64::from(value), 0.0))
            .collect(),
        domain: plotium_io::Domain::Frequency,
        spectral_width_hz: 4_000.0,
        observe_freq_mhz: 400.0,
        carrier_ppm: 0.0,
        nucleus: "1H".to_owned(),
        source: source.to_owned(),
        group_delay: 0.0,
    }
}

/// One page holding one plot bound to a true-2D spectrum, i.e. the exact shape
/// the driving case has: a contour drawn from a signed scalar grid.
pub(crate) fn contour_app() -> (PlotiumApp, TargetRef) {
    contour_app_with_plane(&default_plane())
}

/// The same page over a plane the caller chooses, so a test can put a field of
/// a given dynamic range in front of the catalog.
pub(crate) fn contour_app_with_plane(values: &[f64]) -> (PlotiumApp, TargetRef) {
    let mut app = PlotiumApp::new();
    app.doc.datasets.push(Dataset::Nmr2D(Box::new(
        crate::nmr_test_support::load_2d(nmr2d_with("contour", values)).unwrap(),
    )));
    let mut canvas = CanvasDocument::new("page".to_owned(), [120.0, 80.0]);
    let id = canvas.allocate_object_id();
    let object = app.build_plot_object(
        0,
        ObjectFrame::new(0.0, 0.0, 100.0, 80.0),
        id,
        "Plot".into(),
    );
    canvas.objects.push(object);
    app.doc.canvases.push(canvas);
    app.session.active_canvas = Some(0);
    let series = app.doc.canvases[0]
        .object(id)
        .and_then(|object| object.plot())
        .and_then(|plot| plot.binding.series.first())
        .map(|series| series.id)
        .expect("the plot has a series");
    let target = app.series_target(0, id, series).expect("target resolves");
    (app, target)
}

pub(crate) fn contour_spec(app: &PlotiumApp, target: &TargetRef) -> plotium_figure::ContourSpec {
    let Some(ComponentRef::Series(series)) = target.component else {
        panic!("the fixture addresses a series");
    };
    let binding = &app.doc.canvases[0]
        .object(
            target
                .resource
                .local_id
                .as_deref()
                .unwrap()
                .parse()
                .unwrap(),
        )
        .and_then(|object| object.plot())
        .expect("plot")
        .binding;
    match &binding
        .series
        .iter()
        .find(|candidate| candidate.id == series)
        .expect("series")
        .encoding
    {
        plotium_figure::SeriesEncoding::Contour(spec) => spec.clone(),
        other => panic!("expected a contour, got {other:?}"),
    }
}

#[test]
fn the_fixture_draws_a_contour() {
    let (app, target) = contour_app();
    let address = PropertyAddress::new(target.clone(), contour::BASE_MAGNITUDE);
    let resolved = app.resolve_property(&address).expect("contour resolves");
    assert_eq!(resolved.availability, Availability::Editable);
}
