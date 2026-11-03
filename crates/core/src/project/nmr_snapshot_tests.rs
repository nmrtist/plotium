use super::*;
use crate::state::NmrImportDraft;
use plotium_io::nmr_view::NmrSource;
use std::sync::Arc;

fn path(suffix: &str) -> PathBuf {
    std::env::temp_dir().join(format!("plotium-nmr-{}-{suffix}", uuid::Uuid::new_v4()))
}

#[test]
fn user_sampling_import_errors_are_visible_and_declarations_reopen_without_vendor_files() {
    let dir = path("sampling-input");
    std::fs::create_dir(&dir).unwrap();
    let fixture =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../io/tests/fixtures/nmr/bruker-nus");
    for name in ["ser", "acqus", "acqu2s"] {
        std::fs::copy(fixture.join(name), dir.join(name)).unwrap();
    }
    let mut draft = NmrImportDraft::new(dir.clone());
    draft.grid = "4".into();
    draft.lanes = "2".into();
    draft.source = "user supplied synthetic table".into();
    draft.rows = "2\n2".into();
    assert!(draft.declaration().is_err());
    draft.one_based = Some(true);
    let declaration = draft.declaration().unwrap();
    let mut invalid = declaration.clone();
    invalid.grid_shape = vec![5];
    let mut app = PlotiumApp::new();
    assert!(!app.load_nmr_with_sampling(&dir, invalid));
    assert!(app.doc.datasets.is_empty());
    assert!(app.session.status.contains("Failed to load"));
    assert!(
        app.load_nmr_with_sampling(&dir, declaration.clone()),
        "{}",
        app.session.status
    );
    let nmr = app.doc.datasets[0].as_nmr2d().unwrap();
    assert_eq!(nmr.data.nus.as_ref().unwrap().schedule, [1, 1]);
    let field = nmr.field_catalog.id_for_key("nmr.observations").unwrap();
    let items = nmr
        .field_catalog
        .trace_collection(field)
        .unwrap()
        .items
        .clone();
    assert_ne!(items[0].id, items[1].id);
    let project = path("declared-nus.plotium");
    save_project(&app, &project, false).unwrap();
    for name in ["ser", "acqus", "acqu2s"] {
        std::fs::remove_file(dir.join(name)).unwrap();
    }
    std::fs::remove_dir(dir).unwrap();
    let restored = load_project(&project).unwrap();
    std::fs::remove_file(project).unwrap();
    let nmr = restored.doc.datasets[0].as_nmr2d().unwrap();
    let source = nmr.data.source_dataset().dataset().as_raw().unwrap();
    assert_eq!(
        source.sampling_schedule().unwrap().declaration(),
        Some(&declaration.into_native().unwrap())
    );
    assert_eq!(
        nmr.field_catalog.trace_collection(field).unwrap().items,
        items
    );
}

fn rewrite(path: &Path, mut change: impl FnMut(&str, &mut Vec<u8>)) {
    let mut archive = ZipArchive::new(File::open(path).unwrap()).unwrap();
    let entries: Vec<_> = (0..archive.len())
        .map(|index| {
            let mut entry = archive.by_index(index).unwrap();
            let name = entry.name().unwrap().into_owned();
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).unwrap();
            change(&name, &mut bytes);
            (name, bytes)
        })
        .collect();
    drop(archive);
    let mut archive = zip::ZipWriter::new(File::create(path).unwrap());
    for (name, bytes) in entries {
        write_bytes(&mut archive, SimpleFileOptions::default(), &name, &bytes).unwrap();
    }
    archive.finish().unwrap();
}

#[test]
fn imported_hertz_spectrum_reopens_without_vendor_files_or_invented_calibration() {
    let vendor = path("spectrum.dx");
    std::fs::write(
        &vendor,
        include_str!("../../../io/tests/fixtures/nmr/jcamp-hz.dx")
            .lines()
            .filter(|line| !line.starts_with("##.OBSERVE FREQUENCY"))
            .collect::<Vec<_>>()
            .join("\n"),
    )
    .unwrap();
    let loaded = plotium_io::load_path(&vendor).unwrap();
    let plotium_io::Acquisition::Nmr(source) = loaded.acquisition else {
        panic!("NMR import");
    };
    let original = source.dataset().canonical_digests();
    let mut app = PlotiumApp::new();
    app.doc
        .datasets
        .push(Dataset::Nmr(Box::new(NmrDataset::load(source).unwrap())));
    let project = path("offline.plotium");
    save_project(&app, &project, false).unwrap();
    std::fs::remove_file(vendor).unwrap();
    let restored = load_project(&project).unwrap();
    std::fs::remove_file(project).unwrap();
    let nmr = restored.doc.datasets[0].as_nmr().unwrap();
    assert_eq!(original, nmr.data.dataset().canonical_digests());
    assert_eq!(nmr.spectrum().unwrap().ppm, [4.0, 3.0, 2.0, 1.0]);
    assert_eq!(nmr.spectrum().unwrap().unit, nmr::axis::AxisUnit::Hertz);
    assert_eq!(nmr.data.axes()[0].observe_frequency_mhz(), None);
    assert!(!nmr.data.has_imaginary(0));
    assert_eq!(
        restored.doc.datasets[0].field_descriptors()[0].units,
        ["Hz"]
    );
    let figure = crate::figures::build_figure(&nmr.data, nmr.spectrum().unwrap(), &[]);
    assert!(figure.x.label.contains("Hz"));
    assert!(
        restored
            .analyze_multiplets(0, 1.0, 4.0)
            .unwrap_err()
            .contains("calibrated in ppm")
    );
}

#[test]
fn project_rejects_corrupt_trailing_old_storage_and_conflicting_shape() {
    let mut app = PlotiumApp::new();
    app.doc.datasets.push(Dataset::Nmr(Box::new(
        NmrDataset::load(super::tests::synthetic_1d()).unwrap(),
    )));
    for damage in ["sample", "trailing", "storage", "shape"] {
        let project = path("corrupt.plotium");
        save_project(&app, &project, false).unwrap();
        rewrite(&project, |name, bytes| {
            if name.ends_with("/data.bin") {
                if damage == "sample" {
                    let last = bytes.len() - 1;
                    bytes[last] ^= 1;
                }
                if damage == "trailing" {
                    bytes.push(0);
                }
            } else if name.ends_with("/object.json") {
                let mut value: serde_json::Value = serde_json::from_slice(bytes).unwrap();
                if value.get("payload").is_some() {
                    assert_eq!(value["payload"]["storage"], "nmr_snapshot_v1");
                    assert_eq!(value["dimensions"], serde_json::json!([]));
                    if damage == "storage" {
                        value["payload"]["storage"] = "complex_f64_le".into();
                    }
                    if damage == "shape" {
                        value["payload"]["shape"] = serde_json::json!([3]);
                    }
                    *bytes = serde_json::to_vec(&value).unwrap();
                }
            }
        });
        let error = load_project(&project)
            .err()
            .expect("untrusted payload must fail")
            .to_string();
        std::fs::remove_file(project).unwrap();
        assert!(
            error.contains("NMR") || error.contains("trailing"),
            "{damage}: {error}"
        );
    }
}

#[test]
fn column_derivation_and_snapshot_preserve_indirect_cartesian_components() {
    use nmr::axis::{AxisCoordinates, AxisDomain, AxisRole, AxisUnit};
    use nmr::processed::{
        ComponentBasis, ProcessedAxis, ProcessedData, ProcessedDataset, ProcessedDescriptor,
        ProcessedOrigin, ProcessedProvenance,
    };
    let axes = [2, 3]
        .into_iter()
        .map(|points| {
            ProcessedAxis::new(
                AxisRole::Signal,
                AxisDomain::Frequency,
                Some(AxisUnit::Hertz),
                points,
                AxisCoordinates::Uniform {
                    start: 0.0,
                    step: 1.0,
                },
                ComponentBasis::Cartesian,
            )
            .unwrap()
        })
        .collect();
    let descriptor = ProcessedDescriptor::new(axes).unwrap();
    let data =
        ProcessedData::from_descriptor(&descriptor, (1..=24).map(f64::from).collect()).unwrap();
    let native = ProcessedDataset::new(
        descriptor,
        data,
        ProcessedProvenance::new(ProcessedOrigin::Unknown, vec![]).unwrap(),
    )
    .unwrap();
    let expected: Vec<_> = (0..2)
        .map(|row| {
            Complex64::new(
                native.data().get(&[row, 1], &[0, 0]).unwrap(),
                native.data().get(&[row, 1], &[1, 0]).unwrap(),
            )
        })
        .collect();
    let source = NmrSource::new(Arc::new(native.into())).unwrap();
    let (column, view) = plotium_processing::slice::extract(
        &source,
        plotium_processing::SliceKind::Column,
        plotium_processing::slice::Reduction::Slice(1),
    )
    .unwrap();
    assert_eq!(view.values, expected);
    let mut app = PlotiumApp::new();
    app.doc.datasets.push(Dataset::Nmr(Box::new(
        NmrDataset::load_with_pipeline(column, Some(AxisPipeline { steps: vec![] }), Some(false))
            .unwrap(),
    )));
    let project = path("column.plotium");
    save_project(&app, &project, false).unwrap();
    let restored = load_project(&project).unwrap();
    std::fs::remove_file(project).unwrap();
    let nmr = restored.doc.datasets[0].as_nmr().unwrap();
    assert_eq!(nmr.data.trace().unwrap(), expected);
    assert!(nmr.data.has_imaginary(0));
    assert!(
        nmr.data
            .dataset()
            .as_processed()
            .unwrap()
            .provenance()
            .history()
            .is_some()
    );
    assert_eq!(nmr.native_processed.reference_frequency_mhz(0), None);
}

#[test]
fn nus_observation_identities_keep_duplicate_schedule_order_across_project_roundtrip() {
    let mut data = super::tests::synthetic_dosy_2d();
    data.pseudo_axis = None;
    data.diffusion = None;
    data.experiment = None;
    data.rows = 3;
    data.data.truncate(3 * data.cols);
    data.nus = Some(plotium_io::NusMeta {
        grid: 8,
        acquired: 3,
        schedule: Some(vec![5, 1, 5]),
    });
    let nmr = Nmr2DDataset::load(data).unwrap();
    let field = nmr.field_catalog.id_for_key("nmr.observations").unwrap();
    let items = nmr
        .field_catalog
        .trace_collection(field)
        .unwrap()
        .items
        .clone();
    assert_eq!(items.len(), 3);
    assert_ne!(items[0].id, items[2].id);
    let mut app = PlotiumApp::new();
    app.doc.datasets.push(Dataset::Nmr2D(Box::new(nmr)));
    assert_eq!(app.doc.datasets[0].default_field_id(), Some(field));
    let project = path("nus.plotium");
    save_project(&app, &project, false).unwrap();
    let restored = load_project(&project).unwrap();
    std::fs::remove_file(project).unwrap();
    let nmr = restored.doc.datasets[0].as_nmr2d().unwrap();
    assert_eq!(nmr.data.nus.as_ref().unwrap().schedule, [5, 1, 5]);
    assert_eq!(
        nmr.field_catalog.trace_collection(field).unwrap().items,
        items
    );
    assert!(nmr.nus_request.is_none());
}

#[test]
fn completed_execution_evidence_records_actual_method_and_input() {
    let nmr = NmrDataset::load(super::tests::synthetic_1d()).unwrap();
    let dataset = Dataset::Nmr(Box::new(nmr));
    let objects = dataset_to_objects(&dataset, "d0", "r0").unwrap();
    let evidence = &objects.data.extensions["plotium.nmr_execution"];
    assert!(evidence["library"].is_object());
    let phases = evidence["automatic_phase"].as_array().unwrap();
    assert_eq!(phases.len(), 1);
    assert!(phases[0]["algorithm"].as_str().unwrap().contains("entropy"));
    assert_eq!(phases[0]["input"].as_str().unwrap().len(), 64);
    assert!(phases[0]["evaluations"].as_u64().unwrap() > 0);
}

#[test]
fn nus_reconstruction_changes_live_bindings_and_reopens_with_grid_identities() {
    use plotium_processing::{Layout2D, ProcessingStep, StepKind, StepSource};
    let mut data = super::tests::synthetic_dosy_2d();
    data.pseudo_axis = None;
    data.diffusion = None;
    data.experiment = None;
    data.rows = 6;
    data.cols = 8;
    data.quad = plotium_io::QuadMode::States;
    data.data = [5, 1, 6]
        .into_iter()
        .flat_map(|row| {
            (0..2).flat_map(move |lane| {
                (0..8).map(move |col| {
                    let phase = std::f64::consts::TAU * row as f64 / 8.0;
                    let amplitude = if lane == 0 { phase.cos() } else { phase.sin() };
                    Complex64::from_polar(amplitude, std::f64::consts::TAU * col as f64 / 8.0)
                })
            })
        })
        .collect();
    data.nus = Some(plotium_io::NusMeta {
        grid: 8,
        acquired: 3,
        schedule: Some(vec![5, 1, 6]),
    });
    let mut app = PlotiumApp::new();
    app.doc
        .datasets
        .push(Dataset::Nmr2D(Box::new(Nmr2DDataset::load(data).unwrap())));
    let canvas = crate::workflow::build_default_canvas(&app.doc.datasets[0], "NUS");
    app.doc.canvases.push(canvas);
    let observations = app.doc.canvases[0].objects[0]
        .plot()
        .unwrap()
        .binding
        .series[0]
        .source;
    for layout in [Layout2D::Stack, Layout2D::Ft, Layout2D::Stack] {
        let nmr = app.doc.datasets[0].as_nmr2d_mut().unwrap();
        nmr.params.layout = layout;
        nmr.params.f2.steps = vec![ProcessingStep::new(
            nmr.allocate_step_id(),
            StepKind::Fft,
            StepSource::User,
        )];
        nmr.params.f1.steps = if layout == Layout2D::Ft {
            vec![ProcessingStep::new(
                nmr.allocate_step_id(),
                StepKind::Fft,
                StepSource::User,
            )]
        } else {
            vec![]
        };
        nmr.nus_request = Some(plotium_processing::nmr_execution::NusRequest {
            max_iterations: 1000,
            noise_standard_deviation: Some(0.0),
        });
        assert!(app.schedule_2d_processing(0, true));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while app.compute_busy() && std::time::Instant::now() < deadline {
            app.poll_compute();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        app.poll_compute();
        assert!(!app.compute_busy());
        assert!(
            !app.session.status.contains("failed"),
            "{}",
            app.session.status
        );
        let dataset = &app.doc.datasets[0];
        let field = dataset.default_field_id().unwrap();
        assert_ne!(field, observations.field);
        let plot = app.doc.canvases[0].objects[0].plot().unwrap();
        let binding = app.display_binding(plot.display_owner, &plot.binding);
        assert!(!binding.series.is_empty());
        assert!(
            binding
                .series
                .iter()
                .all(|series| series.source.field == field)
        );
        if layout == Layout2D::Stack {
            assert_eq!(binding.series.len(), 8);
            assert!(
                binding
                    .series
                    .iter()
                    .all(|series| series.source.item() != observations.item())
            );
        }
        let project = path("reconstructed.plotium");
        save_project(&app, &project, false).unwrap();
        let restored = load_project(&project).unwrap();
        std::fs::remove_file(project).unwrap();
        assert_eq!(
            restored.doc.canvases[0].objects[0].plot().unwrap().binding,
            plot.binding
        );
        let merged = app.merge_display_binding(plot.display_owner, &plot.binding, binding);
        assert!(
            merged
                .series
                .iter()
                .any(|series| series.source == observations)
        );
    }
}

#[test]
fn volume_planes_and_phases_roundtrip_without_original_files() {
    let fixture =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../io/tests/fixtures/nmr/bruker-3d-qf");
    let loaded = plotium_io::load_path(&fixture).unwrap();
    let (mut dataset, _) = crate::workflow::dataset_from_acquisition(loaded.acquisition).unwrap();
    let volume = dataset.as_nmr2d().unwrap().volume.as_ref().unwrap();
    let shape = volume.processed.shape();
    assert_eq!(shape, [5, 3, 4]);
    assert!(crate::data_export::DataExportAvailability::for_dataset(&dataset).is_empty());
    let n = match &mut dataset {
        Dataset::Nmr2D(n) => n,
        _ => unreachable!(),
    };
    for (axis, count) in shape.iter().enumerate() {
        n.select_volume_plane(axis, count - 1).unwrap();
    }
    assert!(n.select_volume_plane(3, 0).is_err());
    assert_eq!(n.volume.as_ref().unwrap().fixed_axis, 2);
    let phases = [plotium_processing::nmr_volume::VolumePhase {
        p0_degrees: 30.0,
        p1_degrees: 0.0,
        pivot_fraction: 0.5,
    }; 3];
    n.set_volume_phases(phases).unwrap();
    let expected = n
        .volume
        .as_ref()
        .unwrap()
        .processed
        .signed_value([1, 1, 1])
        .unwrap();
    let mut app = PlotiumApp::new();
    app.doc.datasets.push(dataset);
    let id = app.doc.datasets[0].resource_id();
    app.select_nmr_volume_plane(id, 0, 0).unwrap();
    assert_eq!(
        app.doc.datasets[0]
            .as_nmr2d()
            .unwrap()
            .volume
            .as_ref()
            .unwrap()
            .fixed_axis,
        0
    );
    app.undo();
    assert_eq!(
        app.doc.datasets[0]
            .as_nmr2d()
            .unwrap()
            .volume
            .as_ref()
            .unwrap()
            .fixed_axis,
        2
    );
    let mut updated = phases;
    updated[0].p0_degrees = 45.0;
    app.set_nmr_volume_phases(id, updated).unwrap();
    assert_eq!(
        app.doc.datasets[0]
            .as_nmr2d()
            .unwrap()
            .volume
            .as_ref()
            .unwrap()
            .phases,
        updated
    );
    app.undo();
    assert_eq!(
        app.doc.datasets[0]
            .as_nmr2d()
            .unwrap()
            .volume
            .as_ref()
            .unwrap()
            .phases,
        phases
    );
    let file = path("volume.plotium");
    save_project(&app, &file, false).unwrap();
    let restored = load_project(&file).unwrap();
    std::fs::remove_file(file).unwrap();
    let volume = restored.doc.datasets[0]
        .as_nmr2d()
        .unwrap()
        .volume
        .as_ref()
        .unwrap();
    assert_eq!(volume.fixed_axis, 2);
    assert_eq!(volume.index, 3);
    assert_eq!(volume.phases, phases);
    assert_eq!(volume.processed.signed_value([1, 1, 1]).unwrap(), expected);
}

#[test]
fn cancelled_volume_import_stops_before_processing() {
    let fixture =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../io/tests/fixtures/nmr/bruker-3d-qf");
    let token = nmr::CancellationToken::new();
    token.cancel();
    let mut context = nmr::ExecutionContext::default().with_cancellation(token);
    let error = crate::workflow::load_dataset_with_context(&fixture, &mut context)
        .err()
        .expect("cancelled FFT must not install a dataset");
    assert!(
        error.to_string().to_lowercase().contains("cancel"),
        "{error}"
    );
}
