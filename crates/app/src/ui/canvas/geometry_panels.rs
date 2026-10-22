use super::*;

#[cfg(test)]
#[path = "hit_order_tests.rs"]
mod tests;

#[derive(Clone, Copy)]
pub(crate) struct ObjectHit {
    pub(crate) object: ObjectId,
    pub(crate) kind: ObjectDragKind,
}

#[derive(Clone, Copy)]
pub(crate) struct PanelHit {
    pub(crate) panel: PanelId,
    pub(crate) kind: ObjectDragKind,
}

pub(crate) fn hit_object(canvas: &CanvasDocument, p: Pos2, zoom: f32) -> Option<ObjectHit> {
    hit_objects(canvas, p, zoom).into_iter().next()
}

pub(crate) fn hit_objects(canvas: &CanvasDocument, p: Pos2, zoom: f32) -> Vec<ObjectHit> {
    hit_frames(canvas, p, zoom, |canvas, id| canvas.content_page_frame(id))
}

/// Child hit-testing uses page-space content frames. In page scope callers hit
/// the Panel first; after entering a Panel this is the only content target.
pub(crate) fn hit_content_objects(
    canvas: &CanvasDocument,
    p: Pos2,
    zoom: f32,
    panel: Option<PanelId>,
) -> Vec<ObjectHit> {
    hit_frames(canvas, p, zoom, |canvas, id| {
        (panel.is_none_or(|panel_id| canvas.parent_panel(id) == Some(panel_id)))
            .then(|| canvas.content_page_frame(id))
            .flatten()
    })
}

pub(crate) fn hit_content_object(
    canvas: &CanvasDocument,
    p: Pos2,
    zoom: f32,
    panel: Option<PanelId>,
) -> Option<ObjectHit> {
    hit_content_objects(canvas, p, zoom, panel)
        .into_iter()
        .next()
}

fn hit_frames(
    canvas: &CanvasDocument,
    p: Pos2,
    zoom: f32,
    frame: impl Fn(&CanvasDocument, ObjectId) -> Option<ObjectFrame>,
) -> Vec<ObjectHit> {
    let handle_radius = (HANDLE_SIZE_PX / zoom.max(0.01)).max(3.0);
    canvas
        .objects
        .iter()
        .rev()
        .filter_map(|object| {
            if !object.visible {
                return None;
            }
            if let Some(panel) = canvas
                .parent_panel(object.id)
                .and_then(|id| canvas.panel(id))
                && (!panel.visible
                    || (panel.clip_children && !plot_contains(panel.frame.rect(), p)))
            {
                return None;
            }
            let frame = frame(canvas, object.id)?;
            let r = egui::Rect::from_min_size(
                Pos2::new(frame.x, frame.y),
                egui::vec2(frame.width, frame.height),
            );
            let handles = [
                (r.left_top(), ResizeHandle::TopLeft),
                (r.right_top(), ResizeHandle::TopRight),
                (r.left_bottom(), ResizeHandle::BottomLeft),
                (r.right_bottom(), ResizeHandle::BottomRight),
            ];
            handles
                .into_iter()
                .find_map(|(pos, handle)| {
                    (pos.distance(p) <= handle_radius).then_some(ObjectHit {
                        object: object.id,
                        kind: ObjectDragKind::Resize(handle),
                    })
                })
                .or_else(|| {
                    r.contains(p).then_some(ObjectHit {
                        object: object.id,
                        kind: ObjectDragKind::Move,
                    })
                })
        })
        .collect()
}

/// Visible content follows the document's object paint order. Only after
/// choosing that object do we resolve its semantic Panel; empty Panel frames
/// must never intercept a graph drawn above them.
pub(crate) fn hit_panel(canvas: &CanvasDocument, p: Pos2, zoom: f32) -> Option<PanelHit> {
    if let Some(hit) = hit_object(canvas, p, zoom) {
        let panel = canvas
            .parent_panel(hit.object)
            .and_then(|id| canvas.panel(id))?;
        return Some(PanelHit {
            panel: panel.id,
            kind: panel_frame_hit(panel.frame, p, zoom).unwrap_or(ObjectDragKind::Move),
        });
    }
    canvas
        .panels
        .iter()
        .rev()
        .filter(|panel| panel.visible)
        .find_map(|panel| {
            panel_frame_hit(panel.frame, p, zoom).map(|kind| PanelHit {
                panel: panel.id,
                kind,
            })
        })
}

fn panel_frame_hit(frame: ObjectFrame, p: Pos2, zoom: f32) -> Option<ObjectDragKind> {
    let handle_radius = (HANDLE_SIZE_PX / zoom.max(0.01)).max(3.0);
    let r = egui::Rect::from_min_size(
        Pos2::new(frame.x, frame.y),
        egui::vec2(frame.width, frame.height),
    );
    [
        (r.left_top(), ResizeHandle::TopLeft),
        (r.right_top(), ResizeHandle::TopRight),
        (r.left_bottom(), ResizeHandle::BottomLeft),
        (r.right_bottom(), ResizeHandle::BottomRight),
    ]
    .into_iter()
    .find_map(|(pos, handle)| {
        (pos.distance(p) <= handle_radius).then_some(ObjectDragKind::Resize(handle))
    })
    .or_else(|| r.contains(p).then_some(ObjectDragKind::Move))
}

pub(crate) fn content_screen_rect(
    board: BoardViewport,
    canvas: &CanvasDocument,
    object_id: ObjectId,
    screen: egui::Rect,
) -> Option<PlotRect> {
    let page = BoardTransform::from_board(board, screen).page_screen_rect(canvas);
    let frame = canvas.content_page_frame(object_id)?;
    Some(PlotRect::new(
        page.left() + frame.x * board.zoom,
        page.top() + frame.y * board.zoom,
        frame.width * board.zoom,
        frame.height * board.zoom,
    ))
}
