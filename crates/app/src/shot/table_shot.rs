use plotium_core::data::{FiniteOrSpecial, LiteralValue, RevisionId};
use plotium_core::state::{PlotiumApp, TableCellEditDraft};

pub(super) fn conflict(app: &mut PlotiumApp) -> Result<(), String> {
    super::workspace_shot::setup(app, false);
    app.new_table_dataset();
    let table = app.doc.datasets[0]
        .as_table()
        .ok_or("The starter table is unavailable")?;
    let column = table.series_bindings[0].value_column;
    let row = table
        .typed_rows(1, &[column])
        .map_err(|error| error.to_string())?
        .row_ids[0];
    let revision = &table.typed_state.envelope.revision;
    app.session.ui.table_cell_edit = Some(TableCellEditDraft {
        dataset: table.resource_id,
        table: revision.table_id,
        revision: RevisionId::new(),
        row,
        column,
        original: LiteralValue::Float64(FiniteOrSpecial::Finite(0.0)),
        original_text: "0".into(),
        text: "12.5".into(),
        error: None,
        take_focus: false,
    });
    Ok(())
}
