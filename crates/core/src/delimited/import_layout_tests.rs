use super::origin::plain_text;
use super::*;

fn messages(table: &DelimitedTable) -> Vec<&str> {
    table
        .diagnostics
        .iter()
        .map(|d| d.message.as_str())
        .collect()
}

#[test]
fn origin_label_rows_become_column_names_units_and_comments() {
    let parsed = parse_delimited(
        "Short Name,A,B\n\
         Long Name,Wavenumbers,Transmitance\n\
         Units,cm\\+(-1),%\n\
         Comments,,PorC1\n\
         1,3998.25,1.002\n\
         2,3997.29,1.001\n",
        ParseOptions::default(),
    )
    .unwrap();
    assert_eq!(
        parsed.headers.as_deref().unwrap(),
        ["Wavenumbers (cm⁻¹)", "Transmitance (%)"]
    );
    assert_eq!(parsed.column_comments, ["", "PorC1"]);
    assert_eq!(parsed.rows.len(), 2);
    assert!(matches!(parsed.rows[0].cells[0].value, CellValue::Number(v) if v == 3998.25));
    assert!(messages(&parsed)[0].starts_with("Detected Origin label rows"));
}

#[test]
fn origin_single_column_keeps_row_numbers_and_names_it_by_comment() {
    let parsed = parse_delimited(
        "Short Name,H\nComments,PorC1\n1,0.5\n2,0.7\n",
        ParseOptions::default(),
    )
    .unwrap();
    assert_eq!(parsed.headers.as_deref().unwrap(), ["Row", "PorC1"]);
    assert_eq!(parsed.rows[1].cells[0].raw, "2");
    assert!(
        messages(&parsed)
            .iter()
            .any(|m| m.contains("kept the row numbers"))
    );
}

#[test]
fn origin_duplicate_names_are_told_apart_by_short_name() {
    let parsed = parse_delimited(
        "Short Name,A,B\nLong Name,Current,Current\n1,1,2\n",
        ParseOptions::default(),
    )
    .unwrap();
    assert_eq!(
        parsed.headers.as_deref().unwrap(),
        ["Current [A]", "Current [B]"]
    );
}

#[test]
fn origin_comments_reach_column_metadata() {
    let parsed = parse_delimited(
        "Short Name,A,B\nComments,,PorC4\n1,1,2\n2,2,3\n",
        ParseOptions::default(),
    )
    .unwrap();
    let store = plotium_data::MemoryBlockStore::default();
    let codecs = plotium_data::CodecRegistry::with_arrow_ipc();
    let snapshot = parsed
        .into_typed_snapshot(plotium_data::TableId::new(), &store, &codecs)
        .unwrap();
    assert_eq!(
        snapshot.schema.columns[1].metadata.get("import.comment"),
        Some(&serde_json::json!("PorC4"))
    );
    assert!(
        !snapshot.schema.columns[0]
            .metadata
            .contains_key("import.comment")
    );
}

#[test]
fn origin_rich_text_becomes_plain_text() {
    assert_eq!(plain_text("cm\\+(3) g\\+(-1)"), "cm³ g⁻¹");
    assert_eq!(plain_text("mA cm\\+(-2)"), "mA cm⁻²");
    assert_eq!(plain_text("\\g(m)m"), "μm");
    assert_eq!(plain_text("\\g(D)E"), "ΔE");
    assert_eq!(plain_text("E\\-(1/2)"), "E_{1/2}");
    assert_eq!(plain_text("\\i(t) (s)"), "t (s)");
    assert_eq!(plain_text("C:\\data"), "C:\\data");
    assert_eq!(plain_text("\\+(2"), "\\+(2");
}

#[test]
fn short_rows_are_filled_and_xy_pairs_are_named() {
    let parsed = parse_delimited(
        "t1,v1,t2,v2\n0,1.0,0,2.0\n1,1.1,1,2.1\n2,1.2\n",
        ParseOptions::default(),
    )
    .unwrap();
    assert!(matches!(parsed.rows[2].cells[3].value, CellValue::Missing));
    let messages = messages(&parsed);
    assert!(messages.contains(&"Filled 1 short rows with missing cells."));
    assert!(messages.contains(&"Columns form x/y pairs of different lengths: t1 → v1; t2 → v2."));
}

#[test]
fn short_rows_are_filled_with_an_explicit_delimiter() {
    let parsed = parse_delimited_columns(
        "x,y\n1,2\n3\n",
        ParseOptions {
            delimiter: Some(Delimiter::Comma),
            header: HeaderMode::Present,
        },
        1,
    )
    .unwrap();
    assert!(matches!(parsed.rows[1].cells[1].value, CellValue::Missing));
}

#[test]
fn wide_rows_still_fail() {
    let options = ParseOptions {
        delimiter: Some(Delimiter::Comma),
        header: HeaderMode::Auto,
    };
    let error = parse_delimited("x,y\n1,2,3\n", options).unwrap_err();
    assert!(matches!(error, DelimitedError::RaggedRow { .. }));
}

#[test]
fn shared_x_tables_are_not_reported_as_pairs() {
    let parsed = parse_delimited("x,a,b,c\n0,1,2,3\n1,1,2,3\n", ParseOptions::default()).unwrap();
    assert!(!messages(&parsed).iter().any(|m| m.contains("x/y pairs")));
}

#[test]
fn blank_lines_are_skipped_rather_than_padded() {
    let parsed = parse_delimited("x,y\n1,2\n\n3,4\n", ParseOptions::default()).unwrap();
    assert_eq!(parsed.rows.len(), 2);
    assert!(!messages(&parsed).iter().any(|m| m.contains("short rows")));
}

#[test]
fn text_without_a_complete_data_row_is_not_a_table() {
    assert!(parse_delimited("Temperature, K\n1\n2\n", ParseOptions::default()).is_err());
}

#[test]
fn origin_detection_needs_numbered_rows() {
    let parsed = parse_delimited("Short Name,Value\n7,2\n9,3\n", ParseOptions::default()).unwrap();
    assert_eq!(parsed.headers.as_deref().unwrap(), ["Short Name", "Value"]);
    assert_eq!(parsed.rows[0].cells.len(), 2);
}

#[test]
fn origin_detection_is_skipped_without_headers() {
    let options = ParseOptions {
        delimiter: None,
        header: HeaderMode::Absent,
    };
    let parsed = parse_delimited("Short Name,A\n1,2\n2,3\n", options).unwrap();
    assert!(parsed.headers.is_none());
    assert_eq!(parsed.rows.len(), 3);
}

#[test]
fn origin_names_stay_unique_when_short_names_repeat() {
    let parsed = parse_delimited(
        "Long Name,I,I,I\nShort Name,A,A,\n1,1,2,3\n2,1,2,3\n",
        ParseOptions::default(),
    )
    .unwrap();
    let headers = parsed.headers.unwrap();
    assert_eq!(headers, ["I [A]", "I [A] (2)", "I [3]"]);
}

#[test]
fn origin_rich_text_handles_nesting_and_symbol_font_quirks() {
    assert_eq!(plain_text("µm\\+(2)"), "µm²");
    assert_eq!(plain_text("\\b(\\g(a)\\+(2))"), "α²");
    assert_eq!(plain_text("\\g(JV)"), "ϑς");
    let deep = format!("{}x{}", "\\i(".repeat(5000), ")".repeat(5000));
    assert!(plain_text(&deep).ends_with(&")".repeat(4990)));
}
