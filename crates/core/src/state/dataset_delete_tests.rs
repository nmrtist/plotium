use super::*;
use crate::automation::{
    AutomationError, CallerType, DATASET_DELETE, DocumentRevision, ExecutionAuthority,
    TargetSelector, ToolRequest, execute_tool, plan_tool,
};

/// A plotted table at index 0 and two unused copies after it.
fn fixture() -> PlotiumApp {
    let mut app = super::super::curve_tests::fixture(false);
    for name in ["Spare one", "Spare two"] {
        let Dataset::Table(mut table) = app.doc.datasets[0].clone() else {
            unreachable!("curve fixture is a table");
        };
        table.resource_id = DatasetId::new();
        table.name = Some(name.into());
        app.doc.datasets.push(Dataset::Table(table));
    }
    app.clear_history();
    app
}

fn ids(app: &PlotiumApp) -> Vec<DatasetId> {
    app.doc.datasets.iter().map(Dataset::resource_id).collect()
}

fn request(app: &PlotiumApp, id: DatasetId) -> ToolRequest {
    ToolRequest {
        tool_id: DATASET_DELETE.into(),
        tool_version: 1,
        parameters: serde_json::json!({}),
        targets: TargetSelector::Explicit {
            ids: vec![id.to_string()],
        },
        expected_revision: DocumentRevision(app.doc.automation_revision),
        caller: CallerType::Agent,
    }
}

#[test]
fn delete_repairs_selection_and_undo_restores_index_and_id() {
    let mut app = fixture();
    let before = ids(&app);
    app.session.ui.data_selection = vec![2, 1];
    app.session.ui.sheet_open = Some(2);

    app.delete_dataset(1);
    assert_eq!(ids(&app), [before[0], before[2]], "{}", app.session.status);
    assert_eq!(app.session.ui.data_selection, [1]);
    assert_eq!(app.session.ui.sheet_open, Some(1));
    assert_eq!(app.session.status, "Deleted dataset \"Spare one\".");

    app.undo();
    assert_eq!(ids(&app), before);
    assert_eq!(app.session.ui.data_selection, [2]);
    assert_eq!(app.session.ui.sheet_open, Some(2));

    app.redo();
    assert_eq!(ids(&app), [before[0], before[2]]);
}

#[test]
fn plotted_dataset_is_kept_and_the_panels_using_it_are_named() {
    let mut app = fixture();
    let before = ids(&app);
    app.delete_dataset(0);
    assert_eq!(ids(&app), before);
    assert!(!app.can_undo());
    let status = &app.session.status;
    assert!(
        status.contains("page \"R4 explicit bindings\" panel a"),
        "{status}"
    );
    assert!(status.contains("panel b"), "{status}");
}

#[test]
fn dataset_with_a_derived_dataset_is_kept() {
    let mut app = fixture();
    let source = app.doc.datasets[1].resource_id();
    app.doc.datasets[2].as_table_mut().unwrap().lineage = Some(DatasetLineage::new(
        DerivationKind::RelationalTransform,
        [source],
    ));
    let Err(message) = app.delete_dataset_action(1) else {
        panic!("a dataset with a derived dataset was deletable");
    };
    assert!(
        message.contains("derived dataset \"Spare two\""),
        "{message}"
    );
}

#[test]
fn tool_deletes_undoably_and_rejects_used_datasets_at_preflight() {
    let mut app = fixture();
    let before = ids(&app);

    let refused = plan_tool(&app, request(&app, before[0])).unwrap_err();
    assert!(
        matches!(&refused, AutomationError::InvalidParameters { message, .. } if message.contains("panel a")),
        "{refused}"
    );

    let plan = plan_tool(&app, request(&app, before[2])).unwrap();
    let result = execute_tool(&mut app, plan, ExecutionAuthority::ReversibleModify).unwrap();
    assert_eq!(ids(&app), before[..2]);
    assert_eq!(result.modified[0].id, before[2].to_string());

    app.undo();
    assert_eq!(ids(&app), before);
}

#[test]
fn dataset_with_a_report_is_kept() {
    let mut app = fixture();
    let dataset = app.doc.datasets[2].resource_id();
    app.doc.reports.push(AnalysisReportRecord {
        id: ReportId(1),
        name: "Run summary".into(),
        kind: ReportKindId::new("plotium.test.report"),
        source: ReportSource {
            dataset,
            craft_run: CraftRunId(0),
        },
        definition: serde_json::Value::Null,
        snapshot: serde_json::Value::Null,
        source_fingerprint: String::new(),
        schema_version: 1,
    });
    let Err(message) = app.delete_dataset_action(2) else {
        panic!("a dataset with a report was deletable");
    };
    assert!(
        message.contains("analysis report \"Run summary\""),
        "{message}"
    );
}

#[test]
fn positional_session_state_follows_its_dataset() {
    let mut app = fixture();
    let ui = &mut app.session.ui;
    ui.rename = Some(RenameState {
        target: RenameTarget::Data(2),
        buffer: "New name".into(),
        focus: false,
    });
    ui.curve_fit_task_dataset = Some(2);
    ui.fit_dataset = Some(1);
    ui.data_browser_collapsed_datasets = [0, 1, 2].into();

    app.delete_dataset(1);
    let ui = &app.session.ui;
    assert!(matches!(
        ui.rename.as_ref().map(|r| r.target),
        Some(RenameTarget::Data(1))
    ));
    assert_eq!(ui.curve_fit_task_dataset, Some(1));
    assert_eq!(ui.fit_dataset, None);
    assert_eq!(ui.data_browser_collapsed_datasets, [0, 1].into());

    app.undo();
    let ui = &app.session.ui;
    assert!(matches!(
        ui.rename.as_ref().map(|r| r.target),
        Some(RenameTarget::Data(2))
    ));
    assert_eq!(ui.curve_fit_task_dataset, Some(2));
}
