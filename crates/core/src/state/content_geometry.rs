//! Content-local numeric geometry, distinct from panel-level layout gestures.
use super::*;

pub const MIN_CONTENT_FRAME_PT: f32 = 1.0;

/// Share the native dominant-relative-change aspect rule without importing a
/// mouse handle or its minimum drag size into numeric geometry.
pub fn aspect_locked_size(before: ObjectFrame, candidate: ObjectFrame, minimum: f32) -> [f32; 2] {
    let width_scale = candidate.width / before.width;
    let height_scale = candidate.height / before.height;
    let scale = if (width_scale - 1.0).abs() >= (height_scale - 1.0).abs() {
        width_scale
    } else {
        height_scale
    }
    .max(minimum / before.width)
    .max(minimum / before.height);
    [before.width * scale, before.height * scale]
}

impl CanvasDocument {
    /// Only this content and its parent govern eligibility. A locked sibling
    /// must not prevent an operation that does not move or resize that sibling.
    pub fn validate_content_edit(&self, id: ContentId, geometry: bool) -> Result<(), String> {
        let item = self.object(id).ok_or("Content target does not exist.")?;
        if item.locked {
            return Err("Content is locked; unlock it before editing.".into());
        }
        if let Some(parent) = self.parent_panel(id).and_then(|p| self.panel(p)) {
            if parent.locked {
                return Err("Parent panel is locked; unlock it before editing content.".into());
            }
            // A stack member's height is its share of the stack.
            if geometry && !matches!(parent.layout, PanelLayout::Free | PanelLayout::SharedXStack) {
                return Err(
                    "Use a free-layout parent panel before editing a content frame.".into(),
                );
            }
        }
        if geometry && self.auto_height {
            return Err("Turn off Auto height before editing an exact content frame.".into());
        }
        Ok(())
    }

    /// Set only an item's parent-local frame (page pt for loose content).
    /// Unlike `set_layout_frame`, this never resizes a parent or its children.
    pub fn set_content_frame(
        &mut self,
        id: ContentId,
        requested: ObjectFrame,
    ) -> Result<ObjectFrame, String> {
        self.validate_content_edit(id, true)?;
        validate_numeric_frame(requested)?;
        let item = self.object(id).ok_or("Content target disappeared.")?;
        let mut effective = requested;
        if matches!(&item.kind, ContentKind::RasterImage(image) if image.preserve_aspect) {
            [effective.width, effective.height] =
                aspect_locked_size(item.frame, requested, MIN_CONTENT_FRAME_PT);
        }
        self.validate_content_frame_geometry(id, effective)?;
        let item = self.object_mut(id).ok_or("Content target disappeared.")?;
        let before = std::mem::replace(&mut item.frame, effective);
        if let Some(stack) = self.shared_x_stack_of(id).map(|panel| panel.id)
            && let Err(error) = self.reflow_shared_x_stack(stack)
        {
            if let Some(item) = self.object_mut(id) {
                item.frame = before;
            }
            return Err(error);
        }
        Ok(self.object(id).map_or(effective, |item| item.frame))
    }

    /// Validate against the current parent, including after later batch edits.
    pub(super) fn validate_content_frame_geometry(
        &self,
        id: ContentId,
        frame: ObjectFrame,
    ) -> Result<(), String> {
        validate_numeric_frame(frame)?;
        let parent = self.parent_panel(id).and_then(|p| self.panel(p));
        if let Some(parent) = parent {
            validate_numeric_frame(ObjectFrame {
                x: parent.frame.x + frame.x,
                y: parent.frame.y + frame.y,
                ..frame
            })?;
        }
        Ok(())
    }
}

fn validate_numeric_frame(frame: ObjectFrame) -> Result<(), String> {
    validate_frame(frame, "content")?;
    if frame.width < MIN_CONTENT_FRAME_PT || frame.height < MIN_CONTENT_FRAME_PT {
        return Err("Content frame dimensions must be at least 1 pt.".into());
    }
    if !(frame.x + frame.width).is_finite() || !(frame.y + frame.height).is_finite() {
        return Err("Content frame edges must be finite in local and page coordinates.".into());
    }
    Ok(())
}
