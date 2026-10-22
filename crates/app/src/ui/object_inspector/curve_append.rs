//! Explicit source/column selection; adoption shares the native MCP planner.
use super::*;
use plotium_core::automation::{DocumentRevision, TargetRef, canvas_object_ref};
use plotium_core::data::LogicalType;
use plotium_core::state::{CurveAppendParams, CurveKind, DatasetId, NewCurve, TableCurveSource};

#[derive(Clone, Default)]
struct Draft {
    dataset: Option<DatasetId>,
    source: Option<TableCurveSource>,
    scatter: bool,
    error: String,
}
pub(super) fn show(app: &mut PlotiumApp, ci: usize, object: ObjectId, ui: &mut Ui) -> bool {
    let canvas = app.doc.canvases[ci].resource_id;
    let key = egui::Id::new((
        "append_explicit_curve",
        app.session.project_context.to_string(),
        canvas,
        object,
        app.doc.automation_revision,
    ));
    let mut draft = ui
        .ctx()
        .data_mut(|d| d.get_temp::<Draft>(key))
        .unwrap_or_default();
    let mut apply = false;
    ui.push_id(key, |ui| {
        ui.collapsing("Add curve", |ui| {
            let tables = app
                .doc
                .datasets
                .iter()
                .filter_map(|d| d.as_table().map(|_| (d.resource_id(), d.display_name())))
                .collect::<Vec<_>>();
            let selected = tables
                .iter()
                .find(|(id, _)| Some(*id) == draft.dataset)
                .map(|(_, name)| name.as_str())
                .unwrap_or("Choose source table…");
            let old = draft.dataset;
            egui::ComboBox::from_id_salt("source")
                .selected_text(selected)
                .show_ui(ui, |ui| {
                    for (id, name) in &tables {
                        if ui
                            .selectable_value(
                                &mut draft.dataset,
                                Some(*id),
                                format!("{name} · {}", &id.to_string()[..8]),
                            )
                            .clicked()
                        {
                            ui.close();
                        }
                    }
                });
            if old != draft.dataset {
                draft.source = None;
                draft.error.clear();
            }
            if let Some(dataset) = draft.dataset.and_then(|id| app.doc.dataset_by_id(id))
                && let Some(table) = dataset.as_table()
            {
                let revision = &table.typed_state.envelope.revision;
                let columns = revision
                    .snapshot
                    .schema
                    .columns
                    .iter()
                    .filter(|c| matches!(c.logical_type, LogicalType::Int64 | LogicalType::Float64))
                    .map(|c| (c.id, c.name.clone()))
                    .collect::<Vec<_>>();
                if let Some(first) = columns.first() {
                    let source = draft.source.get_or_insert_with(|| TableCurveSource {
                        dataset_id: dataset.resource_id(),
                        field_id: dataset.default_field_id().expect("table field"),
                        table_id: revision.table_id,
                        table_revision: revision.id,
                        x_column_id: first.0,
                        y_column_id: columns.get(1).unwrap_or(first).0,
                        y_error_column_id: None,
                    });
                    source.table_revision = revision.id;
                    super::curve_binding::column_choice(
                        ui,
                        "X",
                        &mut source.x_column_id,
                        &columns,
                        "x",
                    );
                    super::curve_binding::column_choice(
                        ui,
                        "Y",
                        &mut source.y_column_id,
                        &columns,
                        "y",
                    );
                    egui::ComboBox::from_id_salt("error")
                        .selected_text(
                            source
                                .y_error_column_id
                                .and_then(|id| {
                                    columns
                                        .iter()
                                        .find(|c| c.0 == id)
                                        .map(|c| format!("Y error: {}", c.1))
                                })
                                .unwrap_or_else(|| "Y error: none".into()),
                        )
                        .show_ui(ui, |ui| {
                            if ui
                                .selectable_value(&mut source.y_error_column_id, None, "None")
                                .clicked()
                            {
                                ui.close();
                            }
                            for (id, name) in &columns {
                                if ui
                                    .selectable_value(
                                        &mut source.y_error_column_id,
                                        Some(*id),
                                        name,
                                    )
                                    .clicked()
                                {
                                    ui.close();
                                }
                            }
                        });
                    ui.horizontal(|ui| {
                        ui.radio_value(&mut draft.scatter, false, "Line");
                        ui.radio_value(&mut draft.scatter, true, "Scatter");
                    });
                    apply = ui.button("Append curve").clicked();
                } else {
                    ui.label("Choose a table with numeric columns.");
                }
            }
            if !draft.error.is_empty() {
                ui.colored_label(ui.visuals().error_fg_color, &draft.error);
            }
        });
    });
    if apply && let Some(source) = draft.source.clone() {
        app.finish_series_presentation_edit();
        app.finish_axis_overrides_edit();
        let params = CurveAppendParams {
            plot: TargetRef::resource(canvas_object_ref(canvas, object)),
            series: vec![NewCurve {
                y_axis: Default::default(),
                kind: if draft.scatter {
                    CurveKind::Scatter
                } else {
                    CurveKind::Line
                },
                source,
            }],
        };
        match app
            .plan_append_curves(DocumentRevision(app.doc.automation_revision), &params)
            .and_then(|p| app.commit_curve_bindings(p))
        {
            Ok(()) => {
                ui.ctx().data_mut(|d| d.remove::<Draft>(key));
                app.session.status = "Appended curve.".into();
                return true;
            }
            Err(error) => {
                app.session.status = format!("Append curve: {error}");
                draft.error = error;
            }
        }
    }
    ui.ctx().data_mut(|d| d.insert_temp(key, draft));
    false
}
