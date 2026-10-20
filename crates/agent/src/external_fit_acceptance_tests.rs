//! Optional local acceptance: external acquisitions never become test fixtures.
//! Fixed protocol: 14 increments × 20,480 points, default processing, and Area
//! over 3.0–3.5 ppm. Run explicitly with --ignored and PLOTIUM_ACCEPTANCE_INPUT;
//! PLOTIUM_SCIENCE_ACCEPTANCE_DIR optionally retains local evidence.
use super::*;

#[test]
#[ignore = "requires PLOTIUM_ACCEPTANCE_INPUT pointing to a local JEOL diffusion acquisition"]
fn external_diffusion_fit_acceptance() {
    let path =
        PathBuf::from(std::env::var_os("PLOTIUM_ACCEPTANCE_INPUT").expect("external input path"));
    let mut app = PlotiumApp::new_with_settings(Default::default());
    let mut session = Session::default();
    session.set_allow_modify(true);
    edit(
        &mut session,
        &mut app,
        "data.import",
        json!({"paths":[path]}),
        None,
    );
    let field = resource(&app, KIND_FIELD, "nmr.stack");
    let owner = field.parent_id.clone().unwrap();
    let processing = call(
        &mut session,
        &mut app,
        "plotium_processing_inspect",
        json!({"ids":[owner]}),
    );
    let diffusion = call(
        &mut session,
        &mut app,
        "plotium_diffusion_inspect",
        json!({"ids":[field.id]}),
    );
    assert_eq!(diffusion["value"]["executable"], true, "{diffusion}");
    assert_eq!(diffusion["value"]["increment_count"], 14);
    let extracted = edit(
        &mut session,
        &mut app,
        REGIONS_EXTRACT,
        json!({"regions":[{"source":"explicit","bounds":[3.,3.5],"metric":"Area","name":"Fixed acceptance region"}]}),
        Some(json!([field.id])),
    );
    let extracted = settle(&mut session, &mut app, &extracted);
    let table_id = extracted["produced"][0]["id"].clone();
    let table = app
        .doc
        .datasets
        .iter()
        .find(|d| json!(d.resource_id()) == table_id)
        .unwrap()
        .as_table()
        .unwrap();
    let column = table.series_bindings[0].value_column;
    let fitted = edit(
        &mut session,
        &mut app,
        DIFFUSION_FIT,
        json!({"response_column":column}),
        Some(json!([table_id])),
    );
    let fitted = settle(&mut session, &mut app, &fitted);
    let analysis: ResourceRef = serde_json::from_value(fitted["produced"][0].clone()).unwrap();
    let report = call(
        &mut session,
        &mut app,
        "plotium_analysis_results_inspect",
        json!({"ids":[analysis.id]}),
    );
    assert_eq!(report["value"]["valid"], true);
    let table = app
        .doc
        .datasets
        .iter()
        .find(|d| json!(d.resource_id()) == table_id)
        .unwrap()
        .as_table()
        .unwrap();
    let result = &table.curve_fit_analyses.last().unwrap().result;
    assert!(result.converged, "{:?}", result.attempts);
    let selected = result
        .attempts
        .iter()
        .filter(|a| a.cost.is_finite())
        .reduce(|best, next| if next.cost < best.cost { next } else { best })
        .unwrap();
    assert!(selected.converged);
    let frozen = &result.datasets[0];
    let b = &frozen.inputs["b"];
    let y = &frozen.responses["y"];
    assert_eq!(b.len(), 14);
    assert_eq!(result.points.len(), 14);
    for (i, point) in result.points.iter().enumerate() {
        assert_eq!(point.row, i);
        assert_eq!(point.observed, y[i]);
        assert_eq!(point.weight, 1.);
        assert_eq!(
            b[i],
            diffusion["value"]["input"]["b_values"][i]["b"]
                .as_f64()
                .unwrap()
        );
    }
    let (reference_d, reference_sse, reference_se) = independent_reference(b, y);
    let d = result
        .parameters
        .iter()
        .find(|p| p.parameter == "D")
        .unwrap();
    let reference = json!({"d":reference_d,"sse":reference_sse,"standard_error":reference_se});
    eprintln!(
        "FIT D={} SE={} SSE={}\nREFERENCE {reference}\nTERMINATION {:?}",
        d.value, d.standard_error, result.statistics.chi_squared, result.attempts
    );
    assert!((d.value / reference_d - 1.).abs() < 1e-6);
    assert!((result.statistics.chi_squared / reference_sse - 1.).abs() < 1e-8);
    assert!((d.standard_error / reference_se - 1.).abs() < 1e-5);
    if let Some(dir) = std::env::var_os("PLOTIUM_SCIENCE_ACCEPTANCE_DIR") {
        let dir = PathBuf::from(dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("external_fit.json"), serde_json::to_vec_pretty(
            &json!({"processing":processing,"diffusion":diffusion,"frozen":frozen,"result":report,"reference":reference})
        ).unwrap()).unwrap();
    }
    let plotted = edit(
        &mut session,
        &mut app,
        DIFFUSION_PLOT_CREATE,
        json!({"source":analysis,"name":"External decay fit"}),
        None,
    );
    check_plot(&mut session, &mut app, &plotted, true);
    for (method, key) in [("mono_exp", "nmr.dosy_map"), ("ilt", "nmr.ilt_map")] {
        let built = edit(
            &mut session,
            &mut app,
            DOSY_BUILD,
            json!({"method":method}),
            Some(json!([field.id])),
        );
        settle(&mut session, &mut app, &built);
        let map = resource(&app, KIND_FIELD, key);
        let nmr = app
            .doc
            .datasets
            .iter()
            .find(|d| d.resource_id().to_string() == owner)
            .unwrap()
            .as_nmr2d()
            .unwrap();
        let ppm = if method == "mono_exp" {
            &nmr.dosy_map.as_ref().unwrap().ppm
        } else {
            &nmr.ilt_map.as_ref().unwrap().ppm
        };
        let center = ppm
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| (*a - 3.25).abs().total_cmp(&(*b - 3.25).abs()))
            .unwrap()
            .0;
        let diagnostics = call(
            &mut session,
            &mut app,
            "plotium_analysis_results_inspect",
            json!({"ids":[map.id],"offset":center.saturating_sub(50),"limit":100}),
        );
        assert_eq!(diagnostics["value"]["valid"], true, "{diagnostics}");
        assert_eq!(diagnostics["value"]["status"], "available");
        assert!(diagnostics["value"]["accepted_columns"].as_u64().unwrap() > 0);
        assert_eq!(diagnostics["value"]["total"], 20_480);
        if let Some(dir) = std::env::var_os("PLOTIUM_SCIENCE_ACCEPTANCE_DIR") {
            std::fs::write(
                PathBuf::from(dir).join(format!("external_{method}_diagnostics.json")),
                serde_json::to_vec_pretty(&diagnostics).unwrap(),
            )
            .unwrap();
        }
        let plotted = edit(
            &mut session,
            &mut app,
            FIELD_PLOT_CREATE,
            json!({"source":map,"name":format!("External DOSY {method}")}),
            None,
        );
        check_plot(&mut session, &mut app, &plotted, false);
    }
}

// Eliminate I0 analytically and minimize just D, without the production solver
// or numerical derivatives. The final covariance uses the analytic Jacobian.
fn independent_reference(b: &[f64], y: &[f64]) -> (f64, f64, f64) {
    let objective = |scaled_d: f64| {
        let e: Vec<f64> = b.iter().map(|b| (-scaled_d * 1e-9 * b).exp()).collect();
        let i0 = y.iter().zip(&e).map(|(y, e)| y * e).sum::<f64>()
            / e.iter().map(|e| e * e).sum::<f64>();
        let sse = y
            .iter()
            .zip(&e)
            .map(|(y, e)| (y - i0 * e).powi(2))
            .sum::<f64>();
        (sse, i0)
    };
    let (mut lo, mut hi) = (0., 100.);
    let ratio = (5_f64.sqrt() - 1.) / 2.;
    for _ in 0..150 {
        let left = hi - ratio * (hi - lo);
        let right = lo + ratio * (hi - lo);
        if objective(left).0 < objective(right).0 {
            hi = right;
        } else {
            lo = left;
        }
    }
    let scaled_d = (lo + hi) / 2.;
    let (sse, i0) = objective(scaled_d);
    let d = scaled_d * 1e-9;
    let (mut aa, mut ad, mut dd) = (0., 0., 0.);
    for b in b {
        let e = (-d * b).exp();
        let derivative = -i0 * b * e;
        aa += e * e;
        ad += e * derivative;
        dd += derivative * derivative;
    }
    let se = (sse / (y.len() - 2) as f64 * aa / (aa * dd - ad * ad)).sqrt();
    (d, sse, se)
}
