//! Normalization and rendering of embedded SVG assets.
//!
//! An asset stores usvg's normal form: text is outlines and nothing refers
//! outside the file, so it renders the same on every machine. It ends with a
//! record of the finest marks the source drew.

use super::{ImageError, ImageProbe, ProxyImage, RasterFormat, ResourceClass, SOFT_PIXEL_LIMIT};
use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
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
/// Attributes on the empty `metadata` child that ends a normalized SVG. They
/// are part of the stored asset, in the user units of its `width` and `height`.
/// Outlining erases font sizes, so they cannot be measured again later.
const MIN_TEXT_ATTRIBUTE: &str = "data-plotium-min-text-size";
const MIN_STROKE_ATTRIBUTE: &str = "data-plotium-min-stroke-width";

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
    let mut normalized = tree.to_string(&options);
    // A normalized source already lost its text elements but keeps its record.
    // Compressed sources are not XML here, and a malformed record is not worth
    // rejecting a drawing usvg accepted; either way it has nothing to keep.
    let recorded = svg_marks(bytes).unwrap_or_default();
    let marks = measure_marks(tree.root()).finest(recorded);
    if let Some(close) = normalized.rfind("</svg>") {
        normalized.insert_str(close, &marks_element(marks));
    }
    Ok(normalized.into_bytes())
}

/// The finest marks a normalized SVG draws, in the user units of its `width`
/// and `height`, with transforms applied. Text a source file already stored as
/// paths has no font size, and filled shapes have no stroke, so neither counts.
/// Patterns, masks, and embedded SVG images are not measured.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SvgMarks {
    pub min_text_size: Option<f32>,
    pub min_stroke_width: Option<f32>,
}

impl SvgMarks {
    fn finest(self, other: Self) -> Self {
        let finer = |a: Option<f32>, b: Option<f32>| a.into_iter().chain(b).reduce(f32::min);
        Self {
            min_text_size: finer(self.min_text_size, other.min_text_size),
            min_stroke_width: finer(self.min_stroke_width, other.min_stroke_width),
        }
    }
}

pub fn svg_marks(bytes: &[u8]) -> Result<SvgMarks, ImageError> {
    let mut reader = Reader::from_reader(bytes);
    let mut buffer = Vec::new();
    let mut depth = 0usize;
    let mut marks = SvgMarks::default();
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(_)) => depth += 1,
            Ok(Event::End(_)) => depth = depth.saturating_sub(1),
            Ok(Event::Empty(element)) if depth == 1 && element.name().as_ref() == b"metadata" => {
                marks = SvgMarks {
                    min_text_size: recorded_size(&element, MIN_TEXT_ATTRIBUTE)?,
                    min_stroke_width: recorded_size(&element, MIN_STROKE_ATTRIBUTE)?,
                };
            }
            Ok(Event::Eof) => return Ok(marks),
            Err(error) => return Err(ImageError::Decode(error.to_string())),
            Ok(_) => {}
        }
        buffer.clear();
    }
}

fn recorded_size(element: &BytesStart<'_>, name: &str) -> Result<Option<f32>, ImageError> {
    let malformed = || ImageError::Decode(format!("malformed {name} record"));
    let attribute = element.try_get_attribute(name).map_err(|_| malformed())?;
    attribute
        .map(|attribute| {
            std::str::from_utf8(&attribute.value)
                .ok()
                .and_then(|value| value.parse().ok())
                .filter(|size: &f32| size.is_finite() && *size > 0.0)
                .ok_or_else(malformed)
        })
        .transpose()
}

fn marks_element(marks: SvgMarks) -> String {
    let attribute = |name: &str, value: Option<f32>| {
        value.map_or(String::new(), |value| format!(" {name}=\"{value}\""))
    };
    let text = attribute(MIN_TEXT_ATTRIBUTE, marks.min_text_size);
    let stroke = attribute(MIN_STROKE_ATTRIBUTE, marks.min_stroke_width);
    if text.is_empty() && stroke.is_empty() {
        return String::new();
    }
    format!("<metadata{text}{stroke}/>\n")
}

fn measure_marks(group: &usvg::Group) -> SvgMarks {
    group
        .children()
        .iter()
        .fold(SvgMarks::default(), |marks, node| {
            marks.finest(match node {
                usvg::Node::Group(child) => measure_marks(child),
                usvg::Node::Text(text) => SvgMarks {
                    min_text_size: text_size(text),
                    ..Default::default()
                },
                usvg::Node::Path(path) => SvgMarks {
                    min_stroke_width: stroke_width(path),
                    ..Default::default()
                },
                _ => SvgMarks::default(),
            })
        })
}

fn smallest_scale(transform: tiny_skia::Transform) -> f32 {
    let (x, y) = transform.get_scale();
    x.min(y)
}

/// Extreme transforms overflow or vanish and leave nothing worth recording.
fn recordable(size: f32) -> Option<f32> {
    (size.is_finite() && size > 0.0).then_some(size)
}

fn stroke_width(path: &usvg::Path) -> Option<f32> {
    let stroke = path
        .stroke()
        .filter(|stroke| path.is_visible() && stroke.opacity().get() > 0.0)?;
    recordable(stroke.width().get() * smallest_scale(path.abs_transform()))
}

fn text_size(text: &usvg::Text) -> Option<f32> {
    let scale = smallest_scale(text.abs_transform());
    text.chunks()
        .iter()
        .flat_map(|chunk| {
            chunk.spans().iter().filter(|span| {
                span.is_visible()
                    && chunk
                        .text()
                        .get(span.start()..span.end())
                        .is_some_and(|run| !run.trim().is_empty())
            })
        })
        .filter_map(|span| recordable(span.font_size().get() * scale))
        .reduce(f32::min)
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
