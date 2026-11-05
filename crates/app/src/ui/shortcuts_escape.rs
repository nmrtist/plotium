use super::*;

pub(super) fn handle_escape(app: &mut PlotiumApp, now: f64) {
    if app.interaction().is_active() {
        let phase = matches!(app.interaction(), Interaction::Phase(_));
        app.cancel_interaction();
        app.session.status = if phase {
            "Cancelled phase drag.".to_owned()
        } else {
            "Cancelled interaction.".to_owned()
        };
        return;
    }

    if app.session.ui.wheel_zoom.is_some() || app.session.ui.wheel_property.is_some() {
        app.finish_pending_wheel_zoom(now, true);
        app.finish_pending_wheel_property(now, true);
        app.session.status = "Cancelled interaction.".to_owned();
        return;
    }
    if app.session.tool == plotium_core::state::Tool::ManualPhase {
        app.toggle_tool(plotium_core::state::Tool::ManualPhase);
        app.session.status = "Exited canvas phasing.".to_owned();
        return;
    }
    // Esc steps back one level: analysis region -> plot part -> panel-letter
    // sub-selection -> selection -> active tool.
    if app.session.ui.analysis_selection.is_some() {
        if let Some(ci) = app.session.active_canvas {
            canvas::clear_canvas_interaction_state(
                app,
                ci,
                canvas::CanvasInteractionClearScope::All,
            );
        } else {
            app.session.ui.analysis_selection = None;
        }
        app.session.status = "Cleared analysis selection.".to_owned();
        return;
    }

    if app.selected_plot_part().is_some() {
        app.session.ui.plot_part_selection = None;
        app.session.status = "Selected the whole plot.".to_owned();
        return;
    }

    if app.session.ui.panel_label_selection.is_some() {
        app.session.ui.panel_label_selection = None;
        app.session.status = "Selection cleared.".to_owned();
        return;
    }

    if app
        .session
        .ui
        .hierarchical_selection
        .lead()
        .is_some_and(|path| path.panel.is_some() && path.content.is_some())
    {
        app.exit_panel_scope();
        app.session.status = "Selected parent panel.".to_owned();
        return;
    }

    if !app.session.ui.frame_selection.is_empty() {
        app.session.ui.frame_selection.clear();
        app.session.status = "Frame selection cleared.".to_owned();
        return;
    }

    if !app.session.ui.data_selection.is_empty() {
        app.focus_datasets(&[], None);
        app.session.status = "Dataset selection cleared.".to_owned();
        return;
    }

    if !matches!(app.session.ui.selection, Selection::None)
        || !app.session.ui.hierarchical_selection.paths().is_empty()
    {
        exit_to_page(app, "Selection cleared.");
        return;
    }

    let rest = app.session.tool.rest();
    if app.session.tool != rest {
        app.set_tool(rest);
        app.session.status = "Exited tool mode.".to_owned();
    }
}
