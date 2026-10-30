use super::*;
use plotium_core::actions::Action;
#[cfg(unix)]
use plotium_core::properties::{AggregateValue, PropertyAddress, PropertyValue, app_preferences};
use plotium_core::settings::Settings;
use plotium_core::state::{
    BoardFitTarget, CanvasDocument, DEFAULT_CANVAS_SIZE_MM, FrameRef, ViewportMode,
};

fn app_with_active_canvases(count: usize) -> PlotiumApp {
    let mut app = PlotiumApp::new_with_settings(Settings::default());
    for index in 0..count {
        app.doc.canvases.push(CanvasDocument::new(
            format!("page {index}"),
            DEFAULT_CANVAS_SIZE_MM,
        ));
    }
    app.session.active_canvas = (count > 0).then_some(0);
    app
}

#[cfg(unix)]
fn catalog_snap(app: &mut PlotiumApp, enabled: bool) {
    let commit = app
        .plan_property_write(
            app_preferences::SNAP_ENABLED,
            std::slice::from_ref(&app.app_target()),
            &PropertyValue::Bool(enabled),
        )
        .expect("the Preferences catalog row plans");
    app.commit_property(commit);
}

#[test]
fn zoom_to_fit_command_seeds_springs_from_the_live_camera() {
    let mut app = app_with_active_canvases(1);
    let ctx = egui::Context::default();
    app.session.board.zoom = 2.5;
    app.session.board.world_center = [30.0, -40.0];

    for (id, stale) in [
        ("board_fit_zoom", 9.0),
        ("board_fit_center_x", 900.0),
        ("board_fit_center_y", -900.0),
    ] {
        crate::ui::switcher::animate_spring(&ctx, egui::Id::new(id), stale, 0.0);
    }

    execute_without_clipboard(CommandId::ZoomToFit, &mut app, &ctx);

    assert_eq!(
        app.session.viewport_mode,
        ViewportMode::Fit(BoardFitTarget::AllFrames)
    );
    for (id, live) in [
        ("board_fit_zoom", 2.5),
        ("board_fit_center_x", 30.0),
        ("board_fit_center_y", -40.0),
    ] {
        let seeded = crate::ui::switcher::animate_spring(&ctx, egui::Id::new(id), live, 0.0);
        assert_eq!(seeded, live);
    }
    assert_eq!(app.session.status, "Fit all 1 board frame to view.");
}

#[test]
fn zoom_to_selection_command_reports_one_selected_frame() {
    let mut app = app_with_active_canvases(1);
    let ctx = egui::Context::default();
    app.session.ui.selection_scope = plotium_core::state::SelectionScope::Board;
    app.session.ui.frame_selection =
        vec![plotium_core::state::board_frame_id(&app, FrameRef::Page(0)).unwrap()];

    execute_without_clipboard(CommandId::ZoomToSelection, &mut app, &ctx);

    assert_eq!(app.session.status, "Fit 1 selected board item to view.");
}

#[test]
fn new_panel_frame_avoids_existing_panel_overlap_when_space_exists() {
    let mut page = CanvasDocument::new("page".to_owned(), [120.0, 90.0]);
    let first = next_panel_frame(&page);
    page.create_panel("Panel".to_owned(), first);
    let second = next_panel_frame(&page);
    assert!(
        second.x >= first.x + first.width
            || second.x + second.width <= first.x
            || second.y >= first.y + first.height
            || second.y + second.height <= first.y
    );
}

#[cfg(unix)]
fn resolved_snap(app: &PlotiumApp) -> AggregateValue<PropertyValue> {
    app.resolve_property(&PropertyAddress::new(
        app.app_target(),
        app_preferences::SNAP_ENABLED,
    ))
    .expect("snap resolves through the catalog")
    .value
}

#[test]
#[cfg(unix)]
fn settings_toolbar_and_toggle_command_share_the_snap_catalog_value() {
    crate::test_settings::isolated(
        "ui::command_exec::tests::settings_toolbar_and_toggle_command_share_the_snap_catalog_value",
        || {
            let mut app = PlotiumApp::new_with_settings(Settings::default());

            // Preferences rows submit this same catalog write.
            catalog_snap(&mut app, false);
            assert_eq!(
                resolved_snap(&app),
                AggregateValue::Uniform(PropertyValue::Bool(false))
            );

            // The canvas toolbar keeps its existing setter surface, whose
            // implementation now plans and commits the catalog property.
            app.set_snap_enabled(true);
            assert_eq!(
                resolved_snap(&app),
                AggregateValue::Uniform(PropertyValue::Bool(true))
            );

            let mut clipboard = ClipboardTablePaste::default();
            execute(
                CommandId::ToggleSnap,
                &mut app,
                &mut clipboard,
                &egui::Context::default(),
            );
            assert_eq!(
                resolved_snap(&app),
                AggregateValue::Uniform(PropertyValue::Bool(false))
            );
            assert!(!app.settings.general.snap_enabled);
        },
    );
}

#[test]
fn symmetry_command_opens_its_review_surface_and_toggles_cleanly() {
    let mut app = PlotiumApp::new_with_settings(Settings::default());
    let action = Action::insert_dataset_with_default_canvas(
        &app,
        crate::ui::properties::fixture::homonuclear_frequency_2d(),
        "COSY review".to_owned(),
        DEFAULT_CANVAS_SIZE_MM,
    );
    app.execute_action(action);
    app.session.secondary_sidebar_visible = false;
    let mut clipboard = ClipboardTablePaste::default();
    let ctx = egui::Context::default();

    execute(
        CommandId::Tool(Tool::Symmetry),
        &mut app,
        &mut clipboard,
        &ctx,
    );
    assert_eq!(app.session.tool, Tool::Symmetry);
    assert!(app.session.secondary_sidebar_visible);
    assert!(app.session.ui.requested_tool_group == Some(ToolGroup::Nmr2dExperiment));

    execute(
        CommandId::Tool(Tool::Symmetry),
        &mut app,
        &mut clipboard,
        &ctx,
    );
    assert_eq!(app.session.tool, Tool::BrowseZoom);
}

#[test]
fn cursor_command_has_a_stable_three_press_cycle_on_eligible_2d_data() {
    let mut app = PlotiumApp::new_with_settings(Settings::default());
    let action = Action::insert_dataset_with_default_canvas(
        &app,
        crate::ui::properties::fixture::homonuclear_frequency_2d(),
        "COSY cursors".to_owned(),
        DEFAULT_CANVAS_SIZE_MM,
    );
    app.execute_action(action);
    let mut clipboard = ClipboardTablePaste::default();
    let ctx = egui::Context::default();

    for expected in [
        Tool::InspectCursor,
        Tool::DeltaCursor,
        Tool::Symmetry,
        Tool::InspectCursor,
    ] {
        execute(CommandId::CycleCursor, &mut app, &mut clipboard, &ctx);
        assert_eq!(app.session.tool, expected);
    }
    assert!(app.session.status.contains("Cursor 1/3"));
    assert!(app.session.secondary_sidebar_visible);
    assert!(app.session.ui.requested_tool_group == Some(ToolGroup::Nmr2dExperiment));
}

#[test]
fn board_commands_resolve_tables_and_ignore_stale_selected_frames() {
    use plotium_core::state::{BoardFrameId, CanvasId, SelectionScope};
    let mut app = app_with_active_canvases(0);
    app.new_table_dataset();
    app.new_table_dataset();
    let ctx = egui::Context::default();
    assert!(commands::describe(&app, CommandId::ZoomToFit).enabled);
    execute_without_clipboard(CommandId::ZoomToFit, &mut app, &ctx);
    assert!(app.session.board_reveal.is_none());
    assert_eq!(
        app.session.viewport_mode,
        ViewportMode::Fit(BoardFitTarget::AllFrames)
    );
    assert_eq!(app.session.status, "Fit all 2 board frames to view.");
    app.session.ui.selection_scope = SelectionScope::Board;
    app.session.ui.frame_selection = vec![
        BoardFrameId::Page(CanvasId::new()),
        BoardFrameId::Sheet(app.doc.datasets[0].resource_id()),
    ];
    execute_without_clipboard(CommandId::ZoomToSelection, &mut app, &ctx);
    assert_eq!(app.session.status, "Fit 1 selected board item to view.");
    app.session.ui.frame_selection.clear();
    app.session.viewport_mode = ViewportMode::Manual;
    let status = app.session.status.clone();
    execute_without_clipboard(CommandId::ZoomToSelection, &mut app, &ctx);
    assert_eq!(app.session.viewport_mode, ViewportMode::Manual);
    assert_eq!(app.session.status, status);
}
