//! Author intent on a plot: set at creation, edited as properties, undone,
//! and saved in the project.
use super::*;
use crate::automation::*;
use crate::properties::{PropertyAddress, PropertyValue, intent};
use plotium_data::{ColumnChunk, ColumnSchema, ColumnValues, LogicalType};
use serde_json::json;

fn fixture() -> PlotiumApp {
    let mut app = PlotiumApp::new();
    let table = TableDataset::from_materialized(
        vec![
            (
                ColumnSchema::new("group", LogicalType::Utf8),
                ColumnChunk::all_valid(ColumnValues::Utf8(
                    ["PEM", "BPM"].map(String::from).to_vec(),
                )),
            ),
            (
                ColumnSchema::new("fe", LogicalType::Float64),
                ColumnChunk::optional_f64([Some(10.0), Some(0.03)]),
            ),
        ],
        vec![],
        None,
        vec![],
        "plotium.test.intent",
    )
    .unwrap();
    app.doc.datasets.push(Dataset::Table(Box::new(table)));
    app.clear_history();
    app
}

fn create(app: &mut PlotiumApp, intent: Option<PlotIntent>) -> Result<TargetRef, String> {
    let dataset = &app.doc.datasets[0];
    let r = &dataset.as_table().unwrap().typed_state.envelope.revision;
    let c = &r.snapshot.schema.columns;
    let source = TableCurveSource {
        dataset_id: dataset.resource_id(),
        field_id: dataset.default_field_id().unwrap(),
        table_id: r.table_id,
        table_revision: r.id,
        x_column_id: c[0].id,
        y_column_id: c[1].id,
        y_error_column_id: None,
    };
    let params = PlotCreateParams {
        parent: document_root_target(),
        name: "Faradaic efficiency".into(),
        canvas: None,
        panel_name: None,
        panel_frame_pt: None,
        series: vec![NewCurve {
            kind: CurveKind::Bar,
            source,
            y_axis: plotium_figure::YAxisSide::Left,
        }],
        intent,
    };
    let planned = app.plan_plot_creation(DocumentRevision(app.doc.automation_revision), &params)?;
    let plot = planned.creation.clone().unwrap().plot;
    app.commit_curve_bindings(planned)?;
    Ok(plot)
}

fn plot_intent(app: &PlotiumApp) -> &PlotIntent {
    &app.doc.canvases[0].objects[0].plot().unwrap().intent
}

fn run(app: &mut PlotiumApp, plot: &TargetRef, tool: &str, parameters: serde_json::Value) {
    try_run(app, plot, tool, parameters).unwrap();
}

fn try_run(
    app: &mut PlotiumApp,
    plot: &TargetRef,
    tool: &str,
    parameters: serde_json::Value,
) -> Result<(), AutomationError> {
    let request = ToolRequest {
        tool_id: tool.into(),
        tool_version: 1,
        parameters,
        targets: TargetSelector::Explicit {
            ids: vec![plot.resource.id.clone()],
        },
        expected_revision: DocumentRevision(app.doc.automation_revision),
        caller: CallerType::Agent,
    };
    let plan = plan_tool(app, request)?;
    execute_tool(app, plan, ExecutionAuthority::ReversibleModify).map(drop)
}

fn read(app: &PlotiumApp, plot: &TargetRef, key: crate::properties::PropertyId) -> PropertyValue {
    let resolved = app
        .resolve_property(&PropertyAddress::new(plot.clone(), key))
        .unwrap();
    match resolved.value {
        crate::properties::AggregateValue::Uniform(value) => value,
        other => panic!("{other:?}"),
    }
}

#[test]
fn plot_create_records_the_stated_intent() {
    let mut app = fixture();
    let plot = create(
        &mut app,
        Some(PlotIntent {
            purpose: Some(PlotPurpose::Change),
            claim: "  BPM suppresses Cl formation over 100-fold.  ".into(),
        }),
    )
    .unwrap();
    assert_eq!(plot_intent(&app).purpose, Some(PlotPurpose::Change));
    assert_eq!(
        plot_intent(&app).claim,
        "BPM suppresses Cl formation over 100-fold."
    );
    assert_eq!(
        read(&app, &plot, intent::PURPOSE),
        PropertyValue::Enum("change")
    );

    let mut app = fixture();
    let long = PlotIntent {
        purpose: None,
        claim: "x".repeat(CLAIM_MAX_CHARS + 1),
    };
    assert!(create(&mut app, Some(long)).is_err());
}

#[test]
fn intent_properties_edit_reset_and_undo() {
    let mut app = fixture();
    let plot = create(&mut app, None).unwrap();
    assert_eq!(
        read(&app, &plot, intent::PURPOSE),
        PropertyValue::Enum("unspecified")
    );
    assert_eq!(
        read(&app, &plot, intent::CLAIM),
        PropertyValue::Text(String::new())
    );
    let figure = app.doc.canvases[0].objects[0]
        .plot()
        .unwrap()
        .figure()
        .clone();

    run(
        &mut app,
        &plot,
        "properties.set",
        json!({"key":"object.intent.purpose","value":"compare"}),
    );
    run(
        &mut app,
        &plot,
        "properties.set",
        json!({"key":"object.intent.claim","value":"PEM makes far more Cl than BPM."}),
    );
    assert_eq!(plot_intent(&app).purpose, Some(PlotPurpose::Compare));
    assert_eq!(plot_intent(&app).claim, "PEM makes far more Cl than BPM.");
    // Intent is advisory: the drawing does not change.
    assert_eq!(
        format!(
            "{:?}",
            app.doc.canvases[0].objects[0].plot().unwrap().figure()
        ),
        format!("{figure:?}")
    );

    app.undo();
    assert!(plot_intent(&app).claim.is_empty());
    assert_eq!(plot_intent(&app).purpose, Some(PlotPurpose::Compare));
    app.redo();
    assert_eq!(plot_intent(&app).claim, "PEM makes far more Cl than BPM.");

    run(
        &mut app,
        &plot,
        "properties.reset",
        json!({"key":"object.intent.claim"}),
    );
    run(
        &mut app,
        &plot,
        "properties.set",
        json!({"key":"object.intent.purpose","value":"unspecified"}),
    );
    assert!(plot_intent(&app).is_empty());

    let rejected = [
        ("object.intent.purpose", json!("ranking")),
        (
            "object.intent.claim",
            json!("x".repeat(CLAIM_MAX_CHARS + 1)),
        ),
    ];
    for (key, value) in rejected {
        let set = json!({"key":key,"value":value});
        assert!(
            try_run(&mut app, &plot, "properties.set", set).is_err(),
            "{key}"
        );
    }
}

#[test]
fn intent_survives_save_and_load() {
    let mut app = fixture();
    let intent = PlotIntent {
        purpose: Some(PlotPurpose::Trend),
        claim: "Selectivity holds at every current density.".into(),
    };
    create(&mut app, Some(intent.clone())).unwrap();
    let base = std::env::var_os("CARGO_TARGET_TMPDIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let path = base.join(format!("plotium-intent-{}.plotium", std::process::id()));
    crate::project::save_project(&app, &path, false).unwrap();
    let loaded = crate::project::load_project(&path);
    let _ = std::fs::remove_file(&path);
    assert_eq!(plot_intent(&loaded.unwrap()), &intent);
}

#[test]
fn purpose_choices_follow_the_purpose_list() {
    let Some(crate::properties::PropertyDefinition {
        value_schema: crate::properties::ValueSchema::Enum { variants },
        ..
    }) = crate::properties::definition(intent::PURPOSE)
    else {
        panic!("purpose is an enum property");
    };
    let ids: Vec<&str> = variants.iter().map(|v| v.id).collect();
    let mut expected = vec!["unspecified"];
    expected.extend(PlotPurpose::ALL.map(PlotPurpose::id));
    assert_eq!(ids, expected);
}
