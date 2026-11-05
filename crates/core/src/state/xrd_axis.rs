//! What a change of an XRD pattern's display axis does to the plots showing it.
use super::*;
use crate::actions::DatasetProcessingState;
use crate::operation::{Diagnostic, DiagnosticCode, OperationKind, OperationReport, Severity};
use plotium_processing::xrd::XrdAxis;

impl PlotiumApp {
    /// Carries manual x ranges onto the new axis, so a zoomed plot keeps showing
    /// the same reflections, and warns when a stack ends up with mixed axes.
    ///
    /// `pending` holds datasets installed in the same step, which count at their
    /// new axes. Returns the plots whose x view must show the whole new range once
    /// rebuilt: those that showed the whole old one, and views the new axis cannot hold.
    pub(crate) fn follow_xrd_axis(
        &mut self,
        dataset: usize,
        state: &DatasetProcessingState,
        pending: &[Dataset],
    ) -> Vec<(usize, ObjectId)> {
        let mut refit = Vec::new();
        let (Some(Dataset::Xrd(xrd)), DatasetProcessingState::Xrd(next)) =
            (self.doc.datasets.get(dataset), state)
        else {
            return refit;
        };
        let (from, to) = (
            (xrd.params.axis, xrd.wavelength(&xrd.params)),
            (next.axis, xrd.wavelength(next)),
        );
        if from == to {
            return refit;
        }
        let id = xrd.resource_id;
        let axes: Vec<(DatasetId, XrdAxis)> = self
            .doc
            .datasets
            .iter()
            .filter_map(Dataset::as_xrd)
            .map(|other| {
                pending
                    .iter()
                    .filter_map(Dataset::as_xrd)
                    .find(|next| next.resource_id == other.resource_id)
                    .unwrap_or(other)
            })
            .map(|other| (other.resource_id, other.params.axis))
            .collect();
        let other_axis = |series: &SeriesBinding| {
            axes.iter()
                .find(|(other, _)| *other == series.source.resource && *other != id)
                .map(|(_, axis)| *axis)
        };
        let mut mixed = false;
        for (index, canvas) in self.doc.canvases.iter_mut().enumerate() {
            for object in &mut canvas.objects {
                let id_of_object = object.id;
                let Some(plot) = object.plot_mut() else {
                    continue;
                };
                let series = &plot.binding.series;
                if !series.iter().any(|s| s.source.resource == id) {
                    continue;
                }
                mixed |= series
                    .iter()
                    .filter_map(other_axis)
                    .any(|axis| axis != next.axis);
                // The first curve sets the plot's axis; the others follow it.
                let leads = series.first().is_some_and(|s| s.source.resource == id);
                if !leads {
                    continue;
                }
                if let Some(range) = plot.axis_overrides.x_range {
                    plot.axis_overrides.x_range = convert_range(range, from, to);
                }
                let viewport = &mut plot.viewport;
                let zoomed = viewport.view_x != viewport.full_x;
                match convert_range(viewport.view_x, from, to).filter(|_| zoomed) {
                    Some(view) => viewport.view_x = view,
                    None => refit.push((index, id_of_object)),
                }
            }
        }
        if mixed {
            let summary = format!(
                "This pattern is stacked with others on a different axis. Set them all to {}.",
                next.axis.menu_label()
            );
            let operation = self.session.begin_operation();
            let report = OperationReport::warning(
                operation,
                OperationKind::XrdAxisChange,
                summary.clone(),
                (),
            )
            .with_diagnostic(Diagnostic::new(
                Severity::Warning,
                DiagnosticCode::XrdStackAxesDiffer,
                summary,
            ));
            self.session.record_operation(report);
        }
        refit
    }

    /// After the rebuild, opens the views [`Self::follow_xrd_axis`] listed.
    pub(crate) fn refit_xrd_views(&mut self, plots: Vec<(usize, ObjectId)>) {
        for (canvas, id) in plots {
            let plot = self
                .doc
                .canvases
                .get_mut(canvas)
                .and_then(|canvas| canvas.object_mut(id))
                .and_then(|object| object.plot_mut());
            if let Some(plot) = plot {
                plot.show_full_x();
            }
        }
    }

    /// Patterns can share a plot only on the same axis.
    pub(super) fn xrd_axes_agree(&self, datasets: &[usize]) -> bool {
        let mut axes = datasets
            .iter()
            .filter_map(|index| self.doc.datasets.get(*index)?.as_xrd())
            .map(|xrd| xrd.params.axis);
        let first = axes.next();
        axes.all(|axis| Some(axis) == first)
    }
}

type AxisState = (XrdAxis, Option<f64>);

fn convert_range(range: AxisRange, from: AxisState, to: AxisState) -> Option<AxisRange> {
    let convert = |value: f64| {
        let angle = match from {
            (XrdAxis::TwoTheta, _) => value,
            (axis, Some(wavelength)) => axis.two_theta(value, wavelength)?,
            _ => return None,
        };
        match to {
            (XrdAxis::TwoTheta, _) => Some(angle),
            (axis, Some(wavelength)) => axis.position(angle, wavelength),
            _ => None,
        }
    };
    let (low, high) = (convert(range.min)?, convert(range.max)?);
    Some(AxisRange::new(low.min(high), low.max(high)))
}

#[cfg(test)]
#[path = "xrd_axis_tests.rs"]
mod tests;
