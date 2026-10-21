use super::*;
use crate::actions::Action;
use crate::state::*;
use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
use std::io::Cursor;

fn asset() -> AssetRecord {
    let im = RgbaImage::from_fn(64, 48, |x, y| {
        Rgba(if x < 32 {
            if y < 24 {
                [230, 40, 50, 255]
            } else {
                [40, 80, 230, 255]
            }
        } else if y < 24 {
            [20, 180, 70, 255]
        } else {
            [245, 180, 10, 255]
        })
    });
    let mut out = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(im)
        .write_to(&mut out, ImageFormat::Png)
        .unwrap();
    let bytes = out.into_inner();
    AssetRecord {
        id: AssetId::new(),
        sha256: plotium_io::image::sha256(&bytes),
        format: "png".into(),
        pixel_size: [64, 48],
        bytes,
    }
}
fn fixture() -> PlotiumApp {
    let mut app = PlotiumApp::new();
    let x = plotium_data::ColumnSchema::new("Time (s)", plotium_data::LogicalType::Float64);
    let y = plotium_data::ColumnSchema::new("Signal (a.u.)", plotium_data::LogicalType::Float64);
    let xi = x.id;
    let yi = y.id;
    let table = TableDataset::from_materialized(
        vec![
            (
                x,
                plotium_data::ColumnChunk::all_valid(plotium_data::ColumnValues::Float64(vec![
                    0., 1., 2., 3., 4.,
                ])),
            ),
            (
                y,
                plotium_data::ColumnChunk::all_valid(plotium_data::ColumnValues::Float64(vec![
                    1., 2., 4., 3., 5.,
                ])),
            ),
        ],
        vec![],
        Some(xi),
        vec![TableSeriesBinding {
            value_column: yi,
            uncertainty_column: None,
            fit: None,
        }],
        "plotium.test.r2",
    )
    .unwrap();
    app.doc.datasets.push(Dataset::Table(Box::new(table)));
    app.doc.datasets[0]
        .as_table_mut()
        .unwrap()
        .board_sheet_visible = false;
    app.doc.datasets.push(Dataset::Nmr(Box::new(
        NmrDataset::load_with_pipeline(
            plotium_io::NmrData {
                points: (0..128)
                    .map(|v| {
                        let x = f64::from(v) / 127.;
                        num_complex::Complex64::new((x * 12.).sin() * (1. - x) + 0.25, 0.)
                    })
                    .collect(),
                domain: plotium_io::Domain::Frequency,
                spectral_width_hz: 4000.,
                observe_freq_mhz: 400.,
                carrier_ppm: 0.,
                nucleus: "1H".into(),
                source: "R2 synthetic line".into(),
                group_delay: 0.,
            },
            Some(plotium_processing::AxisPipeline { steps: vec![] }),
            Some(false),
        )
        .unwrap(),
    )));
    let mut c = CanvasDocument::new("R2 trusted preview".into(), [180., 90.]);
    for left in [10., 260.] {
        let id = c.allocate_object_id();
        let o = app.build_plot_object(
            if left < 100. { 1 } else { 0 },
            ObjectFrame::new(left, 22., 230., 190.),
            id,
            "Synthetic curve".into(),
        );
        c.objects.push(o);
        c.create_panel_for_plot(id).unwrap();
    }
    let a = asset();
    let aid = a.id;
    app.doc.assets.insert(aid, a);
    let id = c.allocate_object_id();
    let mut i = RasterImageContent::new(aid);
    i.crop = [0.125, 0.125, 0.875, 0.875];
    i.rotation = QuarterTurn::Clockwise90;
    i.fit = ImageFit::Cover;
    c.objects.push(ContentItem {
        id,
        name: "Local quadrant PNG".into(),
        frame: ObjectFrame::new(175., 115., 80., 95.),
        locked: false,
        visible: true,
        kind: ContentKind::RasterImage(i),
    });
    c.panels[1].item_order.push(id);
    c.panels[1].clip_children = true;
    let id = c.allocate_object_id();
    c.objects.push(ContentItem {
        id,
        name: "Preview caption".into(),
        frame: ObjectFrame::new(10., 225., 490., 20.),
        locked: false,
        visible: true,
        kind: ContentKind::Text(TextBox::label(
            "R2 PREVIEW - native text, curves & local image".into(),
        )),
    });
    app.doc.canvases.push(c);
    app.session.active_canvas = Some(0);
    app.set_active_dataset(Some(0));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while app.compute_busy() && std::time::Instant::now() < deadline {
        app.poll_compute();
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(!app.compute_busy());
    app.rebuild_canvas(0);
    assert!(
        app.doc.canvases[0].objects[0]
            .plot()
            .unwrap()
            .figure()
            .series
            .iter()
            .any(|s| s.points.len() > 10),
        "acceptance fixture must contain a rendered continuous curve"
    );
    app
}
fn request(app: &PlotiumApp, edge: u32) -> ToolRequest {
    let target = TargetRef::resource(app.doc.canvases[0].resource_id.into());
    ToolRequest {
        tool_id: IMAGE_PREVIEW.into(),
        tool_version: 1,
        parameters: json!(ImagePreviewParams {
            target: target.clone(),
            max_edge: edge
        }),
        targets: TargetSelector::Explicit {
            ids: vec![target.resource.id],
        },
        expected_revision: DocumentRevision(app.doc.automation_revision),
        caller: CallerType::Agent,
    }
}
fn snapshot(app: &PlotiumApp, edge: u32) -> Result<ImagePreviewSnapshot, AutomationError> {
    ImagePreviewSnapshot::capture(app, &plan_tool(app, request(app, edge))?)
}
fn pixels(result: &ToolResult) -> Vec<u8> {
    base64::engine::general_purpose::STANDARD
        .decode(result.value["png_base64"].as_str().unwrap())
        .unwrap()
}
#[test]
fn native_text_curve_asset_snapshot_matches_existing_renderer_and_survives_edits() {
    let mut app = fixture();
    let snap = snapshot(&app, 720).unwrap();
    let mut opts = RasterOptions::new(72);
    opts.target_width_mm = Some(254.);
    let expected = crate::export::rasterize_canvas_with_assets(
        &app.doc.canvases[0],
        &app.doc.assets,
        opts,
        MissingImagePolicy::Block,
    )
    .unwrap()
    .to_png()
    .unwrap();
    let plan = plan_tool(&app, request(&app, 720)).unwrap();
    let run = PreparedToolRun::begin(&app, &plan);
    app.execute_action(Action::rename_canvas(
        0,
        app.doc.canvases[0].name.clone(),
        "Edited after capture".into(),
    ));
    app.doc.assets.clear();
    let result = snap.render().unwrap();
    assert_eq!(pixels(&result), expected);
    assert_eq!(result.value["width_px"], 720);
    assert_eq!(result.value["height_px"], 360);
    assert_eq!(result.value["render_revision"], 0);
    assert_eq!(result.before_revision, DocumentRevision(0));
    assert_eq!(result.after_revision, DocumentRevision(0));
    run.finish(&mut app, &Ok(result)).unwrap();
    assert!(app.doc.automation_runs.is_empty());
    assert_eq!(app.doc.automation_revision, 1);
    assert!(matches!(
        plan_tool(&app, plan.request),
        Err(AutomationError::StaleRevision { .. })
    ));
}
#[test]
fn whole_target_and_stale_plan_are_strict_without_selection_fallback() {
    let mut app = fixture();
    let req = request(&app, 2048);
    let plan = plan_tool(&app, req.clone()).unwrap();
    for key in ["kind", "parent_id", "local_id", "id"] {
        let mut bad = req.clone();
        bad.parameters["target"]["resource"][key] = json!("forged");
        assert!(plan_tool(&app, bad).is_err(), "{key}");
    }
    let mut bad = req.clone();
    bad.parameters["target"]["component"] = json!({"kind":"series","id":1});
    assert!(plan_tool(&app, bad).is_err());
    let mut bad = req.clone();
    bad.targets = TargetSelector::CurrentSelection;
    assert!(plan_tool(&app, bad).is_err());
    app.doc.automation_revision += 1;
    assert!(matches!(
        ImagePreviewSnapshot::capture(&app, &plan),
        Err(AutomationError::StaleRevision { .. })
    ));
    assert!(matches!(
        snapshot(&app, 2049),
        Err(AutomationError::PreviewBudget(_))
    ));
    assert!(snapshot(&app, 0).is_err());
}
#[test]
fn missing_corrupt_page_and_encoded_asset_overflow_never_succeed_partially() {
    let mut app = fixture();
    let id = *app.doc.assets.keys().next().unwrap();
    let original = app.doc.assets.remove(&id).unwrap();
    assert!(matches!(
        snapshot(&app, 64),
        Err(AutomationError::PreviewAsset(_))
    ));
    app.doc.assets.insert(id, original.clone());
    app.doc.assets.get_mut(&id).unwrap().bytes[0] ^= 1;
    assert!(matches!(
        snapshot(&app, 64).unwrap().render(),
        Err(AutomationError::PreviewAsset(_))
    ));
    app.doc.assets.insert(id, original.clone());
    app.doc.assets.get_mut(&id).unwrap().sha256 = [0; 32];
    assert!(matches!(
        snapshot(&app, 64).unwrap().render(),
        Err(AutomationError::PreviewAsset(_))
    ));
    app.doc.assets.insert(id, original.clone());
    if let ContentKind::RasterImage(i) = &mut app.doc.canvases[0].objects[2].kind {
        i.page_index = 1;
    }
    assert!(matches!(
        snapshot(&app, 64).unwrap().render(),
        Err(AutomationError::PreviewAsset(_))
    ));
    app.doc.assets.insert(id, original);
    app.doc
        .assets
        .get_mut(&id)
        .unwrap()
        .bytes
        .resize(PREVIEW_ASSET_BYTES + 1, 0);
    assert!(matches!(
        snapshot(&app, 64),
        Err(AutomationError::PreviewBudget(_))
    ));
}
#[test]
fn every_byte_budget_accepts_boundary_and_rejects_next_byte() {
    for limit in [
        PREVIEW_ASSET_BYTES as u64,
        PREVIEW_DECODED_BYTES,
        PREVIEW_SVG_BYTES as u64,
        PREVIEW_PNG_BYTES as u64,
    ] {
        assert!(check_budget(limit, limit, "test").is_ok());
        assert!(matches!(
            check_budget(limit + 1, limit, "test"),
            Err(AutomationError::PreviewBudget(_))
        ));
    }
    let app = fixture();
    let image = snapshot(&app, PREVIEW_MAX_EDGE).unwrap().render().unwrap();
    assert_eq!(image.value["width_px"], PREVIEW_MAX_EDGE);
}
#[test]
fn svg_and_image_queries_preserve_document_state() {
    let mut app = fixture();
    let mut req = request(&app, 64);
    req.tool_id = "render.preview".into();
    req.parameters = json!({});
    let plan = plan_tool(&app, req).unwrap();
    let svg = execute_tool(&mut app, plan, ExecutionAuthority::Read).unwrap();
    assert!(svg.value[0]["svg"].as_str().unwrap().contains("<svg"));
    let plan = plan_tool(&app, request(&app, 64)).unwrap();
    let image = execute_tool(&mut app, plan, ExecutionAuthority::Read).unwrap();
    assert!(!pixels(&image).is_empty());
    assert_eq!(app.doc.automation_revision, 0);
    assert!(app.doc.automation_runs.is_empty());
    let mut req = request(&app, 64);
    req.tool_id = "resource.rename".into();
    req.parameters = json!({"name":"Rename"});
    let plan = plan_tool(&app, req).unwrap();
    app.doc.automation_revision = 7;
    assert!(execute_tool(&mut app, plan, ExecutionAuthority::ReversibleModify).is_err());
    let audit = app.doc.automation_runs.last().unwrap();
    assert_eq!(audit.start_revision, DocumentRevision(7));
    assert_eq!(audit.end_revision, DocumentRevision(7));
}
#[test]
#[ignore = "writes isolated manual acceptance fixtures when explicitly requested"]
fn write_r2_acceptance_fixture() {
    let app = fixture();
    crate::project::save_project(
        &app,
        std::path::Path::new("/tmp/plotium-r2-acceptance.plotium"),
        false,
    )
    .unwrap();
    let image = snapshot(&app, 1440).unwrap().render().unwrap();
    std::fs::write("/tmp/plotium-r2-native-preview.png", pixels(&image)).unwrap();
}

#[test]
fn header_dimensions_not_saved_hints_enforce_decoded_budget_before_decode() {
    let mut app = fixture();
    let asset = app.doc.assets.values_mut().next().unwrap();
    // A valid PNG header for a 5000x5000 page with a tiny, inconsistent IDAT.
    // The preview must reject the header budget without attempting pixel decode.
    asset.bytes[16..20].copy_from_slice(&5000u32.to_be_bytes());
    asset.bytes[20..24].copy_from_slice(&5000u32.to_be_bytes());
    let mut crc = 0xffff_ffffu32;
    for byte in &asset.bytes[12..29] {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ if crc & 1 != 0 { 0xedb8_8320 } else { 0 };
        }
    }
    asset.bytes[29..33].copy_from_slice(&(!crc).to_be_bytes());
    asset.sha256 = plotium_io::image::sha256(&asset.bytes);
    assert_eq!(asset.pixel_size, [64, 48]);
    assert!(matches!(
        snapshot(&app, 64).unwrap().render(),
        Err(AutomationError::PreviewBudget(_))
    ));
}
#[test]
fn svg_overflow_and_invalid_canvas_size_are_explicit() {
    let mut app = fixture();
    app.doc.canvases[0].size_mm = [f32::NAN, 90.];
    assert!(snapshot(&app, 64).is_err());
    app.doc.canvases[0].size_mm = [180., 90.];
    if let ContentKind::Text(t) = &mut app.doc.canvases[0].objects[3].kind {
        t.text = "A".repeat(PREVIEW_SVG_BYTES);
    }
    assert!(matches!(
        snapshot(&app, 64).unwrap().render(),
        Err(AutomationError::PreviewBudget(_))
    ));
}
