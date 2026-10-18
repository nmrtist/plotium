use crate::session::Session;
use plotium_core::{automation::*, state::*};
use serde_json::{Value, json};

pub(super) fn fixture(contour: bool) -> (PlotiumApp, String) {
    let mut app = PlotiumApp::new();
    app.session.project_present = true;
    let dataset = if contour {
        let dim = |nucleus: &str| plotium_io::Dim {
            spectral_width_hz: 4000.0,
            observe_freq_mhz: 400.0,
            carrier_ppm: 0.0,
            nucleus: nucleus.into(),
            group_delay: 0.0,
        };
        Dataset::Nmr2D(Box::new(
            Nmr2DDataset::load(plotium_io::NmrData2D {
                data: (0..16)
                    .map(|v| num_complex::Complex64::new(f64::from(v) - 7.0, 0.5))
                    .collect(),
                rows: 4,
                cols: 4,
                domain: plotium_io::Domain::Frequency,
                direct: dim("1H"),
                indirect: dim("13C"),
                quad: plotium_io::QuadMode::Complex,
                indirect_conjugate: false,
                experiment: None,
                pseudo_axis: None,
                diffusion: None,
                nus: None,
                source: "contour fixture".into(),
            })
            .unwrap(),
        ))
    } else {
        Dataset::Nmr(Box::new(
            NmrDataset::load(plotium_io::NmrData {
                points: (0..32)
                    .map(|v| num_complex::Complex64::new(f64::from(v), 0.0))
                    .collect(),
                domain: plotium_io::Domain::Frequency,
                spectral_width_hz: 4000.0,
                observe_freq_mhz: 400.0,
                carrier_ppm: 0.0,
                nucleus: "1H".into(),
                source: "line fixture".into(),
                group_delay: 0.0,
            })
            .unwrap(),
        ))
    };
    app.doc.datasets.push(dataset);
    let mut canvas = CanvasDocument::new("Figure".into(), [120.0, 80.0]);
    let object_id = canvas.allocate_object_id();
    let object = app.build_plot_object(
        0,
        ObjectFrame::new(0.0, 0.0, 100.0, 80.0),
        object_id,
        "Plot".into(),
    );
    canvas.objects.push(object);
    let id = format!("{}/{}", canvas.resource_id, object_id);
    app.doc.canvases.push(canvas);
    revision(&mut app);
    (app, id)
}
fn revision(app: &mut PlotiumApp) -> u64 {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while app.compute_busy() && std::time::Instant::now() < deadline {
        app.poll_compute();
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(!app.compute_busy(), "fixture computation finished");
    app.doc.automation_revision
}
fn invoke(
    session: &mut Session,
    app: &mut PlotiumApp,
    id: &str,
    tool: &str,
    parameters: Value,
) -> Value {
    let context = app.session.project_context;
    let rev = revision(app);
    let plan = session
        .call(
            app,
            "plotium_plan",
            json!({"tool_id": tool, "parameters":parameters,"ids":[id],"expected_project_context":context,"expected_revision":rev}),
        )
        .unwrap();
    session
        .call(app, "plotium_execute", json!({"plan_id":plan["plan_id"]}))
        .unwrap()
}
#[test]
fn line_and_contour_properties_roundtrip_through_shared_catalog() {
    for (contour, key, value) in [
        (false, "series.line.stroke_width", json!(2.5)),
        (true, "series.contour.line_width", json!(2.5)),
        (true, "series.contour.count", json!(12)),
    ] {
        let (mut app, id) = fixture(contour);
        let context = app.session.project_context;
        app.session.project_present = true;
        let mut session = Session::default();
        session.allow_modify = true;
        let rev = revision(&mut app);
        let inspect = session
            .call(
                &mut app,
                "plotium_properties_inspect",
                json!({"ids":[id],"key":key,"expected_project_context":context,"expected_revision":rev}),
            )
            .unwrap();
        let before = inspect["value"]["readings"][0]["value"].clone();
        let result = invoke(
            &mut session,
            &mut app,
            &id,
            "properties.set",
            json!({"key":key,"value":value}),
        );
        assert_eq!(result["targets"][0]["outcome"], "succeeded");
        let rev = revision(&mut app);
        let inspect = session
            .call(
                &mut app,
                "plotium_properties_inspect",
                json!({"ids":[id],"key":key,"expected_project_context":context,"expected_revision":rev}),
            )
            .unwrap();
        assert_eq!(
            inspect["value"]["readings"][0]["value"]["value"]["value"],
            value
        );
        assert!(
            inspect["value"]["readings"][0]["schema"]
                .get("min")
                .is_some()
        );
        let result = invoke(
            &mut session,
            &mut app,
            &id,
            "properties.set",
            json!({"key":key,"value":value}),
        );
        assert_eq!(result["targets"][0]["outcome"], "skipped");
        app.undo();
        let rev = revision(&mut app);
        let inspect = session
            .call(
                &mut app,
                "plotium_properties_inspect",
                json!({"ids":[id],"key":key,"expected_project_context":context,"expected_revision":rev}),
            )
            .unwrap();
        assert_eq!(inspect["value"]["readings"][0]["value"], before);
        app.redo();
        invoke(
            &mut session,
            &mut app,
            &id,
            "properties.reset",
            json!({"key":key}),
        );
        assert!(
            app.doc
                .automation_runs
                .iter()
                .all(|r| r.caller == CallerType::Agent)
        );
    }
}
#[test]
fn inapplicable_targets_are_skipped_and_deleted_targets_cannot_execute() {
    let (mut app, id) = fixture(false);
    let context = app.session.project_context;
    app.session.project_present = true;
    let mut session = Session::default();
    session.allow_modify = true;
    let result = invoke(
        &mut session,
        &mut app,
        &id,
        "properties.set",
        json!({"key":"series.contour.line_width","value":2.5}),
    );
    assert_eq!(result["targets"][0]["outcome"], "skipped");
    let rev = revision(&mut app);
    let p = session.call(&mut app, "plotium_plan", json!({"tool_id":"resource.rename","parameters":{"name":"Renamed"},"ids":[id],"expected_project_context":context,"expected_revision":rev})).unwrap();
    let object = app.doc.canvases[0].objects[0].clone();
    app.execute_action(plotium_core::actions::Action::DeleteObject {
        canvas: 0,
        index: 0,
        object: Box::new(object),
        selection_before: app.session.ui.selection.clone(),
    });
    assert_eq!(
        session
            .call(&mut app, "plotium_execute", json!({"plan_id":p["plan_id"]}))
            .unwrap_err()
            .code,
        "stale_revision"
    );
}
