use super::pseudo_tests::{synthetic_dosy, wait_for_compute};
use super::*;
use crate::{DiffusionInput, DosyMethod};

pub(super) fn direct_input_stack() -> (Nmr2DDataset, DiffusionInput) {
    let mut raw = synthetic_dosy(1.2e-9);
    let meta = raw.diffusion.take().unwrap();
    let values: Vec<_> = raw
        .pseudo_axis
        .take()
        .unwrap()
        .values
        .iter()
        .map(|g| meta.b_factor(*g))
        .collect();
    let input = DiffusionInput::direct(&values, "s/m²").unwrap();
    let data = Nmr2DDataset::load_with_pipeline(
        raw,
        Some(Params2D::default_for(Preset2D::Dosy)),
        Some(false),
        None,
        false,
    )
    .unwrap();
    (data, input)
}

#[test]
fn direct_b_without_gradient_supports_both_maps_regions_and_undo() {
    let (data, input) = direct_input_stack();
    assert!(data.data.pseudo_axis.is_none());
    assert!(data.data.diffusion.is_none());
    assert!(data.resolve_diffusion_input(None).is_err());
    let mut app = PlotiumApp::new_with_settings(crate::settings::Settings::default());
    app.doc.datasets.push(Dataset::Nmr2D(Box::new(data)));
    app.set_diffusion_input(0, Some(input.clone()));
    let data = app.doc.datasets[0].as_nmr2d().unwrap();
    assert_eq!(data.resolve_diffusion_input(None).unwrap(), input);
    assert!(app.doc.datasets[0].supports_region_analysis());
    app.request_dosy_map(0);
    wait_for_compute(&mut app);
    let data = app.doc.datasets[0].as_nmr2d().unwrap();
    let mono = data
        .dosy_provenance
        .as_ref()
        .expect("mono result")
        .encoding
        .as_ref()
        .unwrap()
        .clone();
    assert_eq!(mono, input);
    assert!(
        data.dosy_map
            .as_ref()
            .unwrap()
            .d
            .iter()
            .any(|v| v.is_finite())
    );
    app.request_ilt_map_with_params(
        0,
        Some(crate::IltParams {
            n_grid: 16,
            ..crate::IltParams::default()
        }),
    );
    wait_for_compute(&mut app);
    let data = app.doc.datasets[0].as_nmr2d().unwrap();
    assert_eq!(
        data.ilt_provenance.as_ref().unwrap().encoding.as_ref(),
        Some(&mono)
    );
    let data = app.doc.datasets[0].as_nmr2d_mut().unwrap();
    let id = data.region_analysis.allocate_region_id().unwrap();
    data.region_analysis.regions.push(Region {
        id,
        lo: 0.8,
        hi: 1.2,
        name: "Signal".into(),
        label_position: None,
        color: [100, 100, 100],
        metric: None,
    });
    app.create_region_table(0);
    let table = app.doc.datasets[1].as_table().expect("region table");
    assert_eq!(table.meta.diffusion_input.as_ref(), Some(&mono));
    let view = table.fit_analysis_view().unwrap();
    assert_eq!(
        view.x.values,
        input
            .b_values(16)
            .unwrap()
            .into_iter()
            .map(Some)
            .collect::<Vec<_>>()
    );
    let mut edited = input.clone();
    for increment in &mut edited.increments {
        increment.b *= 2.0;
    }
    app.set_diffusion_input(0, Some(edited.clone()));
    let data = app.doc.datasets[0].as_nmr2d().unwrap();
    assert!(data.dosy_map.is_none() && data.ilt_map.is_none());
    assert_eq!(data.resolve_diffusion_input(None).unwrap(), edited);
    app.undo();
    assert_eq!(
        app.doc.datasets[0]
            .as_nmr2d()
            .unwrap()
            .resolve_diffusion_input(None)
            .unwrap(),
        input
    );
    app.redo();
    assert_eq!(
        app.doc.datasets[0]
            .as_nmr2d()
            .unwrap()
            .resolve_diffusion_input(None)
            .unwrap(),
        edited
    );
}

#[test]
fn input_edit_cancels_pending_map_and_rejects_its_result() {
    let (mut data, input) = direct_input_stack();
    data.diffusion_input = Some(input.clone());
    let mut app = PlotiumApp::new_with_settings(crate::settings::Settings::default());
    app.doc.datasets.push(Dataset::Nmr2D(Box::new(data)));
    app.request_dosy_map(0);
    assert!(app.compute_busy());
    app.set_diffusion_input(0, None);
    wait_for_compute(&mut app);
    let data = app.doc.datasets[0].as_nmr2d().unwrap();
    assert!(data.dosy_map.is_none());
    assert!(data.resolve_diffusion_input(None).is_err());
    app.undo();
    assert_eq!(
        app.doc.datasets[0]
            .as_nmr2d()
            .unwrap()
            .resolve_diffusion_input(None)
            .unwrap(),
        input
    );
}

#[test]
fn saved_direct_input_and_result_roundtrip_without_gradient_metadata() {
    let (mut data, input) = direct_input_stack();
    data.diffusion_input = Some(input.clone());
    assert!(data.build_dosy_map());
    let diagnostics = data.dosy_provenance.as_ref().unwrap().columns.clone();
    assert!(!diagnostics.is_empty());
    let mut app = PlotiumApp::new_with_settings(crate::settings::Settings::default());
    app.doc.datasets.push(Dataset::Nmr2D(Box::new(data)));
    let path = std::env::temp_dir().join(format!(
        "plotium-direct-b-roundtrip-{}.plotium",
        std::process::id()
    ));
    crate::project::save_project(&app, &path, false).unwrap();
    let loaded = crate::project::load_project(&path).unwrap();
    std::fs::remove_file(&path).unwrap();
    let data = loaded.doc.datasets[0].as_nmr2d().unwrap();
    assert_eq!(data.diffusion_input.as_ref(), Some(&input));
    assert_eq!(
        data.dosy_provenance.as_ref().unwrap().encoding.as_ref(),
        Some(&input)
    );
    assert!(data.dosy_provenance_warning.is_none());
    assert!(data.dosy_map.is_some());
    assert_eq!(data.dosy_method, DosyMethod::MonoExp);
    assert_eq!(data.dosy_provenance.as_ref().unwrap().columns, diagnostics);
}

fn app_with_direct_region() -> (PlotiumApp, DiffusionInput) {
    let (mut data, input) = direct_input_stack();
    data.diffusion_input = Some(input.clone());
    let id = data.region_analysis.allocate_region_id().unwrap();
    data.region_analysis.regions.push(Region {
        id,
        lo: 0.8,
        hi: 1.2,
        name: "Signal".into(),
        label_position: None,
        color: [100, 100, 100],
        metric: None,
    });
    let mut app = PlotiumApp::new_with_settings(crate::settings::Settings::default());
    app.doc.datasets.push(Dataset::Nmr2D(Box::new(data)));
    (app, input)
}

#[test]
fn frozen_region_retains_encoding_after_source_clear_and_project_roundtrip() {
    let (mut app, input) = app_with_direct_region();
    app.freeze_region_table(0);
    app.set_diffusion_input(0, None);
    let table = app.doc.datasets[1].as_table().unwrap();
    assert!(table.provenance.is_none());
    assert_eq!(table.meta.diffusion_input.as_ref(), Some(&input));
    assert_eq!(table.fit_analysis_view().unwrap().x.values.len(), 16);
    let path = std::env::temp_dir().join(format!(
        "plotium-frozen-b-roundtrip-{}.plotium",
        std::process::id()
    ));
    crate::project::save_project(&app, &path, false).unwrap();
    let loaded = crate::project::load_project(&path).unwrap();
    std::fs::remove_file(path).unwrap();
    let table = loaded.doc.datasets[1].as_table().unwrap();
    assert!(table.provenance.is_none());
    assert_eq!(table.meta.diffusion_input.as_ref(), Some(&input));
}

#[test]
fn clearing_only_ruler_invalidates_linked_values_and_fits_and_undo_refreshes() {
    let (mut app, input) = app_with_direct_region();
    app.create_region_table(0);
    let column = app.doc.datasets[1].as_table().unwrap().series_bindings[0].value_column;
    app.fit_table_columns(
        1,
        plotium_analysis::models::DIFFUSION_B_ID,
        true,
        column,
        false,
        plotium_analysis::fit_model::FitOptions::default(),
    );
    let table = app.doc.datasets[1].as_table().unwrap();
    assert!(
        !table.curve_fit_analyses.is_empty(),
        "{}",
        app.session.status
    );
    assert!(table.series_bindings[0].fit.is_some());
    let original_revision = table.typed_state.envelope.revision.id;
    let original_fits = table.curve_fit_analyses.clone();
    let original_bindings = table.series_bindings.clone();
    app.set_diffusion_input(0, None);
    let table = app.doc.datasets[1].as_table().unwrap();
    assert!(table.provenance.is_some());
    assert!(table.meta.diffusion_input.is_none());
    assert!(table.curve_fit_analyses.is_empty());
    assert!(
        table
            .series_bindings
            .iter()
            .all(|binding| binding.fit.is_none())
    );
    assert_eq!(table.typed_state.envelope.revision.snapshot.row_count, 0);
    app.undo();
    let table = app.doc.datasets[1].as_table().unwrap();
    assert_eq!(table.meta.diffusion_input.as_ref(), Some(&input));
    assert_eq!(table.fit_analysis_view().unwrap().x.values.len(), 16);
    assert_eq!(table.typed_state.envelope.revision.id, original_revision);
    assert!(table.curve_fit_analyses == original_fits);
    assert!(table.series_bindings == original_bindings);
    app.redo();
    assert_eq!(
        app.doc.datasets[1]
            .as_table()
            .unwrap()
            .typed_state
            .envelope
            .revision
            .snapshot
            .row_count,
        0
    );
}
