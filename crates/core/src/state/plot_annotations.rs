//! Author-placed plot annotations anchored in data coordinates.
use super::AnnotationId;
use plotium_figure::{Annotation, Color};

pub const MAX_ANNOTATION_TEXT_BYTES: usize = 1024;
/// Bounds one plot's inspect payload; labels are sparse figure furniture.
pub const MAX_PLOT_ANNOTATIONS: usize = 256;

/// A text label whose anchor is a point in the plot's data coordinates.
/// Renderers project the anchor through the current axis ranges, so the label
/// follows range overrides, zoom and pan instead of staying at a page offset.
#[derive(Clone, Debug, PartialEq)]
pub struct PlotAnnotation {
    pub id: AnnotationId,
    pub at: [f64; 2],
    pub text: String,
    pub font_size: f32,
    pub color: Color,
    /// Offset of the text from its anchor in pt, x right and y up.
    pub placement: super::LabelPlacement<[f32; 2]>,
}

impl PlotAnnotation {
    pub fn validate(&self) -> Result<(), String> {
        if !self.at.into_iter().all(f64::is_finite) {
            return Err("Annotation anchor must be finite data coordinates.".into());
        }
        if self.text.trim().is_empty() || self.text.len() > MAX_ANNOTATION_TEXT_BYTES {
            return Err(format!(
                "Annotation text must be nonblank and at most {MAX_ANNOTATION_TEXT_BYTES} UTF-8 bytes."
            ));
        }
        if !self.font_size.is_finite() || !(1.0..=72.0).contains(&self.font_size) {
            return Err("Annotation font size must be 1–72 pt.".into());
        }
        self.placement
            .pinned()
            .map_or(Ok(()), super::plot_label_placement::validate_offset)
    }

    fn lower(&self) -> Annotation {
        Annotation {
            display_baseline: 0.0,
            text: self.text.clone(),
            at: self.at,
            color: self.color,
            size: self.font_size,
            anchor: plotium_figure::TextAnchor::Point,
            offset_pt: self.placement.pinned(),
        }
    }
}

/// A plot's authored annotations plus their identity allocator.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlotAnnotations {
    /// Persistent high-water mark: a deleted id is never reissued, so a stale
    /// automation reference cannot silently address a newer annotation.
    pub next_id: AnnotationId,
    pub items: Vec<PlotAnnotation>,
    pub reference_lines: Vec<super::PlotReferenceLine>,
    pub arrows: Vec<super::PlotArrow>,
}

impl PlotAnnotations {
    pub fn is_empty(&self) -> bool {
        self.items.is_empty() && self.reference_lines.is_empty() && self.arrows.is_empty()
    }

    /// Annotations of every kind, which share one per-plot budget.
    pub fn len(&self) -> usize {
        self.items.len() + self.reference_lines.len() + self.arrows.len()
    }

    pub fn get(&self, id: AnnotationId) -> Option<&PlotAnnotation> {
        self.items.iter().find(|a| a.id == id)
    }

    pub fn get_mut(&mut self, id: AnnotationId) -> Option<&mut PlotAnnotation> {
        self.items.iter_mut().find(|a| a.id == id)
    }

    /// Validate and append a text annotation, returning its new identity.
    pub fn add_text(
        &mut self,
        at: [f64; 2],
        text: String,
        font_size: f32,
    ) -> Result<AnnotationId, String> {
        if self.len() >= MAX_PLOT_ANNOTATIONS {
            return Err(format!(
                "A plot holds at most {MAX_PLOT_ANNOTATIONS} annotations."
            ));
        }
        let id = self.next_id;
        let annotation = PlotAnnotation {
            id,
            at,
            text,
            font_size,
            color: Color::BLACK,
            placement: Default::default(),
        };
        annotation.validate()?;
        self.next_id = id
            .try_advance(1)
            .ok_or("Annotation identity allocator exhausted.")?;
        self.items.push(annotation);
        Ok(id)
    }

    pub fn remove(&mut self, id: AnnotationId) -> Option<PlotAnnotation> {
        let index = self.items.iter().position(|a| a.id == id)?;
        Some(self.items.remove(index))
    }

    /// Reject files whose annotations could alias an id or fail to render.
    pub fn validate(&self) -> Result<(), String> {
        if self.len() > MAX_PLOT_ANNOTATIONS {
            return Err(format!(
                "a plot holds at most {MAX_PLOT_ANNOTATIONS} annotations"
            ));
        }
        let mut seen = std::collections::BTreeSet::new();
        for annotation in &self.items {
            annotation.validate()?;
        }
        for line in &self.reference_lines {
            line.validate()?;
        }
        for arrow in &self.arrows {
            arrow.validate()?;
        }
        let ids = self.items.iter().map(|a| a.id);
        for id in ids
            .chain(self.reference_lines.iter().map(|l| l.id))
            .chain(self.arrows.iter().map(|a| a.id))
        {
            if id >= self.next_id || !seen.insert(id) {
                return Err(format!(
                    "annotation {id} is duplicated or not below the allocator"
                ));
            }
        }
        Ok(())
    }

    pub fn lower(&self) -> Vec<Annotation> {
        self.items.iter().map(PlotAnnotation::lower).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deleted_ids_are_never_reissued() {
        let mut annotations = PlotAnnotations::default();
        let first = annotations
            .add_text([1.0, 2.0], "a".into(), 8.0)
            .expect("valid annotation");
        assert!(annotations.remove(first).is_some());
        let second = annotations
            .add_text([1.0, 2.0], "b".into(), 8.0)
            .expect("valid annotation");
        assert_ne!(first, second);
        assert!(annotations.validate().is_ok());
    }

    #[test]
    fn invalid_annotations_are_rejected_without_consuming_an_id() {
        let mut annotations = PlotAnnotations::default();
        assert!(
            annotations
                .add_text([f64::NAN, 0.0], "a".into(), 8.0)
                .is_err()
        );
        assert!(annotations.add_text([0.0, 0.0], " ".into(), 8.0).is_err());
        assert!(annotations.add_text([0.0, 0.0], "a".into(), 0.5).is_err());
        assert_eq!(annotations.next_id, AnnotationId::new(0));
    }

    #[test]
    fn loaded_ids_at_or_above_the_allocator_are_rejected() {
        let mut annotations = PlotAnnotations::default();
        annotations
            .add_text([0.0, 0.0], "a".into(), 8.0)
            .expect("valid annotation");
        annotations.next_id = AnnotationId::new(0);
        assert!(annotations.validate().is_err());
    }
}
