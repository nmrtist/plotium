use super::*;
use crate::automation::TargetRef;
use crate::properties::{PropertyId, PropertyValue, axis, plot_display};
use crate::state::{
    Dataset, FloatSeries, PlotiumApp, document_items, materialized_float_series_table,
};
use plotium_figure::AxisScale;

/// Two table line plots composed into one free Panel: `a` (60 pt tall) above
/// `b` (20 pt tall), so a stack must give them a 3:1 share.
fn stack_app() -> (PlotiumApp, PanelId, [ObjectId; 2]) {
    let table = materialized_float_series_table(
        (
            "x".to_owned(),
            String::new(),
            (1..=50).map(|value| Some(f64::from(value))).collect(),
        ),
        vec![FloatSeries {
            name: "signal".to_owned(),
            unit: String::new(),
            values: (1..=50)
                .map(|value| Some(f64::from(value * 1000)))
                .collect(),
            uncertainty: None,
            fit: None,
        }],
        "plotium.test.shared-x-stack.v1",
    )
    .expect("table fixture");
    let mut app = PlotiumApp::new();
    app.doc.datasets.push(Dataset::Table(Box::new(table)));
    let mut canvas = CanvasDocument::new("stack".to_owned(), [120.0, 120.0]);
    let mut ids = [ObjectId::new(0); 2];
    for (index, frame) in [
        ObjectFrame::new(10.0, 10.0, 200.0, 60.0),
        ObjectFrame::new(10.0, 80.0, 200.0, 20.0),
    ]
    .into_iter()
    .enumerate()
    {
        let id = canvas.allocate_object_id();
        canvas
            .objects
            .push(app.build_plot_object(0, frame, id, format!("plot {index}")));
        ids[index] = id;
    }
    app.doc.canvases.push(canvas);
    app.session.active_canvas = Some(0);
    let (panel, action) = app
        .compose_panel_action(0, "Stack".to_owned(), &ids, 0.0)
        .expect("compose");
    app.execute_action(action);
    app.clear_history();
    (app, panel, ids)
}

fn stacked() -> (PlotiumApp, PanelId, [ObjectId; 2]) {
    let (mut app, panel, ids) = stack_app();
    let action = app
        .set_shared_x_stack_action(0, panel, true, None)
        .expect("stack");
    app.execute_action(action);
    (app, panel, ids)
}

fn frame(app: &PlotiumApp, id: ObjectId) -> ObjectFrame {
    app.doc.canvases[0].object(id).expect("member").frame
}

fn x_scale(app: &PlotiumApp, id: ObjectId) -> AxisScale {
    app.doc.canvases[0]
        .object(id)
        .and_then(|item| item.plot())
        .expect("plot")
        .figure()
        .x
        .scale
}

fn write(app: &mut PlotiumApp, id: ObjectId, property: PropertyId, value: PropertyValue) {
    let target: TargetRef = app.object_target(0, id).expect("plot target");
    let commit = app
        .plan_property_write(property, std::slice::from_ref(&target), &value)
        .expect("write plans");
    assert!(commit.skipped.is_empty(), "{:?}", commit.skipped);
    app.commit_property(commit);
}

#[test]
fn stack_fills_the_panel_in_item_order_with_frame_proportions() {
    let (app, panel, [a, b]) = stacked();
    let page = &app.doc.canvases[0];
    let panel = page.panel(panel).expect("panel");
    assert_eq!(panel.layout, PanelLayout::SharedXStack);
    assert_eq!(panel.layout_gap, 0.0, "entering a stack closes the gap");
    let pad = panel.layout_padding;
    let (fa, fb) = (frame(&app, a), frame(&app, b));
    assert_eq!((fa.x, fa.y), (pad, pad));
    assert_eq!(fa.width, panel.frame.width - 2.0 * pad);
    assert_eq!(fb.width, fa.width);
    assert!((fa.y + fa.height - fb.y).abs() < 1e-3, "members touch");
    assert!((fa.height / fb.height - 3.0).abs() < 1e-3);
    assert!((fb.y + fb.height - (panel.frame.height - pad)).abs() < 1e-3);
}

#[test]
fn stack_links_x_viewports_without_a_stored_record() {
    let (mut app, panel, [a, b]) = stacked();
    let page = &app.doc.canvases[0];
    assert!(page.x_viewport_links.is_empty());
    assert_eq!(page.linked_x_members(a), vec![a, b]);
    assert_eq!(page.linked_x_members(b), vec![a, b]);

    let action = app
        .set_shared_x_stack_action(0, panel, false, None)
        .expect("unstack");
    app.execute_action(action);
    let page = &app.doc.canvases[0];
    assert!(page.linked_x_members(a).is_empty());
    assert_eq!(page.panel(panel).unwrap().layout, PanelLayout::Free);
}

#[test]
fn placements_share_margins_and_join_only_inner_boundaries() {
    let (app, _, [a, b]) = stacked();
    let page = &app.doc.canvases[0];
    let placements = page.shared_x_stack_placements();
    let (pa, pb) = (placements[&a], placements[&b]);
    assert_eq!((pa.left, pa.right), (pb.left, pb.right));
    for id in [a, b] {
        let item = page.object(id).unwrap();
        let natural = plotium_render::axis_layout(
            item.plot().unwrap().figure(),
            item.frame.width,
            item.frame.height,
        );
        assert!(pa.left >= natural.margins.left && pa.right >= natural.margins.right);
    }
    assert!(!pa.joined_above && pa.joined_below);
    assert!(pb.joined_above && !pb.joined_below);
    assert_eq!(page.shared_x_stack_placement(b), Some(pb));

    let stacks: Vec<_> = document_items(page)
        .into_iter()
        .filter_map(|item| match item {
            plotium_render::DocumentItem::Plot(plot) => Some(plot.stack),
            _ => None,
        })
        .collect();
    assert_eq!(stacks, vec![Some(pa), Some(pb)]);
}

#[test]
fn leaving_the_stack_restores_member_rendering() {
    let (mut app, panel, [a, _]) = stacked();
    let overrides = |app: &PlotiumApp| {
        app.doc.canvases[0]
            .object(a)
            .and_then(|item| item.plot())
            .map(|plot| plot.axis_overrides.clone())
    };
    let before = overrides(&app);
    assert_eq!(before.as_ref().unwrap().x_show_tick_labels, None);
    let action = app
        .set_shared_x_stack_action(0, panel, false, None)
        .expect("unstack");
    app.execute_action(action);
    assert!(app.doc.canvases[0].shared_x_stack_placements().is_empty());
    assert_eq!(overrides(&app), before);
}

#[test]
fn x_axis_edits_fan_out_to_every_member_as_one_undo_step() {
    let (mut app, _, [a, b]) = stacked();
    app.clear_history();
    write(&mut app, b, axis::X_SCALE, PropertyValue::Enum(axis::LOG));
    assert_eq!(x_scale(&app, a), AxisScale::Log10);
    assert_eq!(x_scale(&app, b), AxisScale::Log10);
    app.undo();
    assert_eq!(x_scale(&app, a), AxisScale::Linear);
    assert_eq!(x_scale(&app, b), AxisScale::Linear);
    assert!(!app.can_undo());
    app.redo();
    assert_eq!(x_scale(&app, a), AxisScale::Log10);

    let manual = |min, max| {
        PropertyValue::AxisRange(crate::properties::AxisRangeValue::Manual {
            min,
            max,
            unit: None,
        })
    };
    write(&mut app, a, plot_display::X_RANGE, manual(2.0, 40.0));
    write(&mut app, a, plot_display::Y_RANGE, manual(1.0, 9000.0));
    let range = |id| {
        app.doc.canvases[0]
            .object(id)
            .and_then(|item| item.plot())
            .map(|plot| (plot.axis_overrides.x_range, plot.axis_overrides.y_range))
            .unwrap()
    };
    let (ax, ay) = range(a);
    let (bx, by) = range(b);
    assert_eq!(ax, bx, "x range is shared");
    assert!(ay.is_some() && by.is_none(), "y settings stay per member");
}

#[test]
fn member_height_sets_its_share_and_the_stack_reflows() {
    let (mut app, panel, [a, b]) = stacked();
    let page = &mut app.doc.canvases[0];
    let before = frame_of(page, b);
    let applied = page
        .set_content_frame(
            b,
            ObjectFrame::new(0.0, 0.0, 10.0, frame_of(page, a).height),
        )
        .expect("height edit");
    let (fa, fb) = (frame_of(page, a), frame_of(page, b));
    assert_eq!(applied, fb);
    assert!((fa.height - fb.height).abs() < 1e-3, "now 1:1");
    assert_eq!((fb.x, fb.width), (before.x, before.width));
    assert!((fa.y + fa.height - fb.y).abs() < 1e-3);
    assert_eq!(
        page.panel(panel).unwrap().item_order,
        vec![a, b],
        "order is unchanged"
    );
}

fn frame_of(page: &CanvasDocument, id: ObjectId) -> ObjectFrame {
    page.object(id).expect("member").frame
}

#[test]
fn stacking_panels_merges_them_in_page_order_as_one_undo_step() {
    let (mut app, panel, [a, b]) = stack_app();
    let action = app.split_panel_action(0, panel, &[a], "Upper".to_owned());
    let (upper, action) = action.expect("split");
    app.execute_action(action);
    // Put the split-off panel below the original so page order differs from
    // object order.
    let page = &mut app.doc.canvases[0];
    page.panel_mut(upper).unwrap().frame.y += 200.0;
    app.clear_history();
    let before = app.doc.canvases[0].panels.clone();

    let action = app
        .stack_panels_shared_x_action(0, upper, &[panel])
        .expect("stack panels");
    app.execute_action(action);
    let page = &app.doc.canvases[0];
    assert_eq!(page.panels.len(), 1);
    let merged = &page.panels[0];
    assert_eq!(merged.layout, PanelLayout::SharedXStack);
    assert_eq!(merged.item_order, vec![b, a], "top to bottom on the page");
    assert_eq!(page.linked_x_members(a), vec![b, a]);

    app.undo();
    assert_eq!(app.doc.canvases[0].panels, before);
    assert!(!app.can_undo());
}

#[test]
fn shared_x_stack_layout_survives_a_project_roundtrip() {
    let (app, panel, ids) = stacked();
    let path = std::env::var_os("CARGO_TARGET_TMPDIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join(format!("plotium-shared-x-{}.plotium", std::process::id()));
    crate::project::save_project(&app, &path, false).expect("save");
    let loaded = crate::project::load_project(&path);
    let _ = std::fs::remove_file(&path);
    let loaded = loaded.expect("load");
    let page = &loaded.doc.canvases[0];
    let restored = page.panel(panel).expect("panel");
    assert_eq!(restored.layout, PanelLayout::SharedXStack);
    assert_eq!(restored.item_order, ids.to_vec());
    assert_eq!(page.linked_x_members(ids[0]), ids.to_vec());
}

#[test]
fn zooming_one_member_moves_every_member_as_one_undo_step() {
    let (mut app, _, [a, b]) = stacked();
    app.clear_history();
    let view = |app: &PlotiumApp, id| {
        app.doc.canvases[0]
            .object(id)
            .and_then(|item| item.plot())
            .unwrap()
            .viewport
            .clone()
    };
    let before = view(&app, a);
    let mut after = before.clone();
    after.view_x = crate::state::AxisRange::new(10.0, 20.0);
    app.commit_object_viewport(0, a, before.clone(), after);
    assert_eq!(
        view(&app, b).view_x,
        crate::state::AxisRange::new(10.0, 20.0)
    );
    app.undo();
    assert_eq!(view(&app, b).view_x, before.view_x);
    assert!(!app.can_undo());
}

fn figure_edit(app: &PlotiumApp, panel: PanelId, op: serde_json::Value) -> Result<(), String> {
    let page = &app.doc.canvases[0];
    let target = TargetRef::resource(crate::automation::panel_resource(
        page,
        page.panel(panel).unwrap(),
    ));
    let mut op = op;
    op["panel"] = serde_json::json!(target);
    let params = serde_json::from_value(serde_json::json!({
        "canvas": TargetRef::resource(page.resource_id.into()),
        "edits": [op],
    }))
    .expect("params");
    app.plan_figure_edit(
        crate::automation::DocumentRevision(app.doc.automation_revision),
        &params,
    )
    .map(|_| ())
}

#[test]
fn mcp_panel_stack_stacks_and_inspect_reports_member_order() {
    use crate::automation::*;
    let (mut app, panel, [a, b]) = stack_app();
    let page = &app.doc.canvases[0];
    let target = TargetRef::resource(panel_resource(page, page.panel(panel).unwrap()));
    let params = serde_json::from_value(serde_json::json!({
        "canvas": TargetRef::resource(page.resource_id.into()),
        "edits": [{"op": "panel_stack", "panel": target, "shared_x": true, "gap_pt": 2}],
    }))
    .expect("params");
    let plan = app
        .plan_figure_edit(DocumentRevision(app.doc.automation_revision), &params)
        .expect("plan");
    let edit = &plan.result["panel_edits"][0];
    assert_eq!(edit["layout"], "shared_x_stack");
    assert_eq!(edit["gap_pt"], 2.0);
    app.commit_figure_edit(plan).expect("commit");
    let page = &app.doc.canvases[0];
    assert_eq!(page.panel(panel).unwrap().layout, PanelLayout::SharedXStack);
    assert!((frame(&app, b).y - frame(&app, a).y - frame(&app, a).height - 2.0).abs() < 1e-3);

    let request = ToolRequest {
        tool_id: DOCUMENT_INSPECT.into(),
        tool_version: 1,
        parameters: serde_json::json!({"target": target, "view": "self", "offset": 0, "limit": DOCUMENT_PAGE_DEFAULT}),
        targets: TargetSelector::Explicit {
            ids: vec![target.resource.id.clone()],
        },
        expected_revision: DocumentRevision(app.doc.automation_revision),
        caller: CallerType::Agent,
    };
    let plan = plan_tool(&app, request).expect("inspect plan");
    let value = execute_tool(&mut app, plan, ExecutionAuthority::Read)
        .expect("inspect")
        .value;
    let node = &value["node"];
    assert_eq!(node["layout"], "shared_x_stack");
    let page = &app.doc.canvases[0];
    let members: Vec<_> = [a, b]
        .map(|id| serde_json::json!(TargetRef::resource(canvas_object_ref(page.resource_id, id))))
        .into();
    assert_eq!(
        node["shared_x_stack"]["members_top_to_bottom"],
        serde_json::json!(members)
    );

    app.undo();
    assert_eq!(
        app.doc.canvases[0].panel(panel).unwrap().layout,
        PanelLayout::Free
    );
}

#[test]
fn mcp_panel_stack_rejects_single_plots_and_stray_gaps() {
    let (mut app, panel, [_, b]) = stack_app();
    let error = figure_edit(
        &app,
        panel,
        serde_json::json!({"op":"panel_stack","shared_x":false,"gap_pt":3}),
    )
    .unwrap_err();
    assert!(error.contains("gap_pt"), "{error}");
    let (_, action) = app
        .split_panel_action(0, panel, &[b], "Lone".to_owned())
        .expect("split");
    app.execute_action(action);
    let error = figure_edit(
        &app,
        panel,
        serde_json::json!({"op":"panel_stack","shared_x":true}),
    )
    .unwrap_err();
    assert!(error.contains("two plots"), "{error}");
}

#[test]
fn mcp_panel_stack_merges_plot_panels_in_page_order() {
    use crate::automation::*;
    let (mut app, panel, [a, b]) = stack_app();
    let (lower, action) = app
        .split_panel_action(0, panel, &[a], "Lower".to_owned())
        .expect("split");
    app.execute_action(action);
    app.doc.canvases[0].panel_mut(lower).unwrap().frame.y += 200.0;
    app.clear_history();
    let page = &app.doc.canvases[0];
    let target = |id| TargetRef::resource(panel_resource(page, page.panel(id).unwrap()));
    let params = serde_json::from_value(serde_json::json!({
        "canvas": TargetRef::resource(page.resource_id.into()),
        "edits": [{"op": "panel_stack", "panel": target(panel), "shared_x": true, "merge": [target(lower)]}],
    }))
    .expect("params");
    let plan = app
        .plan_figure_edit(DocumentRevision(app.doc.automation_revision), &params)
        .expect("plan");
    app.commit_figure_edit(plan).expect("commit");
    let page = &app.doc.canvases[0];
    assert_eq!(page.panels.len(), 1);
    assert_eq!(page.panels[0].item_order, vec![b, a]);
    assert_eq!(page.panels[0].layout, PanelLayout::SharedXStack);
    app.undo();
    assert_eq!(app.doc.canvases[0].panels.len(), 2);
}

#[test]
fn stacked_plots_keep_their_shared_left_edge_under_a_panel_letter() {
    let (mut app, panel, [top, _]) = stacked();
    let page = &mut app.doc.canvases[0];
    page.panel_mut(panel).expect("panel").label.mode = crate::state::PanelLabelMode::Manual {
        value: "a".to_owned(),
    };
    let page = &app.doc.canvases[0];
    let clearance = page.y_title_clearance(top);
    // The title may still move down, but never the plot sideways.
    assert!(clearance.below > 0.0, "{clearance:?}");
    assert_eq!(clearance.beside, 0.0);
    let rendered = document_items(page)
        .into_iter()
        .find_map(|item| match item {
            plotium_render::DocumentItem::Plot(plot) if plot.stack.is_some() => {
                Some(plot.y_title_clearance)
            }
            _ => None,
        });
    assert_eq!(rendered.map(|c| c.beside), Some(0.0));
}
