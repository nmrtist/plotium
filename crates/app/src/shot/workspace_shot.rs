use plotium_core::state::{CanvasDocument, PlotiumApp};

pub(super) fn setup(app: &mut PlotiumApp, content: bool) {
    let empty = PlotiumApp::new();
    app.doc = empty.doc;
    app.session = empty.session;
    app.session.recent_files.clear();
    if content {
        app.new_table_dataset();
        app.new_table_dataset();
        app.doc.canvases.push(CanvasDocument::new(
            "Publication figure".into(),
            [180.0, 120.0],
        ));
        app.session.ui.sheet_open = None;
        app.session.active_canvas = None;
    }
}

pub(super) fn imports(app: &mut PlotiumApp) {
    use plotium_core::state::{ImportItem, ImportTarget};
    setup(app, false);
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../io/tests/fixtures/nmr/jcamp-ppm.dx");
    for _ in 0..2 {
        app.load_from(&fixture);
    }
    let items = app
        .doc
        .datasets
        .iter()
        .map(|dataset| ImportItem {
            path: fixture.clone(),
            result: Ok(ImportTarget {
                dataset: dataset.resource_id(),
                canvas: app.doc.canvases.iter().find_map(|canvas| {
                    canvas
                        .objects
                        .iter()
                        .any(|object| {
                            object.plot().is_some_and(|plot| {
                                plot.binding.contains_dataset(dataset.resource_id())
                            })
                        })
                        .then_some(canvas.resource_id)
                }),
            }),
        })
        .chain(std::iter::once(ImportItem {
            path: "incomplete.raw".into(),
            result: Err(
                "The data file is incomplete. Select a complete acquisition to retry.".into(),
            ),
        }))
        .collect();
    let batch = app.record_completed_user_import("Imported data".into(), items);
    app.session.active_canvas = None;
    app.session.ui.data_selection.clear();
    app.request_completed_import_open(batch);
}

pub(super) fn activity(
    app: &mut PlotiumApp,
    ctx: &egui::Context,
    open: bool,
) -> Result<(), String> {
    use plotium_core::automation::{
        CallerType, DocumentRevision, ExecutionAuthority, TargetSelector, ToolRequest,
        execute_tool, plan_tool,
    };
    if !open {
        app.session.ui.diagnostics_open = false;
        return Ok(());
    }
    let dataset = app.doc.datasets[0].resource_id().to_string();
    let canvas = app.doc.canvases[0].resource_id.to_string();
    for (id, name) in [
        (dataset.clone(), "Sample signal"),
        (canvas, "Comparison figure"),
        (dataset, "Updated sample signal"),
    ] {
        let request = ToolRequest {
            tool_id: "resource.rename".into(),
            tool_version: 1,
            parameters: serde_json::json!({"name": name}),
            targets: TargetSelector::Explicit { ids: vec![id] },
            expected_revision: DocumentRevision(app.doc.automation_revision),
            caller: CallerType::Agent,
        };
        let plan = plan_tool(app, request).map_err(|error| error.to_string())?;
        execute_tool(app, plan, ExecutionAuthority::ReversibleModify)
            .map_err(|error| error.to_string())?;
    }
    crate::ui::commands::execute_without_clipboard(
        crate::ui::commands::CommandId::OperationHistory,
        app,
        ctx,
    );
    Ok(())
}

pub(super) fn scientific_conflict(app: &mut PlotiumApp, ctx: &egui::Context) -> Result<(), String> {
    use plotium_core::automation::{
        CallerType, DocumentRevision, ExecutionAuthority, TargetSelector, ToolRequest,
        execute_tool, plan_tool,
    };
    let dataset = app.doc.datasets[0].resource_id().to_string();
    for (tool_id, parameters) in [
        (
            "processing.edit",
            serde_json::json!({"edits": [{"op": "add", "axis": "direct", "kind": "Invert"}]}),
        ),
        (
            "resource.rename",
            serde_json::json!({"name": "Edited during calculation"}),
        ),
    ] {
        let request = ToolRequest {
            tool_id: tool_id.into(),
            tool_version: 1,
            parameters,
            targets: TargetSelector::Explicit {
                ids: vec![dataset.clone()],
            },
            expected_revision: DocumentRevision(app.doc.automation_revision),
            caller: CallerType::Human,
        };
        let plan = plan_tool(app, request).map_err(|error| error.to_string())?;
        execute_tool(app, plan, ExecutionAuthority::ReversibleModify)
            .map_err(|error| error.to_string())?;
    }
    crate::ui::commands::execute_without_clipboard(
        crate::ui::commands::CommandId::OperationHistory,
        app,
        ctx,
    );
    Ok(())
}
