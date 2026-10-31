//! Bounded native figure-export planning and in-memory encoding.
use super::*;
use crate::{automation::TargetRef, state::PlotiumApp};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExportOutput {
    pub format: String,
    pub path: PathBuf,
    #[serde(default)]
    pub overwrite: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FigureExportParams {
    pub canvas: TargetRef,
    pub outputs: Vec<ExportOutput>,
    pub size_mm: Option<[f32; 2]>,
    #[serde(default = "default_dpi")]
    pub dpi: u16,
}
fn default_dpi() -> u16 {
    300
}
#[derive(Clone, Debug)]
pub struct PreparedFigureExport {
    pub parameters: FigureExportParams,
    pub size_mm: [f32; 2],
    pub pixels: [u32; 2],
    pub outputs: Vec<Result<ExportTarget, String>>,
}
pub fn figure_export_schema() -> Value {
    json!({"type":"object","additionalProperties":false,"required":["canvas","outputs"],"properties":{
 "canvas":crate::automation::image_preview_schema()["properties"]["target"].clone(),"outputs":{"type":"array","minItems":1,"maxItems":8,"items":{"type":"object","additionalProperties":false,"required":["format","path"],"properties":{"format":{"enum":["svg","pdf","png"]},"path":{"type":"string","description":"Absolute path with matching extension; existing parent directory required."},"overwrite":{"type":"boolean","default":false,"description":"Request replacement; separate unchecked desktop confirmation is still required."}}}},"size_mm":{"type":"array","minItems":2,"maxItems":2,"items":{"type":"number","exclusiveMinimum":0},"description":"Optional physical output size; must retain canvas aspect ratio. Omit for native page size. No document edit or trimming."},"dpi":{"type":"integer","minimum":1,"maximum":2400,"default":300,"description":"PNG only: each axis rounded to nearest integer from physical points / 72 * DPI, minimum 1 pixel."}}})
}
pub fn prepare_figure_export(
    app: &PlotiumApp,
    parameters: FigureExportParams,
) -> Result<PreparedFigureExport, String> {
    let canvas = app
        .doc
        .canvases
        .iter()
        .find(|c| TargetRef::resource(c.resource_id.into()) == parameters.canvas)
        .ok_or("Export requires an existing whole canvas with its full identity")?;
    if parameters.outputs.is_empty()
        || parameters.outputs.len() > 8
        || parameters.dpi == 0
        || parameters.dpi > 2400
    {
        return Err("Use 1–8 outputs and DPI 1–2400".into());
    }
    let size = parameters.size_mm.unwrap_or(canvas.size_mm);
    if size.iter().any(|v| !v.is_finite() || *v <= 0.0)
        || canvas.size_mm.iter().any(|v| !v.is_finite() || *v <= 0.0)
    {
        return Err("Output and canvas dimensions must be finite and positive".into());
    }
    let ratio = size[0] as f64 / canvas.size_mm[0] as f64;
    if (size[1] as f64 - canvas.size_mm[1] as f64 * ratio).abs() > 0.001 {
        return Err("Output size must retain canvas aspect ratio".into());
    }
    let pt = canvas.size_pt();
    let native_width_mm = pt[0] * 25.4 / 72.0;
    let raster_scale = size[0] as f64 / native_width_mm as f64;
    let pixels = pt.map(|v| {
        ((v as f64 / 72.0 * f64::from(parameters.dpi) * raster_scale)
            .round()
            .max(1.0))
        .min(u32::MAX as f64) as u32
    });
    if parameters.outputs.iter().any(|o| o.format == "png")
        && (pixels.iter().any(|v| *v > 16384)
            || u64::from(pixels[0]) * u64::from(pixels[1]) > 16_777_216)
    {
        return Err("PNG exceeds 16,384-pixel edge or 16,777,216-pixel export budget".into());
    }
    if parameters
        .outputs
        .iter()
        .any(|o| o.path.as_os_str().len() > 4096)
    {
        return Err("Output path exceeds 4096-byte budget".into());
    }
    let mut paths = std::collections::BTreeSet::new();
    let mut duplicate = false;
    let outputs=parameters.outputs.iter().map(|o|{
  if !matches!(o.format.as_str(),"svg"|"pdf"|"png"){return Err("Supported formats are svg, pdf and png".into());}
  let target=ExportTarget::inspect(&o.path,&o.format)?;
  if !paths.insert(target.path.clone()){duplicate=true;return Err("Duplicate output path in batch".into());}
  if target.exists()&&!o.overwrite{return Err("overwrite_not_requested: existing file preserved; request overwrite and confirm it in Plotium".into());}
  Ok(target)
 }).collect();
    if duplicate {
        return Err("Duplicate output path in batch".into());
    }
    Ok(PreparedFigureExport {
        parameters,
        size_mm: size,
        pixels,
        outputs,
    })
}
impl PreparedFigureExport {
    pub fn preflight(&self) -> Value {
        json!({"canvas":self.parameters.canvas,"size_mm":self.size_mm,"dpi":self.parameters.dpi,"png_dimensions":self.pixels,"outputs":self.parameters.outputs.iter().zip(&self.outputs).map(|(o,t)|json!({"format":o.format,"path":t.as_ref().map(|t|&t.path).unwrap_or(&o.path),"overwrite_requested":o.overwrite,"overwrite_required":t.as_ref().is_ok_and(|t|t.exists()),"error":t.as_ref().err()})).collect::<Vec<_>>(),"permission":"per_request_desktop_export_authorization","undoable":false})
    }
}
/// Shared native serializers/renderers, without opening a user-named output.
pub(crate) fn encode_canvas(
    canvas: &CanvasDocument,
    assets: &BTreeMap<AssetId, AssetRecord>,
    format: &str,
    size_mm: [f32; 2],
    dpi: u16,
) -> Result<Vec<u8>, ExportError> {
    // Native document text uses generic sans-serif. Do not silently emit a fontless
    // PNG/PDF; SVG also reports this local-render dependency before delivery.
    let mut fonts = resvg::usvg::fontdb::Database::new();
    super::fonts::load_system_fonts(&mut fonts);
    require_export_fonts(&fonts)?;
    let mut pages = std::collections::BTreeSet::new();
    let mut decoded = 0u64;
    for o in &canvas.objects {
        if let crate::state::CanvasObjectKind::RasterImage(i) = &o.kind {
            i.validate().map_err(ExportError::SvgParse)?;
            if !pages.insert((i.asset, i.page_index)) {
                continue;
            }
            let a = assets
                .get(&i.asset)
                .ok_or(ExportError::MissingImageAsset { asset: i.asset })?;
            let probe =
                plotium_io::image::probe(&a.bytes).map_err(|e| ExportError::CorruptImageAsset {
                    asset: i.asset,
                    reason: e.to_string(),
                })?;
            let bytes = if i.page_index == 0 {
                probe.decoded_bytes
            } else {
                let [w, h] = plotium_io::image::tiff_page_dimensions(&a.bytes, i.page_index)
                    .ok_or_else(|| ExportError::CorruptImageAsset {
                        asset: i.asset,
                        reason: "Image page unavailable".into(),
                    })?;
                u64::from(w) * u64::from(h) * 4
            };
            decoded = decoded.saturating_add(bytes);
            if decoded > 64 * 1024 * 1024 {
                return Err(ExportError::SvgParse(
                    "Decoded export assets exceed 64 MiB".into(),
                ));
            }
        }
    }
    let svg =
        document_svg_with_assets(canvas, assets, Some(size_mm[0]), MissingImagePolicy::Block)?;
    if svg.len() > 8 * 1024 * 1024 {
        return Err(ExportError::SvgParse(
            "Export SVG exceeds 8 MiB budget".into(),
        ));
    }
    match format {
        "svg" => Ok(svg.into_bytes()),
        "pdf" => pdf::render(&[svg]),
        "png" => {
            let raster = rasterize_canvas_with_assets(
                canvas,
                assets,
                RasterOptions {
                    dpi,
                    target_width_mm: Some(size_mm[0]),
                    limits: RasterLimits {
                        max_width: 16384,
                        max_height: 16384,
                        max_pixels: 16_777_216,
                        max_bytes: 67_108_864,
                    },
                },
                MissingImagePolicy::Block,
            )?;
            Ok(raster.to_png()?)
        }
        _ => Err(ExportError::SvgParse("Unsupported export format".into())),
    }
}

fn require_export_fonts(fonts: &resvg::usvg::fontdb::Database) -> Result<(), ExportError> {
    if fonts.faces().next().is_none() {
        Err(ExportError::MissingFont)
    } else {
        Ok(())
    }
}
#[cfg(test)]
#[path = "request_tests.rs"]
mod tests;
