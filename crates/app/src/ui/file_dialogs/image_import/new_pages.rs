use super::*;

pub(super) fn commit(
    app: &mut PlotiumApp,
    job: &mut ImportJob,
    results: Vec<Result<Candidate, Failure>>,
) {
    let sizes: Vec<_> = results
        .iter()
        .filter_map(|result| result.as_ref().ok())
        .map(|candidate| CanvasDocument::new(String::new(), candidate.auto_page_size_mm).size_pt())
        .collect();
    let positions = plotium_core::state::imported_board_positions(
        app,
        &sizes,
        app.session.ui.board_viewport_aspect,
    );
    let first_canvas = app.doc.canvases.len();
    let active_before = app.session.active_canvas;
    let selection = app.session.ui.selection.clone();
    let hierarchical = app.session.ui.hierarchical_selection.clone();
    let panel_label = app.session.ui.panel_label_selection;
    let plot_part = app.session.ui.plot_part_selection;
    let data_selection = app.session.ui.data_selection.clone();
    let view = app.session.view;
    let mut failures = Vec::new();
    let mut warnings = Vec::new();
    let mut staged_assets: Vec<AssetRecord> = Vec::new();
    let mut page_actions = Vec::new();
    let mut imported = 0usize;

    for result in results {
        let candidate = match result {
            Ok(candidate) => candidate,
            Err(error) => {
                failures.push(error);
                continue;
            }
        };
        let asset = app
            .doc
            .assets
            .iter()
            .find_map(|(id, asset)| (asset.sha256 == candidate.sha256).then_some(*id))
            .or_else(|| {
                staged_assets
                    .iter()
                    .find_map(|asset| (asset.sha256 == candidate.sha256).then_some(asset.id))
            })
            .unwrap_or_else(AssetId::new);
        if !app.doc.assets.contains_key(&asset)
            && !staged_assets.iter().any(|record| record.id == asset)
        {
            staged_assets.push(AssetRecord {
                id: asset,
                sha256: candidate.sha256,
                format: candidate.format.clone(),
                pixel_size: candidate.pixel_size,
                bytes: candidate.bytes.as_ref().clone(),
            });
        }

        let index = first_canvas + imported;
        let name = candidate.display_name().to_owned();
        let mut page = CanvasDocument::new(name, candidate.auto_page_size_mm);
        page.board_pos = positions[imported];
        let id = page.allocate_object_id();
        let [width, height] = page.size_pt();
        page.objects.push(CanvasObject {
            id,
            name: candidate.display_name().to_owned(),
            frame: ObjectFrame::new(0.0, 0.0, width, height),
            locked: false,
            visible: true,
            kind: CanvasObjectKind::RasterImage({
                let mut image = RasterImageContent::new(asset);
                image.page_index = candidate.page_index;
                image
            }),
        });
        page.create_panel_for_content(id)
            .expect("a raster image supports a one-item panel");
        let mut empty = CanvasDocument::new(page.name.clone(), page.size_mm);
        empty.board_pos = page.board_pos;
        let mut insert = Action::insert_canvas(index, empty.clone(), active_before);
        if let Action::InsertCanvas {
            auto_place,
            activate,
            ..
        } = &mut insert
        {
            *auto_place = false;
            *activate = false;
        }
        page_actions.push(insert);
        page_actions.push(Action::ReplacePanelState {
            canvas: index,
            before: PanelState::of(&empty),
            after: PanelState::of(&page),
        });
        insert_proxy(
            app,
            candidate.sha256,
            candidate.page_index,
            candidate.preview,
        );
        if let Some(warning) = candidate.warning {
            warnings.push((candidate.basename, candidate.format, warning));
        }
        imported += 1;
    }

    if imported > 0 {
        let mut actions: Vec<_> = staged_assets
            .into_iter()
            .map(|asset| Action::SetAsset {
                id: asset.id,
                before: None,
                after: Some(asset),
            })
            .collect();
        actions.extend(page_actions);
        match app.try_execute_action(Action::Composite(actions)) {
            Ok(()) => job.state = ImportImageState::Committed,
            Err(error) => {
                imported = 0;
                job.state = ImportImageState::Failed;
                failures.push(failure(
                    "<batch>",
                    "unknown",
                    "commit",
                    error.to_string(),
                    "Retry the import; if it still fails, review the diagnostic history.",
                ));
            }
        }
    } else {
        job.state = ImportImageState::Failed;
    }
    app.session.active_canvas = active_before;
    app.session.ui.selection = selection;
    app.session.ui.hierarchical_selection = hierarchical;
    app.session.ui.panel_label_selection = panel_label;
    app.session.ui.plot_part_selection = plot_part;
    app.session.ui.data_selection = data_selection;
    app.session.view = view;
    record_result(app, job.operation, imported, failures, warnings);
}
