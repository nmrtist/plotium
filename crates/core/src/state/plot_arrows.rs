//! Author-placed plot arrows between two points in data coordinates.
use super::{AnnotationId, MAX_PLOT_ANNOTATIONS, PlotAnnotations};
use plotium_figure::{ArrowAnnotation, ArrowHead, Color};

/// Accepted shaft stroke width in pt.
pub const ARROW_LINE_WIDTH_PT: std::ops::RangeInclusive<f32> = 0.1..=20.0;
pub const DEFAULT_ARROW_LINE_WIDTH_PT: f32 = 1.0;

/// An arrow annotation. Like text annotations both ends are data coordinates,
/// so the arrow follows axis ranges, zoom and pan and is clipped to the plot.
#[derive(Clone, Debug, PartialEq)]
pub struct PlotArrow {
    pub id: AnnotationId,
    /// Tail point; the anchor, like a text annotation's `at`.
    pub at: [f64; 2],
    /// Head point.
    pub to: [f64; 2],
    pub head: ArrowHead,
    pub line_width: f32,
    pub color: Color,
    /// Moved on the canvas; automation keeps its ends unless the user asks.
    pub hand_moved: bool,
}

impl PlotArrow {
    pub fn validate(&self) -> Result<(), String> {
        if !self.at.into_iter().chain(self.to).all(f64::is_finite) {
            return Err("Arrow ends must be finite data coordinates.".into());
        }
        if self.at == self.to {
            return Err("Arrow ends must be distinct points.".into());
        }
        if !ARROW_LINE_WIDTH_PT.contains(&self.line_width) {
            return Err(format!(
                "Arrow line width must be {}–{} pt.",
                ARROW_LINE_WIDTH_PT.start(),
                ARROW_LINE_WIDTH_PT.end()
            ));
        }
        Ok(())
    }

    fn lower(&self) -> ArrowAnnotation {
        ArrowAnnotation {
            from: self.at,
            to: self.to,
            color: self.color,
            width: self.line_width,
            head: self.head,
        }
    }
}

impl PlotAnnotations {
    pub fn arrow(&self, id: AnnotationId) -> Option<&PlotArrow> {
        self.arrows.iter().find(|a| a.id == id)
    }

    pub fn arrow_mut(&mut self, id: AnnotationId) -> Option<&mut PlotArrow> {
        self.arrows.iter_mut().find(|a| a.id == id)
    }

    /// Validate and append an arrow, returning its new identity from the
    /// allocator shared with text annotations.
    pub fn add_arrow(
        &mut self,
        at: [f64; 2],
        to: [f64; 2],
        head: ArrowHead,
        line_width: f32,
        color: Color,
    ) -> Result<AnnotationId, String> {
        if self.len() >= MAX_PLOT_ANNOTATIONS {
            return Err(format!(
                "A plot holds at most {MAX_PLOT_ANNOTATIONS} annotations."
            ));
        }
        let id = self.next_id;
        let arrow = PlotArrow {
            id,
            at,
            to,
            head,
            line_width,
            color,
            hand_moved: false,
        };
        arrow.validate()?;
        self.next_id = id
            .try_advance(1)
            .ok_or("Annotation identity allocator exhausted.")?;
        self.arrows.push(arrow);
        Ok(id)
    }

    pub fn remove_arrow(&mut self, id: AnnotationId) -> Option<PlotArrow> {
        let index = self.arrows.iter().position(|a| a.id == id)?;
        Some(self.arrows.remove(index))
    }

    pub fn lower_arrows(&self) -> Vec<ArrowAnnotation> {
        self.arrows.iter().map(PlotArrow::lower).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn add(
        annotations: &mut PlotAnnotations,
        to: [f64; 2],
        width: f32,
    ) -> Result<AnnotationId, String> {
        annotations.add_arrow([0.0, 0.0], to, ArrowHead::Filled, width, Color::BLACK)
    }

    #[test]
    fn arrows_share_the_text_allocator_and_limit() {
        let mut annotations = PlotAnnotations::default();
        let text = annotations
            .add_text([1.0, 2.0], "a".into(), 8.0)
            .expect("valid annotation");
        let arrow = add(&mut annotations, [1.0, 1.0], 1.0).expect("valid arrow");
        assert_ne!(text, arrow);
        assert!(annotations.remove_arrow(arrow).is_some());
        let next = add(&mut annotations, [1.0, 1.0], 1.0).expect("valid arrow");
        assert!(next > arrow);
        assert!(annotations.validate().is_ok());

        while annotations.len() < MAX_PLOT_ANNOTATIONS {
            add(&mut annotations, [1.0, 1.0], 1.0).expect("below the limit");
        }
        assert!(add(&mut annotations, [1.0, 1.0], 1.0).is_err());
        assert!(annotations.add_text([0.0, 0.0], "b".into(), 8.0).is_err());
    }

    #[test]
    fn invalid_arrows_are_rejected_without_consuming_an_id() {
        let mut annotations = PlotAnnotations::default();
        assert!(add(&mut annotations, [0.0, 0.0], 1.0).is_err());
        assert!(add(&mut annotations, [f64::INFINITY, 0.0], 1.0).is_err());
        assert!(add(&mut annotations, [1.0, 0.0], 0.0).is_err());
        assert!(add(&mut annotations, [1.0, 0.0], f32::NAN).is_err());
        assert_eq!(annotations.next_id, AnnotationId::new(0));
    }

    #[test]
    fn loaded_arrow_ids_must_not_alias_text_ids() {
        let mut annotations = PlotAnnotations::default();
        let text = annotations
            .add_text([0.0, 0.0], "a".into(), 8.0)
            .expect("valid annotation");
        add(&mut annotations, [1.0, 1.0], 1.0).expect("valid arrow");
        annotations.arrows[0].id = text;
        assert!(annotations.validate().is_err());
    }
}
