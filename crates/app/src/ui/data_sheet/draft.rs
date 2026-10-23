use egui::Ui;
use plotium_core::state::{TableCellEditDraft, TableDataset, TableEditDelta};

use super::cell;

/// Draft actions stay next to the editor, including when its target disappears.
pub(super) fn controls(
    ui: &mut Ui,
    draft: &mut Option<TableCellEditDraft>,
    table: Option<&TableDataset>,
) -> bool {
    let Some(edit) = draft.as_mut() else {
        return false;
    };
    let conflict = table
        .ok_or_else(|| "The original table is unavailable. Copy your draft or discard it.".into())
        .and_then(|table| edit.validate_target(table));
    let mut apply = false;
    let mut discard = false;
    ui.horizontal_wrapped(|ui| {
        ui.label("Unfinished cell edit");
        apply = ui
            .add_enabled(conflict.is_ok(), egui::Button::new("Apply"))
            .clicked();
        if ui.button("Copy draft").clicked() {
            ui.ctx().copy_text(edit.text.clone());
        }
        discard = ui.button("Discard").clicked();
        if conflict.is_ok() {
            ui.weak("Enter or Tab applies · Escape discards");
        } else {
            ui.weak("Copy your draft before discarding it");
        }
    });
    if let Err(error) = conflict {
        ui.colored_label(ui.visuals().warn_fg_color, error);
        let response =
            ui.add(egui::TextEdit::singleline(&mut edit.text).desired_width(f32::INFINITY));
        if response.has_focus() && ui.input(|input| input.key_pressed(egui::Key::Escape)) {
            discard = true;
        }
    }
    if let Some(error) = &edit.error {
        ui.colored_label(ui.visuals().error_fg_color, error);
    }
    if discard {
        *draft = None;
    }
    apply
}

pub(super) fn prepare(
    draft: &TableCellEditDraft,
    table: &TableDataset,
) -> Result<Option<TableEditDelta>, String> {
    draft.validate_target(table)?;
    if draft.text == draft.original_text {
        return Ok(None);
    }
    let column = table
        .typed_state
        .envelope
        .revision
        .snapshot
        .schema
        .column(draft.column)
        .ok_or_else(|| {
            "The original column is unavailable. Copy your draft or discard it.".to_owned()
        })?;
    let after = cell::parse_typed_cell(&draft.text, column)?;
    if after == draft.original {
        return Ok(None);
    }
    let mut delta = TableEditDelta::new_dataset(table);
    delta.record_typed_value(draft.row, draft.column, draft.original.clone(), after);
    delta.finish_dataset(table);
    if let Some(error) = &delta.typed_diagnostic {
        return Err(error.clone());
    }
    Ok(Some(delta))
}
