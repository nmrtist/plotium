//! Axis presentation captured before author overrides are applied.

use plotium_figure::{AxisFrame, AxisScale, Figure};

#[derive(Clone, Debug, PartialEq)]
pub struct DerivedAxes {
    pub x_label: String,
    pub y_label: String,
    pub x_show_tick_labels: bool,
    pub x_show_label: bool,
    pub y_show_tick_labels: bool,
    pub y_show_label: bool,
    pub x_thousands_separator: bool,
    pub y_thousands_separator: bool,
    pub frame: AxisFrame,
    pub x_scale: AxisScale,
    pub y_scale: AxisScale,
    pub y2_label: String,
    pub y2_show_tick_labels: bool,
    pub y2_show_label: bool,
    pub y2_scale: AxisScale,
}

impl DerivedAxes {
    pub fn from_figure(figure: &Figure) -> Self {
        let right = figure.y2.as_ref();
        Self {
            x_label: figure.x.label.clone(),
            y_label: figure.y.label.clone(),
            x_show_tick_labels: figure.x.show_tick_labels,
            x_show_label: figure.x.show_label,
            y_show_tick_labels: figure.y.show_tick_labels,
            y_show_label: figure.y.show_label,
            x_thousands_separator: figure.x.thousands_separator,
            y_thousands_separator: figure.y.thousands_separator,
            frame: figure.axis_frame,
            x_scale: figure.x.scale,
            y_scale: figure.y.scale,
            y2_label: right.map(|axis| axis.label.clone()).unwrap_or_default(),
            y2_show_tick_labels: right.is_none_or(|axis| axis.show_tick_labels),
            y2_show_label: right.is_none_or(|axis| axis.show_label),
            y2_scale: right.map_or(AxisScale::Linear, |axis| axis.scale),
        }
    }
}
