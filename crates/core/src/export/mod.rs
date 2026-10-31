mod asset_render;
mod fonts;
pub(crate) use fonts::diagnostic_font_status;
pub use fonts::normalize_svg_asset;
mod pdf;
mod precheck;
mod preset;
mod request;
mod target;
pub(crate) use request::encode_canvas as encode_figure_export;
pub use request::{
    ExportOutput, FigureExportParams, PreparedFigureExport, figure_export_schema,
    prepare_figure_export,
};
pub use target::{ExportAuthorization, ExportTarget};
mod raster;
#[cfg(test)]
mod state_tests;
mod trim;

pub use asset_render::{MissingImagePolicy, prepare_render_document, rasterize_vector_fallbacks};
pub use precheck::{
    ComplianceStatus, PrecheckReport, image_precheck_items, page_metrics, precheck_report,
};
pub use preset::ExportPreset;
pub use raster::{
    DEFAULT_MAX_RASTER_BYTES, DEFAULT_MAX_RASTER_HEIGHT, DEFAULT_MAX_RASTER_PIXELS,
    DEFAULT_MAX_RASTER_WIDTH, RasterError, RasterImage, RasterLimits, RasterOptions,
    rasterize_canvas, rasterize_canvas_with_assets, rasterize_svg,
};

use crate::state::{AssetId, AssetRecord, CanvasDocument};
use image::codecs::jpeg::{JpegEncoder, PixelDensity};
use image::{ExtendedColorType, ImageFormat};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use thiserror::Error;

pub const DEFAULT_BITMAP_DPI: u16 = 300;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportFormat {
    Svg,
    Pdf,
    Png,
    Jpeg,
    Tiff,
}

impl ExportFormat {
    pub fn label(self) -> &'static str {
        match self {
            Self::Svg => "SVG",
            Self::Pdf => "PDF",
            Self::Png => "PNG",
            Self::Jpeg => "JPEG",
            Self::Tiff => "TIFF",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::Svg => "svg",
            Self::Pdf => "pdf",
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::Tiff => "tif",
        }
    }

    pub fn default_file_name(self) -> &'static str {
        match self {
            Self::Svg => "spectrum.svg",
            Self::Pdf => "spectrum.pdf",
            Self::Png => "spectrum.png",
            Self::Jpeg => "spectrum.jpg",
            Self::Tiff => "spectrum.tif",
        }
    }

    pub fn dialog_title(self) -> String {
        format!("Export figure as {}", self.label())
    }

    pub fn is_bitmap(self) -> bool {
        matches!(self, Self::Png | Self::Jpeg | Self::Tiff)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportPageScope {
    Current,
    All,
    Range { start: usize, end: usize },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportScopeKind {
    Current,
    All,
    Range,
}

#[derive(Clone, Debug)]
pub struct ExportDialogState {
    pub format: ExportFormat,
    pub scope: ExportPageScope,
    pub dpi: u16,
    pub preset: Option<ExportPreset>,
    pub trim_to_visible_content: bool,
    pub allow_missing_images: bool,
}

impl ExportDialogState {
    pub fn new(format: ExportFormat) -> Self {
        Self {
            format,
            scope: ExportPageScope::Current,
            dpi: DEFAULT_BITMAP_DPI,
            preset: None,
            trim_to_visible_content: false,
            allow_missing_images: false,
        }
    }

    pub fn from_defaults(format: ExportFormat, defaults: &crate::settings::ExportDefaults) -> Self {
        let mut state = Self::new(format);
        state.dpi = defaults.dpi;
        state.trim_to_visible_content = defaults.trim_to_visible_content;
        state
    }

    pub fn apply_preset(&mut self, preset: Option<ExportPreset>) {
        self.preset = preset;
        if let Some(preset) = preset {
            self.format = preset.format();
            if preset.format().is_bitmap() {
                self.dpi = preset.dpi();
            }
        }
    }

    pub fn target_width_mm(&self) -> Option<f32> {
        self.preset.and_then(ExportPreset::target_width_mm)
    }

    pub fn scope_kind(&self) -> ExportScopeKind {
        match self.scope {
            ExportPageScope::Current => ExportScopeKind::Current,
            ExportPageScope::All => ExportScopeKind::All,
            ExportPageScope::Range { .. } => ExportScopeKind::Range,
        }
    }

    pub fn set_scope_kind(&mut self, kind: ExportScopeKind, active_page: usize, page_count: usize) {
        self.scope = match kind {
            ExportScopeKind::Current => ExportPageScope::Current,
            ExportScopeKind::All => ExportPageScope::All,
            ExportScopeKind::Range => {
                let max_page = page_count.max(1);
                let page = active_page.saturating_add(1).clamp(1, max_page);
                match self.scope {
                    ExportPageScope::Range { start, end } => ExportPageScope::Range {
                        start: start.clamp(1, max_page),
                        end: end.clamp(1, max_page),
                    },
                    _ => ExportPageScope::Range {
                        start: page,
                        end: page,
                    },
                }
            }
        };
    }
}

#[derive(Clone, Debug)]
pub struct ExportSettings {
    pub format: ExportFormat,
    pub scope: ExportPageScope,
    pub dpi: u16,
    pub target_width_mm: Option<f32>,
    pub trim_to_visible_content: bool,
    pub allow_missing_images: bool,
}

impl From<&ExportDialogState> for ExportSettings {
    fn from(value: &ExportDialogState) -> Self {
        Self {
            format: value.format,
            scope: value.scope,
            dpi: value.dpi,
            target_width_mm: value.target_width_mm(),
            trim_to_visible_content: value.trim_to_visible_content,
            allow_missing_images: value.allow_missing_images,
        }
    }
}

#[derive(Debug, Error)]
pub enum ExportError {
    #[error("no pages are available to export")]
    EmptyDocument,
    #[error("no usable system font is available; install a sans-serif font before exporting")]
    MissingFont,
    #[error("current page is no longer available")]
    MissingCurrentPage,
    #[error("image asset {asset} is missing; replace it or allow placeholder export")]
    MissingImageAsset { asset: crate::state::AssetId },
    #[error("image asset {asset} is damaged: {reason}")]
    CorruptImageAsset {
        asset: crate::state::AssetId,
        reason: String,
    },
    #[error("page range must be between 1 and {page_count}")]
    InvalidRange { page_count: usize },
    #[error("SVG parse failed: {0}")]
    SvgParse(String),
    #[error("PDF conversion failed: {0}")]
    Pdf(String),
    #[error("image encoding failed: {0}")]
    Image(#[from] image::ImageError),
    #[error("I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Raster(#[from] RasterError),
}

pub fn resolve_page_scope(
    scope: ExportPageScope,
    active_page: Option<usize>,
    page_count: usize,
) -> Result<Vec<usize>, ExportError> {
    if page_count == 0 {
        return Err(ExportError::EmptyDocument);
    }

    match scope {
        ExportPageScope::Current => active_page
            .filter(|&page| page < page_count)
            .map(|page| vec![page])
            .ok_or(ExportError::MissingCurrentPage),
        ExportPageScope::All => Ok((0..page_count).collect()),
        ExportPageScope::Range { start, end } => {
            if start == 0 || end == 0 || start > end || end > page_count {
                return Err(ExportError::InvalidRange { page_count });
            }
            Ok((start - 1..end).collect())
        }
    }
}

pub fn export_canvases(
    canvases: &[CanvasDocument],
    active_page: Option<usize>,
    settings: &ExportSettings,
    base_path: &Path,
) -> Result<Vec<PathBuf>, ExportError> {
    export_canvases_with_assets(canvases, &BTreeMap::new(), active_page, settings, base_path)
}

pub fn export_canvases_with_assets(
    canvases: &[CanvasDocument],
    assets: &BTreeMap<AssetId, AssetRecord>,
    active_page: Option<usize>,
    settings: &ExportSettings,
    base_path: &Path,
) -> Result<Vec<PathBuf>, ExportError> {
    let pages = resolve_page_scope(settings.scope, active_page, canvases.len())?;
    let target = settings.target_width_mm;
    let missing_policy = if settings.allow_missing_images {
        MissingImagePolicy::Placeholder
    } else {
        MissingImagePolicy::Block
    };
    match settings.format {
        ExportFormat::Svg => export_svg(
            canvases,
            assets,
            &pages,
            target,
            settings.trim_to_visible_content,
            missing_policy,
            base_path,
        ),
        ExportFormat::Pdf => export_pdf(
            canvases,
            assets,
            &pages,
            target,
            settings.trim_to_visible_content,
            missing_policy,
            base_path,
        ),
        ExportFormat::Png | ExportFormat::Jpeg | ExportFormat::Tiff => export_bitmap(
            canvases,
            assets,
            &pages,
            settings,
            missing_policy,
            base_path,
        ),
    }
}

fn document_svg_with_assets(
    canvas: &CanvasDocument,
    assets: &BTreeMap<AssetId, AssetRecord>,
    target_width_mm: Option<f32>,
    missing_policy: MissingImagePolicy,
) -> Result<String, ExportError> {
    let document = prepare_render_document(canvas, assets, missing_policy)?;
    let svg = plotium_render::svg::export_document(&document);
    let Some(target) = target_width_mm else {
        return Ok(svg);
    };
    let [w, h] = canvas.size_pt();
    let scale = target / canvas.size_mm[0].max(f32::MIN_POSITIVE);
    let from = format!(r#"width="{w}pt" height="{h}pt""#);
    let to = format!(r#"width="{}pt" height="{}pt""#, w * scale, h * scale);
    Ok(svg.replacen(&from, &to, 1))
}

pub fn export_output_paths(
    base_path: &Path,
    format: ExportFormat,
    output_count: usize,
) -> Vec<PathBuf> {
    if output_count <= 1 || matches!(format, ExportFormat::Pdf) {
        return vec![with_extension(base_path, format.extension())];
    }

    (1..=output_count)
        .map(|ordinal| numbered_output_path(base_path, ordinal, format.extension()))
        .collect()
}

fn export_svg(
    canvases: &[CanvasDocument],
    assets: &BTreeMap<AssetId, AssetRecord>,
    pages: &[usize],
    target_width_mm: Option<f32>,
    trim_to_visible_content: bool,
    missing_policy: MissingImagePolicy,
    base_path: &Path,
) -> Result<Vec<PathBuf>, ExportError> {
    let paths = export_output_paths(base_path, ExportFormat::Svg, pages.len());
    for (&page, path) in pages.iter().zip(&paths) {
        let svg = if trim_to_visible_content {
            trim::trim_document_svg_with_assets(
                &canvases[page],
                assets,
                target_width_mm,
                missing_policy,
            )?
        } else {
            document_svg_with_assets(&canvases[page], assets, target_width_mm, missing_policy)?
        };
        std::fs::write(path, svg)?;
    }
    Ok(paths)
}

fn export_pdf(
    canvases: &[CanvasDocument],
    assets: &BTreeMap<AssetId, AssetRecord>,
    pages: &[usize],
    target_width_mm: Option<f32>,
    trim_to_visible_content: bool,
    missing_policy: MissingImagePolicy,
    base_path: &Path,
) -> Result<Vec<PathBuf>, ExportError> {
    let path = with_extension(base_path, ExportFormat::Pdf.extension());
    let svgs: Vec<String> = pages
        .iter()
        .map(|&page| {
            if trim_to_visible_content {
                trim::trim_document_svg_with_assets(
                    &canvases[page],
                    assets,
                    target_width_mm,
                    missing_policy,
                )
            } else {
                document_svg_with_assets(&canvases[page], assets, target_width_mm, missing_policy)
            }
        })
        .collect::<Result<_, ExportError>>()?;
    let pdf = pdf::render(&svgs)?;
    std::fs::write(&path, pdf)?;
    Ok(vec![path])
}

fn export_bitmap(
    canvases: &[CanvasDocument],
    assets: &BTreeMap<AssetId, AssetRecord>,
    pages: &[usize],
    settings: &ExportSettings,
    missing_policy: MissingImagePolicy,
    base_path: &Path,
) -> Result<Vec<PathBuf>, ExportError> {
    let paths = export_output_paths(base_path, settings.format, pages.len());
    for (&page, path) in pages.iter().zip(&paths) {
        let raster = rasterize_canvas_with_assets(
            &canvases[page],
            assets,
            RasterOptions {
                dpi: settings.dpi,
                target_width_mm: settings.target_width_mm,
                limits: RasterLimits::default(),
            },
            missing_policy,
        )?;
        let raster = if settings.trim_to_visible_content {
            let background = canvases[page].background;
            trim::crop_raster(
                raster,
                [background.r, background.g, background.b, 255],
                trim::raster_trim_padding(settings.dpi),
            )?
        } else {
            raster
        };
        match settings.format {
            ExportFormat::Png => {
                image::save_buffer_with_format(
                    path,
                    raster.rgba(),
                    raster.width(),
                    raster.height(),
                    image::ColorType::Rgba8,
                    ImageFormat::Png,
                )?;
            }
            ExportFormat::Tiff => {
                image::save_buffer_with_format(
                    path,
                    raster.rgba(),
                    raster.width(),
                    raster.height(),
                    image::ColorType::Rgba8,
                    ImageFormat::Tiff,
                )?;
            }
            ExportFormat::Jpeg => {
                let rgb = raster.to_rgb_over([255, 255, 255]);
                let file = std::fs::File::create(path)?;
                let mut encoder = JpegEncoder::new_with_quality(file, 90);
                encoder.set_pixel_density(PixelDensity::dpi(settings.dpi));
                encoder.encode(
                    &rgb,
                    raster.width(),
                    raster.height(),
                    ExtendedColorType::Rgb8,
                )?;
            }
            _ => unreachable!("bitmap export called for non-bitmap format"),
        }
    }
    Ok(paths)
}

fn with_extension(path: &Path, extension: &str) -> PathBuf {
    let mut path = path.to_path_buf();
    path.set_extension(extension);
    path
}

fn numbered_output_path(base_path: &Path, ordinal: usize, extension: &str) -> PathBuf {
    let parent = base_path.parent();
    let stem = base_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or("export");
    let file_name = format!("{stem}-{ordinal:03}.{extension}");
    if let Some(parent) = parent {
        parent.join(file_name)
    } else {
        PathBuf::from(file_name)
    }
}

#[cfg(test)]
#[path = "export_tests.rs"]
mod tests;
