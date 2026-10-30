use super::{
    BoardFitItem, BoardFitTarget, BoardFrameId, FrameRef, PlotiumApp, SelectionScope,
    board_frame_id, board_frame_ref, board_frames, frame_board_rect, page_frame_showing_dataset,
};
use plotium_render::Rect;

/// Resolve selection once, capturing identity rather than a snapshot of geometry.
/// Data selection indices are consumed immediately and never retained by the fit.
pub fn selected_board_fit(app: &PlotiumApp) -> Result<BoardFitTarget, &'static str> {
    let mut items = Vec::new();
    match app.session.ui.selection_scope {
        SelectionScope::Board | SelectionScope::CanvasList => {
            items.extend(app.session.ui.frame_selection.iter().filter_map(|id| {
                (app.session.ui.selection_scope == SelectionScope::Board
                    || matches!(id, BoardFrameId::Page(_)))
                .then_some(BoardFitItem::Frame(*id))
            }));
        }
        SelectionScope::DataList => {
            for &di in &app.session.ui.data_selection {
                let Some(table) = app.doc.datasets.get(di).and_then(|d| d.as_table()) else {
                    continue;
                };
                let frame = if table.board_sheet_visible() {
                    Some(FrameRef::Sheet(di))
                } else {
                    page_frame_showing_dataset(app, di)
                };
                if let Some(id) = frame.and_then(|frame| board_frame_id(app, frame)) {
                    items.push(BoardFitItem::Frame(id));
                }
            }
        }
        SelectionScope::CanvasObjects | SelectionScope::Layers => {
            // Whole-object selection includes group members; hierarchical paths
            // additionally represent selected panels and their content addresses.
            if let Some(canvas) = app
                .session
                .active_canvas
                .and_then(|ci| app.doc.canvases.get(ci))
            {
                items.extend(app.session.ui.selection.objects().iter().map(|&content| {
                    BoardFitItem::Content {
                        canvas: canvas.resource_id,
                        content,
                    }
                }));
            }
            for path in app.session.ui.hierarchical_selection.paths() {
                if let Some(content) = path.content {
                    items.push(BoardFitItem::Content {
                        canvas: path.canvas,
                        content,
                    });
                } else if let Some(panel) = path.panel {
                    items.push(BoardFitItem::Panel {
                        canvas: path.canvas,
                        panel,
                    });
                }
            }
        }
    }
    captured_board_fit(app, items).ok_or(
        "Select visible pages, tables, or objects on the board before showing the selection.",
    )
}

/// Internal navigation supplies explicit targets without changing selection scope.
pub fn captured_board_fit(
    app: &PlotiumApp,
    items: impl IntoIterator<Item = BoardFitItem>,
) -> Option<BoardFitTarget> {
    let mut valid = Vec::new();
    for item in items {
        if !valid.contains(&item) && board_fit_item_rect(app, item).is_some() {
            valid.push(item);
        }
    }
    (!valid.is_empty()).then_some(BoardFitTarget::Items(valid))
}

pub fn board_fit_item_rect(app: &PlotiumApp, item: BoardFitItem) -> Option<Rect> {
    let rect = match item {
        BoardFitItem::Frame(id) => frame_board_rect(app, board_frame_ref(app, id)?)?,
        BoardFitItem::Content { canvas, content } => {
            let page = app.doc.canvases.get(app.doc.canvas_index(canvas)?)?;
            let object = page.object(content).filter(|object| object.visible)?;
            let mut rect = page.content_page_frame(object.id)?.rect();
            if let Some(panel) = page.parent_panel(content).and_then(|id| page.panel(id)) {
                if !panel.visible {
                    return None;
                }
                if panel.clip_children {
                    let clip = panel.frame.rect();
                    let left = rect.left.max(clip.left);
                    let top = rect.top.max(clip.top);
                    rect = Rect::new(
                        left,
                        top,
                        rect.right().min(clip.right()) - left,
                        rect.bottom().min(clip.bottom()) - top,
                    );
                }
            }
            rect.left += page.board_pos[0];
            rect.top += page.board_pos[1];
            rect
        }
        BoardFitItem::Panel { canvas, panel } => {
            let page = app.doc.canvases.get(app.doc.canvas_index(canvas)?)?;
            let panel = page.panel(panel).filter(|panel| panel.visible)?;
            let mut rect = panel.frame.rect();
            rect.left += page.board_pos[0];
            rect.top += page.board_pos[1];
            rect
        }
    };
    valid_rect(&rect).then_some(rect)
}

fn valid_rect(rect: &Rect) -> bool {
    [rect.left, rect.top, rect.right(), rect.bottom()]
        .iter()
        .all(|v| v.is_finite())
        && rect.right() > rect.left
        && rect.bottom() > rect.top
}

/// Re-evaluate live bounds and count only targets that can actually be shown.
pub fn board_fit_bounds(app: &PlotiumApp, target: &BoardFitTarget) -> Option<([f32; 4], usize)> {
    let rects: Box<dyn Iterator<Item = Rect> + '_> = match target {
        // FrameRef is an immediate lookup here. Avoid resolving every ID through
        // a linear document search when fitting a large board.
        BoardFitTarget::AllFrames => Box::new(
            board_frames(app)
                .into_iter()
                .filter_map(|frame| frame_board_rect(app, frame))
                .filter(valid_rect),
        ),
        BoardFitTarget::Frame(id) => {
            Box::new(board_fit_item_rect(app, BoardFitItem::Frame(*id)).into_iter())
        }
        BoardFitTarget::Items(items) => Box::new(
            items
                .iter()
                .filter_map(|item| board_fit_item_rect(app, *item)),
        ),
        BoardFitTarget::Region(bounds) => return Some((*bounds, 1)),
        BoardFitTarget::Viewport { .. } => return None,
    };
    let mut bounds: Option<[f32; 4]> = None;
    let mut count = 0;
    for rect in rects {
        bounds = Some(match bounds {
            None => [rect.left, rect.top, rect.right(), rect.bottom()],
            Some(b) => [
                b[0].min(rect.left),
                b[1].min(rect.top),
                b[2].max(rect.right()),
                b[3].max(rect.bottom()),
            ],
        });
        count += 1;
    }
    bounds.map(|b| (b, count))
}

#[cfg(test)]
#[path = "board_fit_tests.rs"]
mod tests;
