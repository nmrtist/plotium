use super::*;
use plotium_core::state::{
    CurveFitReference, DatasetId, DatasetLineage, DerivationKind, FloatSeries, LineShapeKind,
    MultipletPatternKind, NmrDataset, PeakMark, PeakOrigin, StoredLineFit, StoredMultiplet,
    materialized_float_series_table,
};
use plotium_core::{DisplayModeLabel, IntegralResult};
use plotium_io::{Domain, NmrData};

fn root(name: &str) -> Dataset {
    let mut dataset = NmrDataset::load_with_pipeline(
        NmrData {
            points: vec![1.0.into(), 0.0.into()],
            domain: Domain::Frequency,
            spectral_width_hz: 1.0,
            observe_freq_mhz: 1.0,
            carrier_ppm: 0.0,
            nucleus: "1H".into(),
            source: name.into(),
            group_delay: 0.0,
        },
        Some(plotium_processing::AxisPipeline { steps: Vec::new() }),
        Some(false),
    )
    .unwrap();
    dataset.name = Some(name.into());
    Dataset::Nmr(Box::new(dataset))
}

fn derived(name: &str, kind: DerivationKind, sources: &[DatasetId]) -> Dataset {
    let mut table = materialized_float_series_table(
        ("x".into(), "".into(), vec![Some(0.0)]),
        Vec::new(),
        "plotium.test.derived-table.v1",
    )
    .unwrap();
    table.name = Some(name.into());
    table.lineage = Some(DatasetLineage::new(kind, sources.iter().copied()));
    Dataset::Table(Box::new(table))
}

#[test]
fn builds_deep_multi_source_references_in_stable_order() {
    let mut app = PlotiumApp::new();
    app.doc.datasets = vec![root("A"), root("B")];
    let roots = [
        app.doc.datasets[0].resource_id(),
        app.doc.datasets[1].resource_id(),
    ];
    app.doc
        .datasets
        .push(derived("AB", DerivationKind::SpectrumArithmetic, &roots));
    let ab = app.doc.datasets[2].resource_id();
    app.doc
        .datasets
        .push(derived("deep", DerivationKind::LineFitTable, &[ab]));
    let tree = DataTree::build(&app);
    assert_eq!(
        tree.roots.iter().map(|n| n.dataset).collect::<Vec<_>>(),
        vec![0, 1]
    );
    assert_eq!(tree.roots[0].derived[0].dataset, 2);
    assert!(tree.roots[0].derived[0].linked_reference);
    assert_eq!(tree.roots[0].derived[0].derived[0].dataset, 3);
    assert_eq!(tree.roots[1].derived[0].dataset, 2);
}

#[test]
fn filtering_keeps_ancestor_path() {
    let mut app = PlotiumApp::new();
    app.doc.datasets = vec![root("source")];
    let source = app.doc.datasets[0].resource_id();
    app.doc
        .datasets
        .push(derived("result", DerivationKind::LineFitTable, &[source]));
    let filtered = DataTree::build(&app).filtered(&app, "peak fit table");
    assert_eq!(filtered.roots.len(), 1);
    assert_eq!(filtered.roots[0].dataset, 0);
    assert_eq!(filtered.roots[0].derived[0].dataset, 1);
}

#[test]
fn visible_dataset_order_follows_the_rendered_lineage_tree() {
    let mut app = PlotiumApp::new();
    app.doc.datasets = vec![root("A"), root("B")];
    let source = app.doc.datasets[0].resource_id();
    app.doc
        .datasets
        .push(derived("A child", DerivationKind::Projection, &[source]));

    let tree = DataTree::build(&app);
    assert_eq!(tree.visible_datasets(&app, false), vec![0, 2, 1]);

    app.session.ui.data_browser_collapsed_derived.insert(0);
    assert_eq!(tree.visible_datasets(&app, false), vec![0, 1]);
}

#[test]
fn cycles_are_cut_and_remain_accessible() {
    let mut app = PlotiumApp::new();
    app.doc.datasets = vec![root("A"), root("B")];
    let ids = [
        app.doc.datasets[0].resource_id(),
        app.doc.datasets[1].resource_id(),
    ];
    app.doc.datasets[0].set_lineage(Some(DatasetLineage::new(DerivationKind::Slice, [ids[1]])));
    app.doc.datasets[1].set_lineage(Some(DatasetLineage::new(
        DerivationKind::Projection,
        [ids[0]],
    )));
    let tree = DataTree::build(&app);
    assert!(!tree.roots.is_empty());
    assert!(tree.roots[0].derived[0].derived[0].cycle_cut);
}

#[test]
fn enumerates_stable_analysis_ids_and_fitted_columns() {
    let mut spectrum = root("analysis");
    let nmr = spectrum.as_nmr_mut().unwrap();
    nmr.peaks.marks.push(PeakMark {
        id: 11,
        x: 2.5,
        y: 1.0,
        origin: PeakOrigin::Manual,
        label: Some("solvent".to_owned()),
    });
    nmr.integrals.push(IntegralResult {
        id: 12,
        start_ppm: 1.0,
        end_ppm: 2.0,
        area: 1.0,
        normalized_area: 1.0,
        mode: DisplayModeLabel::Real,
        reference_value: None,
    });
    nmr.line_fits.push(StoredLineFit {
        id: 13,
        lo: 1.0,
        hi: 2.0,
        shape: LineShapeKind::Lorentzian,
        peaks: Vec::new(),
        offset: 0.0,
        offset_sigma: None,
        r2: 0.99,
    });
    nmr.multiplets.push(StoredMultiplet {
        id: 14,
        lo: 1.0,
        hi: 2.0,
        center_ppm: 1.5,
        pattern: MultipletPatternKind::Singlet,
        j_values: Vec::new(),
        area: 1.0,
        peak_ppm: vec![1.5],
    });

    let table = materialized_float_series_table(
        ("x".into(), "".into(), vec![Some(0.0)]),
        vec![FloatSeries {
            name: "decay".to_owned(),
            unit: String::new(),
            values: vec![Some(1.0)],
            uncertainty: None,
            fit: Some(CurveFitReference {
                analysis_id: 0,
                instance_id: "column-0".into(),
                response: "y".into(),
            }),
        }],
        "plotium.test.analysis-table.v1",
    )
    .unwrap();

    let mut app = PlotiumApp::new();
    app.doc.datasets = vec![spectrum, Dataset::Table(Box::new(table))];
    let tree = DataTree::build(&app);
    let kinds: Vec<_> = tree.roots[0]
        .analysis
        .iter()
        .map(|item| item.kind.clone())
        .collect();
    assert_eq!(
        kinds,
        vec![
            AnalysisKind::Peak(11),
            AnalysisKind::Integral(12),
            AnalysisKind::LineFit(13),
            AnalysisKind::Multiplet(14),
        ]
    );
    assert_eq!(
        tree.roots[1].analysis[0].kind,
        AnalysisKind::CurveFitResponse(
            app.doc.datasets[1].as_table().unwrap().series_bindings[0].value_column
        )
    );
}

#[test]
fn external_focus_reveals_ancestor_branches() {
    let mut app = PlotiumApp::new();
    app.doc.datasets = vec![root("source")];
    let source = app.doc.datasets[0].resource_id();
    app.doc
        .datasets
        .push(derived("child", DerivationKind::Slice, &[source]));
    let child = app.doc.datasets[1].resource_id();
    app.doc.datasets.push(derived(
        "grandchild",
        DerivationKind::LineFitTable,
        &[child],
    ));
    app.session
        .ui
        .data_browser_collapsed_datasets
        .extend([0, 1, 2]);
    app.session.ui.data_browser_collapsed_derived.extend([0, 1]);
    app.focus_single(2);

    reveal_active_path(&mut app);

    assert!(app.session.ui.data_browser_collapsed_datasets.is_empty());
    assert!(app.session.ui.data_browser_collapsed_derived.is_empty());
}

#[test]
fn search_matches_scientific_summary_alongside_name() {
    let mut app = PlotiumApp::new();
    app.doc.datasets = vec![root("sample"), root("control")];
    let tree = DataTree::build(&app);
    assert_eq!(
        tree.filtered(&app, "sample 1.0 MHZ")
            .visible_datasets(&app, true),
        vec![0]
    );
}

#[test]
fn search_crosses_fields_and_combines_with_batch_and_tag() {
    use plotium_core::state::{ImportItem, ImportTarget, OrganizationEdit};
    let mut app = PlotiumApp::new();
    app.doc.datasets = vec![root("Protein_A_011"), root("Protein_A_012")];
    let id = app.doc.datasets[1].resource_id();
    let mut identity = app.doc.datasets[1].acquisition_identity().clone();
    identity.acquisition = Some("HSQC".into());
    app.doc.datasets[1].set_acquisition_identity(identity);
    let batch = app.record_completed_user_import(
        "Batch folder".into(),
        vec![ImportItem {
            path: "batch-12/acquisition.dx".into(),
            result: Ok(ImportTarget {
                dataset: id,
                canvas: None,
            }),
        }],
    );
    let tree = DataTree::build(&app);
    assert_eq!(
        tree.filtered(&app, "protein HSQC 1d BATCH-12")
            .visible_datasets(&app, true),
        vec![1]
    );
    crate::ui::import_feedback::view_batch(&mut app, &egui::Context::default(), batch);
    assert!(app.session.view == plotium_core::state::PrimaryView::Data);
    assert!(app.session.ui.data_selection.is_empty());
    assert_eq!(
        tree.filtered(&app, "nmr").visible_datasets(&app, true),
        vec![1]
    );
    app.edit_dataset_organization(&[id], OrganizationEdit::Experiment(Some("HNCO")));
    app.edit_dataset_organization(&[id], OrganizationEdit::AddTag("Protein A"));
    app.session.ui.data_browser_tag = Some("protein a".into());
    assert_eq!(
        tree.filtered(&app, "HNCO 1d").visible_datasets(&app, true),
        vec![1]
    );
    assert!(tree.filtered(&app, "HSQC").roots.is_empty());
}
