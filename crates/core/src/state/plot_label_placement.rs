//! Placement of author plot labels: automatic, pinned by automation, or
//! dragged by hand on the canvas.
use super::{AnnotationId, CanvasDocument, ObjectId, PlotAnnotations, PlotPart, PlotiumApp};
use crate::automation::{TargetRef, canvas_object_ref};
use plotium_figure::{ReferenceLabelPosition, ReferenceLabelSide, ReferenceOrientation};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Largest pinned text offset from its anchor, in pt.
pub const MAX_LABEL_OFFSET_PT: f32 = 1000.0;

/// How an author label is placed.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum LabelPlacement<T> {
    /// The renderer places it clear of data.
    #[default]
    Auto,
    /// Pinned by automation; a later automation edit may move it.
    Fixed(T),
    /// Dragged on the canvas; automation keeps it unless the user asks.
    Hand(T),
}

impl<T: Copy> LabelPlacement<T> {
    pub fn pinned(&self) -> Option<T> {
        match *self {
            Self::Auto => None,
            Self::Fixed(value) | Self::Hand(value) => Some(value),
        }
    }

    pub fn is_hand(&self) -> bool {
        matches!(self, Self::Hand(_))
    }

    pub fn mode(&self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Fixed(_) => "fixed",
            Self::Hand(_) => "hand",
        }
    }
}

/// The only placement automation sets by name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlacementRequest {
    Auto,
}

/// Apply an automation placement change, keeping a hand placement unless the
/// caller says the user asked.
pub(crate) fn request_placement<T: Copy>(
    current: &mut LabelPlacement<T>,
    requested: LabelPlacement<T>,
    id: AnnotationId,
    override_hand_edits: bool,
) -> Result<(), String> {
    if current.is_hand() && !override_hand_edits {
        return Err(format!(
            "the user placed annotation {id}'s label by hand; change it only when the user \
             asks, by passing override_hand_edits: true"
        ));
    }
    *current = requested;
    Ok(())
}

pub(crate) fn validate_offset(offset: [f32; 2]) -> Result<(), String> {
    if offset
        .iter()
        .all(|v| v.is_finite() && v.abs() <= MAX_LABEL_OFFSET_PT)
    {
        Ok(())
    } else {
        Err(format!(
            "Label offset must be finite and within ±{MAX_LABEL_OFFSET_PT} pt."
        ))
    }
}

pub(crate) fn validate_reference_position(position: ReferenceLabelPosition) -> Result<(), String> {
    if position.along.is_finite() && (0.0..=1.0).contains(&position.along) {
        Ok(())
    } else {
        Err("Reference label position must lie within 0–1 along the line.".into())
    }
}

/// Where a plot's author labels are drawn at its page size, as exported.
pub(crate) fn plot_label_layout(
    page: &CanvasDocument,
    object: ObjectId,
) -> Option<plotium_render::LabelLayout> {
    let figure = page.object(object)?.plot()?.figure();
    let page_frame = page.content_page_frame(object)?;
    let stack = page.shared_x_stack_placement(object);
    let frame = plotium_render::Rect::new(0.0, 0.0, page_frame.width, page_frame.height);
    let clearance = page.y_title_clearance(object);
    let (outer, _) = plotium_render::y_title_area(figure, frame, clearance, 1.0);
    let layout =
        plotium_render::axis_layout_placed(figure, outer.width, outer.height, stack.as_ref());
    let plot = plotium_render::Projector::new(figure, outer, &layout.margins).plot;
    Some(plotium_render::label_layout(figure, plot, 1.0))
}

/// Text annotation placement as drawn: how it was placed and its offset in pt.
pub(crate) fn text_placement_reading(
    placement: LabelPlacement<[f32; 2]>,
    drawn: Option<&plotium_render::TextLabelBox>,
) -> Value {
    let offset = drawn.map(|label| label.offset_pt).or(placement.pinned());
    json!({"placement":placement.mode(),"offset_pt":offset})
}

/// Reference label placement as drawn: how it was placed, its position along
/// the line and the side it sits on.
pub(crate) fn reference_placement_reading(
    orientation: ReferenceOrientation,
    placement: LabelPlacement<ReferenceLabelPosition>,
    drawn: Option<&plotium_render::ReferenceLabelBox>,
) -> Value {
    let position = drawn.map(|label| label.position).or(placement.pinned());
    json!({"label_placement":placement.mode(),"label_position":position.map(|p| json!({
        "along":p.along,
        "side":side_name(orientation, p.side),
    }))})
}

fn side_name(orientation: ReferenceOrientation, side: ReferenceLabelSide) -> &'static str {
    match (orientation, side) {
        (ReferenceOrientation::Vertical, ReferenceLabelSide::Before) => "left",
        (ReferenceOrientation::Vertical, ReferenceLabelSide::After) => "right",
        (ReferenceOrientation::Horizontal, ReferenceLabelSide::Before) => "above",
        (ReferenceOrientation::Horizontal, ReferenceLabelSide::After) => "below",
    }
}

/// The label positions the user set by hand, as hand-edit readings.
pub(crate) fn hand_placed_label_readings(app: &PlotiumApp) -> Vec<Value> {
    let mut readings = Vec::new();
    for page in &app.doc.canvases {
        for object in &page.objects {
            let Some(plot) = object.plot() else {
                continue;
            };
            let target = TargetRef::resource(canvas_object_ref(page.resource_id, object.id));
            readings.extend(
                hand_placed_labels(&plot.axis_overrides.annotations)
                    .into_iter()
                    .map(|value| {
                        json!({
                            "target":target,
                            "key":HAND_PLACED_LABEL_KEY,
                            "label":"Label position",
                            "value":value,
                        })
                    }),
            );
        }
    }
    readings
}

/// Hand-edit key of a label the user dragged.
pub const HAND_PLACED_LABEL_KEY: &str = "annotation.label_position";

fn hand_placed_labels(annotations: &PlotAnnotations) -> Vec<Value> {
    let texts = annotations.items.iter().filter_map(|a| match a.placement {
        LabelPlacement::Hand(offset) => Some(json!({"annotation_id":a.id,"offset_pt":offset})),
        _ => None,
    });
    let lines = annotations
        .reference_lines
        .iter()
        .filter_map(|l| match l.label_placement {
            LabelPlacement::Hand(p) => Some(json!({
                "annotation_id":l.id,
                "along":p.along,
                "side":side_name(l.orientation, p.side),
            })),
            _ => None,
        });
    texts.chain(lines).collect()
}

/// The plot part the user selected on the canvas, with its current placement
/// and the edit that moves it.
pub(crate) fn plot_part_reading(app: &PlotiumApp) -> Option<Value> {
    let (ci, selected) = app.selected_plot_part()?;
    let page = app.doc.canvases.get(ci)?;
    let plot = page.object(selected.object)?.plot()?;
    let target = TargetRef::resource(canvas_object_ref(page.resource_id, selected.object));
    let mut reading = match selected.part {
        PlotPart::Legend => json!({
            "part":"legend",
            "property":crate::properties::guide::POSITION.as_str(),
            "value":plot.figure().legend_position,
        }),
        PlotPart::Label(id) => {
            let annotations = &plot.axis_overrides.annotations;
            let layout = plot_label_layout(page, selected.object);
            let mut reading = if let Some(index) = annotations.items.iter().position(|a| a.id == id)
            {
                let drawn = layout
                    .as_ref()
                    .and_then(|l| l.texts.get(index).copied().flatten());
                let mut reading =
                    text_placement_reading(annotations.items[index].placement, drawn.as_ref());
                reading["kind"] = json!("text");
                reading
            } else {
                let index = annotations
                    .reference_lines
                    .iter()
                    .position(|l| l.id == id)?;
                let line = &annotations.reference_lines[index];
                let drawn = layout
                    .as_ref()
                    .and_then(|l| l.reference.get(index).copied().flatten());
                let mut reading = reference_placement_reading(
                    line.orientation,
                    line.label_placement,
                    drawn.as_ref(),
                );
                reading["kind"] = json!("reference_line");
                reading
            };
            reading["part"] = json!("label");
            reading["annotation_id"] = json!(id);
            reading["edit"] = json!("figure.edit annotation_set");
            reading
        }
        PlotPart::RegionLabel { dataset, region } => {
            let position = app
                .doc
                .dataset_by_id(dataset)
                .and_then(super::Dataset::region_analysis)
                .and_then(|state| state.regions.iter().find(|item| item.id == region))
                .and_then(|item| item.label_position);
            json!({"part":"region_label","dataset":crate::automation::ResourceRef::from(dataset),"region_id":region,"label_position":position})
        }
    };
    reading["target"] = json!(target);
    Some(reading)
}
