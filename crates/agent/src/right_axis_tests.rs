use plotium_core::{automation::*, data::*, state::*};
use serde_json::{Value, json};

fn fixture() -> PlotiumApp {
    let mut app = PlotiumApp::new();
    let cols = [
        ("time", vec![0., 1., 2.]),
        ("yield", vec![10., 20., 40.]),
        ("rate", vec![1000., 3000., 2000.]),
    ]
    .into_iter()
    .map(|(n, v)| {
        (
            ColumnSchema::new(n, LogicalType::Float64),
            ColumnChunk::optional_f64(v.into_iter().map(Some)),
        )
    })
    .collect::<Vec<_>>();
    let t = TableDataset::from_materialized(cols, vec![], None, vec![], "plotium.test.y2.agent")
        .unwrap();
    app.doc.datasets.push(Dataset::Table(Box::new(t)));
    app
}

fn create(app: &PlotiumApp, kind: &str, y_axis: &str) -> Value {
    let t = app.doc.datasets[0].as_table().unwrap();
    let r = &t.typed_state.envelope.revision;
    let c = &r.snapshot.schema.columns;
    let source = |y: usize| json!({"dataset_id":t.resource_id,"field_id":app.doc.datasets[0].default_field_id().unwrap(),"table_id":r.table_id,"table_revision":r.id,"x_column_id":c[0].id,"y_column_id":c[y].id,"y_error_column_id":null});
    json!({"expected_project_context":app.session.project_context,"expected_revision":app.doc.automation_revision,"tool_id":PLOT_CREATE,"parameters":{"parent":document_root_target(),"name":"Two axes","series":[
        {"kind":"line","source":source(1)},
        {"kind":kind,"source":source(2),"y_axis":y_axis}]}})
}

fn run(s: &mut crate::Session, app: &mut PlotiumApp, plan: Value) -> Value {
    let p = s.call(app, "plotium_plan", plan).unwrap();
    s.call(app, "plotium_execute", json!({"plan_id":p["plan_id"]}))
        .unwrap()
}

#[test]
fn plot_create_puts_curves_on_the_right_axis_and_exposes_its_properties() {
    let mut app = fixture();
    let mut s = crate::Session::default();
    s.allow_modify = true;
    let bar = create(&app, "bar", "right");
    let refused = s.call(&mut app, "plotium_plan", bar).unwrap_err();
    assert_eq!(refused.code, "invalid_parameters");
    let scatter = create(&app, "scatter", "right");
    let created = run(&mut s, &mut app, scatter);
    let figure = app.doc.canvases[0].objects[0].plot().unwrap().figure();
    assert!(figure.series[1].appearance.y_axis.is_right());
    assert_eq!(figure.y2.as_ref().unwrap().label, "rate");

    let plot = created["value"]["creation"]["plot"]["resource"]["id"].clone();
    let set = |app: &PlotiumApp, key: &str, value: Value| {
        json!({"expected_project_context":app.session.project_context,"expected_revision":app.doc.automation_revision,
            "ids":[plot],"tool_id":"properties.set","parameters":{"key":key,"value":value}})
    };
    let title = set(&app, "object.axes.y2_label", json!("Rate (Hz)"));
    run(&mut s, &mut app, title);
    let log = set(&app, "object.axes.y2_scale", json!("log"));
    run(&mut s, &mut app, log);
    let figure = app.doc.canvases[0].objects[0].plot().unwrap().figure();
    let right = figure.y2.as_ref().unwrap();
    assert_eq!(right.label, "Rate (Hz)");
    assert!(!right.scale.is_linear());

    let listed = crate::session::property_catalog();
    let ids: Vec<&str> = listed["properties"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|p| p["id"].as_str())
        .collect();
    for id in [
        "series.y_axis",
        "object.axes.y2_label",
        "object.axes.y2_range",
    ] {
        assert!(ids.contains(&id), "{id} missing from {ids:?}");
    }
}
