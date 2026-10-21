//! Regressions from stage-three acceptance, using original synthetic data.
use super::*;

pub(super) fn check_unconverged_fit(
    s: &mut Session,
    a: &mut PlotiumApp,
    table: Value,
    column: Value,
) {
    let fitted = edit(
        s,
        a,
        DIFFUSION_FIT,
        json!({"response_column":column,"options":{
            "solver":"bounded_trust_region","weights":{"type":"equal"},"robust_loss":{"type":"none"},
            "non_finite":"reject","max_iterations":1,"tolerance":1e-8,"multi_start":1,
            "ode_solver":"auto","relative_tolerance":1e-6,"absolute_tolerance":1e-9
        }}),
        Some(json!([table])),
    );
    let receipt = settle(s, a, &fitted);
    assert_eq!(receipt["summary"]["converged"], false, "{receipt}");
    let analysis = receipt["produced"][0].clone();
    let report = call(
        s,
        a,
        "plotium_analysis_results_inspect",
        json!({"ids":[analysis["id"]]}),
    );
    let fit = &report["value"]["analyses"][0];
    assert_eq!(fit["converged"], false, "{report}");
    assert_eq!(fit["termination"][0]["converged"], false);
    assert_eq!(fit["termination"][0]["reason"], "iteration limit reached");
    assert!(
        fit["notices"]
            .as_array()
            .unwrap()
            .iter()
            .any(|n| n.as_str().unwrap().contains("did not converge"))
    );
    let failure = s.call(a, "plotium_plan", json!({"tool_id":DIFFUSION_PLOT_CREATE,
        "parameters":{"source":analysis,"name":"Unconverged fit must not plot"},
        "expected_revision":a.doc.automation_revision,"expected_project_context":a.session.project_context})).unwrap_err();
    assert!(failure.message.contains("did not converge"), "{failure:?}");
}

pub(super) fn reject_failed_selected_attempt(
    s: &mut Session,
    a: &mut PlotiumApp,
    analysis: &ResourceRef,
) {
    let owner = analysis.parent_id.as_ref().unwrap();
    let table = a
        .doc
        .datasets
        .iter_mut()
        .find(|d| d.resource_id().to_string() == *owner)
        .unwrap()
        .as_table_mut()
        .unwrap();
    let result = &mut table.curve_fit_analyses[0].result;
    let attempts = result.attempts.clone();
    assert!(result.converged);
    result.attempts[0].converged = false;
    result.attempts[0].reason = "trust region collapsed".into();
    // A successful but higher-cost start must not validate the selected failure.
    let mut other = result.attempts[0].clone();
    other.converged = true;
    other.cost += 1.;
    result.attempts.push(other);
    let before = (
        a.doc.canvases.len(),
        a.doc.datasets.len(),
        a.doc.automation_revision,
    );
    let failure = s.call(a, "plotium_plan", json!({"tool_id":DIFFUSION_PLOT_CREATE,
        "parameters":{"source":analysis,"name":"Must reject failed fit"},
        "expected_revision":a.doc.automation_revision,"expected_project_context":a.session.project_context})).unwrap_err();
    assert!(failure.message.contains("did not converge"), "{failure:?}");
    assert_eq!(
        before,
        (
            a.doc.canvases.len(),
            a.doc.datasets.len(),
            a.doc.automation_revision
        )
    );
    a.doc
        .datasets
        .iter_mut()
        .find(|d| d.resource_id().to_string() == *owner)
        .unwrap()
        .as_table_mut()
        .unwrap()
        .curve_fit_analyses[0]
        .result
        .attempts = attempts;
}

#[test]
fn default_ilt_of_20480_points_plots_and_invalidated_result_explains_recomputation() {
    use num_complex::Complex64;
    use plotium_io::{AxisSource, Dim, Domain, NmrData2D, PseudoAxis, PseudoKind, QuadMode};
    let cols = 20_480;
    let rows = 14;
    let dim = Dim {
        spectral_width_hz: 1000.,
        observe_freq_mhz: 400.,
        carrier_ppm: 0.,
        nucleus: "1H".into(),
        group_delay: 0.,
    };
    // Two isolated frequency bins keep the real 128×20480 computation bounded
    // without substituting a fabricated result or reducing the default grid.
    let data = (0..rows)
        .flat_map(|row| {
            (0..cols).map(move |col| {
                let intensity = if col == 10_240 || col == 10_241 {
                    1000. * (-D * row as f64 * 1e8).exp()
                } else {
                    0.
                };
                Complex64::new(intensity, 0.)
            })
        })
        .collect();
    let dataset = Nmr2DDataset::load(NmrData2D {
        data,
        rows,
        cols,
        domain: Domain::Frequency,
        direct: dim.clone(),
        indirect: dim,
        quad: QuadMode::Complex,
        indirect_conjugate: false,
        experiment: None,
        pseudo_axis: Some(PseudoAxis {
            name: "increment".into(),
            kind: PseudoKind::Gradient,
            values: (0..rows).map(|i| i as f64).collect(),
            unit: "T/m".into(),
            source: AxisSource::EmbeddedRamp,
        }),
        diffusion: None,
        nus: None,
        source: "Original 20480-point ILT regression".into(),
    })
    .unwrap();
    let mut app = PlotiumApp::new_with_settings(Default::default());
    app.doc.datasets.push(Dataset::Nmr2D(Box::new(dataset)));
    let mut session = Session::default();
    session.set_allow_modify(true);
    let source = resource(&app, KIND_FIELD, "nmr.stack");
    let mono_built = edit(
        &mut session,
        &mut app,
        DOSY_BUILD,
        json!({"method":"mono_exp","input":input()}),
        Some(json!([source.id])),
    );
    settle(&mut session, &mut app, &mono_built);
    let mono_field = resource(&app, KIND_FIELD, "nmr.dosy_map");
    let built = edit(
        &mut session,
        &mut app,
        DOSY_BUILD,
        json!({"method":"ilt","input":input()}),
        Some(json!([source.id])),
    );
    settle(&mut session, &mut app, &built);
    let field = resource(&app, KIND_FIELD, "nmr.ilt_map");
    let descriptor = ProjectResourceProvider::new(&app)
        .inspect(&field.id)
        .unwrap();
    assert_eq!(descriptor.dimensions, vec![128, cols]);
    let planned = call(
        &mut session,
        &mut app,
        "plotium_plan",
        json!({"tool_id":FIELD_PLOT_CREATE,"parameters":{"source":field,"name":"Default 128 ILT 20480 points"}}),
    );
    assert!(
        planned["preflight"]["scientific_plot"]["retained_elements"]
            .as_u64()
            .unwrap()
            < 2_000_000
    );
    let plotted = session
        .call(
            &mut app,
            "plotium_execute",
            json!({"plan_id":planned["plan_id"]}),
        )
        .unwrap();
    check_plot(&mut session, &mut app, &plotted, false);
    let mut changed = input();
    for increment in changed["increments"].as_array_mut().unwrap() {
        increment["b"] = json!(increment["b"].as_f64().unwrap() * 1.01);
    }
    edit(
        &mut session,
        &mut app,
        DIFFUSION_SET_INPUT,
        json!({"input":changed}),
        Some(json!([source.parent_id.as_ref().unwrap()])),
    );
    let before = (
        app.doc.canvases.len(),
        app.doc.datasets.len(),
        app.doc.automation_revision,
    );
    for stale in [mono_field, field] {
        let args = json!({"tool_id":FIELD_PLOT_CREATE,
            "parameters":{"source":stale,"name":"Stale result must not plot"},
            "expected_revision":app.doc.automation_revision,"expected_project_context":app.session.project_context});
        let failure = session.call(&mut app, "plotium_plan", args).unwrap_err();
        assert!(
            failure.message.contains("invalidated") && failure.message.contains("Recompute"),
            "{failure:?}"
        );
    }
    assert_eq!(
        before,
        (
            app.doc.canvases.len(),
            app.doc.datasets.len(),
            app.doc.automation_revision
        )
    );
}
