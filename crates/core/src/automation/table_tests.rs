use super::*;
use crate::state::{Dataset, PlotiumApp};
use plotium_data::{ColumnId, ColumnSchema, LogicalType, RevisionId, TableSchema, UnitSpec};
use serde_json::{Value, json};
use std::{collections::BTreeSet, path::PathBuf};
struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!("plotium-r3-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn put(&self, name: &str, text: &str) -> PathBuf {
        let p = self.0.join(name);
        std::fs::write(&p, text).unwrap();
        p
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
fn request(app: &PlotiumApp, tool: &str, params: Value) -> ToolRequest {
    ToolRequest {
        tool_id: tool.into(),
        tool_version: 1,
        targets: TargetSelector::Explicit {
            ids: params
                .get("dataset_id")
                .and_then(Value::as_str)
                .map(|s| vec![s.into()])
                .unwrap_or_default(),
        },
        parameters: params,
        expected_revision: DocumentRevision(app.doc.automation_revision),
        caller: CallerType::Agent,
    }
}
fn import(app: &mut PlotiumApp, params: Value) -> ToolResult {
    let plan = plan_tool(app, request(app, "data.import", params.clone())).unwrap();
    let prepared =
        PreparedTableImport::prepare(serde_json::from_value(params).unwrap(), || false).unwrap();
    execute_prepared_table_import(app, plan, prepared, ExecutionAuthority::ReversibleModify)
        .unwrap()
}
fn read(app: &mut PlotiumApp, tool: &str, params: Value) -> Result<ToolResult, AutomationError> {
    let plan = plan_tool(app, request(app, tool, params))?;
    execute_tool(app, plan, ExecutionAuthority::Read)
}
fn address(app: &PlotiumApp) -> Value {
    let Dataset::Table(t) = &app.doc.datasets[0] else {
        panic!()
    };
    json!({"dataset_id":t.resource_id,"table_revision":t.typed_state.envelope.revision.id})
}
#[test]
fn csv_tsv_equivalence_null_text_exact_ints_and_frozen_preflight() {
    let files = Files::new();
    let csv=files.put("a.csv","time,signal,note,big\n0,10,first,9007199254740993\n1,,\"two, words\",9223372036854775807\n2,oops,last,3\n");
    let tsv=files.put("a.tsv","time\tsignal\tnote\tbig\n0\t10\tfirst\t9007199254740993\n1\t\ttwo, words\t9223372036854775807\n2\toops\tlast\t3\n");
    let mut app = PlotiumApp::new();
    let canvases = app.doc.canvases.len();
    let params = json!({"mode":"table_data_only","paths":[csv,tsv],"header":"present"});
    let plan = plan_tool(&app, request(&app, "data.import", params.clone())).unwrap();
    let prepared =
        PreparedTableImport::prepare(serde_json::from_value(params).unwrap(), || false).unwrap();
    assert!(app.doc.datasets.is_empty());
    assert_eq!(prepared.preflight()["items"][0]["status"], "prepared");
    std::fs::write(&csv, "broken!").unwrap();
    let result = execute_prepared_table_import(
        &mut app,
        plan,
        prepared,
        ExecutionAuthority::ReversibleModify,
    )
    .unwrap();
    assert_eq!(result.value["status"], "succeeded");
    assert_eq!(app.doc.canvases.len(), canvases);
    assert_eq!(app.doc.datasets.len(), 2);
    let Dataset::Table(a) = &app.doc.datasets[0] else {
        panic!()
    };
    let Dataset::Table(b) = &app.doc.datasets[1] else {
        panic!()
    };
    let ar = a.typed_rows(10, &[]).unwrap();
    let br = b.typed_rows(10, &[]).unwrap();
    for (a, b) in ar.columns.iter().zip(&br.columns) {
        assert_eq!(a.schema.logical_type, b.schema.logical_type);
        assert_eq!(a.values, b.values);
        assert_eq!(a.schema.unit, None);
        assert_eq!(a.schema.metadata["import.unit_source"], "unknown");
    }
    assert_eq!(ar.columns[1].schema.logical_type, LogicalType::Utf8);
    assert_eq!(ar.columns[3].schema.logical_type, LogicalType::Int64);
    let params = address(&app);
    let rows = read(&mut app, TABLE_READ_ROWS, params).unwrap();
    assert_eq!(
        rows.value["rows"][0]["values"][3]["value"],
        "9007199254740993"
    );
    assert_eq!(rows.value["rows"][1]["values"][1]["kind"], "null");
    let ids = result.produced.clone();
    app.undo();
    assert!(app.doc.datasets.is_empty());
    app.redo();
    assert_eq!(app.doc.datasets[0].resource_id().to_string(), ids[0].id);
    assert_eq!(app.doc.datasets[1].resource_id().to_string(), ids[1].id);
    let Dataset::Table(t) = &app.doc.datasets[0] else {
        panic!()
    };
    assert_eq!(t.typed_rows(10, &[]).unwrap().row_ids, ar.row_ids);
}
#[test]
fn mixed_errors_report_each_item_and_no_failed_item_is_installed() {
    let f = Files::new();
    let good = f.put("good.csv", "x,y\n1,2\n");
    let bad = f.put("bad.csv", "x,y\n1,2,3\n");
    let missing = f.0.join("missing.csv");
    let mut app = PlotiumApp::new();
    let result = import(
        &mut app,
        json!({"mode":"table_data_only","paths":[bad,good,missing],"header":"present"}),
    );
    assert_eq!(result.value["status"], "partial");
    assert_eq!(result.value["items"][0]["status"], "failed");
    assert_eq!(result.value["items"][1]["status"], "succeeded");
    assert_eq!(result.produced.len(), 1);
    assert_eq!(app.doc.datasets.len(), 1);
    assert_eq!(app.session.undo_stack.len(), 1);
    assert!(!app.doc.automation_runs.last().unwrap().errors.is_empty());
    let old = app.doc.automation_revision;
    let result = import(
        &mut app,
        json!({"mode":"table_data_only","paths":[bad],"header":"present"}),
    );
    assert_eq!(result.value["status"], "failed");
    assert_eq!(app.doc.automation_revision, old);
    assert!(result.produced.is_empty());
}
#[test]
fn declarations_are_strict_and_sources_are_not_guessed() {
    let f = Files::new();
    let csv = f.put("a.csv", "time,current\n0,2\n1,3\n");
    let mut x = ColumnSchema::new("time", LogicalType::Int64);
    x.unit = Some(UnitSpec::dimensionless("1"));
    let y = ColumnSchema::new("current", LogicalType::Float64);
    let schema = TableSchema::new(vec![x, y]).unwrap();
    let mut app = PlotiumApp::new();
    let result = import(
        &mut app,
        json!({"mode":"table_data_only","paths":[csv],"header":"present","schema":schema}),
    );
    assert_eq!(result.value["status"], "succeeded");
    let columns = &result.value["items"][0]["metadata"]["columns"];
    assert_eq!(columns[0]["type_source"], "user");
    assert_eq!(columns[0]["unit_source"], "user");
    assert_eq!(columns[1]["unit_source"], "unknown");
    let sidecar = f.put(
        "schema.json",
        &json!({"schema_version":1,"table_id":plotium_data::TableId::new(),"schema":schema})
            .to_string(),
    );
    let result = import(
        &mut app,
        json!({"mode":"table_data_only","paths":[csv],"header":"present","schema_path":sidecar}),
    );
    assert_eq!(
        result.value["items"][0]["metadata"]["columns"][0]["type_source"],
        "file"
    );
    let bad = f.put("bad.csv", "time,current\n0,oops\n");
    let result = import(
        &mut app,
        json!({"mode":"table_data_only","paths":[bad],"header":"present","schema":schema}),
    );
    assert_eq!(result.value["status"], "failed");
    assert_eq!(app.doc.datasets.len(), 2);
    let result = import(
        &mut app,
        json!({"mode":"table_data_only","paths":[csv],"header":"absent","schema":schema}),
    );
    assert_eq!(result.value["status"], "failed");
}
#[test]
fn full_pagination_crosses_batches_with_stable_row_ids_and_projection() {
    let f = Files::new();
    let mut text = String::from("index,value,note\n");
    for i in 0..70003 {
        text.push_str(&format!("{i},{},row{i}\n", i * 3));
    }
    let csv = f.put("large.csv", &text);
    let mut app = PlotiumApp::new();
    import(
        &mut app,
        json!({"mode":"table_data_only","paths":[csv],"header":"present"}),
    );
    let Dataset::Table(t) = &app.doc.datasets[0] else {
        panic!()
    };
    let original = t.typed_rows(80000, &[]).unwrap();
    let first = t.typed_state.envelope.revision.snapshot.schema.columns[0].id;
    let note = t.typed_state.envelope.revision.snapshot.schema.columns[2].id;
    let mut p = address(&app);
    p["limit"] = json!(1000);
    p["column_ids"] = json!([note, first]);
    let mut offset = 0;
    let mut ids = BTreeSet::new();
    loop {
        p["offset"] = json!(offset);
        let page = read(&mut app, TABLE_READ_ROWS, p.clone()).unwrap();
        let rows = page.value["rows"].as_array().unwrap();
        assert_eq!(page.value["total_rows"], 70003);
        assert!(!rows.is_empty());
        for (j, row) in rows.iter().enumerate() {
            let index = offset as usize + j;
            assert_eq!(row["row_id"], json!(original.row_ids[index]));
            assert!(ids.insert(row["row_id"].as_str().unwrap().to_owned()));
            assert_eq!(row["values"][0]["value"], format!("row{index}"));
            assert_eq!(row["values"][1]["value"], index.to_string());
        }
        offset += rows.len() as u64;
        if page.value["end"] == true {
            assert!(page.value["next_offset"].is_null());
            break;
        }
        assert_eq!(page.value["next_offset"], offset);
    }
    assert_eq!(offset, 70003);
    assert_eq!(ids.len(), 70003);
    p["offset"] = json!(10000);
    p["limit"] = json!(1);
    let row = read(&mut app, TABLE_READ_ROWS, p).unwrap();
    assert_eq!(
        row.value["rows"][0]["row_id"],
        json!(original.row_ids[10000])
    );
}
#[test]
fn invalid_revision_target_projection_and_budgets_fail_without_fallback() {
    let f = Files::new();
    let csv = f.put("one.csv", "x\n1\n2\n");
    let mut app = PlotiumApp::new();
    import(
        &mut app,
        json!({"mode":"table_data_only","paths":[csv],"header":"present"}),
    );
    let base = address(&app);
    for (key, value) in [
        ("offset", json!(-1)),
        ("offset", json!(3)),
        ("limit", json!(0)),
        ("limit", json!(1001)),
        ("column_ids", json!([ColumnId::new()])),
        ("dataset_id", json!(crate::state::DatasetId::new())),
    ] {
        let mut p = base.clone();
        p[key] = value;
        assert!(read(&mut app, TABLE_READ_ROWS, p).is_err(), "{key}");
    }
    let mut p = base.clone();
    p["table_revision"] = json!(RevisionId::new());
    assert!(matches!(
        read(&mut app, TABLE_READ_ROWS, p),
        Err(AutomationError::StaleTableRevision { .. })
    ));
    let mut p = base.clone();
    p["offset"] = json!(2);
    let end = read(&mut app, TABLE_READ_ROWS, p).unwrap();
    assert_eq!(end.value["returned_rows"], 0);
    assert_eq!(end.value["end"], true);
    let plan = plan_tool(&app, request(&app, TABLE_COLUMNS, base)).unwrap();
    app.undo();
    assert!(matches!(
        execute_tool(&mut app, plan, ExecutionAuthority::Read),
        Err(AutomationError::StaleRevision { .. })
    ));
}
#[test]
fn byte_budget_pages_whole_rows_and_rejects_one_oversized_row() {
    let f = Files::new();
    let wide = "z".repeat(10000);
    let csv = f.put(
        "wide.csv",
        &format!(
            "x,note\n{}",
            (0..40).map(|i| format!("{i},{wide}\n")).collect::<String>()
        ),
    );
    let mut app = PlotiumApp::new();
    import(
        &mut app,
        json!({"mode":"table_data_only","paths":[csv],"header":"present"}),
    );
    let p = address(&app);
    let page = read(&mut app, TABLE_READ_ROWS, p).unwrap();
    assert!(page.value["returned_rows"].as_u64().unwrap() < 40);
    assert!(serde_json::to_vec(&page).unwrap().len() <= TABLE_RESPONSE_BYTES);
    assert_eq!(page.value["end"], false);
    let huge = f.put(
        "huge.csv",
        &format!("x,note\n1,{}\n", "z".repeat(TABLE_RESPONSE_BYTES)),
    );
    import(
        &mut app,
        json!({"mode":"table_data_only","paths":[huge],"header":"present"}),
    );
    let Dataset::Table(t) = &app.doc.datasets[1] else {
        panic!()
    };
    let p = json!({"dataset_id":t.resource_id,"table_revision":t.typed_state.envelope.revision.id});
    assert!(matches!(
        read(&mut app, TABLE_READ_ROWS, p),
        Err(AutomationError::ResponseTooLarge)
    ));
}
#[test]
fn prepared_import_rechecks_authority_and_revision_and_does_not_read_neighbors() {
    let f = Files::new();
    let csv = f.put("a.csv", "x,y\n1,2\n");
    f.put(
        "a.plotium-schema.json",
        "invalid schema deliberately ignored",
    );
    let mut app = PlotiumApp::new();
    let params = json!({"mode":"table_data_only","paths":[csv],"header":"present"});
    let plan = plan_tool(&app, request(&app, "data.import", params.clone())).unwrap();
    let prepared =
        PreparedTableImport::prepare(serde_json::from_value(params.clone()).unwrap(), || false)
            .unwrap();
    assert!(matches!(
        execute_prepared_table_import(&mut app, plan, prepared, ExecutionAuthority::Read),
        Err(AutomationError::InsufficientAuthority { .. })
    ));
    assert!(app.doc.datasets.is_empty());
    let plan = plan_tool(&app, request(&app, "data.import", params.clone())).unwrap();
    let prepared =
        PreparedTableImport::prepare(serde_json::from_value(params).unwrap(), || false).unwrap();
    app.doc.automation_revision += 1;
    assert!(matches!(
        execute_prepared_table_import(
            &mut app,
            plan,
            prepared,
            ExecutionAuthority::ReversibleModify
        ),
        Err(AutomationError::StaleRevision { .. })
    ));
    assert!(app.doc.datasets.is_empty());
}

#[test]
fn strict_declarations_never_mask_bad_cells_or_drop_metadata() {
    let f = Files::new();
    let csv = f.put("typed.CSV", "x,note\n1,hello\n2,\n");
    let mut schema = TableSchema::new(vec![
        ColumnSchema::new("x", LogicalType::Int64),
        ColumnSchema::new("note", LogicalType::Utf8),
    ])
    .unwrap();
    schema
        .metadata
        .insert("source_label".into(), json!("declared"));
    let mut app = PlotiumApp::new();
    let result = import(
        &mut app,
        json!({"mode":"table_data_only","paths":[csv],"header":"present","schema":schema}),
    );
    assert_eq!(result.value["status"], "succeeded");
    assert_eq!(
        result.value["items"][0]["metadata"]["columns"][0]["source_column_id"],
        json!(schema.columns[0].id)
    );
    let Dataset::Table(table) = &app.doc.datasets[0] else {
        panic!()
    };
    assert_eq!(
        table.typed_state.envelope.revision.snapshot.schema.metadata["source_label"],
        "declared"
    );
    assert!(table.typed_state.envelope.revision.snapshot.metadata["space.nmrtist.plotium.import.inference"].as_array().unwrap().is_empty());
    let missing = f.put("missing.csv", "x,note\n1,\n");
    schema.columns[1].logical_type = LogicalType::Duration;
    let result = import(
        &mut app,
        json!({"mode":"table_data_only","paths":[missing],"header":"present","schema":schema}),
    );
    assert_eq!(result.value["status"], "failed");
    assert_eq!(app.doc.datasets.len(), 1);
    let plain = f.put("plain.csv", "x,note\n9223372036854775808,hello\n");
    let result = import(
        &mut app,
        json!({"mode":"table_data_only","paths":[plain],"header":"present"}),
    );
    assert_eq!(
        result.value["items"][0]["metadata"]["columns"][0]["logical_type"]["kind"],
        "utf8"
    );
}
#[test]
fn import_and_page_budgets_and_duplicate_columns_are_enforced() {
    let f = Files::new();
    let p = f.0.join("too-large.csv");
    std::fs::File::create(&p)
        .unwrap()
        .set_len(TABLE_IMPORT_FILE_BYTES + 1)
        .unwrap();
    let mut app = PlotiumApp::new();
    let result = import(
        &mut app,
        json!({"mode":"table_data_only","paths":[p],"header":"present"}),
    );
    assert_eq!(result.value["status"], "failed");
    assert!(app.doc.datasets.is_empty());
    let mut text = (0..21)
        .map(|i| format!("c{i}"))
        .collect::<Vec<_>>()
        .join(",");
    text.push('\n');
    text.push_str(&vec!["1"; 21].join(","));
    let p = f.put("wide.csv", &text);
    import(
        &mut app,
        json!({"mode":"table_data_only","paths":[p],"header":"present"}),
    );
    let mut p = address(&app);
    p["limit"] = json!(1000);
    assert!(read(&mut app, TABLE_READ_ROWS, p.clone()).is_err());
    let Dataset::Table(t) = &app.doc.datasets[0] else {
        panic!()
    };
    let id = t.typed_state.envelope.revision.snapshot.schema.columns[0].id;
    p["limit"] = json!(1);
    p["column_ids"] = json!([id, id]);
    assert!(read(&mut app, TABLE_READ_ROWS, p).is_err());
    let mut request = request(
        &app,
        "data.import",
        json!({"mode":"table_data_only","paths":[f.0.join("unused.csv")]}),
    );
    request.targets = TargetSelector::CurrentSelection;
    assert!(matches!(
        plan_tool(&app, request),
        Err(AutomationError::InvalidSelector(_))
    ));
}

#[test]
fn native_import_shares_bounded_schema_and_rejects_non_explicit_paths() {
    let app = PlotiumApp::new();
    let paths = vec!["relative-acquisition.dx"; 17];
    assert!(plan_tool(&app, request(&app, "data.import", json!({"paths":paths}))).is_err());
    let registry = ToolRegistry::built_in();
    let schema = &registry.get("data.import").unwrap().parameter_schema;
    assert_eq!(schema["type"], "object");
    assert_eq!(schema["properties"]["paths"]["type"], "array");
    assert_eq!(schema["properties"]["paths"]["maxItems"], 16);
    assert_eq!(schema["properties"]["paths"]["minItems"], 1);
}
