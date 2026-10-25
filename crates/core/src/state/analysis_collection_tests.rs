use super::*;
use std::sync::Arc;

fn source(kind: ComputeKind) -> Dataset {
    if kind == ComputeKind::Craft {
        Dataset::Nmr(Box::new(NmrDataset::load(craft_data()).unwrap()))
    } else {
        Dataset::Nmr2D(Box::new(
            Nmr2DDataset::load(data_2d("analysis source")).unwrap(),
        ))
    }
}

fn fixture(kind: ComputeKind) -> PlotiumApp {
    let mut app = PlotiumApp::new_with_settings(crate::settings::Settings::default());
    app.doc.datasets.push(source(kind));
    app.doc.datasets.push(source(kind));
    app
}

fn queue(app: &mut PlotiumApp, index: usize, kind: ComputeKind) -> impl FnOnce() + use<> {
    let (service, complete) = ComputeService::controlled_for_test();
    app.session.compute = service;
    let dataset = app.doc.datasets[index].resource_id();
    if kind == ComputeKind::Craft {
        app.session.ui.craft_task_dataset = Some(dataset);
        let mut params = plotium_processing::craft::CraftParams::conventional();
        params.fir_filter_taps = 31;
        params.maximum_modeled_sample_count = 512;
        params.maximum_model_order = 2;
        assert!(app.request_craft_analysis(
            index,
            plotium_processing::craft::CraftParamOverrides::from_params(params),
            None,
        ));
    } else {
        // Exercise the direct completion path. Retained scientific API tasks
        // separately require their submitted document revision to stay current.
        let stack = Arc::new(plotium_processing::StackSpectrum {
            direct_domain: Domain::Frequency,
            ppm: vec![0.0, 1.0],
            traces: [1.0, 0.5, 0.25]
                .into_iter()
                .map(|value| vec![Complex64::new(value, 0.0); 2])
                .collect(),
            direct: plotium_processing::AxisMeta {
                nucleus: "1H".into(),
                observe_freq_mhz: Some(400.0),
                unit: Some(nmr::axis::AxisUnit::Ppm),
            },
            source: "analysis source".into(),
        });
        let input = crate::DiffusionInput::direct(&[0.0, 1e9, 2e9], "s/m²").unwrap();
        if kind == ComputeKind::Dosy {
            app.session
                .compute
                .enqueue_dosy(
                    dataset,
                    app.session.dataset_epoch,
                    stack,
                    input,
                    "1H".into(),
                    "source".into(),
                )
                .unwrap();
        } else {
            let params = crate::IltParams {
                n_grid: 8,
                ..crate::IltParams::default()
            };
            app.session
                .compute
                .enqueue_ilt(
                    dataset,
                    app.session.dataset_epoch,
                    stack,
                    log_grid(params.d_min, params.d_max, params.n_grid),
                    params.lambda,
                    params,
                    input,
                    "1H".into(),
                    "source".into(),
                )
                .unwrap();
        }
    }
    complete
}

fn installed(dataset: &Dataset, kind: ComputeKind) -> bool {
    match kind {
        ComputeKind::Craft => !dataset.as_nmr().unwrap().craft_runs.is_empty(),
        ComputeKind::Dosy => dataset.as_nmr2d().unwrap().dosy_map.is_some(),
        ComputeKind::Ilt => dataset.as_nmr2d().unwrap().ilt_map.is_some(),
        _ => unreachable!(),
    }
}

#[test]
fn analysis_completion_survives_unrelated_delete_and_undo_insertion() {
    for kind in [ComputeKind::Craft, ComputeKind::Dosy, ComputeKind::Ilt] {
        for undo_insert in [false, true] {
            let mut app = fixture(kind);
            let target = app.doc.datasets[1].resource_id();
            if undo_insert {
                app.delete_dataset(0);
            }
            let target_index = app.doc.dataset_index(target).unwrap();
            let complete = queue(&mut app, target_index, kind);
            // Finish before the edit, but deliver after it: cancellation must
            // also handle successes already waiting in the completion channel.
            complete();
            if undo_insert {
                app.undo();
            } else {
                app.delete_dataset(0);
            }
            app.poll_compute();
            assert!(!app.compute_busy());
            assert!(
                installed(app.doc.dataset_by_id(target).unwrap(), kind),
                "{kind:?}"
            );
            if kind == ComputeKind::Craft {
                assert_eq!(
                    app.session.ui.craft_feedback.get(&target),
                    Some(&CraftRunFeedback::Completed(CraftRunId(0)))
                );
            }
        }
    }
}

#[test]
fn removed_source_completion_cannot_land_on_replacement_or_undo_restoration() {
    for kind in [ComputeKind::Craft, ComputeKind::Dosy, ComputeKind::Ilt] {
        for restore in [false, true] {
            let mut app = fixture(kind);
            let removed = app.doc.datasets[1].resource_id();
            let complete = queue(&mut app, 1, kind);
            complete();
            app.delete_dataset(1);
            if restore {
                app.undo();
                assert_eq!(app.doc.datasets[1].resource_id(), removed);
            } else {
                app.doc.datasets.push(source(kind));
                assert_ne!(app.doc.datasets[1].resource_id(), removed);
            }
            app.poll_compute();
            assert!(!app.compute_busy());
            if kind == ComputeKind::Craft {
                assert_eq!(
                    app.session.ui.craft_feedback.get(&removed),
                    Some(&CraftRunFeedback::Cancelled)
                );
            }
            assert!(
                app.doc
                    .datasets
                    .iter()
                    .all(|dataset| !installed(dataset, kind)),
                "{kind:?}"
            );
        }
    }
}

#[test]
fn project_replacement_drops_queued_analysis_even_when_dataset_ids_are_reused() {
    for kind in [ComputeKind::Craft, ComputeKind::Dosy, ComputeKind::Ilt] {
        let mut app = fixture(kind);
        let complete = queue(&mut app, 1, kind);
        complete();
        let mut loaded = PlotiumApp::new_with_settings(crate::settings::Settings::default());
        loaded.doc.datasets = app.doc.datasets.clone();
        app.install_loaded_project(loaded);
        app.poll_compute();
        assert!(!app.compute_busy());
        assert!(
            app.doc
                .datasets
                .iter()
                .all(|dataset| !installed(dataset, kind))
        );
    }
}
