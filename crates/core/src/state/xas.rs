use super::{
    DatasetId, DatasetLineage, FieldCapabilities, FieldCatalog, FieldDescriptor, FieldId,
    FieldMetadata,
};
use crate::automation::{CAP_FIELD_CURVE_1D, CAP_FIELD_XAS_SPECTRUM, CapabilityId};
use crate::state::scientific_summary::SummaryPart;
use plotium_figure::{Axis, Figure, Series};
use plotium_io::xas::{XasColumnId, XasScan};
use plotium_processing::xas::{
    self as processing, XasError, XasPreset, XasProcessing, XasReduction, fluorescence_mu, reduce,
    reference_mu, transmission_mu,
};
use std::collections::BTreeMap;

const MU: &str = "xas.mu";
pub(super) const NORM: &str = "xas.norm";
const FLAT: &str = "xas.flat";
const CHI_K: &str = "xas.chik";
pub(super) const CHI_R: &str = "xas.chir";
const REFERENCE: &str = "xas.reference";
pub(super) const SUMMARY_FIELD_KEYS: [&str; 3] = [NORM, CHI_K, CHI_R];
pub(super) const XAS_FIELD_KEYS: [&str; 6] = [NORM, MU, FLAT, CHI_K, CHI_R, REFERENCE];

/// A transmission edge step below this is too weak to measure the sample, so a
/// file that also records another signal was measured in that signal.
const MIN_TRANSMISSION_STEP: f64 = 0.1;
/// A smaller step in the reference channel means no foil was in the beam.
const MIN_REFERENCE_STEP: f64 = 0.1;
/// Points on each side of an edge used to judge the energy step there.
const EDGE_STEP_POINTS: usize = 3;
/// Beyond this the foil's edge cannot be located well enough to shift by it, in eV.
const MAX_CALIBRATION_STEP: f64 = 1.0;
/// Energies from here on read better in full than with a ×10⁴ multiplier.
const THOUSANDS_FROM: f64 = 10_000.0;
/// Bonds sit inside this range; the transform itself runs further.
const DEFAULT_R_MAX: f64 = 6.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum XasSignal {
    Transmission {
        i0: XasColumnId,
        transmitted: XasColumnId,
    },
    Fluorescence {
        signal: XasColumnId,
        i0: XasColumnId,
    },
    Absorption {
        column: XasColumnId,
    },
}

impl XasSignal {
    pub fn label(self) -> &'static str {
        match self {
            Self::Transmission { .. } => "transmission",
            Self::Fluorescence { .. } => "fluorescence",
            Self::Absorption { .. } => "absorption",
        }
    }
}

fn missing_column() -> XasError {
    XasError::InvalidParameter("the signal names a column the scan does not have".into())
}

pub fn signal_mu(scan: &XasScan, signal: XasSignal) -> Result<Vec<f64>, XasError> {
    let column = |id| scan.column(id).ok_or_else(missing_column);
    match signal {
        XasSignal::Transmission { i0, transmitted } => {
            transmission_mu(column(i0)?, column(transmitted)?)
        }
        XasSignal::Fluorescence { signal, i0 } => fluorescence_mu(column(signal)?, column(i0)?),
        XasSignal::Absorption { column: id } => column(id).map(<[f64]>::to_vec),
    }
}

/// The channel a scan was most likely measured in. `None` when the file names
/// no measured signal, which the dataset reports instead of guessing a column.
pub fn default_signal(scan: &XasScan) -> Option<XasSignal> {
    let channels = &scan.channels;
    let transmission = channels
        .i0
        .zip(channels.transmission)
        .map(|(i0, transmitted)| XasSignal::Transmission { i0, transmitted });
    let fluorescence = channels
        .i0
        .zip(channels.fluorescence.first().copied())
        .map(|(i0, signal)| XasSignal::Fluorescence { signal, i0 });
    let absorption = channels.mu.map(|column| XasSignal::Absorption { column });
    match (transmission, fluorescence.or(absorption)) {
        (Some(transmission), Some(other)) => {
            Some(if edge_step(scan, transmission) < MIN_TRANSMISSION_STEP {
                other
            } else {
                transmission
            })
        }
        (transmission, other) => transmission.or(other),
    }
}

fn edge_step(scan: &XasScan, signal: XasSignal) -> f64 {
    signal_mu(scan, signal)
        .and_then(|mu| {
            let (energy, mu) = processing::prepare_spectrum(scan.energy(), &mu)?;
            processing::pre_edge(&energy, &mu, &Default::default())
        })
        .map_or(0.0, |edge| edge.edge_step)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReferenceEdge {
    pub e0: f64,
    pub step: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct XasCalibration {
    pub element: &'static str,
    pub edge: &'static str,
    pub energy: f64,
    pub shift: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum XasCalibrationError {
    NoReference,
    /// The reference channel shows no edge, as with no foil in the beam.
    NoReferenceEdge,
    /// The mean energy step around the foil's edge, in eV.
    CoarseReference(f64),
}

impl XasCalibrationError {
    pub fn reason(self) -> String {
        match self {
            Self::NoReference => "a scan has no reference channel".to_owned(),
            Self::NoReferenceEdge => "a reference channel shows no edge".to_owned(),
            Self::CoarseReference(step) => {
                format!("the reference steps {step:.1} eV at its edge, too coarse to align on")
            }
        }
    }
}

#[derive(Clone)]
pub struct XasDataset {
    pub resource_id: DatasetId,
    pub field_catalog: FieldCatalog,
    pub acquisition_identity: plotium_io::AcquisitionIdentity,
    pub scan: XasScan,
    pub signal: Option<XasSignal>,
    pub params: XasProcessing,
    pub processed: XasReduction,
    pub reference: Option<XasReduction>,
    pub reference_edge: Option<ReferenceEdge>,
    pub name: Option<String>,
    pub lineage: Option<DatasetLineage>,
}

impl XasDataset {
    pub fn load(scan: XasScan) -> Self {
        let acquisition_identity =
            plotium_io::AcquisitionIdentity::from_path(std::path::Path::new(&scan.source));
        let signal = default_signal(&scan);
        let params = default_processing(&scan, signal);
        let mut field_catalog = FieldCatalog::for_keys(XAS_FIELD_KEYS.map(str::to_owned));
        field_catalog.attach_provenance(&scan.source, None);
        let (processed, reference) = run(&scan, signal, &params);
        let reference_edge = measure_reference(reference.as_ref());
        Self {
            resource_id: DatasetId::new(),
            field_catalog,
            acquisition_identity,
            scan,
            signal,
            params,
            processed,
            reference,
            reference_edge,
            name: None,
            lineage: None,
        }
    }

    pub fn apply_processing(&mut self, signal: Option<XasSignal>, params: XasProcessing) {
        (self.processed, self.reference) = run(&self.scan, signal, &params);
        self.reference_edge = measure_reference(self.reference.as_ref());
        self.signal = signal;
        self.params = params;
    }

    pub fn rebuild(&mut self) {
        self.apply_processing(self.signal, self.params);
    }

    /// Why a stage produced nothing, for the status line and diagnostics.
    pub fn issues(&self) -> &[String] {
        &self.processed.issues
    }

    /// The last k of the extracted chi(k).
    pub fn k_end(&self) -> Option<f64> {
        self.processed.exafs.as_ref()?.k.last().copied()
    }

    /// Where the reference foil's edge is, when it can be located well enough to
    /// move the energy axis by it.
    pub(super) fn foil_edge(&self) -> Result<f64, XasCalibrationError> {
        let reference = self
            .reference
            .as_ref()
            .ok_or(XasCalibrationError::NoReference)?;
        let foil = self
            .reference_edge
            .filter(|foil| foil.step >= MIN_REFERENCE_STEP)
            .ok_or(XasCalibrationError::NoReferenceEdge)?;
        let index = reference.energy.partition_point(|e| *e < foil.e0);
        let start = index.saturating_sub(EDGE_STEP_POINTS);
        let stop = (index + EDGE_STEP_POINTS).min(reference.energy.len() - 1);
        let step = (reference.energy[stop] - reference.energy[start]) / (stop - start) as f64;
        if step > MAX_CALIBRATION_STEP {
            return Err(XasCalibrationError::CoarseReference(step));
        }
        Ok(foil.e0)
    }

    /// The shift that puts the reference foil's edge on its tabulated energy.
    /// A sample's own edge is chemically shifted, so only a foil can calibrate.
    pub fn calibration(&self) -> Result<XasCalibration, XasCalibrationError> {
        let measured = self.foil_edge()?;
        let (element, edge, energy) =
            processing::nearest_edge(measured).ok_or(XasCalibrationError::NoReference)?;
        Ok(XasCalibration {
            element,
            edge,
            energy,
            shift: self.params.energy_shift + energy - measured,
        })
    }

    pub(super) fn field_for_curve(&self, key: &str) -> Option<FieldId> {
        self.field_id(key)
            .filter(|id| self.field_values(*id).is_some())
    }

    /// Every stage side by side: the energy, k and R columns have their own lengths.
    pub(crate) fn export_columns(&self) -> Vec<(&'static str, Vec<f64>)> {
        let reduction = &self.processed;
        let mut columns = vec![
            ("energy_ev", reduction.energy.clone()),
            ("mu", reduction.mu.clone()),
        ];
        if let Some(reference) = &self.reference {
            columns.push(("reference_energy_ev", reference.energy.clone()));
            columns.push(("reference_mu", reference.mu.clone()));
        }
        if let Some(edge) = &reduction.edge {
            columns.push(("norm", edge.norm.clone()));
            columns.push(("flat", edge.flat.clone()));
        }
        if let Some(exafs) = &reduction.exafs {
            columns.push(("background", exafs.background.clone()));
            columns.push(("k_per_angstrom", exafs.k.clone()));
            columns.push(("chi", exafs.chi.clone()));
            columns.push(("chi_k_weighted", exafs.chi_weighted.clone()));
            columns.push(("r_angstrom", exafs.r.clone()));
            columns.push(("chi_r_magnitude", exafs.chi_r_magnitude.clone()));
        }
        columns
    }

    pub fn edge_step(&self) -> Option<f64> {
        self.processed.edge.as_ref().map(|edge| edge.edge_step)
    }

    /// The column ids a signal can name, with the labels the file gave them.
    pub fn column_summary(&self) -> Vec<(XasColumnId, &str)> {
        self.scan
            .columns
            .iter()
            .enumerate()
            .map(|(index, column)| (XasColumnId(index as u32), column.name.as_str()))
            .collect()
    }

    /// `settings` from a scheme, on this scan's own energy axis. Ranges measured
    /// on a longer scan would fit nothing here, so this scan keeps its own.
    pub fn adopt_scheme(&self, settings: XasProcessing) -> XasProcessing {
        let own = &self.params;
        let reach = self.edge_e0().and_then(|e0| {
            let last = self.processed.energy.last()?;
            Some(last - e0)
        });
        let mut pre_edge = settings.pre_edge;
        pre_edge.e0 = own.pre_edge.e0;
        if reach.is_some_and(|reach| pre_edge.norm2.is_some_and(|norm2| norm2 > reach)) {
            pre_edge.norm1 = own.pre_edge.norm1;
            pre_edge.norm2 = own.pre_edge.norm2;
            pre_edge.nnorm = own.pre_edge.nnorm;
        }
        let mut transform = settings.transform;
        if self.k_end().is_some_and(|end| transform.kmax > end) {
            transform.kmax = own.transform.kmax;
        }
        XasProcessing {
            energy_shift: own.energy_shift,
            pre_edge,
            transform,
            ..settings
        }
    }

    pub fn edge_e0(&self) -> Option<f64> {
        self.processed.edge.as_ref().map(|edge| edge.e0)
    }

    pub fn has_spectrum(&self) -> bool {
        !self.processed.mu.is_empty()
    }

    pub fn field_id(&self, key: &str) -> Option<FieldId> {
        self.field_catalog.id_for_key(key)
    }

    /// The field's curve and labels, when that stage has data.
    fn curve(&self, id: FieldId) -> Option<Curve<'_>> {
        let key = self.field_catalog.key_for_id(id)?;
        let reduction = &self.processed;
        let edge = reduction.edge.as_ref();
        let exafs = reduction.exafs.as_ref();
        let weight = self.params.transform.kweight;
        let energy = || &reduction.energy;
        Some(match key {
            MU if !reduction.mu.is_empty() => Curve {
                key,
                name: "μ(E)",
                x: energy(),
                y: &reduction.mu,
                x_label: "Energy (eV)".into(),
                y_label: "μ(E)".into(),
                x_unit: "eV",
                y_unit: "a.u.".into(),
            },
            NORM => Curve {
                key,
                name: "Normalized μ(E)",
                x: energy(),
                y: &edge?.norm,
                x_label: "Energy (eV)".into(),
                y_label: "Normalized μ(E)".into(),
                x_unit: "eV",
                y_unit: "a.u.".into(),
            },
            FLAT => Curve {
                key,
                name: "Flattened μ(E)",
                x: energy(),
                y: &edge?.flat,
                x_label: "Energy (eV)".into(),
                y_label: "Flattened μ(E)".into(),
                x_unit: "eV",
                y_unit: "a.u.".into(),
            },
            CHI_K => Curve {
                key,
                name: "χ(k)",
                x: &exafs?.k,
                y: &exafs?.chi_weighted,
                x_label: "k (Å⁻¹)".into(),
                y_label: weighted_label(weight),
                x_unit: "Å⁻¹",
                y_unit: "a.u.".into(),
            },
            CHI_R => Curve {
                key,
                name: "|χ(R)|",
                x: &exafs?.r,
                y: &exafs?.chi_r_magnitude,
                x_label: "R (Å)".into(),
                y_label: format!("|χ(R)| (Å{})", superscript(-(weight + 1))),
                x_unit: "Å",
                y_unit: "a.u.".into(),
            },
            REFERENCE => {
                let reference = self.reference.as_ref()?;
                Curve {
                    key,
                    name: "Reference μ(E)",
                    x: &reference.energy,
                    y: &reference.mu,
                    x_label: "Energy (eV)".into(),
                    y_label: "Reference μ(E)".into(),
                    x_unit: "eV",
                    y_unit: "a.u.".into(),
                }
            }
            _ => return None,
        })
    }

    pub(super) fn field_values(&self, id: FieldId) -> Option<(&[f64], &[f64])> {
        self.curve(id).map(|curve| (curve.x, curve.y))
    }

    /// The values of the field shown by default.
    pub fn default_values(&self) -> &[f64] {
        self.default_trace().map_or(&[], |(_, values)| values)
    }

    pub(super) fn default_trace(&self) -> Option<(&[f64], &[f64])> {
        self.field_descriptors()
            .first()
            .and_then(|field| self.field_values(field.id))
    }

    pub fn figure(&self) -> Option<Figure> {
        self.field_descriptors()
            .first()
            .and_then(|field| self.field_figure(field.id))
    }

    pub(super) fn field_figure(&self, id: FieldId) -> Option<Figure> {
        let curve = self.curve(id)?;
        let (x0, x1) = if curve.key == CHI_R {
            (0.0, DEFAULT_R_MAX)
        } else {
            bounds(curve.x)
        };
        let (y0, y1) = bounds(curve.y);
        let mut x_axis = Axis::new(curve.x_label, x0, x1);
        x_axis.thousands_separator = x1 >= THOUSANDS_FROM;
        Some(
            Figure::new("", x_axis, Axis::new(curve.y_label, y0, y1)).with_series(Series::line(
                curve.name,
                curve.x.iter().zip(curve.y).map(|(&x, &y)| [x, y]).collect(),
            )),
        )
    }

    pub(crate) fn field_descriptors(&self) -> Vec<FieldDescriptor> {
        XAS_FIELD_KEYS
            .iter()
            .filter_map(|key| {
                let id = self.field_id(key)?;
                let curve = self.curve(id)?;
                let descriptor = FieldDescriptor {
                    id,
                    local_id: curve.key.to_owned(),
                    name: curve.name.to_owned(),
                    scientific_observation: SummaryPart::new(
                        format!("xas:{}", curve.key),
                        format!("XAS {}", curve.name),
                    ),
                    capabilities: FieldCapabilities::new([
                        CapabilityId::new(CAP_FIELD_CURVE_1D),
                        CapabilityId::new(CAP_FIELD_XAS_SPECTRUM),
                    ]),
                    dimensions: vec![curve.x.len()],
                    units: vec![curve.x_unit.to_owned(), curve.y_unit],
                    metadata: FieldMetadata(BTreeMap::from([(
                        "recommended_encoding".to_owned(),
                        "line".to_owned(),
                    )])),
                };
                Some(descriptor.with_line_x_unit(curve.x_unit))
            })
            .collect()
    }
}

struct Curve<'a> {
    key: &'a str,
    name: &'static str,
    x: &'a [f64],
    y: &'a [f64],
    x_label: String,
    y_label: String,
    x_unit: &'static str,
    y_unit: String,
}

/// χ(k) is dimensionless, so its k weight sets the unit.
fn weighted_label(weight: i32) -> String {
    match weight {
        0 => "χ(k)".to_owned(),
        1 => "kχ(k) (Å⁻¹)".to_owned(),
        weight => format!("k{}χ(k) (Å{})", superscript(weight), superscript(-weight)),
    }
}

fn superscript(value: i32) -> String {
    const DIGITS: [char; 10] = ['⁰', '¹', '²', '³', '⁴', '⁵', '⁶', '⁷', '⁸', '⁹'];
    let digits = value.unsigned_abs().to_string();
    let sign = (value < 0).then_some('⁻');
    sign.into_iter()
        .chain(
            digits
                .bytes()
                .map(|digit| DIGITS[usize::from(digit - b'0')]),
        )
        .collect()
}

fn run(
    scan: &XasScan,
    signal: Option<XasSignal>,
    params: &XasProcessing,
) -> (XasReduction, Option<XasReduction>) {
    let shown = signal.map_or_else(
        || {
            Err(XasError::InvalidParameter(
                "the scan has no signal column".into(),
            ))
        },
        |signal| signal_mu(scan, signal),
    );
    let processed = shown
        .and_then(|mu| reduce(scan.energy(), &mu, params))
        .unwrap_or_else(unprocessed);
    let reference = scan
        .channels
        .transmission
        .zip(scan.channels.reference)
        .and_then(|(transmitted, reference)| {
            let mu = reference_mu(scan.column(transmitted)?, scan.column(reference)?).ok()?;
            let (energy, mu) = processing::prepare_spectrum(scan.energy(), &mu).ok()?;
            let energy = energy.iter().map(|e| e + params.energy_shift).collect();
            Some(XasReduction {
                energy,
                mu,
                edge: None,
                exafs: None,
                issues: Vec::new(),
            })
        });
    (processed, reference)
}

/// Athena's settings: a transform that starts past the low-k background and
/// stops before the noisy end of the data, which is what puts peaks where a
/// paper expects them. Its k range depends on where the first pass ends.
pub fn default_processing(scan: &XasScan, signal: Option<XasSignal>) -> XasProcessing {
    let first = XasPreset::Athena.apply(&XasProcessing::default(), None);
    let k_end = run(scan, signal, &first)
        .0
        .exafs
        .and_then(|exafs| exafs.k.last().copied());
    XasPreset::Athena.apply(&first, k_end)
}

fn measure_reference(reference: Option<&XasReduction>) -> Option<ReferenceEdge> {
    let reference = reference?;
    let edge = processing::pre_edge(&reference.energy, &reference.mu, &Default::default()).ok()?;
    Some(ReferenceEdge {
        e0: edge.e0,
        step: edge.edge_step,
    })
}

fn unprocessed(error: XasError) -> XasReduction {
    XasReduction {
        energy: Vec::new(),
        mu: Vec::new(),
        edge: None,
        exafs: None,
        issues: vec![error.to_string()],
    }
}

fn bounds(values: &[f64]) -> (f64, f64) {
    let low = values.iter().copied().fold(f64::INFINITY, f64::min);
    let high = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    if low.is_finite() && high.is_finite() && low < high {
        (low, high)
    } else {
        (0.0, 1.0)
    }
}

#[cfg(test)]
#[path = "xas_tests.rs"]
mod tests;
