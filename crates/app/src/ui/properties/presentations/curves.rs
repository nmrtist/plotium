//! Curve presentations: the Line section of every series, bars included.

use super::super::{routes::*, types::*};
use plotium_core::properties::*;

pub(super) const CURVES: &[PropertyPresentation] = &[
    object_entry(line_display::COLOR, "Color", LINE_HOME),
    object_entry(line_display::LABEL, "Legend label", LINE_HOME),
    object_entry(line_display::STYLE, "Line style", LINE_HOME),
    object_entry(line_display::MARKER, "Marker", LINE_HOME),
    object_entry(line_display::Y_AXIS, "Y axis", LINE_HOME),
    object_entry(line_display::MARKER_SIZE, "Marker diameter", LINE_HOME),
    object_entry(line_display::MARKER_FILL, "Marker fill", LINE_HOME),
    object_entry(line_display::MARKER_HOLLOW, "Hollow marker", LINE_HOME),
    object_entry(
        line_display::MARKER_EVERY,
        "Marker every N points",
        LINE_HOME,
    ),
    object_entry(line_display::BAR_WIDTH, "Bar width", LINE_HOME),
    object_entry(line_display::BAR_EDGE_COLOR, "Bar outline color", LINE_HOME),
    object_entry(line_display::BAR_EDGE_WIDTH, "Bar outline width", LINE_HOME),
    object_entry(line_display::BAR_HATCH, "Bar hatch", LINE_HOME),
    object_entry(chart_layout::BAR_STACK, "Bar stack", LINE_HOME),
    object_entry(chart_layout::SPREAD, "Replicate spread", LINE_HOME),
    object_entry(chart_layout::VALUE_LABELS, "Value labels", LINE_HOME),
    object_entry(replicates::REPLICATES, "Replicates (n)", LINE_HOME),
    object_entry(replicates::ERROR_BARS, "Error bars show", LINE_HOME),
    object_entry(
        line_display::MARKER_EDGE_COLOR,
        "Marker edge color",
        LINE_HOME,
    ),
    object_entry(
        line_display::MARKER_EDGE_WIDTH,
        "Marker edge width",
        LINE_HOME,
    ),
    PropertyPresentation {
        id: line::STROKE_WIDTH,
        localized_label: LocalizedText("Stroke width"),
        localized_aliases: &[LocalizedText("line thickness")],
        home_route: LINE_HOME,
        canvas_step: false,
        uses_canvas_length_unit: false,
    },
];
