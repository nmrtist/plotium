//! Author reference lines: a data value spanning the plot, kept in the plot's
//! annotation set so they share its identity allocator and Undo snapshot.
use super::{AnnotationId, MAX_ANNOTATION_TEXT_BYTES, MAX_PLOT_ANNOTATIONS, PlotAnnotations};
use plotium_figure::{Color, LineStyle, ReferenceLine, ReferenceOrientation};

pub const DEFAULT_REFERENCE_LINE_WIDTH_PT: f32 = 0.75;
pub const DEFAULT_REFERENCE_LINE_STYLE: LineStyle = LineStyle::Dashed;
pub const MAX_REFERENCE_LINE_WIDTH_PT: f32 = 20.0;
pub const MIN_REFERENCE_LINE_WIDTH_PT: f32 = 0.1;

/// An f32 point width as the shortest f64 that round-trips it, so schemas and
/// readings show `0.1` rather than the widened `0.10000000149011612`.
pub fn reference_line_width_bound(width: f32) -> f64 {
    width.to_string().parse().unwrap_or(f64::from(width))
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlotReferenceLine {
    pub id: AnnotationId,
    pub orientation: ReferenceOrientation,
    /// Constant coordinate in the plot's source data units: x for a vertical
    /// line, y for a horizontal one.
    pub value: f64,
    pub color: Color,
    pub width: f32,
    pub line_style: LineStyle,
    /// Empty draws no label.
    pub label: String,
    /// `None` follows the figure's tick text size.
    pub font_size: Option<f32>,
}

impl PlotReferenceLine {
    pub fn new(id: AnnotationId, orientation: ReferenceOrientation, value: f64) -> Self {
        Self {
            id,
            orientation,
            value,
            color: Color::AXIS,
            width: DEFAULT_REFERENCE_LINE_WIDTH_PT,
            line_style: DEFAULT_REFERENCE_LINE_STYLE,
            label: String::new(),
            font_size: None,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if !self.value.is_finite() {
            return Err("Reference line value must be a finite data coordinate.".into());
        }
        if !self.width.is_finite()
            || !(MIN_REFERENCE_LINE_WIDTH_PT..=MAX_REFERENCE_LINE_WIDTH_PT).contains(&self.width)
        {
            return Err(format!(
                "Reference line width must be {MIN_REFERENCE_LINE_WIDTH_PT}–{MAX_REFERENCE_LINE_WIDTH_PT} pt."
            ));
        }
        if self.label.len() > MAX_ANNOTATION_TEXT_BYTES {
            return Err(format!(
                "Reference line label must be at most {MAX_ANNOTATION_TEXT_BYTES} UTF-8 bytes."
            ));
        }
        if self
            .font_size
            .is_some_and(|pt| !pt.is_finite() || !(1.0..=72.0).contains(&pt))
        {
            return Err("Reference line label size must be 1–72 pt.".into());
        }
        Ok(())
    }

    fn lower(&self) -> ReferenceLine {
        ReferenceLine {
            orientation: self.orientation,
            value: self.value,
            color: self.color,
            width: self.width,
            line_style: self.line_style,
            label: self.label.clone(),
            label_size: self.font_size,
        }
    }
}

impl PlotAnnotations {
    pub fn reference_line(&self, id: AnnotationId) -> Option<&PlotReferenceLine> {
        self.reference_lines.iter().find(|l| l.id == id)
    }

    pub fn reference_line_mut(&mut self, id: AnnotationId) -> Option<&mut PlotReferenceLine> {
        self.reference_lines.iter_mut().find(|l| l.id == id)
    }

    /// Validate and append a reference line, assigning it the next identity.
    pub fn add_reference_line(
        &mut self,
        mut line: PlotReferenceLine,
    ) -> Result<AnnotationId, String> {
        if self.len() >= MAX_PLOT_ANNOTATIONS {
            return Err(format!(
                "A plot holds at most {MAX_PLOT_ANNOTATIONS} annotations."
            ));
        }
        line.id = self.next_id;
        line.validate()?;
        self.next_id = line
            .id
            .try_advance(1)
            .ok_or("Annotation identity allocator exhausted.")?;
        let id = line.id;
        self.reference_lines.push(line);
        Ok(id)
    }

    pub fn remove_reference_line(&mut self, id: AnnotationId) -> Option<PlotReferenceLine> {
        let index = self.reference_lines.iter().position(|l| l.id == id)?;
        Some(self.reference_lines.remove(index))
    }

    pub fn lower_reference_lines(&self) -> Vec<ReferenceLine> {
        self.reference_lines
            .iter()
            .map(PlotReferenceLine::lower)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_lines_share_the_text_allocator() {
        let mut annotations = PlotAnnotations::default();
        let text = annotations
            .add_text([0.0, 0.0], "a".into(), 8.0)
            .expect("valid text");
        let line = annotations
            .add_reference_line(PlotReferenceLine::new(
                AnnotationId::new(0),
                ReferenceOrientation::Vertical,
                1.5,
            ))
            .expect("valid line");
        assert_ne!(text, line);
        assert!(annotations.get(line).is_none());
        assert!(annotations.reference_line(text).is_none());
        assert!(annotations.validate().is_ok());
        annotations.next_id = line;
        assert!(annotations.validate().is_err());
        // A line reusing a text id is rejected even below the allocator.
        annotations.next_id = AnnotationId::new(5);
        annotations.reference_lines[0].id = text;
        assert!(annotations.validate().is_err());
    }

    #[test]
    fn text_and_lines_share_the_per_plot_budget() {
        let mut annotations = PlotAnnotations::default();
        for _ in 0..MAX_PLOT_ANNOTATIONS - 1 {
            annotations
                .add_text([0.0, 0.0], "a".into(), 8.0)
                .expect("valid text");
        }
        let line =
            PlotReferenceLine::new(AnnotationId::new(0), ReferenceOrientation::Vertical, 1.0);
        assert!(annotations.add_reference_line(line.clone()).is_ok());
        assert!(annotations.add_reference_line(line).is_err());
        assert!(annotations.add_text([0.0, 0.0], "a".into(), 8.0).is_err());
    }

    #[test]
    fn invalid_reference_lines_do_not_consume_an_id() {
        let mut annotations = PlotAnnotations::default();
        let mut line = PlotReferenceLine::new(
            AnnotationId::new(0),
            ReferenceOrientation::Horizontal,
            f64::INFINITY,
        );
        assert!(annotations.add_reference_line(line.clone()).is_err());
        line.value = 0.0;
        line.width = 0.0;
        assert!(annotations.add_reference_line(line.clone()).is_err());
        line.width = 1.0;
        line.font_size = Some(100.0);
        assert!(annotations.add_reference_line(line).is_err());
        assert_eq!(annotations.next_id, AnnotationId::new(0));
    }
}
