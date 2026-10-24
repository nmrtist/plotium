use super::*;
use crate::automation::*;
use plotium_data::{ColumnChunk, ColumnSchema, ColumnValues, LogicalType};

/// A table of text groups and numeric values, one row per pair.
fn table(app: &mut PlotiumApp, rows: &[(&str, f64)]) {
    let table = TableDataset::from_materialized(
        vec![
            (
                ColumnSchema::new("group", LogicalType::Utf8),
                ColumnChunk::all_valid(ColumnValues::Utf8(
                    rows.iter().map(|(g, _)| (*g).to_owned()).collect(),
                )),
            ),
            (
                ColumnSchema::new("value", LogicalType::Float64),
                ColumnChunk::optional_f64(rows.iter().map(|(_, v)| Some(*v))),
            ),
            (
                ColumnSchema::new("x", LogicalType::Float64),
                ColumnChunk::optional_f64((0..rows.len()).map(|i| Some(i as f64))),
            ),
        ],
        vec![],
        None,
        vec![],
        "plotium.test.hints",
    )
    .unwrap();
    app.doc.datasets.push(Dataset::Table(Box::new(table)));
}

fn source(app: &PlotiumApp, x: usize) -> TableCurveSource {
    let dataset = &app.doc.datasets[0];
    let r = &dataset.as_table().unwrap().typed_state.envelope.revision;
    let c = &r.snapshot.schema.columns;
    TableCurveSource {
        dataset_id: dataset.resource_id(),
        field_id: dataset.default_field_id().unwrap(),
        table_id: r.table_id,
        table_revision: r.id,
        x_column_id: c[x].id,
        y_column_id: c[1].id,
        y_error_column_id: None,
    }
}

const GROUP: usize = 0;
const NUMERIC: usize = 2;

/// A new canvas holding one plot of `kinds`, each over the same columns.
fn plot(rows: &[(&str, f64)], x: usize, kinds: &[CurveKind]) -> (PlotiumApp, PlotCreationSummary) {
    let mut app = PlotiumApp::new();
    table(&mut app, rows);
    let series = kinds
        .iter()
        .map(|kind| NewCurve {
            kind: *kind,
            source: source(&app, x),
            y_axis: plotium_figure::YAxisSide::Left,
        })
        .collect();
    let params = PlotCreateParams {
        parent: document_root_target(),
        name: "Hints".into(),
        canvas: None,
        panel_name: None,
        panel_frame_pt: None,
        series,
        intent: None,
    };
    let planned = app
        .plan_plot_creation(DocumentRevision(app.doc.automation_revision), &params)
        .unwrap();
    let summary = planned.creation.clone().unwrap();
    app.commit_curve_bindings(planned).unwrap();
    (app, summary)
}

/// Hint rows of a full scan of the first canvas.
fn hints(app: &PlotiumApp) -> Vec<Value> {
    rows(app)
        .into_iter()
        .filter(|r| r["severity"] == "hint")
        .collect()
}

/// Every issue and coverage row of a full scan of the first canvas.
fn rows(app: &PlotiumApp) -> Vec<Value> {
    let c = &app.doc.canvases[0];
    let params = FigureDiagnosticParams {
        canvas: TargetRef::resource(c.resource_id.into()),
        output: DiagnosticOutput { size_mm: c.size_mm },
        checks: all_checks(),
        targets: None,
        overlap_targets: vec![],
        view: DiagnosticView::Issues,
        detail: DiagnosticDetail::Full,
        limit: 200,
    };
    let mut scan =
        FigureDiagnosticScan::capture(app, DocumentRevision(app.doc.automation_revision), params)
            .unwrap();
    let mut out = Vec::new();
    loop {
        let page = scan.next_page(|| false).unwrap();
        out.extend(page["items"].as_array().unwrap().iter().cloned());
        if page["coverage"]["scan_complete"] == true {
            return out;
        }
    }
}

fn codes(rows: &[Value]) -> Vec<&str> {
    rows.iter().filter_map(|r| r["code"].as_str()).collect()
}

fn set(app: &mut PlotiumApp, target: &TargetRef, key: &str, value: Value) {
    let request = ToolRequest {
        tool_id: "properties.set".into(),
        tool_version: 1,
        parameters: json!({"key": key, "value": value}),
        targets: match target.component {
            Some(_) => TargetSelector::Components {
                targets: vec![target.clone()],
            },
            None => TargetSelector::Explicit {
                ids: vec![target.resource.id.clone()],
            },
        },
        expected_revision: DocumentRevision(app.doc.automation_revision),
        caller: CallerType::Agent,
    };
    let plan = plan_tool(app, request).unwrap();
    execute_tool(app, plan, ExecutionAuthority::ReversibleModify).unwrap();
}

#[test]
fn default_bar_and_line_plots_have_no_hints() {
    let (bars, _) = plot(
        &[("a", 2.0), ("b", 3.0), ("c", 2.5)],
        GROUP,
        &[CurveKind::Bar],
    );
    assert_eq!(codes(&hints(&bars)), Vec::<&str>::new());
    let (line, _) = plot(
        &[("a", 1.0), ("b", 4.0), ("c", 9.0)],
        NUMERIC,
        &[CurveKind::Line],
    );
    assert_eq!(codes(&hints(&line)), Vec::<&str>::new());
}

#[test]
fn value_labels_beside_their_bars_are_not_text_over_data() {
    let (mut app, summary) = plot(
        &[("a", 2.0), ("b", -3.0), ("c", 2.5)],
        GROUP,
        &[CurveKind::Bar],
    );
    set(
        &mut app,
        &summary.series_defaults[0].target,
        "series.value_labels",
        json!(true),
    );
    assert_eq!(codes(&hints(&app)), Vec::<&str>::new());
    set(
        &mut app,
        &summary.plot,
        "object.chart.orientation",
        json!("horizontal"),
    );
    assert_eq!(codes(&hints(&app)), Vec::<&str>::new());
}

#[test]
fn tiny_bars_beside_tall_ones_suggest_labels_or_a_break() {
    let (app, summary) = plot(&[("PEM", 10.0), ("BPM", 0.03)], GROUP, &[CurveKind::Bar]);
    let rows = hints(&app);
    let row = rows
        .iter()
        .find(|r| r["code"] == "small_values_invisible")
        .unwrap();
    assert_eq!(row["target"], json!(summary.series_defaults[0].target));
    assert_eq!(row["evidence"]["small_bars"], 1);
    let properties: Vec<&Value> = row["suggestions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| &s["property"])
        .collect();
    assert!(properties.contains(&&json!("series.value_labels")));
    assert!(properties.contains(&&json!("object.axes.y_break")));
}

#[test]
fn bar_hints_survive_save_and_load_and_turn_with_the_chart() {
    let (mut app, summary) = plot(&[("PEM", 10.0), ("BPM", 0.03)], GROUP, &[CurveKind::Bar]);
    set(
        &mut app,
        &summary.plot,
        "object.chart.orientation",
        json!("horizontal"),
    );
    let row = hints(&app)
        .into_iter()
        .find(|r| r["code"] == "small_values_invisible")
        .unwrap();
    assert_eq!(row["evidence"]["value_axis"], "x");
    let path = std::env::temp_dir().join(format!("plotium-hints-{}.plotium", uuid::Uuid::new_v4()));
    crate::project::save_project(&app, &path, false).unwrap();
    let loaded = crate::project::load_project(&path).unwrap();
    std::fs::remove_file(path).unwrap();
    assert!(codes(&hints(&loaded)).contains(&"small_values_invisible"));
}

#[test]
fn crowded_categories_report_narrow_bars_and_label_collisions() {
    let names: Vec<String> = (0..160).map(|i| format!("sample {i}")).collect();
    let rows: Vec<(&str, f64)> = names.iter().map(|n| (n.as_str(), 1.0)).collect();
    let (app, _) = plot(&rows, GROUP, &[CurveKind::Bar]);
    let found = hints(&app);
    let codes = codes(&found);
    assert!(codes.contains(&"bars_too_narrow"), "{codes:?}");
    assert!(codes.contains(&"too_many_categories"), "{codes:?}");
    let narrow = found
        .iter()
        .find(|r| r["code"] == "bars_too_narrow")
        .unwrap();
    assert_eq!(
        narrow["suggestions"][0]["property"],
        "object.chart.orientation"
    );
}

#[test]
fn hidden_category_labels_are_not_crowded() {
    let names: Vec<String> = (0..160).map(|i| format!("sample {i}")).collect();
    let rows: Vec<(&str, f64)> = names.iter().map(|n| (n.as_str(), 1.0)).collect();
    let (mut app, summary) = plot(&rows, GROUP, &[CurveKind::Bar]);
    set(
        &mut app,
        &summary.plot,
        "object.axes.x_show_tick_labels",
        json!(false),
    );
    let found = hints(&app);
    assert!(!codes(&found).contains(&"too_many_categories"));
    assert!(!codes(&found).contains(&"labels_overlap"));
}

#[test]
fn few_raw_samples_ask_for_points_until_they_are_drawn() {
    let rows = [
        ("a", 1.0),
        ("a", 2.0),
        ("a", 3.0),
        ("b", 2.0),
        ("b", 4.0),
        ("b", 5.0),
    ];
    let (boxes, _) = plot(&rows, GROUP, &[CurveKind::Box]);
    assert!(codes(&hints(&boxes)).contains(&"few_samples_hidden"));
    let (with_points, _) = plot(&rows, GROUP, &[CurveKind::Box, CurveKind::Scatter]);
    assert!(!codes(&hints(&with_points)).contains(&"few_samples_hidden"));
    // One row per bar is already a summary; its raw values are unknown.
    let (means, _) = plot(&[("a", 2.0), ("b", 3.0)], GROUP, &[CurveKind::Bar]);
    assert!(!codes(&hints(&means)).contains(&"few_samples_hidden"));
}

#[test]
fn decades_on_a_linear_axis_suggest_log_until_it_is_log() {
    let rows = [("a", 1.0), ("b", 30.0), ("c", 900.0), ("d", 25_000.0)];
    let (mut app, summary) = plot(&rows, NUMERIC, &[CurveKind::Scatter]);
    let row = hints(&app)
        .into_iter()
        .find(|r| r["code"] == "log_scale_candidate")
        .unwrap();
    assert_eq!(row["suggestions"][0]["property"], "object.axes.y_scale");
    set(&mut app, &summary.plot, "object.axes.y_scale", json!("log"));
    assert!(!codes(&hints(&app)).contains(&"log_scale_candidate"));
}

#[test]
fn a_value_axis_that_cuts_the_bars_is_reported() {
    let (mut app, summary) = plot(&[("a", 4.0), ("b", 5.0)], GROUP, &[CurveKind::Bar]);
    set(
        &mut app,
        &summary.plot,
        "object.axes.y_range",
        json!({"mode": "manual", "min": 3.0, "max": 6.0, "unit": null}),
    );
    let found = hints(&app);
    assert!(
        codes(&found).contains(&"bar_baseline_not_zero"),
        "{found:?}"
    );
}

#[test]
fn overlapping_canvas_texts_name_both_and_a_direction() {
    let (mut app, _) = plot(&[("a", 2.0), ("b", 3.0)], GROUP, &[CurveKind::Bar]);
    let canvas = &mut app.doc.canvases[0];
    for (left, text) in [(5.0, "Overlapping caption"), (12.0, "Second caption")] {
        let id = canvas.allocate_object_id();
        canvas.objects.push(ContentItem {
            id,
            name: text.into(),
            frame: ObjectFrame::new(left, 5., 120., 20.),
            locked: false,
            visible: true,
            kind: ContentKind::Text(crate::state::TextBox::label(text.into())),
        });
    }
    let found = hints(&app);
    let row = found
        .iter()
        .find(|r| {
            r["code"] == "labels_overlap"
                && r["component"] == "text"
                && r["evidence"]["other_component"] == "text"
        })
        .unwrap();
    // One line height down is a shorter move than past the whole caption.
    assert_eq!(row["evidence"]["move"]["direction"], "down");
}

#[test]
fn text_on_a_bar_is_text_over_data() {
    let (app, summary) = plot(&[("a", 2.0), ("b", 3.0)], GROUP, &[CurveKind::Bar]);
    let canvas = &app.doc.canvases[0];
    let mut figure = canvas.objects[0].plot().unwrap().figure().clone();
    figure.annotations.push(plotium_figure::Annotation {
        display_baseline: 0.0,
        text: "inside".into(),
        at: [0.0, 1.0],
        size: 7.,
        color: plotium_figure::Color::BLACK,
        anchor: plotium_figure::TextAnchor::Point,
        offset_pt: None,
    });
    let target = summary.plot.clone();
    let mut facts = PlotFacts::default();
    let id = summary.series_defaults[0].target.component;
    let Some(ComponentRef::Series(id)) = id else {
        panic!("series target")
    };
    facts.kinds.push((id, SeriesKind::Bar));
    let mut scan = scan_of(&app);
    let frame = canvas
        .content_page_frame(canvas.objects[0].id)
        .unwrap()
        .rect();
    let guide = plotium_render::guide_geometry_placed(&figure, [frame.width, frame.height], None);
    let annotations = geometry::annotation_text_boxes(&figure, [frame.width, frame.height], None);
    scan.plot_hints(
        &target,
        frame,
        &figure,
        guide.plot,
        (&[], &annotations),
        &facts,
    );
    let page = scan.pending.iter().find(|r| r["code"] == "text_over_data");
    assert!(page.is_some());
}

fn scan_of(app: &PlotiumApp) -> FigureDiagnosticScan {
    let c = &app.doc.canvases[0];
    let params = FigureDiagnosticParams {
        canvas: TargetRef::resource(c.resource_id.into()),
        output: DiagnosticOutput { size_mm: c.size_mm },
        checks: all_checks(),
        targets: None,
        overlap_targets: vec![],
        view: DiagnosticView::Issues,
        detail: DiagnosticDetail::Full,
        limit: 200,
    };
    let mut scan =
        FigureDiagnosticScan::capture(app, DocumentRevision(app.doc.automation_revision), params)
            .unwrap();
    scan.items.clear();
    scan
}

#[test]
fn neighbouring_panels_report_offset_edges_and_uneven_gaps() {
    let (app, summary) = plot(&[("a", 2.0)], GROUP, &[CurveKind::Bar]);
    let mut scan = scan_of(&app);
    let target = |_: u64| summary.plot.clone();
    // Three frames in a row with 10 and 30 pt gutters; the middle plot area
    // sits 6 pt higher.
    for (i, left, drop) in [(1, 0.0, 0.0), (2, 110.0, -6.0), (3, 240.0, 0.0)] {
        let frame = Rect::new(left, 0., 100., 100.);
        let area = Rect::new(left + 20., 10., 75., 70. + drop);
        scan.hint_plots.push((target(i), frame, area));
    }
    scan.canvas_hints();
    let rows: Vec<&Value> = scan
        .pending
        .iter()
        .filter(|r| r["code"] == "panels_misaligned")
        .collect();
    assert!(rows.iter().any(|r| r["evidence"]["edge"] == "bottom"));
    assert!(rows.iter().any(|r| r["evidence"]["direction"] == "row"));
}

fn intent_codes(app: &PlotiumApp) -> Vec<String> {
    hints(app)
        .into_iter()
        .filter(|r| r["code"] == "intent_mismatch")
        .map(|r| r["evidence"]["purpose"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn intent_hints_wait_for_a_stated_purpose() {
    let (mut app, summary) = plot(
        &[("a", 1.0), ("b", 4.0), ("c", 9.0)],
        NUMERIC,
        &[CurveKind::Bar],
    );
    let unstated = rows(&app)
        .into_iter()
        .find(|r| r["kind"] == "coverage" && r["check"] == "intent")
        .unwrap();
    assert_eq!(unstated["status"], "not_applicable");
    assert!(intent_codes(&app).is_empty());

    set(
        &mut app,
        &summary.plot,
        "object.intent.purpose",
        json!("trend"),
    );
    set(
        &mut app,
        &summary.plot,
        "object.intent.claim",
        json!("The signal grows with x."),
    );
    let row = hints(&app)
        .into_iter()
        .find(|r| r["code"] == "intent_mismatch")
        .unwrap();
    assert_eq!(row["evidence"]["claim"], "The signal grows with x.");
    assert_eq!(row["evidence"]["chart"]["marks"], json!(["bar"]));
    assert!(
        row["suggestions"][0]["action"]
            .as_str()
            .unwrap()
            .starts_with("Redraw as a line")
    );
    // The same bars suit a comparison.
    set(
        &mut app,
        &summary.plot,
        "object.intent.purpose",
        json!("compare"),
    );
    assert!(intent_codes(&app).is_empty());
}

/// Rows, x column, curve kinds, purpose, and whether it is reported.
type Case<'a> = (&'a [(&'a str, f64)], usize, &'a [CurveKind], &'a str, bool);

#[test]
fn purposes_flag_the_chart_forms_that_cannot_carry_them() {
    let groups = [("PEM", 10.0), ("BPM", 0.03)];
    let numeric = [("a", 1.0), ("b", 30.0), ("c", 900.0)];
    let cases: [Case; 9] = [
        (&groups, GROUP, &[CurveKind::Bar], "change", true),
        (&groups, GROUP, &[CurveKind::Scatter], "change", true),
        (&numeric, NUMERIC, &[CurveKind::Line], "change", true),
        (&groups, GROUP, &[CurveKind::Line], "compare", true),
        (&groups, GROUP, &[CurveKind::Bar], "distribution", true),
        (
            &groups,
            GROUP,
            &[CurveKind::Box, CurveKind::Scatter],
            "distribution",
            false,
        ),
        (&groups, GROUP, &[CurveKind::Scatter], "relationship", true),
        (
            &numeric,
            NUMERIC,
            &[CurveKind::Scatter],
            "relationship",
            false,
        ),
        (&numeric, NUMERIC, &[CurveKind::Line], "composition", true),
    ];
    for (rows, x, kinds, purpose, flagged) in cases {
        let (mut app, summary) = plot(rows, x, kinds);
        set(
            &mut app,
            &summary.plot,
            "object.intent.purpose",
            json!(purpose),
        );
        assert_eq!(
            !intent_codes(&app).is_empty(),
            flagged,
            "{purpose} with {kinds:?}"
        );
    }
    // Points on a log axis give equal folds equal distances.
    let (mut app, summary) = plot(&groups, GROUP, &[CurveKind::Scatter]);
    set(
        &mut app,
        &summary.plot,
        "object.intent.purpose",
        json!("change"),
    );
    set(&mut app, &summary.plot, "object.axes.y_scale", json!("log"));
    assert!(intent_codes(&app).is_empty());
}

#[test]
fn a_stated_replicate_count_reports_few_samples_behind_mean_bars() {
    let (mut app, summary) = plot(&[("a", 2.0), ("b", 3.0)], GROUP, &[CurveKind::Bar]);
    let bar = summary.series_defaults[0].target.clone();
    set(&mut app, &bar, "series.replicates", json!(3));
    let row = hints(&app)
        .into_iter()
        .find(|r| r["code"] == "few_samples_hidden")
        .unwrap();
    assert_eq!(row["evidence"]["source"], "stated");
    assert_eq!(row["evidence"]["replicates"], 3);
    let path = std::env::temp_dir().join(format!("plotium-n-{}.plotium", uuid::Uuid::new_v4()));
    crate::project::save_project(&app, &path, false).unwrap();
    let loaded = crate::project::load_project(&path).unwrap();
    std::fs::remove_file(path).unwrap();
    assert!(codes(&hints(&loaded)).contains(&"few_samples_hidden"));
    // Without a y error column there are no error bars to describe.
    let request = ToolRequest {
        tool_id: "properties.set".into(),
        tool_version: 1,
        parameters: json!({"key":"series.error_bars","value":"sem"}),
        targets: TargetSelector::Components {
            targets: vec![bar.clone()],
        },
        expected_revision: DocumentRevision(app.doc.automation_revision),
        caller: CallerType::Agent,
    };
    let plan = plan_tool(&app, request).unwrap();
    let result = execute_tool(&mut app, plan, ExecutionAuthority::ReversibleModify).unwrap();
    assert_eq!(
        result.targets[0].message,
        "Bind a y error column to this curve first."
    );
    set(&mut app, &bar, "series.replicates", json!(12));
    assert!(!codes(&hints(&app)).contains(&"few_samples_hidden"));
    set(&mut app, &bar, "series.replicates", json!(0));
    assert_eq!(
        app.doc.canvases[0].objects[0]
            .plot()
            .unwrap()
            .binding
            .series[0]
            .replicates,
        None
    );
}

#[test]
fn table_chart_plots_are_outside_the_intent_rules() {
    let mut app = PlotiumApp::new();
    table(&mut app, &[("a", 1.0), ("b", 4.0)]);
    let mut canvas = CanvasDocument::new("table".to_owned(), [120.0, 80.0]);
    let [w, h] = canvas.size_pt();
    let id = canvas.allocate_object_id();
    let mut object =
        app.build_plot_object(0, ObjectFrame::new(0.0, 0.0, w, h), id, "Plot".to_owned());
    object.plot_mut().unwrap().intent.purpose = Some(PlotPurpose::Distribution);
    canvas.objects.push(object);
    app.doc.canvases.push(canvas);
    let row = rows(&app)
        .into_iter()
        .find(|r| r["kind"] == "coverage" && r["check"] == "intent")
        .unwrap();
    assert!(
        row["reason"]
            .as_str()
            .unwrap()
            .starts_with("chart_type_not_covered")
    );
    assert!(intent_codes(&app).is_empty());
}

#[test]
fn intent_rules_read_the_category_axis_of_horizontal_plots() {
    let (mut app, summary) = plot(&[("a", 1.0), ("b", 4.0)], GROUP, &[CurveKind::Bar]);
    set(
        &mut app,
        &summary.plot,
        "object.chart.orientation",
        json!("horizontal"),
    );
    set(
        &mut app,
        &summary.plot,
        "object.intent.purpose",
        json!("trend"),
    );
    let row = hints(&app)
        .into_iter()
        .find(|r| r["code"] == "intent_mismatch")
        .unwrap();
    assert_eq!(row["evidence"]["chart"]["category_axis"], true);
}

#[test]
fn rebinding_a_curve_forgets_its_stated_replicates() {
    let (mut app, summary) = plot(&[("a", 2.0), ("b", 3.0)], GROUP, &[CurveKind::Bar]);
    let bar = summary.series_defaults[0].target.clone();
    set(&mut app, &bar, "series.replicates", json!(3));
    let source = TableCurveSource {
        x_column_id: {
            let r = &app.doc.datasets[0]
                .as_table()
                .unwrap()
                .typed_state
                .envelope
                .revision;
            r.snapshot.schema.columns[NUMERIC].id
        },
        ..source(&app, GROUP)
    };
    let request = ToolRequest {
        tool_id: "plot.series.bind".into(),
        tool_version: 1,
        parameters: json!({"edits":[{"target":bar,"source":source}]}),
        targets: TargetSelector::Explicit { ids: vec![] },
        expected_revision: DocumentRevision(app.doc.automation_revision),
        caller: CallerType::Agent,
    };
    let plan = plan_tool(&app, request).unwrap();
    execute_tool(&mut app, plan, ExecutionAuthority::ReversibleModify).unwrap();
    let series = &app.doc.canvases[0].objects[0]
        .plot()
        .unwrap()
        .binding
        .series[0];
    assert_eq!(series.replicates, None);
}
