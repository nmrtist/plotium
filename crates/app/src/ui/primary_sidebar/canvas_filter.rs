use std::collections::HashSet;

use plotium_core::state::{CanvasId, PlotiumApp};

use crate::ui::data_navigation::PlotIndex;

/// Uses the same query and metadata filters as the dataset browser. Canvas
/// names and image layer names remain searchable without inventing datasets.
pub(crate) fn matching_canvases(app: &PlotiumApp, index: &PlotIndex) -> HashSet<CanvasId> {
    let terms: Vec<_> = app
        .session
        .ui
        .data_browser_filter
        .split_whitespace()
        .map(str::to_lowercase)
        .collect();
    let tag = app.session.ui.data_browser_tag.as_deref();
    let batch = app.session.ui.data_browser_batch;
    if terms.is_empty() && tag.is_none() && batch.is_none() {
        return app
            .doc
            .canvases
            .iter()
            .map(|canvas| canvas.resource_id)
            .collect();
    }
    let mut members = HashSet::new();
    let mut paths = std::collections::HashMap::new();
    for import in app.session.data_imports.batches() {
        for item in &import.items {
            if let Ok(target) = &item.result {
                if Some(import.id) == batch {
                    members.insert(target.dataset);
                }
                paths.insert(target.dataset, item.path.to_string_lossy());
            }
        }
    }
    app.doc
        .canvases
        .iter()
        .filter(|canvas| {
            let datasets: Vec<_> = index
                .summaries
                .get(&canvas.resource_id)
                .into_iter()
                .flat_map(|s| &s.datasets)
                .filter_map(|id| app.doc.dataset_index(*id))
                .map(|di| &app.doc.datasets[di])
                .filter(|d| {
                    tag.is_none_or(|tag| d.organization().has_tag(tag))
                        && (batch.is_none() || members.contains(&d.resource_id()))
                })
                .collect();
            if (tag.is_some() || batch.is_some()) && datasets.is_empty() {
                return false;
            }
            let fields: Vec<_> = std::iter::once(canvas.name.to_lowercase())
                .chain(canvas.objects.iter().map(|o| o.name.to_lowercase()))
                .collect();
            terms.iter().all(|term| {
                fields.iter().any(|field| field.contains(term))
                    || datasets.iter().any(|d| {
                        super::data_browser::matches_dataset(
                            d,
                            std::slice::from_ref(term),
                            paths.get(&d.resource_id()).map(|p| p.as_ref()),
                        )
                    })
            })
        })
        .map(|canvas| canvas.resource_id)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use plotium_core::state::CanvasDocument;

    #[test]
    fn canvas_search_is_case_insensitive_and_requires_every_term() {
        let mut app = PlotiumApp::new();
        app.doc
            .canvases
            .push(CanvasDocument::new("TEM sample A".into(), [100.0, 80.0]));
        app.doc
            .canvases
            .push(CanvasDocument::new("TEM control".into(), [100.0, 80.0]));
        app.session.ui.data_browser_filter = "tem SAMPLE".into();
        let index = PlotIndex::build(&app);
        assert_eq!(
            matching_canvases(&app, &index),
            [app.doc.canvases[0].resource_id].into()
        );
        app.session.ui.data_browser_filter = "TEM missing".into();
        assert!(matching_canvases(&app, &index).is_empty());
        app.session.ui.data_browser_filter.clear();
        assert_eq!(matching_canvases(&app, &index).len(), 2);
        app.session.ui.data_browser_tag = Some("control".into());
        assert!(matching_canvases(&app, &index).is_empty());
    }
    #[test]
    fn metadata_search_uses_bound_dataset_experiment_and_tags() {
        use plotium_core::state::OrganizationEdit;
        let mut app = PlotiumApp::new();
        app.load_from(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../io/tests/fixtures/nmr/jcamp-ppm.dx"),
        );
        assert_eq!(app.doc.datasets.len(), 1);
        let dataset = app.doc.datasets[0].resource_id();
        app.edit_dataset_organization(
            &[dataset],
            OrganizationEdit::Experiment(Some("Cooling run")),
        );
        app.edit_dataset_organization(&[dataset], OrganizationEdit::AddTag("Reference"));
        app.doc.canvases[0].name = "Untitled".into();
        app.session.ui.data_browser_filter = "cooling reference".into();
        let index = PlotIndex::build(&app);
        assert_eq!(matching_canvases(&app, &index).len(), 1);
        app.session.ui.data_browser_tag = Some("missing".into());
        assert!(matching_canvases(&app, &index).is_empty());
    }

    #[test]
    fn image_layer_filename_is_searchable_without_a_dataset() {
        use plotium_core::state::{
            AssetId, CanvasObject, CanvasObjectKind, ObjectFrame, RasterImageContent,
        };
        let mut app = PlotiumApp::new();
        let mut canvas = CanvasDocument::new("Figure 1".into(), [100.0, 80.0]);
        let id = canvas.allocate_object_id();
        canvas.objects.push(CanvasObject {
            id,
            name: "TEM_2026_control.png".into(),
            frame: ObjectFrame::new(0.0, 0.0, 100.0, 80.0),
            locked: false,
            visible: true,
            kind: CanvasObjectKind::RasterImage(RasterImageContent::new(AssetId::new())),
        });
        app.doc.canvases.push(canvas);
        app.session.ui.data_browser_filter = "tem_2026 CONTROL".into();
        assert_eq!(matching_canvases(&app, &PlotIndex::build(&app)).len(), 1);
        assert!(app.doc.datasets.is_empty());
    }
}
