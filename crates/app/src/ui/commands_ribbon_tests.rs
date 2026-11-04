//! Ribbon placement and gating of the Arrange grid popover and the on-plot
//! phase tool.

use super::tests::{app_with_nmr, app_with_table};
use super::*;

fn app() -> PlotiumApp {
    PlotiumApp::new_with_settings(plotium_core::settings::Settings::default())
}

/// The Ribbon's Layout group offers the grid popover in place of fixed
/// preset tiles; the presets keep their menu and palette entries.
#[test]
fn ribbon_offers_the_grid_popover_instead_of_preset_tiles() {
    assert_eq!(
        ribbon_placement(CommandId::ArrangeGridCustom),
        Some(RibbonPlacement {
            tab: WorkflowTab::Arrange,
            group: "Layout",
            priority: 0,
            applicability: Applicability::Always,
        })
    );
    for &(_, rows, cols) in plotium_core::layout::GRID_PRESETS {
        assert_eq!(ribbon_placement(CommandId::ArrangeGrid(rows, cols)), None);
        assert!(
            catalog(&app())
                .iter()
                .any(|command| command.id == CommandId::ArrangeGrid(rows, cols))
        );
    }
    let empty = describe(&app(), CommandId::ArrangeGridCustom);
    assert!(!empty.enabled);
    assert_eq!(
        empty.disabled_reason,
        Some("Open a canvas before arranging its plots.")
    );
    assert!(describe(&app_with_nmr(), CommandId::ArrangeGridCustom).enabled);
}

#[test]
fn manual_phase_explains_missing_targets_and_disabled_steps() {
    let id = CommandId::Tool(Tool::ManualPhase);
    for app in [app(), app_with_table(1)] {
        let command = describe(&app, id);
        assert!(!command.enabled);
        assert_eq!(
            command.disabled_reason,
            Some("Select an NMR spectrum before opening Phase.")
        );
    }
    let mut app = app_with_nmr();
    assert!(describe(&app, id).enabled);
    let di = app.active_dataset().unwrap();
    let pipeline = app.doc.datasets[di]
        .axis_pipeline_mut(plotium_core::state::PhaseAxis::Direct)
        .unwrap();
    for step in &mut pipeline.steps {
        if matches!(step.kind, plotium_processing::StepKind::Phase(_)) {
            step.enabled = false;
        }
    }
    let command = describe(&app, id);
    assert!(!command.enabled);
    assert!(command.disabled_reason.unwrap().contains("Phase step"));
}

#[test]
fn phase_has_one_task_entry() {
    let app = app_with_nmr();
    let interactive = describe(&app, CommandId::Tool(Tool::ManualPhase));
    let settings = describe(
        &app,
        CommandId::PropertyGroup(super::super::properties::panel::PHASE_SECTION),
    );
    assert_eq!(interactive.label, "Phase");
    assert_eq!(settings.label, "Phase settings");
    assert!(interactive.enabled && settings.enabled);
    assert!(interactive.ribbon.is_some() && settings.ribbon.is_none());
    assert_eq!(interactive.checked, Some(false));
    assert_eq!(settings.checked, None);
    assert!(
        !catalog(&app)
            .iter()
            .any(|command| command.id == settings.id)
    );
}

#[test]
fn interactive_phase_remains_a_toggle_for_standalone_spectra() {
    let mut app = app_with_nmr();
    let ctx = egui::Context::default();
    let tool = CommandId::Tool(Tool::ManualPhase);
    execute_without_clipboard(tool, &mut app, &ctx);
    assert_eq!(app.session.tool, Tool::ManualPhase);
    assert_eq!(describe(&app, tool).checked, Some(true));
    execute_without_clipboard(tool, &mut app, &ctx);
    assert_ne!(app.session.tool, Tool::ManualPhase);
    assert_eq!(describe(&app, tool).checked, Some(false));
}

#[test]
fn phase_dispatches_true_2d_to_the_canvas() {
    let mut app = app();
    let action = plotium_core::actions::Action::insert_dataset_with_default_canvas(
        &app,
        super::super::properties::fixture::homonuclear_frequency_2d(),
        "Contour".to_owned(),
        plotium_core::state::DEFAULT_CANVAS_SIZE_MM,
    );
    app.execute_action(action);
    let id = CommandId::Tool(Tool::ManualPhase);
    let description = describe(&app, id);
    assert!(description.enabled, "{:?}", description.disabled_reason);
    execute_without_clipboard(id, &mut app, &egui::Context::default());
    assert_eq!(app.session.tool, Tool::ManualPhase);
    assert_eq!(describe(&app, id).checked, Some(true));
}

#[test]
fn phase_does_not_route_a_data_list_target_to_another_plot() {
    use plotium_core::state::SelectionScope;

    let mut app = app_with_nmr();
    let plotted = app.active_dataset().unwrap();
    let selected = app.doc.datasets.len();
    app.doc
        .datasets
        .push(super::super::properties::fixture::homonuclear_frequency_2d());
    app.focus_single(selected);
    app.session.ui.selection_scope = SelectionScope::DataList;
    let command = CommandId::Tool(Tool::ManualPhase);
    let description = describe(&app, command);
    assert!(!description.enabled);
    assert!(
        description
            .disabled_reason
            .unwrap()
            .contains("chosen spectrum")
    );
    execute_without_clipboard(command, &mut app, &egui::Context::default());
    assert_ne!(app.session.tool, Tool::ManualPhase);
    app.session.ui.selection_scope = SelectionScope::CanvasObjects;
    assert_eq!(
        super::super::nmr_volume::phase_target(&app),
        Some(app.doc.datasets[plotted].resource_id())
    );
    assert!(describe(&app, command).enabled);
}
