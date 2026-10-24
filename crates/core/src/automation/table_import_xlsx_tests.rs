use super::super::*;
use crate::state::PlotiumApp;
use plotium_io::xlsx::{XlsxCell, XlsxSheet, XlsxValue, XlsxWorkbook, write_xlsx};
use serde_json::{Value, json};
use std::path::PathBuf;

fn text(value: &str) -> XlsxCell {
    XlsxCell::value(XlsxValue::Utf8(value.into()))
}

fn number(value: f64) -> XlsxCell {
    XlsxCell::value(XlsxValue::Float64(value))
}

fn sheet(name: &str, rows: Vec<Vec<XlsxCell>>) -> XlsxSheet {
    XlsxSheet {
        name: name.into(),
        hidden: false,
        rows,
    }
}

fn workbook(sheets: Vec<XlsxSheet>) -> PathBuf {
    let path =
        std::env::temp_dir().join(format!("plotium-xlsx-import-{}.xlsx", uuid::Uuid::new_v4()));
    write_xlsx(
        &path,
        &XlsxWorkbook {
            sheets,
            uses_1904_date_system: false,
            plotium_schema: None,
            diagnostics: Vec::new(),
        },
    )
    .unwrap();
    path
}

fn curve(name: &str, scale: f64) -> XlsxSheet {
    sheet(
        name,
        vec![
            vec![text("Voltage (V)"), text("Current (mA)")],
            vec![number(0.0), number(scale)],
            vec![number(1.0), number(2.0 * scale)],
        ],
    )
}

fn prepare(params: Value) -> PreparedTableImport {
    PreparedTableImport::prepare(serde_json::from_value(params).unwrap(), || false).unwrap()
}

fn request(app: &PlotiumApp, params: Value) -> ToolRequest {
    ToolRequest {
        tool_id: "data.import".into(),
        tool_version: 1,
        parameters: params,
        targets: TargetSelector::Explicit { ids: Vec::new() },
        expected_revision: DocumentRevision(app.doc.automation_revision),
        caller: CallerType::Agent,
    }
}

#[test]
fn xlsx_imports_each_worksheet_with_cells_as_a_named_table() {
    let path = workbook(vec![
        curve("Figure 1", 1.0),
        sheet("Blank", Vec::new()),
        curve("Figure 2", 5.0),
    ]);
    let params = json!({"mode":"table_data_only","paths":[path]});
    let prepared = prepare(params.clone());
    let preflight = prepared.preflight();
    assert_eq!(preflight["status"], "prepared", "{preflight}");
    let sheets = preflight["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["sheet"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(sheets, ["Figure 1", "Figure 2"]);
    assert_eq!(preflight["items"][0]["metadata"]["total_rows"], 2);

    let mut app = PlotiumApp::new();
    let plan = plan_tool(&app, request(&app, params)).unwrap();
    let result = execute_prepared_table_import(
        &mut app,
        plan,
        prepared,
        ExecutionAuthority::ReversibleModify,
    )
    .unwrap();
    std::fs::remove_file(path).unwrap();
    assert_eq!(result.value["status"], "succeeded");
    let names = app
        .doc
        .datasets
        .iter()
        .map(|dataset| dataset.name().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(names, ["Figure 1", "Figure 2"]);
    let table = app.doc.datasets[1].as_table().unwrap();
    let columns = &table.typed_state.envelope.revision.snapshot.schema.columns;
    assert_eq!(columns[1].name, "Current (mA)");
    assert_eq!(
        table.import_sources[0].metadata["space.nmrtist.plotium.import.worksheet"],
        "Figure 2"
    );
}

#[test]
fn xlsx_sheets_selects_worksheets_and_names_the_visible_ones_when_missing() {
    let path = workbook(vec![curve("Figure 1", 1.0), curve("Figure 2", 5.0)]);
    let selected = prepare(json!({"mode":"table_data_only","paths":[path],"sheets":["Figure 2"]}));
    let preflight = selected.preflight();
    assert_eq!(preflight["items"].as_array().unwrap().len(), 1);
    assert_eq!(preflight["items"][0]["sheet"], "Figure 2");

    let missing = prepare(json!({"mode":"table_data_only","paths":[path],"sheets":["Figure 3"]}));
    std::fs::remove_file(&path).unwrap();
    let error = missing.preflight()["items"][0]["error"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(error.contains("'Figure 3'"), "{error}");
    assert!(error.contains("'Figure 1', 'Figure 2'"), "{error}");
}

#[test]
fn xlsx_rejects_csv_only_options_and_csv_rejects_sheets() {
    let path = workbook(vec![curve("Figure 1", 1.0)]);
    let header = prepare(json!({"mode":"table_data_only","paths":[path],"header":"present"}));
    std::fs::remove_file(&path).unwrap();
    assert!(
        header.preflight()["items"][0]["error"]
            .as_str()
            .unwrap()
            .contains("header applies to CSV/TSV")
    );

    let csv =
        std::env::temp_dir().join(format!("plotium-xlsx-import-{}.csv", uuid::Uuid::new_v4()));
    std::fs::write(&csv, "x,y\n0,1\n").unwrap();
    let sheets = prepare(json!({"mode":"table_data_only","paths":[csv],"sheets":["Data"]}));
    std::fs::remove_file(&csv).unwrap();
    assert!(
        sheets.preflight()["items"][0]["error"]
            .as_str()
            .unwrap()
            .contains("XLSX workbooks only")
    );
}

#[test]
fn native_import_points_tables_at_table_data_only() {
    let params: NativeImportParams =
        serde_json::from_value(json!({"paths":["/data/Source Data.xlsx"]})).unwrap();
    let message = params.validate().unwrap_err().to_string();
    assert!(message.contains("mode table_data_only"), "{message}");
}

#[test]
fn xlsx_worksheet_failures_leave_the_other_worksheets_importable() {
    let path = workbook(vec![
        curve("Figure 1", 1.0),
        sheet("Notes", vec![vec![text("Source data for Fig. 1")]]),
    ]);
    let preflight = prepare(json!({"mode":"table_data_only","paths":[path]})).preflight();
    std::fs::remove_file(&path).unwrap();
    assert_eq!(preflight["status"], "partial", "{preflight}");
    assert_eq!(preflight["items"][0]["status"], "prepared");
    assert_eq!(preflight["items"][1]["sheet"], "Notes");
    assert_eq!(preflight["items"][1]["status"], "failed");
    assert_eq!(preflight["prepared_files"], 1);
}

#[test]
fn xlsx_worksheets_over_the_cell_budget_fail_before_they_are_read() {
    // A few kilobytes that would expand to more than the cell budget.
    let mut rows = vec![vec![text("x")]];
    rows.extend((0..400_000).map(|_| Vec::new()));
    rows.push(
        (0..25)
            .map(|_| XlsxCell::value(XlsxValue::Empty))
            .chain([number(1.0)])
            .collect(),
    );
    let path = workbook(vec![sheet("Sparse", rows), curve("Figure 1", 1.0)]);
    let preflight = prepare(json!({"mode":"table_data_only","paths":[path]})).preflight();
    std::fs::remove_file(&path).unwrap();
    assert_eq!(preflight["items"][0]["sheet"], "Sparse");
    let error = preflight["items"][0]["error"].as_str().unwrap();
    assert!(error.contains("cell budget"), "{error}");
    assert_eq!(preflight["items"][1]["status"], "prepared");
}

#[test]
fn xlsx_with_one_table_is_named_after_the_file() {
    let path = workbook(vec![curve("Data", 1.0), sheet("Sheet2", Vec::new())]);
    let params = json!({"mode":"table_data_only","paths":[path],"format":"xlsx"});
    let prepared = prepare(params.clone());
    let mut app = PlotiumApp::new();
    let plan = plan_tool(&app, request(&app, params)).unwrap();
    execute_prepared_table_import(
        &mut app,
        plan,
        prepared,
        ExecutionAuthority::ReversibleModify,
    )
    .unwrap();
    std::fs::remove_file(&path).unwrap();
    assert_eq!(app.doc.datasets.len(), 1);
    assert_eq!(
        app.doc.datasets[0].name().as_deref(),
        path.file_stem().and_then(|stem| stem.to_str())
    );
}

#[test]
fn xlsx_rejects_a_declared_schema() {
    let path = workbook(vec![curve("Figure 1", 1.0)]);
    let schema = plotium_data::TableSchema::new(vec![plotium_data::ColumnSchema::new(
        "x",
        plotium_data::LogicalType::Float64,
    )])
    .unwrap();
    let preflight =
        prepare(json!({"mode":"table_data_only","paths":[path],"schema":schema})).preflight();
    std::fs::remove_file(&path).unwrap();
    let error = preflight["items"][0]["error"].as_str().unwrap();
    assert!(
        error.contains("declared schema applies to CSV/TSV"),
        "{error}"
    );
}
