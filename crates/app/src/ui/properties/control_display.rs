//! Controls for atomic range intent; no direct axis state writes.
use super::*;
use plotium_core::properties::{AxisRangeValue, plot_display};

pub(super) fn range(
    row: &Row,
    current: &AxisRangeValue,
    pending: &mut Option<Pending>,
    gesture: &mut Option<(PropertyId, GestureEdge)>,
    ui: &mut Ui,
) {
    let ResolvedSchema::AxisRange {
        unit,
        view,
        manual_allowed,
    } = &row.representative.schema
    else {
        unreachable!("range control requires range schema")
    };
    let manual_allowed = *manual_allowed;
    // An axis break has no automatic value: Auto there means no break.
    let auto_name = if matches!(
        row.presentation.id,
        plot_display::X_BREAK | plot_display::Y_BREAK
    ) {
        "None"
    } else {
        "Auto"
    };
    if row.mixed() {
        for (name, value) in [
            (auto_name, AxisRangeValue::Auto),
            (
                "Manual",
                AxisRangeValue::Manual {
                    min: view[0],
                    max: view[1],
                    unit: unit.clone(),
                },
            ),
        ] {
            if ui
                .add_enabled(
                    manual_allowed || name == auto_name,
                    egui::Button::selectable(false, name),
                )
                .clicked()
            {
                *pending = Some(Pending::Write(
                    row.presentation.id,
                    PropertyValue::AxisRange(value),
                ));
            }
        }
        ui.weak(NO_SINGLE_VALUE);
        return;
    }
    let mut auto = matches!(current, AxisRangeValue::Auto);
    if ui
        .add_enabled(
            manual_allowed || !auto,
            egui::Checkbox::new(&mut auto, auto_name),
        )
        .changed()
    {
        let value = if auto {
            AxisRangeValue::Auto
        } else {
            AxisRangeValue::Manual {
                min: view[0],
                max: view[1],
                unit: unit.clone(),
            }
        };
        *pending = Some(Pending::Write(
            row.presentation.id,
            PropertyValue::AxisRange(value),
        ));
    }
    {
        let (mut min, mut max) = match current {
            AxisRangeValue::Manual { min, max, .. } => (*min, *max),
            AxisRangeValue::Auto => (view[0], view[1]),
        };
        let span = (max - min).abs();
        let a = ui
            .add_enabled(
                manual_allowed && !auto,
                DragValue::new(&mut min)
                    .speed((span / 200.).max(1e-9))
                    .max_decimals(8)
                    .range(f64::MIN..=max.next_down()),
            )
            .on_hover_text("Minimum");
        let b = ui
            .add_enabled(
                manual_allowed && !auto,
                DragValue::new(&mut max)
                    .speed((span / 200.).max(1e-9))
                    .max_decimals(8)
                    .range(min.next_up()..=f64::MAX),
            )
            .on_hover_text("Maximum");
        note_gesture(row, &a, gesture);
        note_gesture(row, &b, gesture);
        if !auto && (a.changed() || b.changed()) {
            *pending = Some(Pending::Write(
                row.presentation.id,
                PropertyValue::AxisRange(AxisRangeValue::Manual {
                    min,
                    max,
                    unit: unit.clone(),
                }),
            ));
        }
    }
    ui.weak(unit.as_deref().unwrap_or("data units"));
}

/// Both coordinates travel as one property write and one gesture.
pub(super) fn position(
    row: &Row,
    current: Option<[f64; 2]>,
    pending: &mut Option<Pending>,
    gesture: &mut Option<(PropertyId, GestureEdge)>,
    ui: &mut Ui,
) {
    if row.mixed() {
        for (label, value) in [("Auto", None), ("Custom", Some([1., 0.]))] {
            if ui.selectable_label(false, label).clicked() {
                *pending = Some(Pending::Write(
                    row.presentation.id,
                    PropertyValue::NormalizedPosition(value),
                ));
            }
        }
        return;
    }
    let mut custom = current.is_some();
    if ui.checkbox(&mut custom, "Custom").changed() {
        *pending = Some(Pending::Write(
            row.presentation.id,
            PropertyValue::NormalizedPosition(custom.then_some(current.unwrap_or([1., 0.]))),
        ));
    }
    let mut position = current.unwrap_or([1., 0.]);
    let mut changed = false;
    for (axis, value) in position.iter_mut().enumerate() {
        let response = ui
            .add_enabled(
                custom,
                hide_value(
                    DragValue::new(value)
                        .speed(0.005)
                        .range(0.0..=1.0)
                        .max_decimals(4),
                    row.mixed(),
                ),
            )
            .on_hover_text(if axis == 0 {
                "Horizontal origin in available plot travel"
            } else {
                "Vertical origin in available plot travel (downward)"
            });
        note_gesture(row, &response, gesture);
        changed |= response.changed();
    }
    if changed {
        *pending = Some(Pending::Write(
            row.presentation.id,
            PropertyValue::NormalizedPosition(Some(position)),
        ));
    }
}
