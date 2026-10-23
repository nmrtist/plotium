use super::*;

fn two_column_schema() -> TableSchema {
    TableSchema::new(vec![
        plotium_core::data::ColumnSchema::new("x", plotium_core::data::LogicalType::Float64),
        plotium_core::data::ColumnSchema::new("y", plotium_core::data::LogicalType::Float64),
    ])
    .unwrap()
}

fn key_event(key: Key) -> Event {
    Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    }
}

#[test]
fn read_only_grid_keeps_arrow_navigation_without_starting_an_edit() {
    let ctx = crate::typography::test_context();
    let schema = two_column_schema();
    let first = schema.columns[0].id;
    let second = schema.columns[1].id;
    let mut state = SheetState {
        cell: Some((0, first)),
        ..Default::default()
    };
    let input = egui::RawInput {
        events: vec![
            key_event(Key::ArrowRight),
            key_event(Key::F2),
            Event::Text("7".to_owned()),
        ],
        ..Default::default()
    };
    let mut edit_request = None;

    let _ = ctx.run_ui(input, |ui| {
        focus::claim(ui.ctx());
        edit_request = keyboard_navigation(ui, &schema, 3, &mut state, false, false, &mut None);
    });

    assert_eq!(state.cell, Some((0, second)));
    assert!(edit_request.is_none());
}

#[test]
fn editable_grid_still_supports_type_to_edit() {
    let ctx = crate::typography::test_context();
    let schema = two_column_schema();
    let first = schema.columns[0].id;
    let mut state = SheetState {
        cell: Some((0, first)),
        ..Default::default()
    };
    let input = egui::RawInput {
        events: vec![Event::Text("7".to_owned())],
        ..Default::default()
    };
    let mut edit_request = None;

    let _ = ctx.run_ui(input, |ui| {
        focus::claim(ui.ctx());
        edit_request = keyboard_navigation(ui, &schema, 3, &mut state, true, false, &mut None);
    });

    assert_eq!(
        edit_request,
        Some(EditRequest {
            row: 0,
            column: first,
            seed: Some("7".to_owned()),
            commit: false,
        })
    );
}

#[test]
fn unfocused_grid_leaves_tool_keys_and_arrows_to_the_canvas() {
    let ctx = crate::typography::test_context();
    let schema = two_column_schema();
    let first = schema.columns[0].id;
    let mut state = SheetState {
        cell: Some((0, first)),
        ..Default::default()
    };
    let mut events = vec![key_event(Key::ArrowRight), key_event(Key::ArrowDown)];
    for key in ["v", "t", "r"] {
        events.push(Event::Text(key.to_owned()));
    }
    let mut edit_request = None;
    let mut remaining = 0;

    let _ = ctx.run_ui(
        egui::RawInput {
            events,
            ..Default::default()
        },
        |ui| {
            edit_request = keyboard_navigation(ui, &schema, 3, &mut state, true, false, &mut None);
            remaining = ui.input(|input| input.events.len());
        },
    );

    assert!(edit_request.is_none());
    assert_eq!(state.cell, Some((0, first)));
    assert_eq!(remaining, 5);
}

fn focused_request(events: Vec<Event>) -> (Option<EditRequest>, Option<String>) {
    let ctx = crate::typography::test_context();
    let schema = two_column_schema();
    let mut state = SheetState {
        cell: Some((1, schema.columns[0].id)),
        ..Default::default()
    };
    let mut request = None;
    let mut notice = None;
    let _ = ctx.run_ui(
        egui::RawInput {
            events,
            ..Default::default()
        },
        |ui| {
            focus::claim(ui.ctx());
            request = keyboard_navigation(ui, &schema, 3, &mut state, true, false, &mut notice);
        },
    );
    (request, notice)
}

#[test]
fn focused_grid_turns_delete_and_single_value_paste_into_cell_edits() {
    let (delete, _) = focused_request(vec![key_event(Key::Delete)]);
    let delete = delete.unwrap();
    assert_eq!((delete.row, delete.seed.as_deref()), (1, Some("")));
    assert!(delete.commit);

    let (paste, _) = focused_request(vec![Event::Paste("42\n".to_owned())]);
    let paste = paste.unwrap();
    assert_eq!(paste.seed.as_deref(), Some("42"));
    assert!(!paste.commit);

    let (multi, notice) = focused_request(vec![Event::Paste("1\t2\n3\t4".to_owned())]);
    assert!(multi.is_none());
    assert!(notice.unwrap().contains("not supported"));
}

#[test]
fn escape_hands_the_keyboard_back() {
    let ctx = crate::typography::test_context();
    let schema = two_column_schema();
    let mut state = SheetState::default();
    let mut focused = true;
    let _ = ctx.run_ui(
        egui::RawInput {
            events: vec![key_event(Key::Escape)],
            ..Default::default()
        },
        |ui| {
            focus::claim(ui.ctx());
            keyboard_navigation(ui, &schema, 3, &mut state, true, false, &mut None);
            focused = focus::grid_has_focus(ui.ctx());
        },
    );
    assert!(!focused);
}

#[test]
fn ten_million_row_lookup_lands_on_the_intersecting_chunk() {
    let chunks = (0..153)
        .map(|index| ChunkDescriptor {
            row_start: index * 65_536,
            row_count: if index == 152 { 38_528 } else { 65_536 },
            codec: plotium_core::data::ARROW_IPC_CODEC_V1.into(),
            byte_hash: plotium_core::data::ContentHash::of(&index.to_le_bytes()),
            logical_fingerprint: plotium_core::data::ContentHash::of(&(index + 1).to_le_bytes()),
        })
        .collect::<Vec<_>>();
    assert_eq!(batch_containing_row(&chunks, 8_765_420), Some(133));
    assert_eq!(batch_containing_row(&chunks, 0), Some(0));
    assert_eq!(batch_containing_row(&chunks, 10_000_000), None);
}

fn editable_table() -> TableDataset {
    plotium_core::state::materialized_float_series_table(
        ("x".into(), "".into(), vec![Some(10.0), Some(20.0)]),
        vec![],
        "plotium.test.cell-draft.v1",
    )
    .unwrap()
}

fn begin_draft(table: &TableDataset, text: &str) -> TableCellEditDraft {
    let rows = table.typed_rows(2, &[]).unwrap();
    let column = table.typed_state.envelope.revision.snapshot.schema.columns[0].clone();
    let mut state = SheetState::default();
    let mut draft = None;
    start_edit(
        &mut state,
        &mut draft,
        table,
        1,
        rows.row_ids[1],
        &ScalarValue::Float64(20.0),
        &column,
        Some(text.into()),
    )
    .unwrap();
    draft.unwrap()
}

#[test]
fn cell_draft_freezes_target_and_original_value_before_rendering() {
    let table = editable_table();
    let draft = begin_draft(&table, "42");
    assert_eq!(draft.dataset, table.resource_id);
    assert_eq!(draft.table, table.typed_state.envelope.revision.table_id);
    assert_eq!(draft.revision, table.typed_state.envelope.revision.id);
    assert_eq!(draft.row, table.typed_rows(2, &[]).unwrap().row_ids[1]);
    assert_eq!(
        draft.original,
        cell::scalar_to_literal(&ScalarValue::Float64(20.0)).unwrap()
    );
    assert_eq!(draft.original_text, "20");
    assert_eq!(draft.text, "42");
}

#[test]
fn valid_cell_draft_prepares_edit_for_its_frozen_row() {
    let table = editable_table();
    let draft = begin_draft(&table, "42");
    let delta = super::super::draft::prepare(&draft, &table)
        .unwrap()
        .unwrap();
    let state = delta.typed_state(true).unwrap();
    let codecs = plotium_core::data::CodecRegistry::with_arrow_ipc();
    let reader = SnapshotReader::new(
        &state.envelope.revision.snapshot,
        state.store.as_ref(),
        &codecs,
    )
    .unwrap();
    let batch = reader.read_batch(0, &[draft.column]).unwrap();
    assert_eq!(
        batch.columns[0].1.value(0),
        Some(ScalarValue::Float64(10.0))
    );
    assert_eq!(
        batch.columns[0].1.value(1),
        Some(ScalarValue::Float64(42.0))
    );
}

#[test]
fn changed_revision_does_not_rebind_draft_to_reordered_rows() {
    let table = editable_table();
    let draft = begin_draft(&table, "42");
    let mut replacement = editable_table();
    replacement.resource_id = table.resource_id;
    replacement.typed_state.envelope.revision.table_id = draft.table;
    let error = super::super::draft::prepare(&draft, &replacement)
        .err()
        .unwrap();
    assert!(error.contains("table changed"));
    assert_eq!(draft.text, "42");
    assert_eq!(draft.row, table.typed_rows(2, &[]).unwrap().row_ids[1]);
}

#[test]
fn removed_column_and_read_only_table_keep_the_draft() {
    let mut table = editable_table();
    let draft = begin_draft(&table, "42");
    table
        .typed_state
        .envelope
        .revision
        .snapshot
        .schema
        .columns
        .clear();
    assert!(
        super::super::draft::prepare(&draft, &table)
            .err()
            .unwrap()
            .contains("column")
    );
    let mut table = editable_table();
    table.resource_id = draft.dataset;
    table.typed_state.envelope.revision.id = draft.revision;
    table.typed_state.envelope.revision.table_id = draft.table;
    table.provenance = Some(plotium_core::state::TableProvenance {
        source_resource: draft.dataset.to_string(),
        source_field: plotium_core::state::FieldId::new(1),
        members: None,
        regions: vec![],
    });
    assert!(
        super::super::draft::prepare(&draft, &table)
            .err()
            .unwrap()
            .contains("read-only")
    );
    assert_eq!(draft.text, "42");
}

#[test]
fn invalid_cell_text_is_recoverable_and_unchanged_values_are_noops() {
    let table = editable_table();
    let draft = begin_draft(&table, "unfinished");
    assert!(super::super::draft::prepare(&draft, &table).is_err());
    assert_eq!(draft.text, "unfinished");
    assert!(
        super::super::draft::prepare(&begin_draft(&table, "20"), &table)
            .unwrap()
            .is_none()
    );
}

fn grid_frame(
    ctx: &egui::Context,
    table: &TableDataset,
    draft: &mut Option<TableCellEditDraft>,
    events: Vec<Event>,
) -> GridOutcome {
    let codecs = plotium_core::data::CodecRegistry::with_arrow_ipc();
    let reader = SnapshotReader::new(
        &table.typed_state.envelope.revision.snapshot,
        table.typed_state.store.as_ref(),
        &codecs,
    )
    .unwrap();
    let mut state = SheetState::default();
    let mut outcome = GridOutcome::default();
    let _ = ctx.run_ui(
        egui::RawInput {
            events,
            ..Default::default()
        },
        |ui| {
            outcome = typed_table_grid(ui, table, &reader, &mut state, draft, &mut |_, _| {});
        },
    );
    outcome
}

#[test]
fn escape_explicitly_discards_but_enter_preserves_draft_until_commit_succeeds() {
    let table = editable_table();
    let ctx = crate::typography::test_context();
    let mut draft = Some(begin_draft(&table, "42"));
    grid_frame(&ctx, &table, &mut draft, vec![]);
    let outcome = grid_frame(&ctx, &table, &mut draft, vec![key_event(Key::Enter)]);
    assert!(outcome.edit.is_some());
    assert_eq!(draft.as_ref().unwrap().text, "42");
    grid_frame(&ctx, &table, &mut draft, vec![key_event(Key::Escape)]);
    assert!(draft.is_none());
}

#[test]
fn grid_retains_draft_when_target_schema_changes() {
    let mut table = editable_table();
    let mut draft = Some(begin_draft(&table, "42"));
    table.typed_state.envelope.revision.id = plotium_core::data::RevisionId::new();
    let outcome = grid_frame(
        &crate::typography::test_context(),
        &table,
        &mut draft,
        vec![key_event(Key::Enter)],
    );
    assert!(outcome.edit.is_none());
    assert_eq!(draft.as_ref().unwrap().text, "42");
}

#[test]
fn applying_unchanged_text_preserves_surrounding_whitespace() {
    use plotium_core::data::{ColumnChunk, ColumnSchema, ColumnValues, LogicalType};
    let column = ColumnSchema::new("label", LogicalType::Utf8);
    let table = TableDataset::from_materialized(
        vec![(
            column.clone(),
            ColumnChunk::all_valid(ColumnValues::Utf8(vec![" a ".into()])),
        )],
        vec![],
        None,
        vec![],
        "plotium.test.cell-draft-text.v1",
    )
    .unwrap();
    let mut draft = None;
    start_edit(
        &mut SheetState::default(),
        &mut draft,
        &table,
        0,
        table.typed_rows(1, &[]).unwrap().row_ids[0],
        &ScalarValue::Utf8(" a ".into()),
        &column,
        None,
    )
    .unwrap();
    assert!(
        super::super::draft::prepare(draft.as_ref().unwrap(), &table)
            .unwrap()
            .is_none()
    );
    assert_eq!(draft.unwrap().text, " a ");
}

fn sheet_app(text: &str) -> plotium_core::state::PlotiumApp {
    let mut app = plotium_core::state::PlotiumApp::new();
    let original = editable_table();
    app.session.ui.table_cell_edit = Some(begin_draft(&original, text));
    app.session.ui.sheet_open = Some(0);
    app.doc
        .datasets
        .push(plotium_core::state::Dataset::Table(Box::new(original)));
    app.doc
        .datasets
        .push(plotium_core::state::Dataset::Table(Box::new(
            editable_table(),
        )));
    app
}

fn sheet_frame(
    ctx: &egui::Context,
    app: &mut plotium_core::state::PlotiumApp,
    events: Vec<Event>,
) -> egui::FullOutput {
    ctx.run_ui(
        egui::RawInput {
            events,
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1000.0, 800.0),
            )),
            ..Default::default()
        },
        |ui| super::super::data_sheet_window(app, ui.ctx()),
    )
}

#[test]
fn switching_tables_applies_valid_draft_without_requiring_a_second_request() {
    let mut app = sheet_app("42");
    app.session.ui.sheet_open = Some(1);
    sheet_frame(&crate::typography::test_context(), &mut app, vec![]);
    assert!(app.session.ui.table_cell_edit.is_none());
    assert_eq!(app.session.ui.sheet_open, Some(1));
    let values = app.doc.datasets[0]
        .as_table()
        .unwrap()
        .typed_rows(2, &[])
        .unwrap();
    assert_eq!(
        values.columns[0].values.get(1).cloned(),
        Some(ScalarValue::Float64(42.0))
    );
}

#[test]
fn invalid_draft_stays_visible_in_its_original_table_on_switch() {
    let mut app = sheet_app("unfinished");
    app.session.ui.sheet_open = Some(1);
    sheet_frame(&crate::typography::test_context(), &mut app, vec![]);
    assert_eq!(app.session.ui.sheet_open, Some(0));
    assert_eq!(
        app.session.ui.table_cell_edit.as_ref().unwrap().text,
        "unfinished"
    );
    assert!(
        app.session
            .ui
            .table_cell_edit
            .as_ref()
            .unwrap()
            .error
            .is_some()
    );
}

#[test]
fn deleting_original_table_and_discarding_draft_does_not_open_shifted_dataset() {
    let mut app = sheet_app("42");
    app.doc.datasets.remove(0);
    let ctx = crate::typography::test_context();
    sheet_frame(&ctx, &mut app, vec![]);
    let output = sheet_frame(&ctx, &mut app, vec![]);
    let position = output
        .shapes
        .iter()
        .find_map(|shape| {
            if let egui::Shape::Text(text) = &shape.shape
                && text.galley.job.text == "Discard"
            {
                return Some(text.pos + text.galley.size() / 2.0);
            }
            None
        })
        .expect("Discard is visible in the original sheet");
    let pointer = |pressed| Event::PointerButton {
        pos: position,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    sheet_frame(
        &ctx,
        &mut app,
        vec![Event::PointerMoved(position), pointer(true)],
    );
    sheet_frame(&ctx, &mut app, vec![pointer(false)]);
    assert!(app.session.ui.table_cell_edit.is_none());
    assert!(app.session.ui.sheet_open.is_none());
    sheet_frame(&ctx, &mut app, vec![]);
    assert!(app.session.ui.sheet_open.is_none());
}
