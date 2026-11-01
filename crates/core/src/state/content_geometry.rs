//! Content-local numeric geometry, distinct from panel-level layout gestures.
use super::*;

pub const MIN_CONTENT_FRAME_PT: f32 = 1.0;

/// Share the native dominant-relative-change aspect rule without importing a
/// mouse handle or its minimum drag size into numeric geometry. `minimum`
/// bounds the long edge only, since a thin image's short edge may start far
/// below it, and an object already under `minimum` is not forced up to it.
/// `MIN_CONTENT_FRAME_PT` is the hard floor for the short edge, so an edge
/// thinner than that rises to it.
pub fn aspect_locked_size(before: ObjectFrame, candidate: ObjectFrame, minimum: f32) -> [f32; 2] {
    let width_scale = candidate.width / before.width;
    let height_scale = candidate.height / before.height;
    let scale = if (width_scale - 1.0).abs() >= (height_scale - 1.0).abs() {
        width_scale
    } else {
        height_scale
    };
    let scale = scale.max(aspect_scale_floor(before, minimum));
    [before.width * scale, before.height * scale]
}

/// The smallest size `aspect_locked_size` can reach from `before`.
pub fn aspect_locked_min_size(before: ObjectFrame, minimum: f32) -> [f32; 2] {
    let floor = aspect_scale_floor(before, minimum);
    [before.width * floor, before.height * floor]
}

fn aspect_scale_floor(before: ObjectFrame, minimum: f32) -> f32 {
    (minimum / before.width.max(before.height))
        .min(1.0)
        .max(MIN_CONTENT_FRAME_PT / before.width.min(before.height))
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

#[cfg(test)]
mod tests {
    use super::*;

    const DRAG_MINIMUM: f32 = 24.0;

    fn resized(before: [f32; 2], candidate: [f32; 2]) -> [f32; 2] {
        aspect_locked_size(
            ObjectFrame::new(0.0, 0.0, before[0], before[1]),
            ObjectFrame::new(0.0, 0.0, candidate[0], candidate[1]),
            DRAG_MINIMUM,
        )
    }

    #[test]
    fn a_short_edge_below_the_minimum_does_not_force_growth() {
        assert_eq!(resized([75.0, 15.0], [90.0, 18.0]), [90.0, 18.0]);
        assert_eq!(resized([75.0, 15.0], [60.0, 12.0]), [60.0, 12.0]);
    }

    #[test]
    fn the_long_edge_stops_at_the_minimum() {
        let [width, height] = resized([75.0, 15.0], [6.0, 1.2]);
        assert_eq!(width, 24.0);
        assert!((height - 4.8).abs() < 1e-5);
    }

    #[test]
    fn a_long_edge_already_below_the_minimum_stays_put() {
        assert_eq!(resized([20.0, 10.0], [10.0, 5.0]), [20.0, 10.0]);
    }

    #[test]
    fn a_thin_object_stops_at_the_one_point_edge() {
        assert_eq!(resized([600.0, 2.0], [601.0, 2.0])[1], 2.0 * 601.0 / 600.0);
        assert_eq!(resized([600.0, 2.0], [100.0, 0.5]), [300.0, 1.0]);
    }

    #[test]
    fn the_reachable_minimum_matches_the_resize_floor() {
        let before = ObjectFrame::new(0.0, 0.0, 100.0, 60.0);
        let tiny = ObjectFrame::new(0.0, 0.0, 2.0, 2.0);
        assert_eq!(
            aspect_locked_min_size(before, DRAG_MINIMUM),
            aspect_locked_size(before, tiny, DRAG_MINIMUM)
        );
    }

    #[test]
    fn an_edge_under_one_point_rises_to_it() {
        let before = ObjectFrame::new(0.0, 0.0, 100.0, 40.0);
        let thin = ObjectFrame {
            height: 0.75,
            ..before
        };
        let requested = ObjectFrame::new(0.0, 0.0, 10.0, 1.0);
        let [width, height] = aspect_locked_size(thin, requested, MIN_CONTENT_FRAME_PT);
        assert!((width - 400.0 / 3.0).abs() < 1e-3);
        assert!((height - 1.0).abs() < 1e-5);
    }

    #[test]
    fn large_objects_stop_when_the_long_edge_reaches_the_minimum() {
        assert_eq!(resized([100.0, 100.0], [10.0, 10.0]), [24.0, 24.0]);
        assert_eq!(resized([200.0, 100.0], [100.0, 50.0]), [100.0, 50.0]);
    }
}
