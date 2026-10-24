use super::{AnnotationId, CanvasId, ObjectId};

pub struct PanelNoteEditState {
    pub canvas: usize,
    pub object: ObjectId,
    pub buffer: String,
    pub focus: bool,
}

pub struct TextEditState {
    pub canvas: usize,
    pub object: ObjectId,
    pub buffer: String,
    pub focus: bool,
}

/// Editing the text of a plot's text annotation or reference-line label.
pub struct PlotTextEditState {
    pub canvas: CanvasId,
    pub object: ObjectId,
    pub annotation: AnnotationId,
    pub buffer: String,
    pub focus: bool,
}

/// Where a plot was right-clicked: the annotation under the pointer, or else
/// the point new annotations are placed at, in the plot's data coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlotMenuPoint {
    pub canvas: CanvasId,
    pub object: ObjectId,
    pub annotation: Option<AnnotationId>,
    pub at: [f64; 2],
    /// Tail of a new arrow pointing at `at`, a short way off it.
    pub tail: [f64; 2],
}
