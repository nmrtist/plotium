use super::*;
fn fixture() -> (PlotiumApp, FigureDiagnosticParams) {
    let mut a = PlotiumApp::new_with_settings(Default::default());
    a.start_new_project();
    let mut c = CanvasDocument::new("Known geometry".into(), [100., 50.]);
    let panel = c.create_panel(
        "Not a general clip".into(),
        ObjectFrame::new(20., 20., 50., 50.),
    );
    c.panel_mut(panel).unwrap().clip_children = true;
    for kind in [
        ContentKind::Text(crate::state::TextBox::label(
            "Long native text outside the panel".into(),
        )),
        ContentKind::RasterImage(RasterImageContent::new(AssetId::new())),
    ] {
        let id = c.allocate_object_id();
        c.objects.push(ContentItem {
            id,
            name: "same".into(),
            frame: ObjectFrame::new(-10., -10., 80., 20.),
            locked: false,
            visible: true,
            kind,
        });
        c.panel_mut(panel).unwrap().item_order.push(id);
    }
    let p = FigureDiagnosticParams {
        canvas: TargetRef::resource(c.resource_id.into()),
        output: DiagnosticOutput { size_mm: c.size_mm },
        checks: all_checks(),
        targets: None,
        overlap_targets: vec![],
        view: DiagnosticView::Both,
        detail: DiagnosticDetail::Full,
        limit: 200,
    };
    a.doc.canvases.push(c);
    (a, p)
}
#[test]
fn native_parent_clip_is_not_applied_to_text_and_missing_asset_is_explicit() {
    let (a, p) = fixture();
    let mut scan =
        FigureDiagnosticScan::capture(&a, DocumentRevision(a.doc.automation_revision), p).unwrap();
    scan.font_status = Some("missing");
    let page = scan.next_page(|| false).unwrap();
    let rows = page["items"].as_array().unwrap();
    let text = rows
        .iter()
        .find(|r| r["kind"] == "layout" && r["component"] == "text")
        .unwrap();
    assert_eq!(text["evidence"]["bounds"]["x"], 10.);
    assert_eq!(text["evidence"]["clip"]["x"], 0.);
    assert_eq!(text["evidence"]["clip_sources"], json!(["output_page"]));
    assert!(
        rows.iter()
            .any(|r| r["status"] == "unavailable" && r["reason"] == "asset_missing")
    );
    assert!(
        rows.iter()
            .any(|r| r["status"] == "unavailable" && r["check"] == "text")
    );
    assert_eq!(page["coverage"]["status"], "limited");
}
#[test]
fn batches_never_sample_and_preserve_exact_point_range() {
    let (a, mut p) = fixture();
    p.checks = vec![DiagnosticCheck::LegendOcclusion];
    let mut scan =
        FigureDiagnosticScan::capture(&a, DocumentRevision(a.doc.automation_revision), p).unwrap();
    scan.items.clear();
    scan.pending.clear();
    scan.font_status = Some("available_glyph_coverage_unverified");
    let mut fig = plotium_figure::Figure::new(
        "",
        plotium_figure::Axis::new("", 0., 1.),
        plotium_figure::Axis::new("", 0., 1.),
    );
    let mut series =
        plotium_figure::Series::points("sparse", vec![[0., 0.]; DIAGNOSTIC_POINTS + 1]);
    series.origin = Some(plotium_figure::SeriesOrigin(9));
    fig.series.push(series);
    let runs = std::mem::take(&mut fig.series).into();
    scan.curves.push_back(CurveWork {
        target: TargetRef::resource(canvas_object_ref(
            a.doc.canvases[0].resource_id,
            a.doc.canvases[0].objects[0].id,
        )),
        frame: Rect::new(0., 0., 100., 100.),
        plot: Rect::new(0., 0., 100., 100.),
        legend: Some(Rect::new(40., 40., 20., 20.)),
        figure: std::sync::Arc::new(fig),
        runs,
        current: None,
    });
    let first = scan.next_page(|| false).unwrap();
    assert_eq!(first["budget"]["batch_points"], DIAGNOSTIC_POINTS);
    assert_eq!(first["truncated"], true);
    assert_eq!(first["coverage"]["sampling"], false);
    let second = scan.next_page(|| false).unwrap();
    assert_eq!(second["budget"]["batch_points"], 1);
    assert_eq!(second["coverage"]["scanned_points"], DIAGNOSTIC_POINTS + 1);
    assert_eq!(second["truncated"], false);
    assert_eq!(
        second["items"][0]["evidence"]["point_range"],
        json!([DIAGNOSTIC_POINTS, DIAGNOSTIC_POINTS + 1])
    );
    assert!(
        !first["items"]
            .as_array()
            .unwrap()
            .iter()
            .chain(second["items"].as_array().unwrap())
            .any(|r| r["code"] == "legend_occlusion_potential")
    );
}
#[test]
fn missing_data_and_unsupported_native_geometry_cannot_claim_clean() {
    let (a, p) = fixture();
    let mut scan =
        FigureDiagnosticScan::capture(&a, DocumentRevision(a.doc.automation_revision), p).unwrap();
    scan.items.clear();
    scan.pending.clear();
    scan.font_status = Some("available_glyph_coverage_unverified");
    let mut f = plotium_figure::Figure::new(
        "",
        plotium_figure::Axis::new("", 0., 1.),
        plotium_figure::Axis::new("", 0., 1.),
    );
    f.annotations.push(plotium_figure::Annotation {
        text: "derived".into(),
        at: [0.5, 0.5],
        size: 8.,
        color: plotium_figure::Color::BLACK,
        anchor: plotium_figure::TextAnchor::Point,
    });
    let target = TargetRef::resource(canvas_object_ref(
        a.doc.canvases[0].resource_id,
        a.doc.canvases[0].objects[0].id,
    ));
    scan.items.push_back(SnapshotItem {
        target,
        frame: Rect::new(0., 0., 100., 100.),
        visible: true,
        kind: SnapshotKind::Plot {
            figure: Box::new(f),
            stack: None,
            selected_series: None,
            declared: vec![(SeriesId::new(1), true, "resolved".into())],
            facts: Default::default(),
        },
    });
    let page = scan.next_page(|| false).unwrap();
    let rows = page["items"].as_array().unwrap();
    assert!(rows.iter().any(|r| r["status"] == "unsupported"));
    assert!(rows.iter().any(|r| {
        r["reason"]
            .as_str()
            .is_some_and(|s| s.starts_with("empty_rendered_data"))
    }));
    assert_eq!(page["coverage"]["status"], "limited");
}
#[test]
fn provenance_does_not_change_v1_serialization_and_capture_is_cancellable() {
    let mut series = plotium_figure::Series::line("same", vec![[0., 0.], [1., 1.]]);
    let before = json!(series);
    series.origin = Some(plotium_figure::SeriesOrigin(91));
    assert_eq!(json!(series), before);
    let restored: plotium_figure::Series = serde_json::from_value(before).unwrap();
    assert_eq!(restored.origin, None);
    let (a, p) = fixture();
    let mut scan =
        FigureDiagnosticScan::capture(&a, DocumentRevision(a.doc.automation_revision), p).unwrap();
    assert!(matches!(
        scan.next_page(|| true),
        Err(AutomationError::DiagnosticCancelled)
    ));
    assert!(scan.font_status.is_none());
}
