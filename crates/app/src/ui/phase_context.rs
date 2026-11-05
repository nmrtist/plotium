use plotium_core::state::{CanvasId, DatasetId, PlotiumApp, ProjectContextId, Tool};

type Context = (ProjectContextId, Option<CanvasId>, Option<DatasetId>);

pub(super) fn reconcile(app: &mut PlotiumApp, ctx: &egui::Context) {
    let key = egui::Id::new("manual_phase_context");
    if app.session.tool != Tool::ManualPhase {
        super::canvas::cancel_volume_drag(app, ctx);
        ctx.data_mut(|data| data.remove::<Context>(key));
        return;
    }
    let current = (
        app.session.project_context,
        app.session
            .active_canvas
            .and_then(|ci| app.doc.canvases.get(ci))
            .map(|c| c.resource_id),
        super::nmr_volume::phase_target(app),
    );
    let previous = ctx.data(|data| data.get_temp::<Context>(key));
    if previous.is_some_and(|previous| previous != current) {
        super::canvas::cancel_volume_drag(app, ctx);
        app.cancel_interaction();
        app.set_tool(Tool::BrowseZoom);
        ctx.data_mut(|data| data.remove::<Context>(key));
    } else {
        ctx.data_mut(|data| data.insert_temp(key, current));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changed_canvas_identity_exits_phase() {
        let mut app = PlotiumApp::new_with_settings(Default::default());
        let ctx = egui::Context::default();
        app.set_tool(Tool::ManualPhase);
        ctx.data_mut(|data| {
            data.insert_temp(
                egui::Id::new("manual_phase_context"),
                (
                    app.session.project_context,
                    Some(CanvasId::new()),
                    None::<DatasetId>,
                ),
            );
        });
        reconcile(&mut app, &ctx);
        assert_eq!(app.session.tool, Tool::BrowseZoom);
    }

    #[test]
    fn unchanged_context_retains_phase() {
        let mut app = PlotiumApp::new_with_settings(Default::default());
        let ctx = egui::Context::default();
        app.set_tool(Tool::ManualPhase);
        reconcile(&mut app, &ctx);
        reconcile(&mut app, &ctx);
        assert_eq!(app.session.tool, Tool::ManualPhase);
    }

    #[test]
    fn plane_navigation_keeps_dirty_draft_but_direct_dataset_switch_exits() {
        use plotium_core::state::{NmrVolumePhaseDraft, SelectionScope};
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../io/tests/fixtures/nmr/bruker-3d-qf");
        let loaded = plotium_io::load_path(path).unwrap();
        let (dataset, _) =
            plotium_core::workflow::dataset_from_acquisition(loaded.acquisition).unwrap();
        let id = dataset.resource_id();
        let volume = dataset.as_nmr2d().unwrap().volume.as_ref().unwrap();
        let mut draft = NmrVolumePhaseDraft {
            canvas_axis: 2,
            source: std::sync::Arc::downgrade(volume.source.dataset()),
            baseline: volume.phases,
            phases: volume.phases,
        };
        draft.phases[2].p0_degrees = 19.0;
        let mut app = PlotiumApp::new();
        app.doc.datasets.push(dataset);
        app.doc
            .datasets
            .push(crate::ui::properties::fixture::homonuclear_frequency_2d());
        app.focus_single(0);
        app.session.ui.selection_scope = SelectionScope::DataList;
        app.session
            .ui
            .nmr_volume_phase_drafts
            .insert(id, draft.clone());
        let ctx = egui::Context::default();
        app.set_tool(Tool::ManualPhase);
        reconcile(&mut app, &ctx);
        app.select_nmr_volume_plane(id, 1, 0).unwrap();
        reconcile(&mut app, &ctx);
        assert_eq!(app.session.tool, Tool::ManualPhase);
        assert_eq!(app.session.ui.nmr_volume_phase_drafts[&id].canvas_axis, 2);
        assert_eq!(
            app.session.ui.nmr_volume_phase_drafts[&id].phases,
            draft.phases
        );
        // Also cover selection paths that mutate state without focus_single.
        app.session.ui.data_selection = vec![1];
        reconcile(&mut app, &ctx);
        assert_eq!(app.session.tool, Tool::BrowseZoom);
        assert_eq!(
            app.session.ui.nmr_volume_phase_drafts[&id].phases,
            draft.phases
        );
    }
}
