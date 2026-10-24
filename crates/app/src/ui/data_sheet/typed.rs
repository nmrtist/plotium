//! Typed table sheet: toolbar, spreadsheet grid, and status bar. Edit
//! recording still flows through `TableEditDelta`; the implementation model
//! (RowId, revisions, patches) never appears in the UI.

use egui::Ui;
use plotium_core::state::{DatasetId, TableDataset};

use super::grid::{self, SheetState};
use super::transform::TableSheetContext;

pub(super) fn typed_table_sheet(ui: &mut Ui, table: &TableDataset, context: TableSheetContext<'_>) {
    let TableSheetContext {
        dataset,
        commit,
        draft,
        transform,
        refresh,
        catalog,
        transform_running,
        notice,
    } = context;
    let apply = super::draft::controls(ui, draft, Some(table));
    let typed = &table.typed_state;
    let snapshot = &typed.envelope.revision.snapshot;
    let editable = table.provenance.is_none();
    if editable {
        toolbar(
            ui,
            table,
            dataset,
            transform_running,
            transform,
            refresh,
            catalog,
        );
    }
    ui.separator();
    let codecs = plotium_core::data::CodecRegistry::with_arrow_ipc();
    let reader =
        match plotium_core::data::SnapshotReader::new(snapshot, typed.store.as_ref(), &codecs) {
            Ok(reader) => reader,
            Err(error) => {
                ui.colored_label(ui.visuals().error_fg_color, error.to_string());
                return;
            }
        };
    let row_count = usize::try_from(snapshot.row_count).unwrap_or(usize::MAX);
    // Keyed by table so switching tables in the dock restores each selection.
    let state_id = egui::Id::new(("typed_table_state", typed.envelope.revision.table_id));
    let mut state = ui
        .ctx()
        .data_mut(|data| data.get_temp::<SheetState>(state_id))
        .unwrap_or_default();
    if super::focus::take_first_cell_request(ui.ctx(), table.resource_id)
        && let Some(first) = snapshot.schema.columns.first()
    {
        state.cell = (row_count > 0).then_some((0, first.id));
        state.row = None;
        super::focus::claim(ui.ctx());
    }
    let status_height = ui.spacing().interact_size.y + ui.spacing().item_spacing.y;
    let mut outcome = grid::GridOutcome::default();
    let columns = egui::ScrollArea::horizontal()
        .id_salt(("typed_table_columns", typed.envelope.revision.table_id))
        .max_height((ui.available_height() - status_height).max(0.0))
        .show(ui, |ui| {
            outcome =
                grid::typed_table_grid(ui, table, &reader, &mut state, draft, &mut |ui, column| {
                    super::column_menu::column_header_menu(
                        ui,
                        table,
                        dataset,
                        column,
                        transform_running,
                        transform,
                    );
                });
        });
    if let Some(cell) = outcome.reveal {
        reveal_column(ui.ctx(), columns, cell);
    }
    status_bar(ui, table, &state, outcome.error.as_deref());
    ui.ctx().data_mut(|data| data.insert_temp(state_id, state));
    if outcome.notice.is_some() {
        *notice = outcome.notice;
    }
    if (apply || outcome.edit.is_some())
        && let Some(edit) = draft.as_mut()
    {
        match super::draft::prepare(edit, table) {
            Ok(Some(delta)) => *commit = Some(delta),
            Ok(None) => *draft = None,
            Err(error) => {
                edit.error = Some(error);
                edit.take_focus = true;
            }
        }
    }
}

fn reveal_column(
    ctx: &egui::Context,
    columns: egui::scroll_area::ScrollAreaOutput<()>,
    cell: egui::Rect,
) {
    let view = columns.inner_rect;
    let shift = if cell.right() > view.right() {
        cell.right() - view.right()
    } else if cell.left() < view.left() {
        cell.left() - view.left()
    } else {
        return;
    };
    let mut state = columns.state;
    state.offset.x = (state.offset.x + shift).max(0.0);
    state.store(ctx, columns.id);
    ctx.request_repaint();
}

fn toolbar(
    ui: &mut Ui,
    table: &TableDataset,
    dataset: usize,
    running: bool,
    request: &mut Option<super::transform::TableTransformRequest>,
    refresh: &mut Option<(usize, Vec<DatasetId>)>,
    catalog: &[super::transform::TableCatalogEntry],
) {
    ui.horizontal(|ui| {
        let refresh_sources = table
            .lineage
            .as_ref()
            .filter(|_| table.typed_state.envelope.revision.operation.plan.is_some())
            .map(|lineage| lineage.sources.clone());
        if ui
            .add_enabled(
                !running && refresh_sources.is_some(),
                egui::Button::new("Refresh"),
            )
            .on_hover_text("Re-run this table's source recipe and keep your cell edits")
            .clicked()
        {
            // Carry the source DatasetIds through untouched; start_table_refresh
            // resolves them and reports any missing source, instead of silently
            // dropping unresolved ones here.
            *refresh = Some((dataset, refresh_sources.unwrap()));
        }
        ui.menu_button("Combine", |ui| {
            super::relationship::combine_menu(ui, table, dataset, running, request, catalog);
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.weak("Right-click a column header to sort, rename, or transform.");
        });
    });
}

/// Size and editability live in the dock header; this line follows the
/// selection.
fn status_bar(ui: &mut Ui, table: &TableDataset, state: &SheetState, error: Option<&str>) {
    let schema = &table.typed_state.envelope.revision.snapshot.schema;
    let mut status = String::new();
    if let Some((row, column)) = state.cell {
        if let Some(column) = schema.column(column) {
            status = format!("Row {}, {}", row + 1, column.name);
        }
    } else if let Some(row) = state.row {
        status = format!("Row {}", row + 1);
    }
    ui.horizontal(|ui| {
        ui.weak(status);
        if let Some(error) = error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
    });
}
