use super::*;
use plotium_core::data::{FiniteOrSpecial, LiteralValue};
use plotium_core::state::{
    FloatSeries, SHEET_MAX_COLS, TableEditDelta, materialized_float_series_table,
};

fn table(columns: usize, x: Vec<Option<f64>>) -> TableDataset {
    let rows = x.len();
    let series = (0..columns.saturating_sub(1))
        .map(|index| FloatSeries {
            name: format!("s{index}"),
            unit: String::new(),
            values: vec![Some(1.0); rows],
            uncertainty: None,
            fit: None,
        })
        .collect();
    materialized_float_series_table(
        ("x".into(), String::new(), x),
        series,
        "plotium.test.sheet-preview.v1",
    )
    .unwrap()
}

#[test]
fn preview_is_read_once_per_table_content() {
    let ctx = egui::Context::default();
    let first = table(2, vec![Some(0.0); 70_000]);
    let preview = sheet_preview(&ctx, &first);
    for _ in 0..60 {
        assert!(Arc::ptr_eq(&preview, &sheet_preview(&ctx, &first)));
    }

    let rows = first.typed_rows(1, &[]).unwrap();
    let mut delta = TableEditDelta::new_dataset(&first);
    delta.record_typed_value(
        rows.row_ids[0],
        rows.columns[0].schema.id,
        LiteralValue::Float64(FiniteOrSpecial::new(0.0)),
        LiteralValue::Float64(FiniteOrSpecial::new(2.5)),
    );
    delta.finish_dataset(&first);
    let mut edited = first.clone();
    edited.typed_state = delta.typed_state(true).unwrap();
    let rebuilt = sheet_preview(&ctx, &edited);
    assert!(!Arc::ptr_eq(&preview, &rebuilt));
    assert_eq!(rebuilt.columns[0].cells[0].text, "2.5");
}

#[test]
fn wide_preview_reads_only_the_shown_columns() {
    let ctx = egui::Context::default();
    let wide = table(200, vec![Some(0.0); 4]);
    let preview = sheet_preview(&ctx, &wide);
    assert_eq!(preview.headers.len(), SHEET_MAX_COLS);
    assert_eq!(preview.columns.len(), SHEET_MAX_COLS);
    assert_eq!(wide.hidden_cols(), 200 - SHEET_MAX_COLS);
}

#[test]
fn preview_text_matches_the_data_sheet() {
    let values = [0.0015, 1e-9, 6.02e23, f64::NAN, f64::INFINITY];
    let ctx = egui::Context::default();
    let mut x = values.iter().copied().map(Some).collect::<Vec<_>>();
    x.push(None);
    let preview = sheet_preview(&ctx, &table(1, x));
    let shown = preview.columns[0]
        .cells
        .iter()
        .map(|cell| cell.text.as_str())
        .collect::<Vec<_>>();
    assert_eq!(shown, ["0.0015", "1e-9", "6.02e23", "NaN", "+Inf", "—"]);
}
