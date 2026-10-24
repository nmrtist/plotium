use super::{
    DatasetId, TypedTableState, execute_typed_plan, execute_typed_plan_cancellable,
    refresh_typed_plan, refresh_typed_plan_cancellable,
};
use plotium_data::TableId;
use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

pub struct TableTransformJob {
    input_datasets: Vec<DatasetId>,
    epoch: u64,
    name: String,
    started_at: Instant,
    cancel: Arc<AtomicBool>,
    rx: mpsc::Receiver<Result<TypedTableState, String>>,
}

pub struct TableRefreshJob {
    dataset: DatasetId,
    context: super::ProjectContextId,
    revision: u64,
    epoch: u64,
    started_at: Instant,
    cancel: Arc<AtomicBool>,
    before: TypedTableState,
    rx: mpsc::Receiver<Result<TypedTableState, String>>,
}

impl crate::state::PlotiumApp {
    pub fn start_table_transform(
        &mut self,
        plan: plotium_data::RelPlanV1,
        input_datasets: Vec<usize>,
        name: String,
        memory_limit_bytes: u64,
    ) -> Result<(), String> {
        if self.session.table_transform_job.is_some() || self.session.table_refresh_job.is_some() {
            return Err("A table transform is already running.".into());
        }
        let input_datasets: Vec<DatasetId> = input_datasets
            .into_iter()
            .map(|index| self.doc.datasets[index].resource_id())
            .collect();
        let inputs = self.typed_inputs(&input_datasets)?;
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancel);
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let refs = inputs.iter().collect::<Vec<_>>();
            let result = execute_typed_plan_cancellable(
                plan,
                &refs,
                TableId::new(),
                memory_limit_bytes,
                &BTreeSet::new(),
                worker_cancel.as_ref(),
            )
            .map_err(|error| error.to_string());
            let _ = tx.send(result);
        });
        self.session.table_transform_job = Some(TableTransformJob {
            input_datasets,
            epoch: self.session.dataset_epoch,
            name,
            started_at: Instant::now(),
            cancel,
            rx,
        });
        self.session.status = "Running table transform…".into();
        Ok(())
    }

    fn typed_inputs(&self, datasets: &[DatasetId]) -> Result<Vec<TypedTableState>, String> {
        datasets
            .iter()
            .map(|id| {
                self.doc
                    .dataset_by_id(*id)
                    .and_then(crate::state::Dataset::as_table)
                    .map(|table| table.typed_state.clone())
                    .ok_or_else(|| {
                        "A source data table for this refresh is missing or is no longer a table."
                            .to_owned()
                    })
            })
            .collect()
    }

    pub fn table_transform_progress(&self) -> Option<Duration> {
        self.session
            .table_transform_job
            .as_ref()
            .map(|job| job.started_at.elapsed())
            .or_else(|| {
                self.session
                    .table_refresh_job
                    .as_ref()
                    .map(|job| job.started_at.elapsed())
            })
    }

    pub fn cancel_table_transform(&mut self) -> bool {
        let cancel = self
            .session
            .table_transform_job
            .as_ref()
            .map(|job| &job.cancel)
            .or_else(|| {
                self.session
                    .table_refresh_job
                    .as_ref()
                    .map(|job| &job.cancel)
            });
        let Some(cancel) = cancel else { return false };
        cancel.store(true, Ordering::Relaxed);
        self.session.status = "Cancelling table transform…".into();
        true
    }

    pub fn poll_table_transform(&mut self) -> bool {
        let Some(job) = &self.session.table_transform_job else {
            return self.poll_table_refresh();
        };
        let result = match job.rx.try_recv() {
            Err(mpsc::TryRecvError::Empty) => return true,
            Err(mpsc::TryRecvError::Disconnected) => {
                Err("The table transform worker stopped unexpectedly.".into())
            }
            Ok(result) => result,
        };
        let job = self
            .session
            .table_transform_job
            .take()
            .expect("job checked above");
        if job.epoch != self.session.dataset_epoch {
            self.session.status =
                "Datasets changed while transforming; the result was discarded.".into();
            return true;
        }
        match result {
            Ok(typed) => self.open_transformed_table(typed, &job),
            Err(_) if job.cancel.load(Ordering::Relaxed) => {
                self.session.status = "Table transform cancelled.".into();
            }
            Err(error) => self.session.status = format!("Table transform failed: {error}"),
        }
        true
    }

    pub fn start_table_refresh(
        &mut self,
        dataset: usize,
        input_datasets: Vec<DatasetId>,
        memory_limit_bytes: u64,
    ) -> Result<(), String> {
        if self.session.table_transform_job.is_some() || self.session.table_refresh_job.is_some() {
            return Err("A table operation is already running.".into());
        }
        let (dataset_id, before) = self
            .doc
            .datasets
            .get(dataset)
            .and_then(|value| {
                let table = value.as_table()?;
                Some((value.resource_id(), table.typed_state.clone()))
            })
            .ok_or_else(|| "Select a derived data table to refresh.".to_owned())?;
        let inputs = self.typed_inputs(&input_datasets)?;
        let worker_derived = before.clone();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancel);
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let refs = inputs.iter().collect::<Vec<_>>();
            let result = refresh_typed_plan_cancellable(
                &worker_derived,
                &refs,
                memory_limit_bytes,
                &BTreeSet::new(),
                worker_cancel.as_ref(),
            )
            .map_err(|error| error.to_string());
            let _ = tx.send(result);
        });
        self.session.table_refresh_job = Some(TableRefreshJob {
            dataset: dataset_id,
            context: self.session.project_context,
            revision: self.doc.automation_revision,
            epoch: self.session.dataset_epoch,
            started_at: Instant::now(),
            cancel,
            before,
            rx,
        });
        self.session.status = "Refreshing table…".into();
        Ok(())
    }

    fn poll_table_refresh(&mut self) -> bool {
        let Some(job) = &self.session.table_refresh_job else {
            return false;
        };
        let result = match job.rx.try_recv() {
            Err(mpsc::TryRecvError::Empty) => return true,
            Err(mpsc::TryRecvError::Disconnected) => {
                Err("The table refresh worker stopped unexpectedly.".into())
            }
            Ok(result) => result,
        };
        let job = self
            .session
            .table_refresh_job
            .take()
            .expect("job checked above");
        if job.epoch != self.session.dataset_epoch
            || job.context != self.session.project_context
            || job.revision != self.doc.automation_revision
        {
            self.session.status =
                "Project or data changed while refreshing; the result was discarded.".into();
            return true;
        }
        if job.cancel.load(Ordering::Relaxed) {
            self.session.status = "Table refresh cancelled.".into();
            return true;
        }
        match result {
            Ok(after) => {
                let revision = after.envelope.revision.id;
                self.session.status = match self.commit_table_revision(
                    job.dataset,
                    job.before.envelope.revision.id,
                    after,
                ) {
                    Ok(()) => format!("Refreshed table revision {revision}."),
                    Err(error) => format!("Table refresh blocked: {error}"),
                };
            }
            Err(_) if job.cancel.load(Ordering::Relaxed) => {
                self.session.status = "Table refresh cancelled.".into();
            }
            Err(error) => self.session.status = format!("Table refresh blocked: {error}"),
        }
        true
    }

    fn transformed_table(
        typed: TypedTableState,
        job: &TableTransformJob,
    ) -> crate::state::TableDataset {
        let mut table = crate::state::TableDataset::from_typed(typed);
        table.name = Some(job.name.clone());
        table.lineage = Some(crate::state::DatasetLineage::new(
            crate::state::DerivationKind::RelationalTransform,
            job.input_datasets.iter().copied(),
        ));
        table
    }

    /// Automation results get their own canvas and selection.
    fn insert_transformed_table(&mut self, typed: TypedTableState, job: &TableTransformJob) {
        let table = Self::transformed_table(typed, job);
        let index = self.doc.datasets.len();
        let action = crate::actions::Action::insert_dataset_with_default_canvas(
            self,
            crate::state::Dataset::Table(Box::new(table)),
            format!("Canvas {} - {}", self.doc.canvases.len() + 1, job.name),
            crate::state::DEFAULT_CANVAS_SIZE_MM,
        );
        self.execute_action(action);
        self.focus_single(index);
        self.session.view = crate::state::PrimaryView::Data;
        self.session.ui.frame_selection = vec![crate::state::BoardFrameId::Sheet(
            self.doc.datasets[index].resource_id(),
        )];
        self.session.ui.sheet_open = Some(index);
        self.session.status = "Table transform completed.".into();
    }

    /// A transform started from the data sheet stays in the sheet: the result
    /// replaces the shown table there and gets no canvas of its own.
    fn open_transformed_table(&mut self, typed: TypedTableState, job: &TableTransformJob) {
        let mut table = Self::transformed_table(typed, job);
        if table.board_sheet_visible() {
            let sheet = table.board_rect_pt();
            table.board_pos = crate::state::next_board_frame_pos(self, [sheet.width, sheet.height]);
        }
        let id = table.resource_id;
        let action = crate::actions::Action::CreateTableData {
            dataset: Box::new(crate::state::Dataset::Table(Box::new(table))),
        };
        if let Err(error) = self.try_execute_action(action) {
            self.session.status = format!("Table transform failed: {error}");
            return;
        }
        self.session.ui.sheet_open = self.doc.dataset_index(id);
        self.session.status = "Table transform completed.".into();
    }

    pub fn derive_table_from_plan(
        &mut self,
        plan: plotium_data::RelPlanV1,
        input_datasets: &[usize],
        name: String,
        memory_limit_bytes: u64,
    ) -> Result<usize, String> {
        let input_ids: Vec<DatasetId> = input_datasets
            .iter()
            .map(|&index| self.doc.datasets[index].resource_id())
            .collect();
        let inputs = self.typed_inputs(&input_ids)?;
        let refs = inputs.iter().collect::<Vec<_>>();
        let typed = execute_typed_plan(
            plan,
            &refs,
            TableId::new(),
            memory_limit_bytes,
            &BTreeSet::new(),
        )
        .map_err(|error| error.to_string())?;
        let index = self.doc.datasets.len();
        let job = TableTransformJob {
            input_datasets: input_ids,
            epoch: self.session.dataset_epoch,
            name,
            started_at: Instant::now(),
            cancel: Arc::new(AtomicBool::new(false)),
            rx: mpsc::channel().1,
        };
        self.insert_transformed_table(typed, &job);
        Ok(index)
    }

    pub fn refresh_derived_table(
        &mut self,
        dataset: usize,
        input_datasets: &[usize],
        memory_limit_bytes: u64,
    ) -> Result<(), String> {
        let (dataset_id, derived) = self
            .doc
            .datasets
            .get(dataset)
            .and_then(|value| {
                let table = value.as_table()?;
                Some((value.resource_id(), table.typed_state.clone()))
            })
            .ok_or_else(|| "Select a derived data table to refresh.".to_owned())?;
        let input_ids: Vec<DatasetId> = input_datasets
            .iter()
            .map(|&index| {
                self.doc
                    .datasets
                    .get(index)
                    .map(crate::state::Dataset::resource_id)
                    .ok_or_else(|| "An input table is no longer available.".to_owned())
            })
            .collect::<Result<_, String>>()?;
        let inputs = self.typed_inputs(&input_ids)?;
        let refs = inputs.iter().collect::<Vec<_>>();
        let refreshed = refresh_typed_plan(&derived, &refs, memory_limit_bytes, &BTreeSet::new())
            .map_err(|error| error.to_string())?;
        let revision = refreshed.envelope.revision.id;
        self.commit_table_revision(dataset_id, derived.envelope.revision.id, refreshed)?;
        self.session.status = format!("Refreshed table revision {revision}.");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Dataset, FloatSeries, materialized_float_series_table};
    use plotium_data::{Relation, SnapshotRead};

    #[test]
    fn typed_inputs_reports_a_missing_source_dataset() {
        let app = crate::state::PlotiumApp::new();
        let missing = DatasetId::new();

        let error = match app.typed_inputs(&[missing]) {
            Ok(_) => panic!("a missing source dataset must be rejected"),
            Err(error) => error,
        };

        assert!(error.contains("source data table"), "{error}");
        assert!(error.contains("missing"), "{error}");
    }

    #[test]
    fn background_transform_commits_only_after_polling_completion() {
        let source = materialized_float_series_table(
            ("time".into(), "s".into(), vec![Some(0.0), Some(1.0)]),
            vec![FloatSeries {
                name: "signal".into(),
                unit: String::new(),
                values: vec![Some(2.0), Some(3.0)],
                uncertainty: None,
                fit: None,
            }],
            "plotium.test.background-transform.v1",
        )
        .unwrap();
        let revision = &source.typed_state.envelope.revision;
        let signal = source.series_bindings[0].value_column;
        let plan = plotium_data::RelPlanV1::new(Relation::Project {
            input: Box::new(Relation::SnapshotRead(SnapshotRead {
                table: revision.table_id,
                revision: revision.id,
                fingerprint: revision.snapshot.fingerprint,
            })),
            columns: vec![signal],
        });
        let mut app = crate::state::PlotiumApp::new();
        app.doc.datasets.push(Dataset::Table(Box::new(source)));
        app.session.view = crate::state::PrimaryView::Canvas;
        app.session.ui.sheet_open = Some(0);
        app.session.ui.frame_selection = vec![crate::state::BoardFrameId::Sheet(
            app.doc.datasets[0].resource_id(),
        )];
        app.start_table_transform(plan, vec![0], "Projected".into(), 16 * 1024 * 1024)
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline && app.session.table_transform_job.is_some() {
            app.poll_table_transform();
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(app.session.table_transform_job.is_none());
        assert_eq!(app.doc.datasets.len(), 2);
        assert!(app.doc.canvases.is_empty());
        assert!(app.session.view == crate::state::PrimaryView::Canvas);
        assert_eq!(app.session.ui.sheet_open, Some(1));
        assert_eq!(
            app.session.ui.frame_selection,
            vec![crate::state::BoardFrameId::Sheet(
                app.doc.datasets[0].resource_id()
            )]
        );
        assert_eq!(
            app.doc.datasets[1]
                .as_table()
                .unwrap()
                .typed_state
                .envelope
                .revision
                .snapshot
                .schema
                .columns[0]
                .id,
            signal
        );
        app.undo();
        assert_eq!(app.doc.datasets.len(), 1);
        assert_ne!(app.session.ui.sheet_open, Some(1));
        app.redo();
        assert_eq!(app.doc.datasets.len(), 2);
    }

    #[test]
    fn background_transform_discards_results_after_dataset_epoch_changes() {
        let source = materialized_float_series_table(
            ("time".into(), "s".into(), vec![Some(0.0), Some(1.0)]),
            vec![FloatSeries {
                name: "signal".into(),
                unit: String::new(),
                values: vec![Some(2.0), Some(3.0)],
                uncertainty: None,
                fit: None,
            }],
            "plotium.test.stale-background-transform.v1",
        )
        .unwrap();
        let revision = &source.typed_state.envelope.revision;
        let plan = plotium_data::RelPlanV1::new(Relation::Project {
            input: Box::new(Relation::SnapshotRead(SnapshotRead {
                table: revision.table_id,
                revision: revision.id,
                fingerprint: revision.snapshot.fingerprint,
            })),
            columns: vec![source.series_bindings[0].value_column],
        });
        let mut app = crate::state::PlotiumApp::new();
        app.doc.datasets.push(Dataset::Table(Box::new(source)));
        app.start_table_transform(plan, vec![0], "Stale".into(), 16 * 1024 * 1024)
            .unwrap();
        app.session.dataset_epoch += 1;
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline && app.session.table_transform_job.is_some() {
            app.poll_table_transform();
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(app.session.table_transform_job.is_none());
        assert_eq!(app.doc.datasets.len(), 1);
        assert!(app.session.status.contains("discarded"));
    }

    fn bound_pivot_fixture(bind_x_to_generated: bool) -> crate::state::PlotiumApp {
        use crate::state::{
            DatasetLineage, DerivationKind, TableDataset, TableSeriesBinding,
            materialized_float_column,
        };
        use plotium_data::*;
        let input = TypedTableState::materialized(
            vec![
                materialized_float_column("x", "s", [Some(0.), Some(1.), Some(2.)]),
                materialized_float_column("y", "mA", [Some(1.), Some(2.), Some(4.)]),
                (
                    ColumnSchema::new("channel", LogicalType::Utf8),
                    ColumnChunk::all_valid(ColumnValues::Utf8(vec!["signal".into(); 3])),
                ),
            ],
            vec![],
            "plotium.test.native-pivot.v1",
        )
        .unwrap();
        let source = TableDataset::from_typed(input);
        let r = &source.typed_state.envelope.revision;
        let columns = &r.snapshot.schema.columns;
        let plan = RelPlanV1::new(Relation::Pivot {
            input: Box::new(Relation::SnapshotRead(SnapshotRead {
                table: r.table_id,
                revision: r.id,
                fingerprint: r.snapshot.fingerprint,
            })),
            groups: vec![columns[0].id],
            names_from: columns[2].id,
            values_from: columns[1].id,
            aggregate: AggregateFunction::SumV1,
        });
        let typed = execute_typed_plan(
            plan,
            &[&source.typed_state],
            TableId::new(),
            16777216,
            &BTreeSet::new(),
        )
        .unwrap();
        let mut derived = TableDataset::from_typed(typed);
        let columns = &derived
            .typed_state
            .envelope
            .revision
            .snapshot
            .schema
            .columns;
        derived.x_binding = Some(columns[usize::from(bind_x_to_generated)].id);
        derived.series_bindings = columns
            .iter()
            .map(|column| TableSeriesBinding {
                value_column: column.id,
                uncertainty_column: None,
                fit: None,
            })
            .collect();
        derived.lineage = Some(DatasetLineage::new(
            DerivationKind::RelationalTransform,
            [source.resource_id],
        ));
        let mut app = crate::state::PlotiumApp::new_with_settings(Default::default());
        app.start_new_project();
        app.doc.datasets = vec![
            Dataset::Table(Box::new(source)),
            Dataset::Table(Box::new(derived)),
        ];
        app
    }

    fn assert_saved_native_table(app: &crate::state::PlotiumApp, path: &std::path::Path) {
        crate::project::save_project(app, path, false).unwrap();
        let loaded = crate::project::load_project(path).unwrap();
        let before = app.doc.datasets[1].as_table().unwrap();
        let after = loaded.doc.datasets[1].as_table().unwrap();
        assert_eq!(after.resource_id, before.resource_id);
        assert_eq!(after.typed_state.envelope, before.typed_state.envelope);
        assert_eq!(after.x_binding, before.x_binding);
        assert!(after.series_bindings == before.series_bindings);
    }

    fn native_pivot_refresh_roundtrip(background: bool) {
        for bind_x_to_generated in [false, true] {
            let mut app = bound_pivot_fixture(bind_x_to_generated);
            let before = app.doc.datasets[1].as_table().unwrap().clone();
            let dir = std::env::temp_dir().join(format!(
                "plotium-r9-native-refresh-{}",
                uuid::Uuid::new_v4()
            ));
            std::fs::create_dir(&dir).unwrap();
            // A schema-valid, saveable native table with dataset-level bindings,
            // not a plot-owned explicit binding or an unbound from_typed table.
            assert_saved_native_table(&app, &dir.join("before.plotium"));
            assert!(!app.can_undo());
            if background {
                let input = app.doc.datasets[0].resource_id();
                app.start_table_refresh(1, vec![input], 16777216).unwrap();
                let deadline = Instant::now() + Duration::from_secs(3);
                while app.session.table_refresh_job.is_some() && Instant::now() < deadline {
                    app.poll_table_transform();
                    std::thread::sleep(Duration::from_millis(1));
                }
                assert!(app.session.table_refresh_job.is_none());
                assert!(app.session.status.starts_with("Refreshed table revision"));
            } else {
                app.refresh_derived_table(1, &[0], 16777216).unwrap();
            }
            // Refresh generates new Pivot value columns. Adoption must retire
            // their obsolete dataset bindings before ordinary project saving.
            assert_saved_native_table(&app, &dir.join("after.plotium"));
            let after = app.doc.datasets[1].as_table().unwrap().clone();
            assert_ne!(
                after.typed_state.envelope.revision.snapshot.schema.columns[1].id,
                before.typed_state.envelope.revision.snapshot.schema.columns[1].id
            );
            assert_eq!(after.series_bindings.len(), 1);
            assert!(after.series_bindings[0] == before.series_bindings[0]);
            assert_eq!(
                after.x_binding,
                if bind_x_to_generated {
                    None
                } else {
                    before.x_binding
                }
            );
            app.undo();
            let undone = app.doc.datasets[1].as_table().unwrap();
            assert_eq!(undone.typed_state.envelope, before.typed_state.envelope);
            assert_eq!(undone.x_binding, before.x_binding);
            assert!(undone.series_bindings == before.series_bindings);
            assert!(!app.can_undo());
            assert_saved_native_table(&app, &dir.join("undo.plotium"));
            app.redo();
            let redone = app.doc.datasets[1].as_table().unwrap();
            assert_eq!(redone.typed_state.envelope, after.typed_state.envelope);
            assert_eq!(redone.x_binding, after.x_binding);
            assert!(redone.series_bindings == after.series_bindings);
            assert_saved_native_table(&app, &dir.join("redo.plotium"));
            std::fs::remove_dir_all(dir).unwrap();
        }
    }

    #[test]
    fn native_pivot_refresh_preserves_saveable_bindings_and_undo() {
        native_pivot_refresh_roundtrip(false);
    }

    #[test]
    fn background_native_pivot_refresh_preserves_saveable_bindings_and_undo() {
        native_pivot_refresh_roundtrip(true);
    }
}
