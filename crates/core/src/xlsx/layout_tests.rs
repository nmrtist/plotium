use super::*;
use crate::xlsx::regions::{XlsxRegion, parse_range};
use crate::xlsx::{ImportedXlsxSheet, import_xlsx_regions, import_xlsx_workbook};
use plotium_data::{CodecRegistry, LogicalType, MemoryBlockStore};
use plotium_io::xlsx::XlsxWorkbook;

fn t(value: &str) -> XlsxCell {
    XlsxCell::value(XlsxValue::Utf8(value.into()))
}

fn n(value: f64) -> XlsxCell {
    XlsxCell::value(XlsxValue::Float64(value))
}

fn e() -> XlsxCell {
    XlsxCell::value(XlsxValue::Empty)
}

fn sheet(rows: Vec<Vec<XlsxCell>>) -> XlsxSheet {
    XlsxSheet {
        name: "Figure 3".into(),
        hidden: false,
        origin: (0, 0),
        rows,
    }
}

fn import(sheet: XlsxSheet) -> Vec<ImportedXlsxSheet> {
    let workbook = XlsxWorkbook {
        sheets: vec![sheet],
        uses_1904_date_system: false,
        plotium_schema: None,
        diagnostics: Vec::new(),
    };
    import_xlsx_workbook(
        &workbook,
        &MemoryBlockStore::default(),
        &CodecRegistry::with_arrow_ipc(),
    )
    .unwrap()
}

fn names(table: &ImportedXlsxSheet) -> Vec<&str> {
    table
        .snapshot
        .schema
        .columns
        .iter()
        .map(|column| column.name.as_str())
        .collect()
}

#[test]
fn plain_sheets_read_as_before() {
    let tables = import(sheet(vec![
        vec![t("sample"), t("x"), t("y")],
        vec![t("a"), n(1.0), n(2.0)],
        vec![t("b"), n(2.0), e()],
    ]));
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].name, "Figure 3");
    assert_eq!(tables[0].range, None);
    assert!(
        tables[0].diagnostics.is_empty(),
        "{:?}",
        tables[0].diagnostics
    );
    assert_eq!(names(&tables[0]), ["sample", "x", "y"]);
    assert_eq!(tables[0].snapshot.row_count, 2);
    assert_eq!(
        tables[0].snapshot.schema.columns[0].logical_type,
        LogicalType::Utf8
    );
}

#[test]
fn panel_letters_and_blank_columns_split_side_by_side_tables() {
    // Panel b starts right after a header-only column of panel a, with no
    // blank column between them.
    let tables = import(sheet(vec![
        vec![t("a"), e(), e(), t("b"), e(), e(), e(), e()],
        vec![
            t("x"),
            t("y"),
            t("orphan"),
            t("t"),
            t("I"),
            e(),
            t("u"),
            t("v"),
        ],
        vec![n(0.0), n(1.0), e(), n(5.0), n(6.0), e(), n(7.0), n(8.0)],
        vec![n(1.0), n(2.0), e(), n(6.0), n(7.0), e(), n(8.0), n(9.0)],
    ]));
    let summary = tables
        .iter()
        .map(|table| (table.name.as_str(), table.range.as_deref(), names(table)))
        .collect::<Vec<_>>();
    assert_eq!(
        summary,
        [
            ("Figure 3 a", Some("A2:C4"), vec!["x", "y"]),
            ("Figure 3 b", Some("D2:E4"), vec!["t", "I"]),
            ("Figure 3 G2:H4", Some("G2:H4"), vec!["u", "v"]),
        ]
    );
    assert!(
        tables[0]
            .diagnostics
            .iter()
            .any(|note| note.contains("C ('orphan')"))
    );
}

#[test]
fn adjacent_single_letter_headers_are_column_names() {
    let tables = import(sheet(vec![vec![t("x"), t("y")], vec![n(0.0), n(1.0)]]));
    assert_eq!(tables.len(), 1);
    assert_eq!(names(&tables[0]), ["x", "y"]);
}

#[test]
fn header_rows_combine_titles_groups_and_units() {
    // Sample names sit over the y column of each x/y pair.
    let tables = import(sheet(vec![
        vec![t("XPS"), e(), e(), e()],
        vec![e(), t("before"), e(), t("after")],
        vec![t("Energy"), t("Intensity"), t("Energy"), t("Intensity")],
        vec![t("eV"), t("--"), t("eV"), t("--")],
        vec![n(1.0), n(10.0), n(1.0), n(12.0)],
    ]));
    assert_eq!(
        names(&tables[0]),
        [
            "before Energy eV",
            "before Intensity",
            "after Energy eV",
            "after Intensity"
        ]
    );
    let notes = tables[0].diagnostics.join("\n");
    assert!(notes.contains("with 4 header row(s)"), "{notes}");
    assert!(notes.contains("Title: XPS."), "{notes}");
}

#[test]
fn stacked_segments_split_into_aligned_columns() {
    // The x column pauses on placeholders where the y column repeats its
    // header, as cyclers export charge and discharge.
    let tables = import(sheet(vec![
        vec![t("Capacity"), t("Voltage")],
        vec![n(0.0), n(2.0)],
        vec![n(1.0), n(4.0)],
        vec![t("--"), t("Voltage")],
        vec![n(0.0), n(4.0)],
        vec![n(2.0), n(2.5)],
        vec![n(3.0), n(2.0)],
        vec![t("end"), e()],
    ]));
    let table = &tables[0];
    assert_eq!(
        names(table),
        ["Capacity (1)", "Voltage (1)", "Capacity (2)", "Voltage (2)"]
    );
    assert_eq!(table.snapshot.row_count, 3);
    assert!(
        table
            .snapshot
            .schema
            .columns
            .iter()
            .all(|column| column.logical_type == LogicalType::Float64)
    );
    let notes = table.diagnostics.join("\n");
    assert!(notes.contains("segments start at row 5"), "{notes}");
    assert!(notes.contains("'end'"), "{notes}");
}

#[test]
fn regions_override_header_rows_names_and_segments() {
    let sheet = sheet(vec![
        vec![t("2 theta"), n(1.0), n(2.0)],
        vec![n(10.0), n(5.0), n(6.0)],
        vec![t("note"), e(), e()],
        vec![n(11.0), n(7.0), n(8.0)],
    ]);
    let workbook = XlsxWorkbook {
        sheets: vec![sheet],
        uses_1904_date_system: false,
        plotium_schema: None,
        diagnostics: Vec::new(),
    };
    let region = XlsxRegion {
        sheet: "Figure 3".into(),
        range: "A1:C4".into(),
        header_rows: Some(1),
        name: Some("XRD".into()),
        split_segments: false,
    };
    let tables = import_xlsx_regions(
        &workbook,
        &[region],
        &MemoryBlockStore::default(),
        &CodecRegistry::with_arrow_ipc(),
    )
    .unwrap();
    assert_eq!(tables[0].name, "XRD");
    assert_eq!(tables[0].range.as_deref(), Some("A1:C4"));
    assert_eq!(names(&tables[0]), ["2 theta", "1", "2"]);
    assert_eq!(tables[0].snapshot.row_count, 3);
}

#[test]
fn ranges_parse_cells_and_whole_columns() {
    assert_eq!(
        parse_range("J2:AC4411"),
        Ok(((Some(1), Some(4410)), (9, 28)))
    );
    assert_eq!(parse_range("j:ac"), Ok(((None, None), (9, 28))));
    for invalid in ["J2", "J2:AC", "AC2:J9", "J0:K2", "2:3"] {
        assert!(parse_range(invalid).is_err(), "{invalid}");
    }
    assert_eq!(column_letters(0), "A");
    assert_eq!(column_letters(28), "AC");
}

fn assert_float64(table: &ImportedXlsxSheet, column: usize) {
    let schema = &table.snapshot.schema.columns[column];
    assert_eq!(schema.logical_type, LogicalType::Float64, "{}", schema.name);
}

#[test]
fn text_inside_values_is_missing_and_keeps_rows_aligned() {
    let tables = import(sheet(vec![
        vec![t("x"), t("y")],
        vec![n(1.0), n(10.0)],
        vec![n(2.0), t("<LOD")],
        vec![n(3.0), n(30.0)],
    ]));
    assert_eq!(names(&tables[0]), ["x", "y"]);
    assert_eq!(tables[0].snapshot.row_count, 3);
    assert_float64(&tables[0], 1);
    let notes = tables[0].diagnostics.join("\n");
    assert!(notes.contains("'<LOD'"), "{notes}");
    assert!(!notes.contains("Split"), "{notes}");
}

#[test]
fn a_first_record_without_a_value_stays_data() {
    let tables = import(sheet(vec![
        vec![t("sample"), t("conc")],
        vec![t("blank"), t("n.d.")],
        vec![t("b"), n(3.0)],
    ]));
    assert_eq!(names(&tables[0]), ["sample", "conc"]);
    assert_eq!(tables[0].snapshot.row_count, 2);
    assert_float64(&tables[0], 1);
}

#[test]
fn plain_sheets_keep_date_and_one_letter_headers() {
    let dates = import(sheet(vec![
        vec![t("sample"), t("2024-01-05")],
        vec![t("a"), n(1.0)],
    ]));
    assert_eq!(names(&dates[0]), ["sample", "2024-01-05"]);
    let letters = import(sheet(vec![
        vec![t("t"), e(), t("V")],
        vec![n(0.0), n(1.0), n(2.0)],
    ]));
    assert_eq!(letters.len(), 1);
    assert_eq!(names(&letters[0]), ["t", "Column B", "V"]);
}

#[test]
fn text_blocks_beside_data_are_kept() {
    let tables = import(sheet(vec![
        vec![t("x"), t("y"), e(), t("Sample"), t("Note")],
        vec![n(1.0), n(2.0), e(), t("a"), t("dried")],
    ]));
    let summary = tables.iter().map(names).collect::<Vec<_>>();
    assert_eq!(summary, [vec!["x", "y"], vec!["Sample", "Note"]]);
}

#[test]
fn regions_are_clamped_and_reject_header_rows_beyond_the_range() {
    let workbook = XlsxWorkbook {
        sheets: vec![sheet(vec![vec![t("x")], vec![n(1.0)]])],
        uses_1904_date_system: false,
        plotium_schema: None,
        diagnostics: Vec::new(),
    };
    let region = |range: &str, header_rows| XlsxRegion {
        sheet: "Figure 3".into(),
        range: range.into(),
        header_rows,
        name: None,
        split_segments: true,
    };
    let import = |region| {
        import_xlsx_regions(
            &workbook,
            &[region],
            &MemoryBlockStore::default(),
            &CodecRegistry::with_arrow_ipc(),
        )
    };
    let wide = import(region("A:ZZZZZ", None)).unwrap();
    assert_eq!(wide[0].range.as_deref(), Some("A1:A2"));
    assert!(import(region("A1:A2", Some(usize::MAX))).is_err());
    assert!(import(region("C1:D9", None)).is_err());
}
