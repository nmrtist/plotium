//! Painted spreadsheet preview inside a board sheet frame. Cell text is read
//! and formatted once per table content, not once per frame.

use std::sync::Arc;

use super::*;
use crate::ui::data_sheet::cell;
use plotium_core::data::SnapshotRead;

/// Formatted preview of the leading rows and columns of one table revision.
pub(super) struct SheetPreview {
    content: SnapshotRead,
    headers: Vec<String>,
    columns: Vec<PreviewColumn>,
    error: Option<String>,
}

struct PreviewColumn {
    right_aligned: bool,
    cells: Vec<PreviewCell>,
}

struct PreviewCell {
    text: String,
    placeholder: bool,
}

impl SheetPreview {
    fn read(t: &TableDataset) -> Self {
        let schema = &t.typed_state.envelope.revision.snapshot.schema;
        let shown = &schema.columns[..t.sheet_cols()];
        let headers = shown
            .iter()
            .map(|column| {
                column.unit.as_ref().map_or_else(
                    || column.name.clone(),
                    |unit| format!("{} ({})", column.name, unit.display_unit),
                )
            })
            .collect();
        let projection = shown.iter().map(|column| column.id).collect::<Vec<_>>();
        let (columns, error) = match t.typed_rows(SHEET_MAX_ROWS, &projection) {
            Ok(rows) => (
                rows.columns
                    .iter()
                    .map(|column| PreviewColumn {
                        right_aligned: cell::right_aligned(&column.schema.logical_type),
                        cells: column
                            .values
                            .iter()
                            .map(|value| PreviewCell {
                                text: cell::display_cell_text(value, &column.schema.logical_type),
                                placeholder: cell::is_placeholder(value),
                            })
                            .collect(),
                    })
                    .collect(),
                None,
            ),
            Err(error) => (Vec::new(), Some(error)),
        };
        Self {
            content: t.snapshot_read(),
            headers,
            columns,
            error,
        }
    }
}

/// The cached preview for `t`, rebuilt only when the table content changes.
pub(super) fn sheet_preview(ctx: &egui::Context, t: &TableDataset) -> Arc<SheetPreview> {
    let id = egui::Id::new(("plotium_sheet_preview", t.resource_id));
    let content = t.snapshot_read();
    if let Some(preview) = ctx.data_mut(|data| data.get_temp::<Arc<SheetPreview>>(id))
        && preview.content == content
    {
        return preview;
    }
    let preview = Arc::new(SheetPreview::read(t));
    ctx.data_mut(|data| data.insert_temp(id, Arc::clone(&preview)));
    preview
}

/// Cell text is only drawn once the board zoom leaves it legible (font size floor).
pub(super) fn paint_sheet_body(
    t: &TableDataset,
    rect: egui::Rect,
    zoom: f32,
    ui: &Ui,
    painter: &egui::Painter,
) {
    let visuals = ui.visuals();
    let grid = Stroke::new(1.0_f32, visuals.widgets.noninteractive.bg_stroke.color);
    let col_w = SHEET_COL_W_PT * zoom;
    let row_h = SHEET_ROW_H_PT * zoom;
    let header_h = SHEET_HEADER_H_PT * zoom;
    let cols = t.sheet_cols();
    let hidden = t.hidden_cols();
    let rows = t.visible_rows();

    painter.rect_filled(rect, 0.0, visuals.extreme_bg_color);
    let header_rect = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), header_h));
    painter.rect_filled(header_rect, 0.0, visuals.widgets.inactive.bg_fill);

    for k in 0..=cols + usize::from(hidden > 0) {
        let x = rect.left() + k as f32 * col_w;
        painter.line_segment(
            [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
            grid,
        );
    }
    let footer = if t.rows_overflow() { 1 } else { 0 };
    for r in 0..=rows + footer {
        let y = rect.top() + header_h + r as f32 * row_h;
        painter.line_segment(
            [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
            grid,
        );
    }
    painter.rect_stroke(rect, 0.0, grid, StrokeKind::Inside);

    let font_size = (SHEET_ROW_H_PT * 0.62 * zoom).min(13.0);
    if font_size < 6.0 {
        return;
    }
    let preview = sheet_preview(ui.ctx(), t);
    let font = egui::FontId::proportional(font_size);
    let pad = 4.0;
    let visible = painter.clip_rect().intersect(rect);
    let column_rect = |k: usize| {
        egui::Rect::from_x_y_ranges(
            rect.left() + k as f32 * col_w..=rect.left() + (k + 1) as f32 * col_w,
            rect.y_range(),
        )
    };
    let row_y = |r: usize| rect.top() + header_h + (r as f32 + 0.5) * row_h;

    for k in 0..cols {
        let bounds = column_rect(k);
        if !visible.intersects(bounds) {
            continue;
        }
        let clip = painter.with_clip_rect(bounds.intersect(visible));
        if let Some(header) = preview.headers.get(k) {
            clip.text(
                Pos2::new(bounds.left() + pad, rect.top() + header_h * 0.5),
                egui::Align2::LEFT_CENTER,
                header,
                font.clone(),
                visuals.strong_text_color(),
            );
        }
        let Some(column) = preview.columns.get(k) else {
            continue;
        };
        let (x, align) = if column.right_aligned {
            (bounds.right() - pad, egui::Align2::RIGHT_CENTER)
        } else {
            (bounds.left() + pad, egui::Align2::LEFT_CENTER)
        };
        for (r, value) in column.cells.iter().take(rows).enumerate() {
            let color = if value.placeholder {
                visuals.weak_text_color()
            } else {
                visuals.text_color()
            };
            clip.text(
                Pos2::new(x, row_y(r)),
                align,
                &value.text,
                font.clone(),
                color,
            );
        }
    }
    let clip = painter.with_clip_rect(visible);
    if hidden > 0 {
        clip.text(
            Pos2::new(column_rect(cols).left() + pad, rect.top() + header_h * 0.5),
            egui::Align2::LEFT_CENTER,
            format!(
                "+{hidden} {}",
                if hidden == 1 { "column" } else { "columns" }
            ),
            font.clone(),
            visuals.weak_text_color(),
        );
    }
    if let Some(error) = &preview.error {
        clip.text(
            Pos2::new(rect.left() + pad, row_y(0)),
            egui::Align2::LEFT_CENTER,
            error,
            font.clone(),
            visuals.error_fg_color,
        );
    }
    if t.rows_overflow() {
        clip.text(
            Pos2::new(rect.left() + pad, row_y(rows)),
            egui::Align2::LEFT_CENTER,
            format!(
                "+{} more rows",
                t.typed_state
                    .envelope
                    .revision
                    .snapshot
                    .row_count
                    .saturating_sub(rows as u64)
            ),
            font,
            visuals.weak_text_color(),
        );
    }
}

#[cfg(test)]
#[path = "board_sheet_tests.rs"]
mod tests;
