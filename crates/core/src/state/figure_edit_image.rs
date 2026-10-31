//! Bounded explicit local image input; the existing native asset store owns bytes.
use super::{AssetId, AssetRecord};
use std::collections::BTreeMap;
use std::{io::Read, path::Path};
pub(super) fn prepare_image(path: &Path, total: &mut usize) -> Result<AssetRecord, String> {
    if !path.is_absolute() {
        return Err("Image path must name an absolute local PNG/JPEG/SVG file.".into());
    }
    if !std::fs::metadata(path)
        .map_err(|e| format!("Image asset unavailable at {}: {e}", path.display()))?
        .is_file()
    {
        return Err("Image path must name a regular file.".into());
    }
    let mut file = std::fs::File::open(path)
        .map_err(|e| format!("Image asset unavailable at {}: {e}", path.display()))?;
    if !file.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("Image path must name a regular file.".into());
    }
    let mut bytes = Vec::new();
    file.by_ref()
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    *total += bytes.len();
    if bytes.len() > 1024 * 1024 || *total > 4 * 1024 * 1024 {
        return Err("Images allow 1 MiB per file and 4 MiB per batch.".into());
    }
    if plotium_io::image::sniff(&bytes) == plotium_io::image::RasterFormat::Svg {
        return prepare_svg(&bytes);
    }
    let probe =
        plotium_io::image::probe(&bytes).map_err(|e| format!("Image asset unavailable: {e}"))?;
    if !matches!(
        probe.format,
        plotium_io::image::RasterFormat::Png | plotium_io::image::RasterFormat::Jpeg
    ) || probe.decoded_bytes > 4 * 1024 * 1024
    {
        return Err("Use SVG, or PNG/JPEG with at most 4 MiB decoded RGBA8 per image.".into());
    }
    let decoded = plotium_io::image::decode_rgba8(&bytes, false)
        .map_err(|e| format!("Image asset unavailable: {e}"))?;
    Ok(AssetRecord {
        id: AssetId::new(),
        sha256: decoded.sha256,
        format: probe.format.name().to_ascii_lowercase(),
        pixel_size: [probe.width, probe.height],
        bytes,
    })
}

fn prepare_svg(source: &[u8]) -> Result<AssetRecord, String> {
    let bytes = crate::export::normalize_svg_asset(source)
        .map_err(|e| format!("Image asset unavailable: {e}"))?;
    let probe =
        plotium_io::image::probe(&bytes).map_err(|e| format!("Image asset unavailable: {e}"))?;
    Ok(AssetRecord {
        id: AssetId::new(),
        sha256: plotium_io::image::sha256(&bytes),
        format: probe.format.name().to_ascii_lowercase(),
        pixel_size: [probe.width, probe.height],
        bytes,
    })
}

/// Metadata for a frozen, deduplicated asset. Encoded bytes stay in the native
/// store or in the candidate's asset actions, never in the protocol response.
pub(super) struct FrozenImageRef {
    pub id: AssetId,
    pub format: String,
    pub pixel_size: [u32; 2],
}

pub(super) fn freeze_image(
    existing: &BTreeMap<AssetId, AssetRecord>,
    pending: &mut Vec<AssetRecord>,
    path: &Path,
    total: &mut usize,
    count: &mut usize,
) -> Result<FrozenImageRef, String> {
    *count += 1;
    if *count > 8 {
        return Err("At most 8 local images per batch, including creation and replacement.".into());
    }
    let candidate = prepare_image(path, total)?;
    let id = existing
        .iter()
        .find_map(|(id, a)| (a.sha256 == candidate.sha256).then_some(*id))
        .or_else(|| {
            pending
                .iter()
                .find_map(|a| (a.sha256 == candidate.sha256).then_some(a.id))
        })
        .unwrap_or(candidate.id);
    let frozen = FrozenImageRef {
        id,
        format: candidate.format.clone(),
        pixel_size: candidate.pixel_size,
    };
    if id == candidate.id {
        pending.push(candidate);
    }
    Ok(frozen)
}
