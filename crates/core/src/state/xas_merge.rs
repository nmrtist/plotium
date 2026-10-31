//! Merging repeated XAS scans. Scans are aligned on their reference foils and
//! averaged in mu(E), so the result is reduced like any measured scan. The
//! reference channel does not carry over: calibrate before merging.
use super::*;
use crate::actions::Action;
use crate::operation::{Diagnostic, DiagnosticCode, OperationKind, OperationReport, Severity};
use plotium_io::xas::{XasChannels, XasColumn, XasColumnId, XasFileKind, XasScan};
use plotium_processing::signal::{self, Trace};
use plotium_processing::xas::{MAX_ALIGNMENT_SHIFT, alignment_shift};

const ENERGY_COLUMN: XasColumnId = XasColumnId(0);
const MU_COLUMN: XasColumnId = XasColumnId(1);

/// Whether scans were aligned, and why not, for the operation report.
#[derive(Debug, Clone, PartialEq)]
pub enum XasMergeAlignment {
    Aligned,
    Unaligned(String),
}

/// The energy added to each scan so its foil meets the first scan's.
fn reference_offsets(scans: &[&XasDataset]) -> Result<Vec<f64>, String> {
    let mut offsets = vec![0.0];
    let first = scans[0].foil_edge().map_err(|error| error.reason())?;
    let reference = |scan: &'_ XasDataset| {
        scan.reference
            .as_ref()
            .map(|r| (r.energy.clone(), r.mu.clone()))
    };
    let (fixed_energy, fixed_mu) = reference(scans[0]).ok_or("the scan has no reference")?;
    for scan in &scans[1..] {
        scan.foil_edge().map_err(|error| error.reason())?;
        let (energy, mu) = reference(scan).ok_or("a scan has no reference")?;
        let shift = alignment_shift((&fixed_energy, &fixed_mu), (&energy, &mu), first)
            .ok_or("the references could not be compared")?;
        offsets.push(shift);
    }
    Ok(offsets)
}

fn same_edge(scans: &[&XasDataset], offsets: &[f64]) -> Result<(f64, f64), String> {
    let mismatch = || "Pick scans of the same absorption edge.".to_owned();
    let mut range = (f64::NEG_INFINITY, f64::INFINITY);
    let mut edges = Vec::new();
    for (scan, offset) in scans.iter().zip(offsets) {
        let (&first, &last) = scan
            .processed
            .energy
            .first()
            .zip(scan.processed.energy.last())
            .ok_or_else(mismatch)?;
        range = (range.0.max(first + offset), range.1.min(last + offset));
        edges.push(scan.edge_e0().ok_or_else(mismatch)? + offset);
    }
    let near = edges
        .iter()
        .all(|edge| (edge - edges[0]).abs() <= MAX_ALIGNMENT_SHIFT);
    let covered = edges.iter().all(|edge| (range.0..=range.1).contains(edge));
    (near && covered).then_some(range).ok_or_else(mismatch)
}

/// The mean mu(E) of `scans` over the energies all of them cover, on the first
/// scan's grid. Beyond that range the average would change its members and step.
pub fn merge_scans(
    scans: &[&XasDataset],
    name: &str,
) -> Result<(XasScan, XasMergeAlignment), String> {
    let [first, rest @ ..] = scans else {
        return Err("Select at least two XAS scans.".into());
    };
    if rest.is_empty() {
        return Err("Select at least two XAS scans.".into());
    }
    // The edge comes first: it is the real difference between two elements,
    // whatever signal each was measured with.
    let edges: Option<Vec<f64>> = scans.iter().map(|scan| scan.edge_e0()).collect();
    if edges.is_some_and(|edges| {
        edges
            .iter()
            .any(|edge| (edge - edges[0]).abs() > MAX_ALIGNMENT_SHIFT)
    }) {
        return Err("Pick scans of the same absorption edge.".into());
    }
    let kind = first.signal.as_ref().map(std::mem::discriminant);
    if rest
        .iter()
        .any(|scan| scan.signal.as_ref().map(std::mem::discriminant) != kind)
    {
        return Err("Pick scans measured with the same signal.".into());
    }
    if scans.iter().any(|scan| !scan.has_spectrum()) {
        return Err("A selected scan has no spectrum to merge.".into());
    }
    let (offsets, alignment) = match reference_offsets(scans) {
        Ok(offsets) => (offsets, XasMergeAlignment::Aligned),
        Err(reason) => (vec![0.0; scans.len()], XasMergeAlignment::Unaligned(reason)),
    };
    if offsets
        .iter()
        .any(|offset| offset.abs() >= MAX_ALIGNMENT_SHIFT - 1.0)
    {
        return Err("Pick scans of the same absorption edge.".into());
    }
    let (low, high) = same_edge(scans, &offsets)?;
    let shifted: Vec<Vec<f64>> = scans
        .iter()
        .zip(&offsets)
        .map(|(scan, offset)| scan.processed.energy.iter().map(|e| e + offset).collect())
        .collect();
    let (grid, values): (Vec<f64>, Vec<f64>) = shifted[0]
        .iter()
        .zip(&first.processed.mu)
        .filter(|(energy, _)| (low..=high).contains(*energy))
        .map(|(energy, mu)| (*energy, *mu))
        .unzip();
    let mut traces = vec![Trace {
        x: &grid,
        y: &values,
    }];
    traces.extend(
        scans[1..]
            .iter()
            .zip(&shifted[1..])
            .map(|(scan, energy)| Trace {
                x: energy,
                y: &scan.processed.mu,
            }),
    );
    let mu = signal::average(&traces).map_err(|error| error.to_string())?;
    let scan = XasScan {
        kind: XasFileKind::Columns,
        source: name.to_owned(),
        columns: vec![
            XasColumn {
                name: "energy".to_owned(),
                values: grid,
            },
            XasColumn {
                name: "mu".to_owned(),
                values: mu,
            },
        ],
        channels: XasChannels {
            energy: ENERGY_COLUMN,
            i0: None,
            transmission: None,
            reference: None,
            fluorescence: Vec::new(),
            mu: Some(MU_COLUMN),
        },
        beamline: first.scan.beamline.clone(),
        sample: first.scan.sample.clone(),
        element: first.scan.element.clone(),
        edge: first.scan.edge.clone(),
        nominal_e0: first.scan.nominal_e0,
        metadata: Default::default(),
        warnings: Vec::new(),
    };
    Ok((scan, alignment))
}

impl PlotiumApp {
    /// Merges the selected scans into a new dataset with its own canvas, and
    /// reports the outcome. A merge left unaligned raises a warning, so that it
    /// is not read as an aligned average.
    pub fn merge_selected_xas(&mut self) -> bool {
        let operation = self.session.begin_operation();
        match self.try_merge_selected_xas() {
            Ok((count, XasMergeAlignment::Aligned)) => {
                let summary = format!("Merged {count} scans, aligned on their reference foils.");
                let report =
                    OperationReport::success(operation, OperationKind::XasMerge, summary, ())
                        .with_diagnostic(Diagnostic::new(
                            Severity::Info,
                            DiagnosticCode::XasMergeSucceeded,
                            "Scans merged after alignment.",
                        ));
                self.session.record_operation(report);
                true
            }
            Ok((count, XasMergeAlignment::Unaligned(reason))) => {
                let summary = format!("Merged {count} scans without alignment: {reason}.");
                let report = OperationReport::warning(
                    operation,
                    OperationKind::XasMerge,
                    summary.clone(),
                    (),
                )
                .with_diagnostic(Diagnostic::new(
                    Severity::Warning,
                    DiagnosticCode::XasMergeUnaligned,
                    summary,
                ));
                self.session.record_operation(report);
                true
            }
            Err(error) => {
                let report = OperationReport::<()>::failure(
                    operation,
                    OperationKind::XasMerge,
                    format!("Merge failed. {error}"),
                    Diagnostic::new(Severity::Error, DiagnosticCode::XasMergeFailed, error),
                );
                self.session.record_operation(report);
                false
            }
        }
    }

    fn try_merge_selected_xas(&mut self) -> Result<(usize, XasMergeAlignment), String> {
        let selection = self.session.ui.data_selection.clone();
        let sources: Vec<&XasDataset> = selection
            .iter()
            .filter_map(|index| self.doc.datasets.get(*index)?.as_xas())
            .collect();
        if sources.len() != selection.len() || sources.is_empty() {
            return Err("Select only XAS scans to merge.".into());
        }
        let name = format!(
            "Merge of {} (+{})",
            crate::workflow::dataset_title(&self.doc.datasets[selection[0]]),
            selection.len().saturating_sub(1)
        );
        let (scan, alignment) = merge_scans(&sources, &name)?;
        let lineage = sources.iter().map(|scan| scan.resource_id);
        let lineage = DatasetLineage::new(DerivationKind::SpectrumArithmetic, lineage);
        let mut merged = XasDataset::load(scan);
        merged.name = Some(name.clone());
        merged.lineage = Some(lineage);
        merged.acquisition_identity = plotium_io::AcquisitionIdentity {
            subject: None,
            acquisition: None,
            source_label: name.clone(),
        };
        let action = Action::insert_dataset_with_default_canvas(
            self,
            Dataset::Xas(Box::new(merged)),
            format!("Canvas {} — {name}", self.doc.canvases.len() + 1),
            DEFAULT_CANVAS_SIZE_MM,
        );
        self.execute_action(action);
        Ok((selection.len(), alignment))
    }
}

#[cfg(test)]
#[path = "xas_merge_tests.rs"]
mod tests;
