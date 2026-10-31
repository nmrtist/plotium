//! Normalization and rendering of embedded SVG assets.
//!
//! An asset stores usvg's normal form: text is outlines and nothing refers
//! outside the file, so it renders the same on every machine.

use super::{ImageError, ImageProbe, ProxyImage, RasterFormat, ResourceClass, SOFT_PIXEL_LIMIT};
use resvg::{tiny_skia, usvg};
use std::io::Cursor;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub use usvg::fontdb::Database as SvgFontDatabase;

const MAX_SVG_BYTES: usize = 64 * 1024 * 1024;
/// Largest total of all filter regions, in square user units.
const MAX_FILTER_AREA: f64 = 16_000_000.0;
/// Upper bound on the long edge probes report as the decode footprint: pixel
/// sizes are nominal for vector content, and rendering is sized by its caller.
const PROBE_EDGE: f32 = 2048.0;

/// Body of a normalized SVG, drawn from the origin in a `size`-sized user space.
#[derive(Clone, Debug, PartialEq)]
pub struct SvgFragment {
    pub markup: String,
    pub size: [f32; 2],
}

pub fn normalize_svg(bytes: &[u8], fonts: Arc<SvgFontDatabase>) -> Result<Vec<u8>, ImageError> {
    let unresolved = AtomicBool::new(false);
    let select_font = usvg::FontResolver::default_font_selector();
    let flag = &unresolved;
    let resolver = usvg::FontResolver {
        select_font: Box::new(move |font, database| {
            let face = select_font(font, database);
            if face.is_none() {
                flag.store(true, Ordering::Relaxed);
            }
            face
        }),
        ..Default::default()
    };
    let tree = parse_with(bytes, fonts, resolver)?;
    if unresolved.load(Ordering::Relaxed) {
        return Err(ImageError::Decode(
            "text could not be converted to outlines; install its font and retry".to_owned(),
        ));
    }
    if tree.root().children().is_empty() {
        return Err(ImageError::Decode("the SVG has nothing to draw".to_owned()));
    }
    let options = usvg::WriteOptions {
        preserve_text: false,
        ..Default::default()
    };
    Ok(tree.to_string(&options).into_bytes())
}

pub fn rasterize_svg_rgba8(bytes: &[u8], max_edge: u32) -> Result<ProxyImage, ImageError> {
    let tree = parse(bytes, Arc::default())?;
    let size = tree.size();
    let scale = max_edge.max(1) as f32 / size.width().max(size.height());
    let width = ((size.width() * scale).round() as u32).max(1);
    let height = ((size.height() * scale).round() as u32).max(1);
    let mut pixmap = tiny_skia::Pixmap::new(width, height)
        .ok_or_else(|| ImageError::Decode(format!("cannot allocate {width} x {height} pixels")))?;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    let rgba8 = pixmap
        .pixels()
        .iter()
        .flat_map(|pixel| {
            let color = pixel.demultiply();
            [color.red(), color.green(), color.blue(), color.alpha()]
        })
        .collect();
    Ok(ProxyImage {
        pixel_size: [width, height],
        rgba8,
    })
}

/// Prefixes every id so several fragments can share one document.
pub fn svg_fragment(bytes: &[u8], id_prefix: &str) -> Result<SvgFragment, ImageError> {
    let tree = parse(bytes, Arc::default())?;
    let options = usvg::WriteOptions {
        id_prefix: Some(id_prefix.to_owned()),
        preserve_text: false,
        ..Default::default()
    };
    let written = tree.to_string(&options);
    // A childless root is self-closing and draws nothing.
    let markup = written
        .find('>')
        .zip(written.rfind("</svg>"))
        .and_then(|(root_end, close)| written.get(root_end + 1..close))
        .unwrap_or_default();
    Ok(SvgFragment {
        markup: markup.trim().to_owned(),
        size: [tree.size().width(), tree.size().height()],
    })
}

pub(super) fn probe(bytes: &[u8]) -> Result<ImageProbe, ImageError> {
    let size = parse(bytes, Arc::default())?.size();
    let footprint = (PROBE_EDGE / size.width().max(size.height())).min(1.0);
    let decoded_bytes =
        (size.width() * footprint).ceil() as u64 * (size.height() * footprint).ceil() as u64 * 4;
    Ok(ImageProbe {
        format: RasterFormat::Svg,
        width: size.width().ceil() as u32,
        height: size.height().ceil() as u32,
        decoded_bytes,
        class: ResourceClass::Normal,
        animated: false,
        has_icc: false,
        has_exif: false,
        high_precision: false,
        pages: 1,
    })
}

fn parse(bytes: &[u8], fonts: Arc<SvgFontDatabase>) -> Result<usvg::Tree, ImageError> {
    parse_with(bytes, fonts, usvg::FontResolver::default())
}

fn parse_with(
    bytes: &[u8],
    fonts: Arc<SvgFontDatabase>,
    font_resolver: usvg::FontResolver<'_>,
) -> Result<usvg::Tree, ImageError> {
    if bytes.len() > MAX_SVG_BYTES {
        return Err(ImageError::TooLarge {
            width: 0,
            height: 0,
            decoded_bytes: bytes.len() as u64,
        });
    }
    let options = usvg::Options {
        fontdb: fonts,
        font_resolver,
        // Embedded data stays; a path or URL must never read another file.
        image_href_resolver: usvg::ImageHrefResolver {
            resolve_string: Box::new(|_, _| None),
            ..Default::default()
        },
        ..Default::default()
    };
    let tree = usvg::Tree::from_data(bytes, &options)
        .map_err(|error| ImageError::Probe(error.to_string()))?;
    check_rendering_cost(tree.root(), &mut RenderingCost::default())?;
    Ok(tree)
}

#[derive(Default)]
struct RenderingCost {
    filter_area: f64,
    embedded_pixels: u64,
}

/// Bounds the work a file can demand of the renderer, which sizes filter
/// surfaces and decodes embedded rasters without limits of its own.
fn check_rendering_cost(group: &usvg::Group, cost: &mut RenderingCost) -> Result<(), ImageError> {
    for filter in group.filters() {
        let region = filter.rect();
        cost.filter_area += f64::from(region.width()) * f64::from(region.height());
        if cost.filter_area > MAX_FILTER_AREA {
            return Err(ImageError::Decode(
                "SVG filters cover too large an area".to_owned(),
            ));
        }
    }
    let mut result = Ok(());
    for node in group.children() {
        if result.is_ok() {
            node.subroots(|root| {
                if result.is_ok() {
                    result = check_rendering_cost(root, cost);
                }
            });
        }
        result?;
        result = match node {
            usvg::Node::Group(child) => check_rendering_cost(child, cost),
            usvg::Node::Image(image) => check_embedded_image(image.kind(), cost),
            _ => Ok(()),
        };
    }
    result
}

fn check_embedded_image(
    kind: &usvg::ImageKind,
    cost: &mut RenderingCost,
) -> Result<(), ImageError> {
    let data = match kind {
        usvg::ImageKind::SVG(tree) => return check_rendering_cost(tree.root(), cost),
        usvg::ImageKind::JPEG(data)
        | usvg::ImageKind::PNG(data)
        | usvg::ImageKind::GIF(data)
        | usvg::ImageKind::WEBP(data) => data,
    };
    let (width, height) = image::ImageReader::new(Cursor::new(data.as_slice()))
        .with_guessed_format()
        .map_err(|error| ImageError::Probe(error.to_string()))?
        .into_dimensions()
        .map_err(|error| ImageError::Probe(error.to_string()))?;
    cost.embedded_pixels += u64::from(width) * u64::from(height);
    if cost.embedded_pixels > SOFT_PIXEL_LIMIT {
        return Err(ImageError::TooLarge {
            width,
            height,
            decoded_bytes: cost.embedded_pixels * 4,
        });
    }
    Ok(())
}

#[cfg(test)]
#[path = "svg_tests.rs"]
mod tests;
