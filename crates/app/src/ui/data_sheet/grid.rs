//! Spreadsheet-style grid for typed tables: painted cells, a single active
//! editor, keyboard navigation, and header context menus. Row identity
//! (RowId) stays internal; users see 1-based row numbers.

use std::sync::Arc;

use egui::{Align, Event, Key, Layout, Modifiers, Sense, StrokeKind, Ui};
use egui_extras::{Column as TableColumn, TableBuilder};
use plotium_core::data::{
    ChunkDescriptor, ColumnId, LiteralValue, RowId, ScalarValue, SnapshotReader, TableBatch,
    TableSchema,
};

use plotium_core::state::{TableCellEditDraft, TableDataset};

use super::batch_cache::BatchCache;
use super::{cell, focus};

#[derive(Clone, Default)]
pub(super) struct SheetState {
    /// Selected cell as (display row index, column).
    pub cell: Option<(usize, ColumnId)>,
    /// Whole-row selection from the row-number gutter.
    pub row: Option<usize>,
    /// Row to scroll into view after keyboard navigation.
    scroll_to: Option<usize>,
}

#[derive(Default)]
pub(super) struct GridOutcome {
    pub edit: Option<(RowId, ColumnId, LiteralValue, LiteralValue)>,
    pub error: Option<String>,
    /// One-shot feedback for the app status line.
    pub notice: Option<String>,
    /// A keyboard-selected cell the enclosing column scroll should reveal.
    pub reveal: Option<egui::Rect>,
}

/// A keyboard or clipboard request to edit the selected cell.
#[derive(Debug, PartialEq)]
pub(super) struct EditRequest {
    pub row: usize,
    pub column: ColumnId,
    pub seed: Option<String>,
    /// Apply the seed immediately instead of opening the editor (Delete).
    pub commit: bool,
}

/// What to do with the editor buffer once it is parsed.
#[derive(Clone, Copy, PartialEq)]
enum CommitMove {
    Stay,
    Down,
    Right,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn typed_table_grid(
    ui: &mut Ui,
    table: &TableDataset,
    reader: &SnapshotReader<'_>,
    state: &mut SheetState,
    draft: &mut Option<TableCellEditDraft>,
    column_menu: &mut dyn FnMut(&mut Ui, ColumnId),
) -> GridOutcome {
    let snapshot = &table.typed_state.envelope.revision.snapshot;
    let schema = &snapshot.schema;
    let chunks = &snapshot.row_id_chunks;
    let row_count = usize::try_from(snapshot.row_count).unwrap_or(usize::MAX);
    let editable = table.provenance.is_none();
    let mut outcome = GridOutcome::default();
    let mut cache = BatchCache::load(ui.ctx(), table.snapshot_read());
    sanitize_state(state, schema, row_count);
    let had_focus = focus::grid_has_focus(ui.ctx());
    // A draft whose target changed shows no editor here; the grid must still
    // navigate and release focus instead of waiting on an editor it cannot show.
    let editor_shown = draft
        .as_ref()
        .is_some_and(|edit| edit.validate_target(table).is_ok());
    if let Some(request) = keyboard_navigation(
        ui,
        schema,
        row_count,
        state,
        editable && draft.is_none(),
        editor_shown,
        &mut outcome.notice,
    ) {
        let index = request.row;
        if let Some(batch) = batch_for_row(reader, chunks, index, &mut cache, &mut outcome.error) {
            let local = index - usize::try_from(batch.row_start).unwrap_or(usize::MAX);
            if let Some((_, values)) = batch.columns.iter().find(|(id, _)| *id == request.column)
                && let Some(column) = schema.column(request.column)
            {
                match start_edit(
                    state,
                    draft,
                    table,
                    index,
                    batch.row_ids[local],
                    &values.value(local).unwrap_or(ScalarValue::Null),
                    column,
                    request.seed,
                ) {
                    Ok(()) if request.commit => commit_seed(draft, column, &mut outcome),
                    Ok(()) => {}
                    Err(error) => outcome.error = Some(error),
                }
            }
        }
        state.scroll_to = Some(index);
    }
    let copy_requested = had_focus
        && !editor_shown
        && ui.ctx().input(|input| {
            input
                .events
                .iter()
                .any(|event| matches!(event, Event::Copy))
        });
    let top_left = ui.cursor().min;
    let row_height = ui.spacing().interact_size.y + 2.0;
    let header_height = row_height * 2.0 - 4.0;
    let gutter_width = 12.0 + 9.0 * (row_count.max(1).ilog10() as f32 + 1.0);
    let body_height = (ui.available_height() - header_height).max(120.0);
    // Per-table salt: switching tables in the dock keeps each table's scroll
    // position and column widths.
    let mut builder = TableBuilder::new(ui)
        .id_salt((
            "typed_table_grid",
            table.typed_state.envelope.revision.table_id,
        ))
        .striped(true)
        .sense(Sense::click())
        .min_scrolled_height(120.0)
        .max_scroll_height(body_height)
        .column(TableColumn::exact(gutter_width));
    for _ in &schema.columns {
        builder = builder.column(
            TableColumn::initial(120.0)
                .at_least(48.0)
                .resizable(true)
                .clip(true),
        );
    }
    // Keyboard moves also reveal the cell horizontally in a wide table. The
    // table's own vertical scroll area swallows horizontal scroll targets, so
    // the enclosing column scroll applies it from `GridOutcome::reveal`.
    let reveal = state.scroll_to.is_some();
    if let Some(row) = state.scroll_to.take() {
        builder = builder.scroll_to_row(row, Some(Align::Center));
    }
    builder
        .header(header_height, |mut header| {
            header.col(|_ui| {});
            for column in &schema.columns {
                let (_, response) = header.col(|ui| {
                    ui.vertical(|ui| {
                        ui.label(crate::typography::headline(&column.name));
                        let mut detail = cell::type_label(&column.logical_type).to_owned();
                        if let Some(unit) = &column.unit {
                            detail = format!("{} · {detail}", unit.display_unit);
                        }
                        ui.weak(detail);
                    });
                });
                if response.clicked() {
                    focus::claim(&response.ctx);
                }
                if editable {
                    response.context_menu(|ui| column_menu(ui, column.id));
                }
            }
        })
        .body(|body| {
            body.rows(row_height, row_count, |mut row| {
                let index = row.index();
                row.set_selected(state.row == Some(index));
                let batch = batch_for_row(reader, chunks, index, &mut cache, &mut outcome.error);
                let local = batch
                    .as_ref()
                    .map(|batch| index - usize::try_from(batch.row_start).unwrap_or(usize::MAX));
                gutter_cell(
                    &mut row,
                    schema,
                    state,
                    index,
                    copy_requested,
                    batch.as_deref().zip(local),
                );
                let (Some(batch), Some(local)) = (batch, local) else {
                    for _ in &schema.columns {
                        row.col(|_ui| {});
                    }
                    return;
                };
                let row_id = batch.row_ids[local];
                for ((column_id, values), column) in batch.columns.iter().zip(&schema.columns) {
                    let value = values.value(local).unwrap_or(ScalarValue::Null);
                    let editing = draft.as_ref().is_some_and(|edit| {
                        edit.row == row_id
                            && edit.column == *column_id
                            && edit.validate_target(table).is_ok()
                    });
                    if editing {
                        editor_cell(
                            &mut row,
                            state,
                            schema,
                            row_count,
                            index,
                            draft,
                            column,
                            &mut outcome,
                        );
                        continue;
                    }
                    let selected = state.cell == Some((index, *column_id));
                    let (rect, response) = row.col(|ui| {
                        painted_value(ui, &value, column, copy_requested && selected);
                    });
                    if selected && reveal {
                        outcome.reveal = Some(rect);
                    }
                    if selected {
                        ui_selection_stroke(&mut row, rect);
                    }
                    if response.clicked() {
                        state.cell = Some((index, *column_id));
                        state.row = None;
                        focus::claim(&response.ctx);
                    }
                    if editable
                        && draft.is_none()
                        && response.double_clicked()
                        && let Err(error) =
                            start_edit(state, draft, table, index, row_id, &value, column, None)
                    {
                        outcome.error = Some(error);
                    }
                }
            });
        });
    cache.store(ui.ctx());
    let rect = egui::Rect::from_min_max(top_left, ui.min_rect().max);
    focus::maintain(ui, rect);
    outcome
}

/// Delete applies an empty seed through the same parse and commit path as a
/// typed edit, so a non-nullable column reports why in the open editor.
fn commit_seed(
    draft: &mut Option<TableCellEditDraft>,
    column: &plotium_core::data::ColumnSchema,
    outcome: &mut GridOutcome,
) {
    let Some(edit) = draft.as_mut() else { return };
    match cell::parse_typed_cell(&edit.text, column) {
        Ok(after) => {
            // The grid keeps the keyboard; the transient editor must not take it.
            edit.take_focus = false;
            outcome.edit = Some((edit.row, edit.column, edit.original.clone(), after));
        }
        Err(error) => edit.error = Some(error),
    }
}

/// Selection may expire when a transform replaces the table; the shared
/// draft must survive until it is committed or explicitly discarded.
fn sanitize_state(state: &mut SheetState, schema: &TableSchema, row_count: usize) {
    let column_exists = |id: &ColumnId| schema.columns.iter().any(|column| column.id == *id);
    if state
        .cell
        .is_some_and(|(row, column)| row >= row_count || !column_exists(&column))
    {
        state.cell = None;
    }
    if state.row.is_some_and(|row| row >= row_count) {
        state.row = None;
    }
}

/// Arrow-key navigation, Escape to hand the keyboard back, plus Enter/F2,
/// type-to-edit, Delete and single-value paste when editing is allowed. Keys
/// are read only while the grid holds focus.
fn keyboard_navigation(
    ui: &mut Ui,
    schema: &TableSchema,
    row_count: usize,
    state: &mut SheetState,
    editable: bool,
    editing: bool,
    notice: &mut Option<String>,
) -> Option<EditRequest> {
    if editing || !focus::grid_has_focus(ui.ctx()) {
        return None;
    }
    if ui.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape)) {
        focus::release(ui.ctx());
        return None;
    }
    if row_count == 0 || schema.columns.is_empty() {
        return None;
    }
    let (mut row, column) = state.cell?;
    let mut column_index = schema
        .columns
        .iter()
        .position(|candidate| candidate.id == column)?;
    let mut moved = false;
    let mut start = None;
    let mut commit = false;
    ui.input_mut(|input| {
        let steps: [(Key, isize, isize); 4] = [
            (Key::ArrowUp, -1, 0),
            (Key::ArrowDown, 1, 0),
            (Key::ArrowLeft, 0, -1),
            (Key::ArrowRight, 0, 1),
        ];
        for (key, dr, dc) in steps {
            if input.consume_key(Modifiers::NONE, key) {
                row = row.saturating_add_signed(dr).min(row_count - 1);
                column_index = column_index
                    .saturating_add_signed(dc)
                    .min(schema.columns.len() - 1);
                moved = true;
            }
        }
        if editable {
            if input.consume_key(Modifiers::NONE, Key::Enter)
                || input.consume_key(Modifiers::NONE, Key::F2)
            {
                start = Some(None);
            }
            if start.is_none() {
                let typed: String = input
                    .events
                    .iter()
                    .filter_map(|event| match event {
                        Event::Text(text) => Some(text.as_str()),
                        _ => None,
                    })
                    .collect();
                if !typed.is_empty() {
                    start = Some(Some(typed));
                    input
                        .events
                        .retain(|event| !matches!(event, Event::Text(_)));
                }
            }
            if start.is_none()
                && let Some(index) = input
                    .events
                    .iter()
                    .position(|event| matches!(event, Event::Paste(_)))
                && let Event::Paste(text) = input.events.remove(index)
            {
                let value = text.trim_end_matches(['\r', '\n']);
                if value.contains(['\r', '\n', '\t']) {
                    *notice = Some("Pasting multiple cells is not supported yet.".to_owned());
                } else {
                    start = Some(Some(value.to_owned()));
                }
            }
            if start.is_none()
                && (input.consume_key(Modifiers::NONE, Key::Delete)
                    || input.consume_key(Modifiers::NONE, Key::Backspace))
            {
                start = Some(Some(String::new()));
                commit = true;
            }
        }
    });
    let column = schema.columns[column_index].id;
    if moved {
        state.cell = Some((row, column));
        state.row = None;
        state.scroll_to = Some(row);
    }
    start.map(|seed| EditRequest {
        row,
        column,
        seed,
        commit,
    })
}

#[allow(clippy::too_many_arguments)]
fn start_edit(
    state: &mut SheetState,
    draft: &mut Option<TableCellEditDraft>,
    table: &TableDataset,
    index: usize,
    row: RowId,
    value: &ScalarValue,
    column: &plotium_core::data::ColumnSchema,
    seed: Option<String>,
) -> Result<(), String> {
    let original = cell::scalar_to_literal(value)?;
    let original_text = cell::typed_cell_text(value, &column.logical_type);
    state.cell = Some((index, column.id));
    state.row = None;
    *draft = Some(TableCellEditDraft {
        dataset: table.resource_id,
        table: table.typed_state.envelope.revision.table_id,
        revision: table.typed_state.envelope.revision.id,
        row,
        column: column.id,
        original,
        text: seed.unwrap_or_else(|| original_text.clone()),
        original_text,
        error: None,
        take_focus: true,
    });
    Ok(())
}

/// Locate the batch containing `index` through the cache; None when reading failed.
fn batch_for_row(
    reader: &SnapshotReader<'_>,
    chunks: &[ChunkDescriptor],
    index: usize,
    cache: &mut BatchCache,
    error: &mut Option<String>,
) -> Option<Arc<TableBatch>> {
    let batch_index = batch_containing_row(chunks, index)?;
    let batch = cache
        .get_or_read(batch_index, || reader.read_batch(batch_index, &[]))
        .map_err(|read_error| *error = Some(read_error.to_string()))
        .ok()?;
    let start = usize::try_from(batch.row_start).unwrap_or(usize::MAX);
    (start..start + batch.row_ids.len())
        .contains(&index)
        .then_some(batch)
}

pub(super) fn batch_containing_row(chunks: &[ChunkDescriptor], row: usize) -> Option<usize> {
    let index = chunks.partition_point(|chunk| {
        let end = chunk.row_start.saturating_add(chunk.row_count);
        usize::try_from(end).unwrap_or(usize::MAX) <= row
    });
    (index < chunks.len()).then_some(index)
}

fn gutter_cell(
    row: &mut egui_extras::TableRow<'_, '_>,
    schema: &TableSchema,
    state: &mut SheetState,
    index: usize,
    copy_requested: bool,
    batch: Option<(&TableBatch, usize)>,
) {
    let selected = state.row == Some(index);
    let (_, response) = row.col(|ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.weak((index + 1).to_string());
        });
    });
    if response.clicked() {
        state.row = (!selected).then_some(index);
        state.cell = None;
        focus::claim(&response.ctx);
    }
    if selected
        && copy_requested
        && let Some((batch, local)) = batch
    {
        let line = batch
            .columns
            .iter()
            .zip(&schema.columns)
            .map(|((_, values), column)| {
                cell::typed_cell_text(
                    &values.value(local).unwrap_or(ScalarValue::Null),
                    &column.logical_type,
                )
            })
            .collect::<Vec<_>>()
            .join("\t");
        response.ctx.copy_text(line);
    }
}

fn painted_value(
    ui: &mut Ui,
    value: &ScalarValue,
    column: &plotium_core::data::ColumnSchema,
    copy_requested: bool,
) {
    if copy_requested {
        ui.ctx()
            .copy_text(cell::typed_cell_text(value, &column.logical_type));
    }
    let placeholder = cell::is_placeholder(value);
    let shown = cell::display_cell_text(value, &column.logical_type);
    let layout = if cell::right_aligned(&column.logical_type) {
        Layout::right_to_left(Align::Center)
    } else {
        Layout::left_to_right(Align::Center)
    };
    ui.with_layout(layout, |ui| {
        if placeholder {
            ui.weak(shown);
        } else {
            ui.label(shown);
        }
    });
}

fn ui_selection_stroke(row: &mut egui_extras::TableRow<'_, '_>, rect: egui::Rect) {
    let painter = row.response().ctx.layer_painter(egui::LayerId::new(
        egui::Order::Middle,
        egui::Id::new("plotium_typed_grid_selection"),
    ));
    let ctx = &row.response().ctx;
    let visuals = &ctx.global_style().visuals;
    // Accent only while the grid owns the keyboard; grey shows the selection
    // without implying that typing will reach it.
    let stroke = if focus::grid_has_focus(ctx) {
        visuals.selection.stroke
    } else {
        egui::Stroke::new(visuals.selection.stroke.width, visuals.weak_text_color())
    };
    painter.rect_stroke(rect, 1.0, stroke, StrokeKind::Inside);
}

#[allow(clippy::too_many_arguments)]
fn editor_cell(
    row: &mut egui_extras::TableRow<'_, '_>,
    state: &mut SheetState,
    schema: &TableSchema,
    row_count: usize,
    index: usize,
    draft: &mut Option<TableCellEditDraft>,
    column: &plotium_core::data::ColumnSchema,
    outcome: &mut GridOutcome,
) {
    let edit = draft.as_mut().unwrap();
    let original = edit.original_text.clone();
    let text = &mut edit.text;
    let mut commit = None;
    let mut cancel = false;
    let error = edit.error.clone();
    let take_focus = std::mem::take(&mut edit.take_focus);
    let editor_id = focus::editor_id(edit.row, edit.column);
    let (rect, _) = row.col(|ui| {
        let response = ui.add(
            egui::TextEdit::singleline(text)
                .id(editor_id)
                // Tab commits and moves right; egui focus traversal would hand
                // the keyboard to the next focusable table cell instead.
                .lock_focus(true)
                .desired_width(f32::INFINITY)
                .margin(egui::vec2(2.0, 0.0)),
        );
        if take_focus {
            response.request_focus();
            // Put the caret at the end instead of selecting everything so
            // type-to-edit keeps appending naturally.
            if let Some(mut editor_state) = egui::TextEdit::load_state(ui.ctx(), response.id) {
                let end = egui::text::CCursor::new(text.chars().count());
                editor_state
                    .cursor
                    .set_char_range(Some(egui::text::CCursorRange::one(end)));
                editor_state.store(ui.ctx(), response.id);
            }
        }
        let (enter, tab, escape) = ui.input(|input| {
            (
                input.key_pressed(Key::Enter),
                input.key_pressed(Key::Tab),
                input.key_pressed(Key::Escape),
            )
        });
        if escape {
            cancel = true;
        } else if enter {
            commit = Some(CommitMove::Down);
        } else if tab {
            commit = Some(CommitMove::Right);
        } else if response.lost_focus() {
            commit = Some(CommitMove::Stay);
        }
        // Keyboard exits hand the keyboard back to the grid so navigation
        // continues; a click elsewhere keeps its new target.
        if (response.has_focus() || response.lost_focus()) && (escape || enter || tab) {
            focus::claim(ui.ctx());
        }
        if let Some(error) = &error {
            response.on_hover_text(error);
        }
    });
    if error.is_some() {
        let painter = row.response().ctx.layer_painter(egui::LayerId::new(
            egui::Order::Middle,
            egui::Id::new("plotium_typed_grid_error"),
        ));
        let color = row.response().ctx.global_style().visuals.error_fg_color;
        painter.rect_stroke(
            rect,
            1.0,
            egui::Stroke::new(1.5_f32, color),
            StrokeKind::Inside,
        );
    }
    if cancel {
        *draft = None;
        return;
    }
    let Some(step) = commit else { return };
    let edit = draft.as_mut().unwrap();
    let text = edit.text.clone();
    if text != original {
        match cell::parse_typed_cell(&text, column) {
            Ok(after) => outcome.edit = Some((edit.row, edit.column, edit.original.clone(), after)),
            Err(error) => {
                edit.error = Some(error);
                edit.take_focus = true;
                return;
            }
        }
    }
    if outcome.edit.is_none() {
        *draft = None;
    }
    let next = match step {
        CommitMove::Stay => Some((index, column.id)),
        CommitMove::Down => (index + 1 < row_count).then_some((index + 1, column.id)),
        CommitMove::Right => {
            let position = schema
                .columns
                .iter()
                .position(|candidate| candidate.id == column.id)
                .unwrap_or_default();
            schema
                .columns
                .get(position + 1)
                .map(|next| (index, next.id))
        }
    };
    if let Some((row, _)) = next.filter(|_| step != CommitMove::Stay) {
        state.scroll_to = Some(row);
    }
    state.cell = next.or(state.cell);
}

#[cfg(test)]
#[path = "grid_tests.rs"]
mod tests;
