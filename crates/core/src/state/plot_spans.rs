//! Author spans: a filled data range across the plot, kept in the plot's
//! annotation set so they share its identity allocator and Undo snapshot.
use super::{AnnotationId, MAX_ANNOTATION_TEXT_BYTES, MAX_PLOT_ANNOTATIONS, PlotAnnotations};
use plotium_figure::{Color, SpanAnnotation, SpanAxis};

pub const DEFAULT_SPAN_COLOR: Color = Color::rgb(0x80, 0x80, 0x80);
pub const DEFAULT_SPAN_OPACITY: f32 = 0.2;

#[derive(Clone, Debug, PartialEq)]
pub struct PlotSpan {
    pub id: AnnotationId,
    pub axis: SpanAxis,
    /// Range ends in the plot's source data units, in either order.
    pub range: [f64; 2],
    pub color: Color,
    pub opacity: f32,
    /// Empty draws no label.
    pub label: String,
    /// `None` follows the figure's tick text size.
    pub font_size: Option<f32>,
    /// Moved on the canvas; automation keeps its range unless the user asks.
    pub hand_moved: bool,
}

impl PlotSpan {
    pub fn new(id: AnnotationId, axis: SpanAxis, range: [f64; 2]) -> Self {
        Self {
            id,
            axis,
            range,
            color: DEFAULT_SPAN_COLOR,
            opacity: DEFAULT_SPAN_OPACITY,
            label: String::new(),
            font_size: None,
            hand_moved: false,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if !self.range.into_iter().all(f64::is_finite) {
            return Err("Span range must be finite data coordinates.".into());
        }
        if self.range[0] == self.range[1] {
            return Err("Span range ends must differ.".into());
        }
        if !(0.0..=1.0).contains(&self.opacity) {
            return Err("Span opacity must be 0–1.".into());
        }
        if self.label.len() > MAX_ANNOTATION_TEXT_BYTES {
            return Err(format!(
                "Span label must be at most {MAX_ANNOTATION_TEXT_BYTES} UTF-8 bytes."
            ));
        }
        if self
            .font_size
            .is_some_and(|pt| !pt.is_finite() || !(1.0..=72.0).contains(&pt))
        {
            return Err("Span label size must be 1–72 pt.".into());
        }
        Ok(())
    }

    fn lower(&self) -> SpanAnnotation {
        SpanAnnotation {
            axis: self.axis,
            range: self.range,
            color: self.color,
            opacity: self.opacity,
            label: self.label.clone(),
            label_size: self.font_size,
        }
    }
}

impl PlotAnnotations {
    pub fn span(&self, id: AnnotationId) -> Option<&PlotSpan> {
        self.spans.iter().find(|s| s.id == id)
    }

    pub fn span_mut(&mut self, id: AnnotationId) -> Option<&mut PlotSpan> {
        self.spans.iter_mut().find(|s| s.id == id)
    }

    /// Validate and append a span, assigning it the next identity.
    pub fn add_span(&mut self, mut span: PlotSpan) -> Result<AnnotationId, String> {
        if self.len() >= MAX_PLOT_ANNOTATIONS {
            return Err(format!(
                "A plot holds at most {MAX_PLOT_ANNOTATIONS} annotations."
            ));
        }
        span.id = self.next_id;
        span.validate()?;
        self.next_id = span
            .id
            .try_advance(1)
            .ok_or("Annotation identity allocator exhausted.")?;
        let id = span.id;
        self.spans.push(span);
        Ok(id)
    }

    pub fn remove_span(&mut self, id: AnnotationId) -> Option<PlotSpan> {
        let index = self.spans.iter().position(|s| s.id == id)?;
        Some(self.spans.remove(index))
    }

    pub fn lower_spans(&self) -> Vec<SpanAnnotation> {
        self.spans.iter().map(PlotSpan::lower).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spans_share_the_allocator_and_budget() {
        let mut annotations = PlotAnnotations::default();
        let text = annotations
            .add_text([0.0, 0.0], "a".into(), 8.0)
            .expect("valid text");
        let span = annotations
            .add_span(PlotSpan::new(AnnotationId::new(0), SpanAxis::X, [1.0, 2.0]))
            .expect("valid span");
        assert_ne!(text, span);
        assert_eq!(annotations.len(), 2);
        assert!(annotations.validate().is_ok());
        annotations.spans[0].id = text;
        assert!(annotations.validate().is_err());
        annotations.spans[0].id = span;
        assert!(annotations.remove_any(span));
        assert!(annotations.spans.is_empty());
    }

    #[test]
    fn swapping_axes_turns_a_span_onto_the_other_axis() {
        let mut overrides = super::super::AxisOverrides::default();
        overrides
            .annotations
            .add_span(PlotSpan::new(AnnotationId::new(0), SpanAxis::X, [1.0, 2.0]))
            .expect("valid span");
        overrides.swap_axes();
        let span = &overrides.annotations.spans[0];
        assert_eq!((span.axis, span.range), (SpanAxis::Y, [1.0, 2.0]));
    }

    #[test]
    fn invalid_spans_do_not_consume_an_id() {
        let mut annotations = PlotAnnotations::default();
        let mut span = PlotSpan::new(AnnotationId::new(0), SpanAxis::Y, [1.0, 1.0]);
        assert!(annotations.add_span(span.clone()).is_err());
        span.range = [0.0, f64::NAN];
        assert!(annotations.add_span(span.clone()).is_err());
        span.range = [0.0, 1.0];
        span.opacity = 1.5;
        assert!(annotations.add_span(span.clone()).is_err());
        span.opacity = 0.5;
        span.font_size = Some(0.5);
        assert!(annotations.add_span(span).is_err());
        assert_eq!(annotations.next_id, AnnotationId::new(0));
    }
}
