use super::*;

mod cell;
mod column_menu;
mod draft;
mod focus;
mod grid;
mod relationship;
mod transform;
mod typed;

pub(super) use focus::grid_has_focus;

pub(super) fn data_sheet_window(app: &mut PlotiumApp, ctx: &egui::Context) {
    let requested_sheet = app.session.ui.table_cell_edit.as_ref().and_then(|edit| {
        app.session
            .ui
            .sheet_open
            .and_then(|index| app.doc.datasets.get(index))
            .map(|dataset| dataset.resource_id())
            .filter(|dataset| *dataset != edit.dataset)
    });
    if let Some(edit) = &app.session.ui.table_cell_edit {
        if let Some(index) = app
            .doc
            .datasets
            .iter()
            .position(|dataset| dataset.resource_id() == edit.dataset)
        {
            app.session.ui.sheet_open = Some(index);
        } else {
            app.session.ui.sheet_open = None;
            egui::Window::new("Data sheet — Unfinished cell edit")
                .id(egui::Id::new("plotium_data_sheet"))
                .collapsible(false)
                .show(ctx, |ui| {
                    super::result_follow::register_editor(ui);
                    draft::controls(ui, &mut app.session.ui.table_cell_edit, None);
                });
            return;
        }
    }
    let Some(di) = app.session.ui.sheet_open else {
        return;
    };
    if di >= app.doc.datasets.len() {
        app.session.ui.sheet_open = None;
        return;
    }
    let mut open = true;
    let mut commit: Option<plotium_core::state::TableEditDelta> = None;
    let mut transform = None;
    let mut refresh = None;
    let live_region_source = app.doc.datasets[di]
        .as_table()
        .and_then(|table| table.provenance.as_ref())
        .and_then(|provenance| {
            app.doc
                .datasets
                .iter()
                .position(|dataset| dataset.resource_id().to_string() == provenance.source_resource)
        });
    let mut freeze_source = None;
    let mut notice = None;
    let catalog = app
        .doc
        .datasets
        .iter()
        .enumerate()
        .filter_map(|(dataset, value)| {
            let table = value.as_table()?;
            let revision = &table.typed_state.envelope.revision;
            Some(transform::TableCatalogEntry {
                dataset,
                name: value.display_name(),
                read: plotium_core::data::SnapshotRead {
                    table: revision.table_id,
                    revision: revision.id,
                    fingerprint: revision.snapshot.fingerprint,
                },
                schema: revision.snapshot.schema.clone(),
            })
        })
        .collect::<Vec<_>>();
    let freshness = app.doc.table_freshness(app.doc.datasets[di].resource_id());
    let title = format!("Data sheet — {}", app.doc.datasets[di].display_name());
    let available = ctx.content_rect().size();
    let maximum = egui::vec2(
        (available.x * 0.8).max(320.0),
        (available.y * 0.8).max(240.0),
    );
    egui::Window::new(title)
        .id(egui::Id::new("plotium_data_sheet"))
        .collapsible(false)
        .resizable(true)
        .default_size([620.0, 380.0])
        .max_size(maximum)
        .open(&mut open)
        .show(ctx, |ui| {
            crate::ui::result_follow::register_editor(ui);
            if matches!(freshness.status, plotium_core::state::TableFreshnessState::Stale | plotium_core::state::TableFreshnessState::Unavailable) && app.doc.datasets[di].as_table().is_some() {
                ui.colored_label(ui.visuals().warn_fg_color, &freshness.reason);
                ui.separator();
            }
            if let Some(elapsed) = app.table_transform_progress() {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(format!(
                        "Running table transform… {:.1} s",
                        elapsed.as_secs_f32()
                    ));
                    if ui.button("Cancel").clicked() {
                        app.cancel_table_transform();
                    }
                });
                ui.separator();
            }
            if let Some(source) = live_region_source {
                ui.horizontal_wrapped(|ui| {
                    ui.weak(
                        "Calculated from the source regions · Values are read-only and stay synchronized.",
                    );
                    if ui.button("Save editable snapshot").clicked() {
                        freeze_source = Some(source);
                    }
                });
                ui.separator();
            }
            if let Some(n) = app.doc.datasets[di].as_nmr() {
                nmr_sheet(ui, n);
            } else if let Some(n) = app.doc.datasets[di].as_nmr2d() {
                nmr2d_sheet(ui, n);
            } else if app.doc.datasets[di].as_table().is_some() {
                let t = app.doc.datasets[di].as_table_mut().unwrap();
                typed::typed_table_sheet(
                    ui,
                    t,
                    transform::TableSheetContext {
                        dataset: di,
                        commit: &mut commit,
                        draft: &mut app.session.ui.table_cell_edit,
                        transform: &mut transform,
                        refresh: &mut refresh,
                        catalog: &catalog,
                        transform_running: app.session.table_transform_job.is_some()
                            || app.session.table_refresh_job.is_some(),
                        notice: &mut notice,
                    },
                );
            }
        });
    if commit.is_none()
        && (requested_sheet.is_some() || !open)
        && let Some(edit) = app.session.ui.table_cell_edit.as_mut()
        && let Some(table) = app.doc.datasets[di].as_table()
    {
        match draft::prepare(edit, table) {
            Ok(Some(delta)) => commit = Some(delta),
            Ok(None) => app.session.ui.table_cell_edit = None,
            Err(error) => edit.error = Some(error),
        }
    }
    if let Some(delta) = commit {
        let typed_diagnostic = delta.typed_diagnostic.clone();
        let result = if let Some(error) = typed_diagnostic {
            Err(error)
        } else if let Some(after) = delta.typed_state(true) {
            match app.commit_table_revision(
                app.doc.datasets[di].resource_id(),
                delta.before_revision,
                after,
            ) {
                Ok(()) => Ok(()),
                Err(error) => Err(error),
            }
        } else {
            Err("The cell edit could not be prepared.".to_owned())
        };
        match result {
            Ok(()) => {
                app.session.ui.table_cell_edit = None;
                app.session.status = "Edited data table.".to_owned();
            }
            Err(error) => {
                if let Some(edit) = app.session.ui.table_cell_edit.as_mut() {
                    edit.error = Some(error.clone());
                }
                app.session.status = error;
            }
        }
    }
    if let Some(request) = transform
        && let Err(error) = app.start_table_transform(
            request.plan,
            request.input_datasets,
            request.name,
            256 * 1024 * 1024,
        )
    {
        app.session.status = error;
    }
    if let Some((dataset, inputs)) = refresh
        && let Err(error) = app.start_table_refresh(dataset, inputs, 256 * 1024 * 1024)
    {
        app.session.status = error;
    }
    if let Some(notice) = notice {
        app.session.status = notice;
    }
    if let Some(source) = freeze_source {
        app.request_region_table_operation(source, false);
    }
    if app.session.ui.table_cell_edit.is_none()
        && let Some(requested) = requested_sheet
    {
        app.session.ui.sheet_open = app
            .doc
            .datasets
            .iter()
            .position(|dataset| dataset.resource_id() == requested);
    }
    if !open {
        if let Some(edit) = app.session.ui.table_cell_edit.as_mut() {
            if edit.error.is_none() {
                edit.error =
                    Some("Apply or discard this cell edit before closing the data sheet.".into());
            }
        } else {
            app.session.ui.sheet_open = None;
        }
    }
}

pub(super) fn nmr2d_sheet(ui: &mut Ui, n: &plotium_core::state::Nmr2DDataset) {
    let d = &n.data;
    ui.label(format!("{} × {} points (F2 × F1)", d.cols, d.rows));
    for (name, axis) in [("Direct (F2)", &d.direct), ("Indirect (F1)", &d.indirect)] {
        ui.label(format!("{name}: {} · {:?}", axis.nucleus, axis.domain));
        if let Some(frequency) = axis.observe_freq_mhz {
            ui.label(format!("Observe frequency: {frequency:.3} MHz"));
        }
        if let Some(width) = axis.spectral_width_hz {
            ui.label(format!("Spectral width: {width:.0} Hz"));
        }
    }
    if let Some(exp) = &d.experiment {
        ui.label(format!("Experiment hint: {exp}"));
    }
    ui.separator();
    match &n.processed {
        plotium_processing::Processed2D::Ft(s) => {
            let (f2lo, f2hi) = s.f2_bounds();
            let (f1lo, f1hi) = s.f1_bounds();
            ui.label(format!(
                "Contour spectrum {}×{} (F1×F2) — F2 {f2lo:.2}..{f2hi:.2} {}, F1 {f1lo:.2}..{f1hi:.2} {}",
                s.f1_size, s.f2_size, s.direct.unit_label(), s.indirect.unit_label()
            ));
        }
        plotium_processing::Processed2D::Stack(s) => {
            ui.label(format!(
                "Pseudo-2D stack of {} direct-dimension spectra",
                s.increments()
            ));
        }
    }
}

pub(super) fn nmr_sheet(ui: &mut Ui, n: &plotium_core::state::NmrDataset) {
    let len = n.processed.values().len();
    ui.label(format!("{} · {} pts", n.data.nucleus(), len));
    ui.separator();

    let columns: Vec<(String, Vec<f64>)> = match &n.processed {
        plotium_processing::Processed1D::Time(trace) => vec![
            ("time (s)".to_owned(), trace.time_s.clone()),
            ("Real".to_owned(), trace.real()),
            (
                "Imag".to_owned(),
                trace.values.iter().map(|value| value.im).collect(),
            ),
        ],
        plotium_processing::Processed1D::Frequency(spec) => vec![
            (
                plotium_processing::axis_unit_label(Some(spec.unit)).to_owned(),
                spec.ppm.clone(),
            ),
            ("Real".to_owned(), spec.real()),
            (
                "Imag".to_owned(),
                spec.values.iter().map(|value| value.im).collect(),
            ),
            ("Magnitude".to_owned(), spec.magnitude()),
        ],
    };

    egui::ScrollArea::horizontal().show(ui, |ui| {
        ui.horizontal_top(|ui| {
            for (header, values) in &columns {
                ui.vertical(|ui| {
                    ui.set_width(140.0);
                    ui.label(crate::typography::headline(header));
                    ui.weak(format!("{len} values (collapsed)"));
                    collapsed_column(ui, values);
                });
            }
        });
    });
}

// Each numeric column has tens of thousands of values, so it is shown collapsed:
// a greyed cell with a sparkline of the values, not the raw rows.
pub(super) fn collapsed_column(ui: &mut Ui, values: &[f64]) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(132.0, 240.0), Sense::hover());
    let painter = ui.painter();
    let grey = if ui.visuals().dark_mode {
        Color32::from_white_alpha(14)
    } else {
        Color32::from_black_alpha(12)
    };
    painter.rect_filled(
        rect,
        ui.visuals().widgets.noninteractive.corner_radius,
        grey,
    );
    sparkline(painter, rect.shrink(8.0), values);
}

pub(super) fn sparkline(painter: &egui::Painter, rect: egui::Rect, values: &[f64]) {
    if values.len() < 2 {
        return;
    }
    let target = 160usize;
    let stride = (values.len() / target).max(1);
    let sampled: Vec<f64> = values.iter().step_by(stride).copied().collect();
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    for &v in &sampled {
        if v.is_finite() {
            lo = lo.min(v);
            hi = hi.max(v);
        }
    }
    if !lo.is_finite() || (hi - lo).abs() < f64::EPSILON {
        let y = rect.center().y;
        painter.line_segment(
            [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
            Stroke::new(1.0_f32, Color32::GRAY),
        );
        return;
    }
    let span = hi - lo;
    let n = sampled.len();
    let pts: Vec<Pos2> = sampled
        .iter()
        .enumerate()
        .map(|(i, &v)| {
            let tx = i as f32 / (n - 1) as f32;
            let ty = ((v - lo) / span) as f32;
            Pos2::new(
                rect.left() + tx * rect.width(),
                rect.bottom() - ty * rect.height(),
            )
        })
        .collect();
    painter.add(egui::Shape::line(
        pts,
        Stroke::new(1.0_f32, Color32::from_rgb(0x1f, 0x6f, 0xeb)),
    ));
}
