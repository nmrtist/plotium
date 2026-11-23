use plotium_core::actions::Action;
use plotium_core::automation::{KIND_FIELD, ProjectResourceProvider, ResourceProvider};
use plotium_core::state::{
    AfmForceBranch, AfmForceXSource, CanvasObjectKind, DEFAULT_CANVAS_SIZE_MM, Dataset,
    DerivedAxes, NATURE_DOUBLE_COLUMN, PlotiumApp, StackSpec,
};
use plotium_figure::{ContourSpec, SeriesEncoding};
use plotium_io::{AfmData, AfmForceSet, AfmFrameDirection, AfmImageChannel, AfmScale};
use std::sync::Arc;

fn afm_dataset(with_image: bool) -> Dataset {
    let images = if with_image {
        vec![AfmImageChannel {
            name: "Height".to_owned(),
            width: 2,
            height: 2,
            scan_size_x: 1.0,
            scan_size_y: 1.0,
            lateral_unit: "nm".to_owned(),
            scale: AfmScale {
                multiplier: 1.0,
                offset: 0.0,
                unit: "nm".to_owned(),
            },
            raw: Arc::<[i32]>::from([1, 2, 3, 4]),
            frame_direction: AfmFrameDirection::Trace,
        }]
    } else {
        Vec::new()
    };
    let data = AfmData {
        images,
        forces: Some(AfmForceSet {
            grid_width: 1,
            grid_height: 1,
            samples_per_curve: 4,
            raw: Arc::<[i32]>::from([1, 2, 3, 4]),
            signal_scale: AfmScale {
                multiplier: 1.0,
                offset: 0.0,
                unit: "V".to_owned(),
            },
            sample_period_s: None,
            z_positions: Some(Arc::<[f64]>::from([-100.0, 0.0, 100.0, 0.0])),
            display_order: Arc::<[usize]>::from([0, 1, 2, 3]),
            approach_samples: 2,
            deflection_sensitivity_m_per_v: Some(1.0e-9),
            spring_constant_n_per_m: Some(0.1),
            tip_radius_nm: None,
        }),
        source: "synthetic.spm".to_owned(),
        import_warnings: Vec::new(),
    };
    Dataset::Afm(Box::new(plotium_core::state::AfmDataset::load(data)))
}

fn insert(dataset: Dataset) -> PlotiumApp {
    let mut app = PlotiumApp::default();
    let action = Action::insert_dataset_with_default_canvas(
        &app,
        dataset,
        "AFM".to_owned(),
        DEFAULT_CANVAS_SIZE_MM,
    );
    app.execute_action(action);
    app
}

#[test]
fn force_only_gui_insertion_builds_a_nonempty_force_curve() {
    let app = insert(afm_dataset(false));
    let CanvasObjectKind::Plot(plot) = &app.doc.canvases[0].objects[0].kind else {
        panic!("expected plot");
    };
    assert_eq!(plot.chart.type_id, "afm_force_curve");
    assert_eq!(plot.figure().series.len(), 2);
    assert_eq!([plot.figure().x.min, plot.figure().x.max], [-100.0, 100.0]);
    assert_eq!(plot.figure().y.label, "Force (nN)");
    assert!((plot.figure().series[0].points[0][1] - 0.1).abs() < 1.0e-12);
}

#[test]
fn map_and_force_plots_survive_project_roundtrip() {
    let app = insert(afm_dataset(true));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("afm.plotium");
    plotium_core::project::save_project(&app, &path, false).unwrap();
    let reopened = plotium_core::project::load_project(&path).unwrap();
    let objects = &reopened.doc.canvases[0].objects;
    let map = objects[0].plot().unwrap();
    let force = objects[1].plot().unwrap();
    assert!(map.figure().heatmap.is_some());
    assert_eq!(force.figure().series.len(), 2);
    assert_eq!(force.figure().x.label, "Relative Z (nm)");
}

#[test]
fn force_curve_context_matches_calibration_and_axes_used_in_plot() {
    use plotium_core::state::AfmDeflectionSource::*;
    let cases = [
        // Stored length is already calibrated; the header sensitivity must not be applied again.
        ("nm", Some(37e-9), 0.1, StoredLength, "Force (nN)", 0.1),
        (
            "V/LSB",
            Some(2e-9),
            0.1,
            VoltageSensitivity,
            "Force (nN)",
            0.2,
        ),
        ("nm", None, 0.0, StoredLength, "Deflection (nm)", 1.0),
        (
            "V/LSB",
            None,
            0.1,
            MissingVoltageSensitivity,
            "Signal (V)",
            1.0,
        ),
        (
            "Arb/LSB",
            None,
            0.1,
            UnsupportedSignalUnit,
            "Signal (Arb)",
            1.0,
        ),
    ];
    for (index, (unit, sensitivity, spring, deflection_source, y_label, expected_y)) in
        cases.into_iter().enumerate()
    {
        let mut dataset = afm_dataset(false);
        let Dataset::Afm(afm) = &mut dataset else {
            unreachable!()
        };
        let forces = Arc::make_mut(&mut afm.data).forces.as_mut().unwrap();
        forces.signal_scale.unit = unit.into();
        forces.deflection_sensitivity_m_per_v = sensitivity;
        forces.spring_constant_n_per_m = Some(spring);
        let (x_source, x_label, expected_x) = match index {
            0 => (AfmForceXSource::ReconstructedZ, "Relative Z (nm)", 0.0),
            1 => {
                forces.z_positions = None;
                forces.sample_period_s = Some(0.002);
                (AfmForceXSource::SamplePeriod, "Branch time (s)", 0.002)
            }
            _ => {
                forces.z_positions = None;
                (AfmForceXSource::SampleIndex, "Sample index", 1.0)
            }
        };
        let context = afm.force_curve_context().unwrap();
        assert_eq!(context.deflection_source, deflection_source);
        assert_eq!(context.deflection_sensitivity_m_per_v, sensitivity);
        assert_eq!(
            context.spring_constant_n_per_m,
            (spring > 0.0).then_some(spring)
        );
        assert_eq!(context.x_source, x_source);
        assert_eq!(context.x.label(), x_label);
        assert_eq!(context.y.label(), y_label);
        let field = afm.field_catalog.id_for_key("afm.force_curve").unwrap();
        let quantity = y_label.split_whitespace().next().unwrap();
        assert_eq!(
            afm.force_figure(field).unwrap().title,
            format!("{quantity} Curve")
        );
        let app = insert(dataset);
        let plot = app.doc.canvases[0].objects[0].plot().unwrap();
        let figure = plot.figure();
        assert_eq!(figure.x.label, x_label);
        assert_eq!(figure.y.label, y_label);
        assert_eq!(figure.series[0].points[1][0], expected_x);
        assert!((figure.series[0].points[0][1] - expected_y).abs() < 1.0e-12);
    }
}

#[test]
fn new_afm_fit_data_preserves_branch_and_curve_quantities() {
    let mut dataset = afm_dataset(false);
    let Dataset::Afm(afm) = &mut dataset else {
        unreachable!()
    };
    let forces = Arc::make_mut(&mut afm.data).forces.as_mut().unwrap();
    forces.samples_per_curve = 8;
    forces.raw = Arc::from([1, 2, 3, 4, 5, 6, 7, 8]);
    forces.signal_scale.unit = "V/LSB".into();
    forces.deflection_sensitivity_m_per_v = None;
    forces.z_positions = Some(Arc::from([0.0, 1.0, 2.0, 3.0, 0.0, 1.0, 2.0, 3.0]));
    forces.display_order = Arc::from([0, 1, 2, 3, 4, 5, 6, 7]);
    forces.approach_samples = 4;
    let mut app = insert(dataset);
    let source = app.doc.datasets[0].resource_id();
    let target = app.afm_fit_target(source, AfmForceBranch::Retract).unwrap();
    app.run_afm_fit(
        target,
        plotium_analysis::models::LINEAR_ID,
        Default::default(),
        true,
    )
    .unwrap();
    let fit = app.afm_fit_result(target).unwrap();
    assert_eq!(fit.branch, AfmForceBranch::Retract);
    assert_eq!((&*fit.x_name, &*fit.x_unit), ("Relative Z", "nm"));
    assert_eq!((&*fit.y_name, &*fit.y_unit), ("Signal", "V"));
    assert_eq!(fit.rows[0][..2], [0.0, 5.0]);

    app.create_afm_fit_output(target, false).unwrap();
    let table = app.doc.datasets.last().unwrap().as_table().unwrap();
    let schema = &table.typed_state.envelope.revision.snapshot.schema;
    let quantity = |id| {
        let column = schema.column(id).unwrap();
        (
            column.name.as_str(),
            column.unit.as_ref().unwrap().display_unit.as_str(),
        )
    };
    assert_eq!(quantity(table.x_binding.unwrap()), ("Relative Z", "nm"));
    assert_eq!(
        quantity(table.series_bindings[0].value_column),
        ("Signal", "V")
    );
}

#[test]
fn map_and_force_gui_insertion_builds_side_by_side_plots() {
    let app = insert(afm_dataset(true));
    let objects = &app.doc.canvases[0].objects;
    assert_eq!(objects.len(), 2);
    assert_eq!(
        app.doc.canvases[0].size_mm,
        [NATURE_DOUBLE_COLUMN.width_mm, DEFAULT_CANVAS_SIZE_MM[1]]
    );
    assert_eq!(
        app.doc.canvases[0].size_preset_id.as_deref(),
        Some(NATURE_DOUBLE_COLUMN.id)
    );
    let chart_ids: Vec<&str> = objects
        .iter()
        .filter_map(|object| match &object.kind {
            CanvasObjectKind::Plot(plot) => Some(plot.chart.type_id.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(chart_ids, ["afm_map", "afm_force_curve"]);
    let CanvasObjectKind::Plot(map) = &objects[0].kind else {
        panic!("expected AFM map plot");
    };
    assert_eq!(
        map.binding.series[0].source.field,
        app.doc.datasets[0]
            .field_descriptors()
            .into_iter()
            .find(|field| field.local_id.starts_with("afm.channel."))
            .unwrap()
            .id
    );
    assert!(matches!(
        map.binding.series[0].encoding,
        SeriesEncoding::Heatmap(_)
    ));
    let CanvasObjectKind::Plot(force) = &objects[1].kind else {
        panic!("expected AFM force plot");
    };
    assert_eq!(
        force.binding.series[0].source.field,
        app.doc.datasets[0]
            .field_descriptors()
            .into_iter()
            .find(|field| field.local_id == "afm.force_curve")
            .unwrap()
            .id
    );
    assert!(matches!(
        force.binding.series[0].encoding,
        SeriesEncoding::Line(_)
    ));
}

#[test]
fn afm_double_view_force_curve_keeps_its_own_derived_axes() {
    let dataset = afm_dataset(true);
    let canvas = plotium_core::workflow::build_default_canvas_for_dataset(
        &dataset,
        0,
        "AFM".to_owned(),
        DEFAULT_CANVAS_SIZE_MM,
    );
    let CanvasObjectKind::Plot(map) = &canvas.objects[0].kind else {
        panic!("expected AFM map plot");
    };
    let CanvasObjectKind::Plot(force) = &canvas.objects[1].kind else {
        panic!("expected AFM force plot");
    };

    assert_eq!(
        force.derived_axes(),
        &DerivedAxes::from_figure(force.figure()),
        "Force Curve derived axes must describe its own rebuilt figure"
    );
    assert_ne!(
        force.derived_axes().x_label,
        map.derived_axes().x_label,
        "AFM map and Force Curve should expose distinct derived x-axis labels"
    );
}

#[test]
fn afm_scalar_field_can_render_a_contour_without_a_domain_chart_branch() {
    let mut app = insert(afm_dataset(true));
    let (mut binding, chart, size) = {
        let CanvasObjectKind::Plot(map) = &app.doc.canvases[0].objects[0].kind else {
            panic!("expected AFM map plot");
        };
        (
            map.binding.clone(),
            map.chart.clone(),
            app.doc.canvases[0].size_mm,
        )
    };
    binding.series[0].encoding = SeriesEncoding::Contour(
        ContourSpec::absolute(1.5, false).expect("positive literal contour base"),
    );
    let initial = app.build_binding_figure(&binding, &chart, &StackSpec::default(), size);
    assert!(
        initial.contours.is_empty(),
        "geometry is queued, never built inline"
    );

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while app.compute_busy() && std::time::Instant::now() < deadline {
        app.poll_compute();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    app.poll_compute();
    let figure = app.build_binding_figure(&binding, &chart, &StackSpec::default(), size);
    assert!(!figure.contours.is_empty());
    assert!(!figure.contours[0].segments.is_empty());
}

#[test]
fn afm_fields_expose_independent_image_and_force_capabilities() {
    let dataset = afm_dataset(true);
    let fields = dataset.field_descriptors();
    assert_eq!(fields.len(), 2);
    assert!(
        fields[0]
            .capabilities
            .contains(plotium_core::automation::CAP_FIELD_SCALAR_GRID_2D_REGULAR)
    );
    assert!(
        fields[0]
            .capabilities
            .contains(plotium_core::automation::CAP_FIELD_LOCATION_SCALE)
    );
    assert!(
        !fields[0]
            .capabilities
            .contains(plotium_core::automation::CAP_FIELD_CURVE_1D)
    );
    assert!(
        fields[1]
            .capabilities
            .contains(plotium_core::automation::CAP_FIELD_CURVE_1D)
    );
    assert!(
        !fields[1]
            .capabilities
            .contains(plotium_core::automation::CAP_FIELD_SCALAR_GRID_2D_REGULAR)
    );
}

#[test]
fn field_descriptors_are_dataset_child_resources() {
    let app = insert(afm_dataset(true));
    let dataset = &app.doc.datasets[0];
    let descriptors = ProjectResourceProvider::new(&app).descriptors();
    let fields = descriptors
        .iter()
        .filter(|descriptor| descriptor.resource.kind.0 == KIND_FIELD)
        .collect::<Vec<_>>();
    assert_eq!(fields.len(), 2);
    assert!(fields.iter().all(|field| {
        field.resource.parent_id.as_deref() == Some(&dataset.resource_id().to_string())
    }));
    let map_key = dataset
        .field_descriptors()
        .into_iter()
        .find(|field| field.local_id.starts_with("afm.channel."))
        .unwrap()
        .local_id;
    assert!(fields.iter().any(|field| {
        field.resource.local_id.as_deref() == Some(map_key.as_str()) && map_key != "afm.channel.0"
    }));
    assert!(
        fields
            .iter()
            .any(|field| field.resource.local_id.as_deref() == Some("afm.force_curve"))
    );
}
