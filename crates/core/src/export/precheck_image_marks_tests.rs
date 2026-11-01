//! Text and strokes inside vector images, measured at the placed size.
use super::*;
use crate::state::{CanvasObject, ObjectFrame, ObjectId, RasterImageContent};
use sha2::{Digest, Sha256};

/// 200 x 100 user units with 16-unit text and a 1-unit stroke: 12 pt and
/// 0.75 pt at natural size.
const LABELLED: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="100"><path d="M0 80H200" fill="none" stroke="#000"/><text x="10" y="50" font-family="sans-serif" font-size="16">N</text></svg>"##;

fn asset() -> AssetRecord {
    let bytes = crate::export::normalize_svg_asset(LABELLED.as_bytes()).unwrap();
    AssetRecord {
        id: AssetId::new(),
        sha256: Sha256::digest(&bytes).into(),
        format: "svg".to_owned(),
        pixel_size: [200, 100],
        bytes,
    }
}

fn page(asset: &AssetRecord, frame: ObjectFrame) -> CanvasDocument {
    let mut canvas = CanvasDocument::new("Page".to_owned(), [89.0, 60.0]);
    canvas.objects.push(CanvasObject {
        id: ObjectId::new(1),
        name: "image".to_owned(),
        frame,
        locked: false,
        visible: true,
        kind: CanvasObjectKind::RasterImage(RasterImageContent::new(asset.id)),
    });
    canvas
}

fn smallest_text(canvas: &CanvasDocument, asset: &AssetRecord) -> Option<f32> {
    page_metrics(canvas, &BTreeMap::from([(asset.id, asset.clone())])).min_font_pt
}

#[test]
fn text_scales_with_the_placed_frame() {
    let asset = asset();
    let natural = page(&asset, ObjectFrame::new(0.0, 0.0, 150.0, 75.0));
    let size = smallest_text(&natural, &asset).unwrap();
    assert!((size - 12.0).abs() < 1e-3, "{size}");

    let halved = page(&asset, ObjectFrame::new(0.0, 0.0, 75.0, 37.5));
    let size = smallest_text(&halved, &asset).unwrap();
    assert!((size - 6.0).abs() < 1e-3, "{size}");
}

#[test]
fn contained_text_follows_the_limiting_axis_and_crop_enlarges_it() {
    let asset = asset();
    let mut canvas = page(&asset, ObjectFrame::new(0.0, 0.0, 75.0, 75.0));
    let size = smallest_text(&canvas, &asset).unwrap();
    assert!((size - 6.0).abs() < 1e-3, "{size}");

    if let CanvasObjectKind::RasterImage(image) = &mut canvas.objects[0].kind {
        image.crop = [0.0, 0.0, 0.5, 1.0];
    }
    let size = smallest_text(&canvas, &asset).unwrap();
    assert!((size - 12.0).abs() < 1e-3, "{size}");
}

#[test]
fn small_image_text_fails_the_font_check_and_export_scale_applies() {
    let asset = asset();
    let canvas = page(&asset, ObjectFrame::new(0.0, 0.0, 75.0, 37.5));
    let metrics = page_metrics(&canvas, &BTreeMap::from([(asset.id, asset.clone())]));
    let fails = precheck_report(&[metrics], None, ExportFormat::Svg, 300);
    assert_eq!(fails.items[0].status, ComplianceStatus::Fail);
    assert!(fails.items[0].detail.starts_with("6.0 pt"));

    let mut nature = canvas.clone();
    nature.size_preset_id = Some("nature-1col".to_owned());
    nature.size_mm = crate::state::NATURE_SINGLE_COLUMN.size_mm();
    let metrics = page_metrics(&nature, &BTreeMap::from([(asset.id, asset.clone())]));
    assert_eq!(metrics.required_font_pt, 5.0);
    let passes = precheck_report(&[metrics], None, ExportFormat::Svg, 300);
    assert_eq!(passes.items[0].status, ComplianceStatus::Pass);
    let shrunk = precheck_report(&[metrics], Some(44.5), ExportFormat::Svg, 300);
    assert_eq!(shrunk.items[0].status, ComplianceStatus::Fail);
}

#[test]
fn thin_image_strokes_fail_the_line_check() {
    let asset = asset();
    let assets = BTreeMap::from([(asset.id, asset.clone())]);
    let natural = page(&asset, ObjectFrame::new(0.0, 0.0, 150.0, 75.0));
    let metrics = page_metrics(&natural, &assets);
    assert!((metrics.min_line_pt.unwrap() - 0.75).abs() < 1e-3);

    let halved = page(&asset, ObjectFrame::new(0.0, 0.0, 75.0, 37.5));
    let metrics = page_metrics(&halved, &assets);
    let report = precheck_report(&[metrics], None, ExportFormat::Svg, 300);
    assert_eq!(report.items[1].status, ComplianceStatus::Fail);
}

#[test]
fn filled_outlines_report_no_marks() {
    let bytes = crate::export::normalize_svg_asset(
        br##"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"><path d="M0 0H8V8Z"/></svg>"##,
    )
    .unwrap();
    let filled = AssetRecord {
        sha256: Sha256::digest(&bytes).into(),
        pixel_size: [8, 8],
        bytes,
        ..asset()
    };
    let canvas = page(&filled, ObjectFrame::new(0.0, 0.0, 6.0, 6.0));
    let metrics = page_metrics(&canvas, &BTreeMap::from([(filled.id, filled.clone())]));
    assert_eq!((metrics.min_font_pt, metrics.min_line_pt), (None, None));
}

#[test]
fn hidden_images_and_raster_assets_report_no_text() {
    let asset = asset();
    let mut hidden = page(&asset, ObjectFrame::new(0.0, 0.0, 75.0, 37.5));
    hidden.objects[0].visible = false;
    assert_eq!(smallest_text(&hidden, &asset), None);

    let mut in_hidden_panel = page(&asset, ObjectFrame::new(0.0, 0.0, 75.0, 37.5));
    let panel =
        in_hidden_panel.create_panel("Panel".to_owned(), ObjectFrame::new(0.0, 0.0, 100.0, 100.0));
    let item = in_hidden_panel.objects[0].id;
    let target = in_hidden_panel.panel_mut(panel).unwrap();
    target.item_order.push(item);
    target.visible = false;
    assert_eq!(smallest_text(&in_hidden_panel, &asset), None);

    let mut raster = asset.clone();
    raster.format = "png".to_owned();
    let canvas = page(&raster, ObjectFrame::new(0.0, 0.0, 75.0, 37.5));
    assert_eq!(smallest_text(&canvas, &raster), None);
}

#[test]
fn vector_images_have_no_effective_ppi() {
    let bytes = br#"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"/>"#.to_vec();
    let id = AssetId::new();
    let asset = AssetRecord {
        id,
        sha256: Sha256::digest(&bytes).into(),
        format: "svg".to_owned(),
        pixel_size: [8, 8],
        bytes,
    };
    let mut canvas = CanvasDocument::new("precheck".to_owned(), [100.0, 100.0]);
    canvas.objects.push(CanvasObject {
        id: ObjectId::new(1),
        name: "image".to_owned(),
        frame: ObjectFrame::new(0.0, 0.0, 144.0, 144.0),
        locked: false,
        visible: true,
        kind: CanvasObjectKind::RasterImage(RasterImageContent::new(id)),
    });

    let items = image_precheck_items(&[&canvas], &BTreeMap::from([(id, asset)]), None);
    assert!(
        items
            .iter()
            .all(|item| item.status == ComplianceStatus::Pass)
    );
    assert!(!items.iter().any(|item| item.label.contains("PPI")));
}

#[test]
fn an_unreadable_record_reports_the_image_as_damaged() {
    let bytes = br##"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"><path d="M0 0H8V8Z"/><metadata data-plotium-min-text-size="tiny"/></svg>"##.to_vec();
    let damaged = AssetRecord {
        sha256: Sha256::digest(&bytes).into(),
        pixel_size: [8, 8],
        bytes,
        ..asset()
    };
    let canvas = page(&damaged, ObjectFrame::new(0.0, 0.0, 6.0, 6.0));
    let assets = BTreeMap::from([(damaged.id, damaged.clone())]);
    assert_eq!(page_metrics(&canvas, &assets).min_font_pt, None);
    let items = image_precheck_items(&[&canvas], &assets, None);
    assert_eq!(items[0].status, ComplianceStatus::Fail);
}

#[test]
fn fully_transparent_images_report_no_marks() {
    let asset = asset();
    let mut canvas = page(&asset, ObjectFrame::new(0.0, 0.0, 75.0, 37.5));
    if let CanvasObjectKind::RasterImage(image) = &mut canvas.objects[0].kind {
        image.opacity = 0.0;
    }
    assert_eq!(smallest_text(&canvas, &asset), None);
}
