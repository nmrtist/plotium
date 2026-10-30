//! Arithmetic and averaging of real-valued traces from non-NMR datasets.
use super::*;
use plotium_io::xps::{XpsEnergyKind, XpsExperiment, XpsRegion};
use plotium_processing::arithmetic::SpectrumBinaryOp;
use plotium_processing::signal::{self, Trace};

const OPERATION_ID: &str = "plotium.trace.arithmetic.v1";

impl PlotiumApp {
    fn operand_trace(&self, index: usize) -> Result<Trace1d, String> {
        let dataset = self
            .doc
            .datasets
            .get(index)
            .ok_or("The dataset is no longer available.")?;
        if matches!(dataset, Dataset::Nmr(_)) {
            return Err("Pick datasets of the same kind.".into());
        }
        let mut trace = dataset
            .displayed_trace(None)
            .ok_or("The dataset has no displayed trace.")?;
        // Blank table cells read as NaN and are not samples.
        let (xs, ys) = trace
            .xs
            .iter()
            .zip(&trace.ys)
            .filter(|(x, y)| x.is_finite() && y.is_finite())
            .map(|(x, y)| (*x, *y))
            .unzip();
        trace.xs = xs;
        trace.ys = ys;
        as_signal(&trace)
            .validate()
            .map_err(|error| error.to_string())?;
        Ok(trace)
    }

    /// Whether A × k and A + c can run on `a`.
    pub fn spectrum_scale_compat(&self, a: usize) -> Result<Option<String>, String> {
        match self.doc.datasets.get(a) {
            Some(Dataset::Nmr(_)) => Ok(None),
            _ => self.operand_trace(a).map(|_| None),
        }
    }

    fn same_kind(&self, a: usize, b: usize) -> Result<(), String> {
        let (left, right) = (&self.doc.datasets[a], &self.doc.datasets[b]);
        if std::mem::discriminant(left) != std::mem::discriminant(right) {
            return Err("Pick datasets of the same kind.".into());
        }
        if left.trace_x_unit() != right.trace_x_unit() {
            return Err(format!(
                "Axis units differ ({} vs {}).",
                left.trace_x_unit(),
                right.trace_x_unit()
            ));
        }
        match (left, right) {
            (Dataset::Xrd(left), Dataset::Xrd(right)) => {
                if let (Some(left), Some(right)) = (
                    left.data.wavelength_angstrom,
                    right.data.wavelength_angstrom,
                ) && (left - right).abs() > 1e-6
                {
                    return Err(format!("Wavelengths differ ({left} Å vs {right} Å)."));
                }
            }
            (Dataset::Xps(left), Dataset::Xps(right)) => {
                let binding = |xps: &XpsDataset| xps.active_region().binding_energy_ev.is_some();
                if binding(left) != binding(right) {
                    return Err("Pick regions on the same energy scale.".into());
                }
            }
            (Dataset::Table(left), Dataset::Table(right)) => {
                let (left, right) = (table_value_unit(left), table_value_unit(right));
                if left != right {
                    return Err(format!("Value units differ ({left} vs {right})."));
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Check that `a` and `b` can combine, returning both traces.
    fn operands(&self, a: usize, b: usize) -> Result<(Trace1d, Trace1d), String> {
        let (left, right) = (self.operand_trace(a)?, self.operand_trace(b)?);
        self.same_kind(a, b)?;
        check_overlap(&left, &right)?;
        Ok((left, right))
    }

    /// `Ok(Some(note))` when `b` is resampled onto `a`, `Ok(None)` when the
    /// grids match.
    pub(super) fn trace_arithmetic_compat(
        &self,
        a: usize,
        b: usize,
    ) -> Result<Option<String>, String> {
        let (left, right) = self.operands(a, b)?;
        Ok((!signal::same_grid(&left.xs, &right.xs)).then(|| {
            "Axes differ: the second trace is interpolated onto the first one's axis \
             (zero outside the overlap)."
                .to_owned()
        }))
    }

    pub(super) fn combine_trace_datasets(
        &mut self,
        a: usize,
        b: usize,
        op: SpectrumBinaryOp,
        k: f64,
    ) {
        let scale = match op {
            SpectrumBinaryOp::Add => k,
            SpectrumBinaryOp::Subtract => -k,
        };
        match self.combined_trace(a, b, scale) {
            Ok(trace) => {
                let name = self.arithmetic_name(a, b, op, k);
                self.insert_trace_dataset(a, trace, name, [a, b]);
            }
            Err(error) => self.session.status = error,
        }
    }

    fn combined_trace(&self, a: usize, b: usize, scale: f64) -> Result<Trace1d, String> {
        let (left, right) = self.operands(a, b)?;
        let ys = signal::linear_combination(as_signal(&left), as_signal(&right), scale)
            .map_err(|error| error.to_string())?;
        Ok(Trace1d { ys, ..left })
    }

    pub(super) fn scale_trace_dataset(&mut self, a: usize, scale: f64, offset: f64) {
        match self.operand_trace(a) {
            Ok(mut trace) => {
                trace
                    .ys
                    .iter_mut()
                    .for_each(|value| *value = *value * scale + offset);
                let name = self.scaled_name(a, scale, offset);
                self.insert_trace_dataset(a, trace, name, [a, a]);
            }
            Err(error) => self.session.status = error,
        }
    }

    /// Whether `sources` can be averaged onto the first one's grid.
    pub fn trace_average_compat(&self, sources: &[usize]) -> Result<Option<String>, String> {
        self.averaged_trace(sources).map(|_| None)
    }

    fn averaged_trace(&self, sources: &[usize]) -> Result<Trace1d, String> {
        let [first, rest @ ..] = sources else {
            return Err("Check at least one dataset to average with A.".into());
        };
        if rest.is_empty() {
            return Err("Check at least one dataset to average with A.".into());
        }
        if matches!(self.doc.datasets.get(*first), Some(Dataset::Nmr(_))) {
            return Err("Average works on XRD patterns, XPS regions and table curves.".into());
        }
        let operands = sources
            .iter()
            .map(|&index| self.operand_trace(index))
            .collect::<Result<Vec<_>, _>>()?;
        for (&other, trace) in rest.iter().zip(&operands[1..]) {
            self.same_kind(*first, other)?;
            check_overlap(&operands[0], trace)?;
        }
        let traces: Vec<_> = operands.iter().map(as_signal).collect();
        let ys = signal::average(&traces).map_err(|error| error.to_string())?;
        Ok(Trace1d {
            ys,
            xs: operands[0].xs.clone(),
            x_reversed: operands[0].x_reversed,
        })
    }

    pub fn average_trace_datasets(&mut self, sources: &[usize]) {
        match self.averaged_trace(sources) {
            Ok(trace) => {
                let first = sources[0];
                let name = format!(
                    "Average of {} (+{})",
                    self.doc.datasets[first].display_name(),
                    sources.len() - 1
                );
                self.insert_trace_dataset(first, trace, name, sources.iter().copied());
            }
            Err(error) => self.session.status = error,
        }
    }

    fn insert_trace_dataset(
        &mut self,
        template: usize,
        trace: Trace1d,
        name: String,
        sources: impl IntoIterator<Item = usize>,
    ) {
        let clipped = match &self.doc.datasets[template] {
            Dataset::Xrd(_) => trace.ys.iter().filter(|value| **value < 0.0).count(),
            _ => 0,
        };
        let mut dataset = match derived_dataset(&self.doc.datasets[template], trace, &name) {
            Ok(dataset) => dataset,
            Err(error) => {
                self.session.status = format!("Spectrum arithmetic failed: {error}");
                return;
            }
        };
        let sources = sources
            .into_iter()
            .filter_map(|index| self.doc.datasets.get(index).map(Dataset::resource_id))
            .collect::<Vec<_>>();
        dataset.set_name(Some(name.clone()));
        dataset.set_acquisition_identity(plotium_io::AcquisitionIdentity {
            subject: None,
            acquisition: None,
            source_label: name.clone(),
        });
        dataset.set_lineage(Some(DatasetLineage::new(
            DerivationKind::SpectrumArithmetic,
            sources,
        )));
        let action = Action::insert_dataset_with_default_canvas(
            self,
            dataset,
            format!("Canvas {} — {}", self.doc.canvases.len() + 1, name),
            DEFAULT_CANVAS_SIZE_MM,
        );
        self.execute_action(action);
        self.session.status = if clipped == 0 {
            format!("Created {name}.")
        } else {
            format!("Created {name}; {clipped} negative points were set to zero.")
        };
    }
}

fn as_signal(trace: &Trace1d) -> Trace<'_> {
    Trace {
        x: &trace.xs,
        y: &trace.ys,
    }
}

fn check_overlap(a: &Trace1d, b: &Trace1d) -> Result<(), String> {
    if signal::overlaps(as_signal(a), as_signal(b)) {
        Ok(())
    } else {
        Err("The traces do not overlap.".into())
    }
}

fn table_value_unit(table: &TableDataset) -> String {
    table
        .series_bindings
        .first()
        .and_then(|binding| {
            table
                .typed_state
                .envelope
                .revision
                .snapshot
                .schema
                .column(binding.value_column)
        })
        .and_then(|column| column.unit.as_ref())
        .map(|unit| unit.display_unit.clone())
        .unwrap_or_default()
}

/// A dataset of `template`'s kind holding `trace` as its raw signal.
fn derived_dataset(template: &Dataset, trace: Trace1d, name: &str) -> Result<Dataset, String> {
    if !trace.ys.iter().all(|value| value.is_finite()) {
        return Err("the result contains non-finite values".into());
    }
    match template {
        Dataset::Xrd(xrd) => {
            // XRD intensities are non-negative wherever they are stored.
            let data = plotium_io::XrdData {
                two_theta_deg: trace.xs,
                intensity: trace.ys.into_iter().map(|value| value.max(0.0)).collect(),
                attenuation: None,
                source: name.to_owned(),
                ..xrd.data.clone()
            };
            Ok(Dataset::Xrd(Box::new(XrdDataset::load(data))))
        }
        Dataset::Xps(xps) => {
            let region = xps.active_region();
            let binding = region.binding_energy_ev.is_some();
            let experiment = XpsExperiment {
                source: name.to_owned(),
                measurements: xps
                    .experiment
                    .measurements
                    .iter()
                    .filter(|measurement| measurement.id == region.measurement)
                    .cloned()
                    .collect(),
                regions: vec![XpsRegion {
                    native_energy_kind: if binding {
                        XpsEnergyKind::Binding
                    } else {
                        region.native_energy_kind
                    },
                    binding_energy_ev: binding.then(|| trace.xs.clone()),
                    native_energy_ev: trace.xs,
                    intensity_cps: trace.ys,
                    counts: None,
                    imported_fit: None,
                    ..region.clone()
                }],
                metadata: xps.experiment.metadata.clone(),
                import_warnings: Vec::new(),
            };
            Ok(Dataset::Xps(Box::new(XpsDataset::load(experiment))))
        }
        Dataset::Table(table) => {
            let schema = &table.typed_state.envelope.revision.snapshot.schema;
            let unit = template.trace_x_unit();
            let x_name = table
                .x_binding
                .and_then(|column| schema.column(column))
                .map_or_else(|| "x".to_owned(), |column| column.name.clone());
            let (x_schema, x) =
                materialized_float_column(x_name, &unit, trace.xs.into_iter().map(Some));
            let (y_schema, y) = materialized_float_column(name, "", trace.ys.into_iter().map(Some));
            let x_binding = x_schema.id;
            let series = TableSeriesBinding {
                value_column: y_schema.id,
                uncertainty_column: None,
                fit: None,
            };
            TableDataset::from_materialized(
                vec![(x_schema, x), (y_schema, y)],
                Vec::new(),
                Some(x_binding),
                vec![series],
                OPERATION_ID,
            )
            .map(|table| Dataset::Table(Box::new(table)))
            .map_err(|error| error.to_string())
        }
        _ => Err("this dataset kind has no trace arithmetic".into()),
    }
}
