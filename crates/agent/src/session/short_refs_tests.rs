use super::*;
use crate::Session;
use plotium_core::{
    data::*,
    state::{Dataset, TableDataset},
};

fn fixture() -> (PlotiumApp, Session) {
    let mut app = PlotiumApp::new_with_settings(Default::default());
    let columns = [
        ("time", vec![Some(0.), Some(1.), Some(2.)]),
        ("s1", vec![Some(1.), Some(2.), Some(3.)]),
        ("s2", vec![Some(3.), Some(2.), Some(1.)]),
        ("err", vec![Some(0.1), Some(0.1), Some(0.2)]),
    ]
    .into_iter()
    .map(|(name, values)| {
        (
            ColumnSchema::new(name, LogicalType::Float64),
            ColumnChunk::optional_f64(values),
        )
    })
    .collect();
    let table =
        TableDataset::from_materialized(columns, vec![], None, vec![], "plotium.test.short")
            .unwrap();
    app.doc.datasets.push(Dataset::Table(Box::new(table)));
    let mut session = Session::default();
    session.set_allow_modify(true);
    (app, session)
}

fn full_source(app: &PlotiumApp, y: usize, error: Option<usize>) -> Value {
    let table = app.doc.datasets[0].as_table().unwrap();
    let r = &table.typed_state.envelope.revision;
    let c = &r.snapshot.schema.columns;
    json!({"dataset_id":table.resource_id,"field_id":app.doc.datasets[0].default_field_id().unwrap(),
        "table_id":r.table_id,"table_revision":r.id,"x_column_id":c[0].id,"y_column_id":c[y].id,
        "y_error_column_id":error.map(|e| c[e].id)})
}

fn plan(app: &mut PlotiumApp, session: &mut Session, tool: &str, parameters: Value) -> Value {
    let args = json!({"expected_project_context":app.session.project_context,
        "expected_revision":app.doc.automation_revision,"tool_id":tool,"parameters":parameters});
    session.call(app, "plotium_plan", args).unwrap()
}

fn stored_sources(session: &Session, plan: &Value) -> Value {
    let stored = &session.plans[plan["plan_id"].as_str().unwrap()];
    json!(
        stored
            .binding_plan
            .as_ref()
            .unwrap()
            .bindings
            .iter()
            .map(|b| &b.source)
            .collect::<Vec<_>>()
    )
}

#[test]
fn column_names_resolve_to_the_same_frozen_sources_as_full_ids() {
    let (mut app, mut session) = fixture();
    let dataset = app.doc.datasets[0].resource_id();
    let root = json!(plotium_core::automation::document_root_target());
    let (first, second) = (full_source(&app, 1, None), full_source(&app, 2, Some(3)));
    let full = plan(
        &mut app,
        &mut session,
        PLOT_CREATE,
        json!({"parent":root,"name":"Full","series":[
            {"kind":"line","source":first},
            {"kind":"line","source":second}]}),
    );
    let mut without_error = full_source(&app, 1, None);
    without_error
        .as_object_mut()
        .unwrap()
        .remove("y_error_column_id");
    let short = plan(
        &mut app,
        &mut session,
        PLOT_CREATE,
        json!({"parent":root,"name":"Short","series":[
            {"kind":"line","source":without_error},
            {"kind":"line","source":{"dataset_id":dataset,"x":"time","y":"s2","y_error":"err"}}]}),
    );
    assert_eq!(
        stored_sources(&session, &full),
        stored_sources(&session, &short)
    );
    assert!(full.get("resolved_sources").is_none());
    assert_eq!(
        short["resolved_sources"],
        json!([full_source(&app, 2, Some(3))])
    );
    let result = session
        .call(
            &mut app,
            "plotium_execute",
            json!({"plan_id":short["plan_id"]}),
        )
        .unwrap();
    assert_eq!(result["summary"]["failed"], 0);
}

#[test]
fn short_series_targets_plan_like_full_targets() {
    let (mut app, mut session) = fixture();
    let dataset = app.doc.datasets[0].resource_id();
    let created = plan(
        &mut app,
        &mut session,
        PLOT_CREATE,
        json!({"parent":plotium_core::automation::document_root_target(),"name":"P",
            "series":[{"kind":"line","source":{"dataset_id":dataset,"x":"time","y":"s1"}}]}),
    );
    let result = session
        .call(
            &mut app,
            "plotium_execute",
            json!({"plan_id":created["plan_id"]}),
        )
        .unwrap();
    let target = result["value"]["bindings"][0]["target"].clone();
    let plot = target["resource"]["id"].as_str().unwrap().to_owned();
    let write = |target: Value| json!({"writes":[{"target":target,"key":"series.line.color","value":"#123456"}]});
    let full = plan(
        &mut app,
        &mut session,
        "properties.set_many",
        write(target.clone()),
    );
    let short = plan(
        &mut app,
        &mut session,
        "properties.set_many",
        write(json!({"plot":plot,"series":0})),
    );
    let stored = |p: &Value| {
        session.plans[p["plan_id"].as_str().unwrap()]
            .request
            .parameters
            .clone()
    };
    assert_eq!(stored(&full), stored(&short));
    let mut bare = target.clone();
    let resource = bare["resource"].as_object_mut().unwrap();
    resource.remove("parent_id");
    resource.remove("local_id");
    let args = json!({"expected_project_context":app.session.project_context,
        "expected_revision":app.doc.automation_revision,"targets":[bare],"key":"series.line.color"});
    let read = session
        .call(&mut app, "plotium_properties_inspect", args)
        .unwrap();
    assert_eq!(read["value"]["readings"][0]["target"], target);
}

#[test]
fn completed_ids_match_every_project_descriptor() {
    let (app, _, _) = crate::guide_fixture::fixture();
    let provider = plotium_core::automation::ProjectResourceProvider::new(&app);
    use plotium_core::automation::ResourceProvider;
    let mut checked = 0;
    for descriptor in provider.descriptors() {
        let full = json!(descriptor.resource);
        if full["parent_id"].is_null() {
            continue;
        }
        let mut bare = json!({"id":full["id"],"kind":full["kind"]});
        complete_resource(&mut bare);
        assert_eq!(bare, full);
        checked += 1;
    }
    assert!(checked > 0);
}

#[test]
fn unknown_and_mixed_columns_report_their_path_and_candidates() {
    let (mut app, mut session) = fixture();
    let dataset = app.doc.datasets[0].resource_id();
    let mut call = |source: Value| {
        let args = json!({"expected_project_context":app.session.project_context,
            "expected_revision":app.doc.automation_revision,"tool_id":PLOT_CREATE,
            "parameters":{"parent":plotium_core::automation::document_root_target(),"name":"P",
            "series":[{"kind":"line","source":source}]}});
        session.call(&mut app, "plotium_plan", args).unwrap_err()
    };
    let error = call(json!({"dataset_id":dataset,"x":"time","y":"S1"}));
    assert_eq!(error.code, "invalid_parameters");
    let details = error.details.unwrap();
    assert_eq!(details["field"], "parameters.series[0].source.y");
    assert_eq!(details["candidates"].as_array().unwrap().len(), 4);
    let error = call(json!({"dataset_id":dataset,"x":"time","y":"s1","x_column_id":"x"}));
    assert_eq!(
        error.details.unwrap()["field"],
        "parameters.series[0].source.x_column_id"
    );
    let error = call(json!({"dataset_id":uuid::Uuid::new_v4(),"x":"time","y":"s1"}));
    assert_eq!(
        error.details.unwrap()["field"],
        "parameters.series[0].source.dataset_id"
    );
    assert!(session.plans.is_empty());
}

#[test]
fn served_schemas_offer_the_short_forms() {
    let create = widened(PLOT_CREATE, plotium_core::automation::plot_create_schema());
    let source = &create["properties"]["series"]["items"]["properties"]["source"]["anyOf"];
    assert_eq!(source.as_array().unwrap().len(), 2);
    assert!(
        !source[0]["required"]
            .as_array()
            .unwrap()
            .contains(&json!("y_error_column_id"))
    );
    assert_eq!(source[1]["required"], json!(["dataset_id", "x", "y"]));
    let plan = crate::protocol::definitions()
        .into_iter()
        .find(|t| t.name == "plotium_plan")
        .unwrap();
    let items = &json!(plan.input_schema)["properties"]["targets"]["items"]["anyOf"];
    assert_eq!(items[0], series_schema());
}
