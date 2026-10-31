use super::*;
use crate::image::{decode_proxy_rgba8, sniff};

const LABELLED: &[u8] = br##"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE svg PUBLIC "-//W3C//DTD SVG 1.1//EN" "http://www.w3.org/Graphics/SVG/1.1/DTD/svg11.dtd">
<svg xmlns="http://www.w3.org/2000/svg" width="120pt" height="60pt" viewBox="0 0 120 60">
  <defs><clipPath id="c"><rect width="120" height="60"/></clipPath></defs>
  <g clip-path="url(#c)">
    <path d="M10 30 C30 5 60 55 110 30" fill="none" stroke="#000" stroke-dasharray="4 2"/>
    <text x="20" y="20" font-family="sans-serif" font-size="10">N</text>
  </g>
</svg>"##;

fn fonts() -> Arc<SvgFontDatabase> {
    let mut database = SvgFontDatabase::new();
    database.load_system_fonts();
    // Arial is absent on Linux runners; any installed family exercises outlining.
    let family = database
        .faces()
        .next()
        .map(|face| face.families[0].0.clone());
    if let Some(family) = family {
        database.set_sans_serif_family(family);
    }
    Arc::new(database)
}

const NO_TEXT: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"><path d="M0 0H8V8Z"/></svg>"##;

#[test]
fn normalization_records_the_smallest_text_size_in_user_units() {
    // 120 pt is 160 user units, so the viewBox draws 10-unit text at 13.33.
    let labelled = normalize_svg(LABELLED, fonts()).unwrap();
    assert!(!String::from_utf8_lossy(&labelled).contains("<text"));
    let size = svg_marks(&labelled).unwrap().min_text_size.unwrap();
    assert!((size - 40.0 / 3.0).abs() < 1e-3, "{size}");

    let plain = normalize_svg(NO_TEXT, fonts()).unwrap();
    assert_eq!(svg_marks(&plain).unwrap(), SvgMarks::default());
}

#[test]
fn smallest_text_follows_transforms_and_skips_blank_runs() {
    let source = br##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
      <text x="5" y="20" font-family="sans-serif" font-size="12">N</text>
      <g transform="scale(0.5)"><text x="5" y="80" font-family="sans-serif" font-size="10">O</text></g>
      <text x="5" y="90" font-family="sans-serif" font-size="2" xml:space="preserve">   </text>
    </svg>"##;
    let normalized = normalize_svg(source, fonts()).unwrap();
    assert_eq!(svg_marks(&normalized).unwrap().min_text_size, Some(5.0));
}

#[test]
fn text_size_reads_the_font_shorthand_through_transforms() {
    let source = br##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
      <g transform="scale(0.1)"><text x="50" y="500" style="font: 240px sans-serif">N</text></g>
    </svg>"##;
    let normalized = normalize_svg(source, fonts()).unwrap();
    let size = svg_marks(&normalized).unwrap().min_text_size.unwrap();
    assert!((size - 24.0).abs() < 1e-3, "{size}");
}

#[test]
fn only_real_strokes_count_and_transforms_scale_them() {
    let source = br##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
      <path d="M0 0H90V90Z" fill="#000" stroke="transparent" stroke-width="0.1"/>
      <path d="M0 5H90" fill="none" stroke="#000" stroke-width="3"/>
      <g transform="scale(0.25)"><path d="M0 9H90" fill="none" stroke="#000" stroke-width="2"/></g>
    </svg>"##;
    let normalized = normalize_svg(source, fonts()).unwrap();
    assert_eq!(svg_marks(&normalized).unwrap().min_stroke_width, Some(0.5));

    let filled = normalize_svg(NO_TEXT, fonts()).unwrap();
    assert_eq!(svg_marks(&filled).unwrap().min_stroke_width, None);
}

#[test]
fn a_record_counts_only_as_an_empty_metadata_child_of_the_root() {
    let source = br##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
      <!-- <metadata data-plotium-min-text-size="0.01"/> -->
      <desc>data-plotium-min-text-size="0.01"</desc>
      <g><metadata data-plotium-min-stroke-width="0.01"/></g>
      <text x="5" y="20" font-family="sans-serif" font-size="12">N</text>
    </svg>"##;
    let normalized = normalize_svg(source, fonts()).unwrap();
    let marks = svg_marks(&normalized).unwrap();
    assert_eq!(marks.min_text_size, Some(12.0));
    assert_eq!(marks.min_stroke_width, None);
}

#[test]
fn extreme_transforms_never_write_an_unreadable_record() {
    let source = br##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
      <g transform="scale(1e20)"><path d="M0 0H8" fill="none" stroke="#000"/></g>
      <g transform="scale(1e-6)"><g transform="scale(1e-6)"><g transform="scale(1e-6)"><g transform="scale(1e-6)">
        <path d="M0 0H8" fill="none" stroke="#000"/>
      </g></g></g></g>
    </svg>"##;
    let normalized = normalize_svg(source, fonts()).unwrap();
    assert!(svg_marks(&normalized).is_ok());
}

#[test]
fn a_malformed_record_is_an_error() {
    let bytes = br#"<svg xmlns="http://www.w3.org/2000/svg"><metadata data-plotium-min-text-size="tiny"/></svg>"#;
    assert!(svg_marks(bytes).is_err());
}

#[test]
fn recorded_text_size_leaves_size_and_fragments_unchanged() {
    let normalized = normalize_svg(LABELLED, fonts()).unwrap();
    let fragment = svg_fragment(&normalized, "a-").unwrap();
    assert!(!fragment.markup.contains("metadata"));
    assert_eq!(fragment.size, [160.0, 80.0]);
}

#[test]
fn sniff_accepts_prologs_that_precede_the_root() {
    assert_eq!(sniff(LABELLED), RasterFormat::Svg);
    assert_eq!(sniff(b"\xef\xbb\xbf<svg/>"), RasterFormat::Svg);
    assert_eq!(
        sniff(b"<!-- Generator: x --><svg xmlns='http://www.w3.org/2000/svg'/>"),
        RasterFormat::Svg
    );
    assert_eq!(sniff(b"<html><svg/></html>"), RasterFormat::Unknown);
}

#[test]
fn probe_reports_physical_size_in_css_pixels() {
    let probe = crate::image::probe(LABELLED).unwrap();
    assert_eq!(probe.format, RasterFormat::Svg);
    assert_eq!([probe.width, probe.height], [160, 80]);
    assert_eq!(probe.class, ResourceClass::Normal);
}

#[test]
fn normalized_text_is_outlines_and_is_stable() {
    if fonts().is_empty() {
        return;
    }
    let normalized = normalize_svg(LABELLED, fonts()).unwrap();
    let text = std::str::from_utf8(&normalized).unwrap();
    assert!(!text.contains("<text"), "{text}");
    assert_eq!(sniff(&normalized), RasterFormat::Svg);
    assert_eq!(normalize_svg(&normalized, fonts()).unwrap(), normalized);
}

#[test]
fn text_that_cannot_be_outlined_is_refused() {
    let error = normalize_svg(LABELLED, Arc::default()).unwrap_err();
    assert!(error.to_string().contains("install its font"), "{error}");
}

#[test]
fn external_references_are_dropped() {
    let source = br#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="10" height="10"><image width="10" height="10" xlink:href="/etc/passwd"/><rect width="5" height="5"/></svg>"#;
    let normalized = normalize_svg(source, Arc::default()).unwrap();
    let text = std::str::from_utf8(&normalized).unwrap();
    assert!(!text.contains("passwd"), "{text}");
    assert!(!text.contains("<image"), "{text}");
}

#[test]
fn rasterizing_enlarges_to_the_requested_edge() {
    let proxy = decode_proxy_rgba8(
        br##"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="5"><rect width="10" height="5" fill="#f00"/></svg>"##,
        400,
    )
    .unwrap();
    assert_eq!(proxy.pixel_size, [400, 200]);
    assert_eq!(&proxy.rgba8[..4], &[255, 0, 0, 255]);
}

#[test]
fn fragment_drops_the_root_and_prefixes_ids() {
    let source = br##"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="5"><defs><clipPath id="c"><rect width="4" height="4"/></clipPath></defs><rect width="10" height="5" clip-path="url(#c)"/></svg>"##;
    let fragment = svg_fragment(source, "svg7-").unwrap();
    assert_eq!(fragment.size, [10.0, 5.0]);
    assert!(!fragment.markup.contains("<svg"), "{}", fragment.markup);
    assert!(!fragment.markup.contains("</svg"), "{}", fragment.markup);
    assert!(fragment.markup.contains("svg7-"), "{}", fragment.markup);
}

#[test]
fn text_that_draws_nothing_needs_no_font() {
    for body in [
        "<text x='1' y='5'></text><rect width='5' height='5'/>",
        "<defs><text id='t'>N</text></defs><rect width='5' height='5'/>",
        "<text display='none'>N</text><rect width='5' height='5'/>",
        "<!-- <text>N</text> --><rect width='5' height='5'/>",
    ] {
        let source =
            format!("<svg xmlns='http://www.w3.org/2000/svg' width='10' height='10'>{body}</svg>");
        normalize_svg(source.as_bytes(), Arc::default())
            .unwrap_or_else(|error| panic!("{body}: {error}"));
    }
}

#[test]
fn svg_without_content_is_refused_but_a_stored_one_stays_harmless() {
    let source = br#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"/>"#;
    let error = normalize_svg(source, Arc::default()).unwrap_err();
    assert!(error.to_string().contains("nothing to draw"), "{error}");
    assert_eq!(svg_fragment(source, "p-").unwrap().markup, "");
}

#[test]
fn oversized_filter_regions_are_refused() {
    let source = br##"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><filter id="f" filterUnits="userSpaceOnUse" x="0" y="0" width="20000" height="20000"><feGaussianBlur stdDeviation="9"/></filter><g><rect width="5" height="5" filter="url(#f)"/></g></svg>"##;
    let error = crate::image::probe(source).unwrap_err();
    assert!(error.to_string().contains("filter"), "{error}");
}

#[test]
fn embedded_pixels_share_the_soft_limit() {
    let mut png = Cursor::new(Vec::new());
    image::RgbaImage::new(2, 2)
        .write_to(&mut png, image::ImageFormat::Png)
        .unwrap();
    let kind = usvg::ImageKind::PNG(Arc::new(png.into_inner()));
    let mut cost = RenderingCost {
        embedded_pixels: SOFT_PIXEL_LIMIT,
        ..Default::default()
    };
    assert!(check_embedded_image(&kind, &mut cost).is_err());
    let mut cost = RenderingCost::default();
    assert!(check_embedded_image(&kind, &mut cost).is_ok());
    assert_eq!(cost.embedded_pixels, 4);
}

#[test]
fn filter_regions_add_up() {
    let region = "x='0' y='0' width='3000' height='3000'";
    let filtered = |count: usize| {
        let shapes = "<rect width='5' height='5' filter='url(#f)'/>".repeat(count);
        format!(
            "<svg xmlns='http://www.w3.org/2000/svg' width='10' height='10'><filter id='f' filterUnits='userSpaceOnUse' {region}><feGaussianBlur stdDeviation='9'/></filter>{shapes}</svg>"
        )
    };
    assert!(crate::image::probe(filtered(1).as_bytes()).is_ok());
    let error = crate::image::probe(filtered(2).as_bytes()).unwrap_err();
    assert!(error.to_string().contains("filters"), "{error}");
}

#[test]
fn large_nominal_size_keeps_a_bounded_footprint() {
    let source = br#"<svg xmlns="http://www.w3.org/2000/svg" width="10000" height="8000"><rect width="5" height="5"/></svg>"#;
    let probe = crate::image::probe(source).unwrap();
    assert!(probe.decoded_bytes <= 2048 * 2048 * 4);
}
