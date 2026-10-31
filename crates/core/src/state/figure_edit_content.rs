//! Exact content adapters over the native content-local geometry service.
use super::*;
use crate::automation::{TargetRef, canvas_object_ref, content_geometry_detail};
use serde_json::{Value, json};

fn content_id(page: &CanvasDocument, target: &TargetRef) -> Result<ContentId, String> {
    page.objects
        .iter()
        .find(|o| *target == TargetRef::resource(canvas_object_ref(page.resource_id, o.id)))
        .map(|o| o.id)
        .ok_or_else(|| "Discover an exact whole-content target on this canvas.".into())
}

fn points(f: ObjectFrame) -> [f32; 4] {
    [f.x, f.y, f.width, f.height]
}

fn reading(page: &CanvasDocument, id: ContentId) -> Result<Value, String> {
    let item = page.object(id).ok_or("Content target disappeared.")?;
    let mut value = content_geometry_detail(page, item);
    value["effective_frame_pt"] = json!(points(item.frame));
    Ok(value)
}

pub(super) fn set_frame(
    page: &mut CanvasDocument,
    target: &TargetRef,
    requested: [f32; 4],
    index: usize,
) -> Result<Value, String> {
    let id = content_id(page, target)?;
    let before = page.object(id).ok_or("Content missing.")?.frame;
    page.set_content_frame(
        id,
        ObjectFrame {
            x: requested[0],
            y: requested[1],
            width: requested[2],
            height: requested[3],
        },
    )?;
    let mut value = reading(page, id)?;
    value["op"] = json!("content_frame");
    value["edit_index"] = json!(index);
    value["target"] = json!(target);
    value["before_frame_pt"] = json!(points(before));
    value["requested_frame_pt"] = json!(requested);
    Ok(value)
}

pub(super) fn image_target(page: &CanvasDocument, target: &TargetRef) -> Result<ContentId, String> {
    let id = content_id(page, target)?;
    page.validate_content_edit(id, false)?;
    let ContentKind::RasterImage(image) = &page.object(id).ok_or("Content missing.")?.kind else {
        return Err("Target must be native raster content.".into());
    };
    if image.page_index != 0 {
        return Err(
            "Image replacement requires page_index 0; the existing page index is never reset."
                .into(),
        );
    }
    Ok(id)
}

pub(super) fn replace_image(
    page: &mut CanvasDocument,
    id: ContentId,
    target: &TargetRef,
    asset: &super::figure_edit_image::FrozenImageRef,
    index: usize,
) -> Result<Value, String> {
    let item = page.object_mut(id).ok_or("Content target disappeared.")?;
    let ContentKind::RasterImage(image) = &mut item.kind else {
        return Err("Target must be native raster content.".into());
    };
    let before = image.asset;
    image.asset = asset.id;
    let mut value = reading(page, id)?;
    value["op"] = json!("image_replace");
    value["edit_index"] = json!(index);
    value["target"] = json!(target);
    value["before_frame_pt"] = value["effective_frame_pt"].clone();
    value["asset_change"] = json!({"before":before,"after":asset.id,"format":asset.format,"pixel_size":asset.pixel_size});
    Ok(value)
}

/// Final geometry is separate from ordered step readings when a later edit
/// changes the same item or resizes its parent panel in this batch.
pub(super) fn final_frames(
    page: &CanvasDocument,
    edits: &[FigureEdit],
) -> Result<Vec<Value>, String> {
    let mut seen = std::collections::BTreeSet::new();
    let mut frames = Vec::new();
    for edit in edits {
        let (FigureEdit::ContentFrame { target, .. } | FigureEdit::ImageReplace { target, .. }) =
            edit
        else {
            continue;
        };
        let id = content_id(page, target)?;
        // Parent edits can change page coordinates or scale local frames after
        // this operation's own validation. Asset-only edits keep their contract.
        if matches!(edit, FigureEdit::ContentFrame { .. }) {
            let frame = page.object(id).ok_or("Content target disappeared.")?.frame;
            page.validate_content_frame_geometry(id, frame)?;
        }
        if seen.insert(id) {
            let mut value = reading(page, id)?;
            value["target"] = json!(target);
            frames.push(value);
        }
    }
    Ok(frames)
}
