use super::{
    AutobkParams, KTOE, PreEdge, PreEdgeParams, XafsWindow, XasError, XftfParams, autobk,
    check_transform, energy_to_k, is_positive, k_weighted, pre_edge, prepare_spectrum, xftf,
};

const MAX_NNORM: usize = 5;
const MAX_VICTOREEN: i32 = 4;
const MIN_RBKG: f64 = 0.1;
const MAX_RBKG: f64 = 5.0;
const MAX_K_WEIGHT: i32 = 3;
/// Below this photoelectron wavenumber a scan has no usable EXAFS region, as
/// in XANES-only measurements.
const MIN_EXAFS_K: f64 = 4.0;
/// Athena stops the post-edge line this far before the end of the scan, in eV.
const ATHENA_POST_EDGE_MARGIN: f64 = 100.0;
const ATHENA_POST_EDGE_START: f64 = 150.0;
/// A narrower quadratic post-edge line follows the EXAFS instead of the step, in eV.
const MIN_POST_EDGE_WIDTH: f64 = 300.0;

#[derive(Debug, Clone, Copy, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct XasProcessing {
    /// Added to the energy axis before any other step, in eV.
    pub energy_shift: f64,
    pub pre_edge: PreEdgeParams,
    pub autobk: AutobkParams,
    pub transform: XftfParams,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum XasPreset {
    Larch,
    Athena,
}

impl XasPreset {
    pub const ALL: [Self; 2] = [Self::Larch, Self::Athena];

    pub fn label(self) -> &'static str {
        match self {
            Self::Larch => "Larch defaults",
            Self::Athena => "Athena defaults",
        }
    }

    /// Keeps the energy shift, which belongs to the scan. `k_end` is the last
    /// k of the data: Athena stops its transform 2 Å⁻¹ before it.
    pub fn apply(self, current: &XasProcessing, k_end: Option<f64>) -> XasProcessing {
        let defaults = XasProcessing {
            energy_shift: current.energy_shift,
            ..XasProcessing::default()
        };
        match self {
            Self::Larch => defaults,
            Self::Athena => XasProcessing {
                pre_edge: PreEdgeParams {
                    pre1: Some(-150.0),
                    pre2: Some(-30.0),
                    ..athena_post_edge(k_end)
                },
                autobk: AutobkParams {
                    kweight: 2,
                    dk: 1.0,
                    nclamp: 5,
                    clamp_hi: 24.0,
                    ..AutobkParams::default()
                },
                transform: XftfParams {
                    kmin: 3.0,
                    kmax: k_end.map_or(defaults.transform.kmax, |k| (k - 2.0).max(4.0)),
                    window: XafsWindow::Hanning,
                    ..XftfParams::default()
                },
                ..defaults
            },
        }
    }
}

/// Athena's fixed post-edge range, when the scan is long enough to hold it.
/// A shorter scan keeps Larch's automatic range, which scales with the data.
fn athena_post_edge(k_end: Option<f64>) -> PreEdgeParams {
    let end = k_end.map(|k| KTOE * k * k - ATHENA_POST_EDGE_MARGIN);
    match end {
        Some(end) if end - ATHENA_POST_EDGE_START >= MIN_POST_EDGE_WIDTH => PreEdgeParams {
            norm1: Some(ATHENA_POST_EDGE_START),
            norm2: Some(end),
            nnorm: Some(2),
            ..PreEdgeParams::default()
        },
        _ => PreEdgeParams::default(),
    }
}

impl XasProcessing {
    /// A fixed E0 is a point on the energy axis, so it moves with the axis.
    pub fn with_energy_shift(mut self, shift: f64) -> Self {
        let delta = shift - self.energy_shift;
        self.pre_edge.e0 = self.pre_edge.e0.map(|e0| e0 + delta);
        self.energy_shift = shift;
        self
    }

    pub fn validate(&self) -> Result<(), XasError> {
        let invalid = |message: &str| Err(XasError::InvalidParameter(message.to_owned()));
        let (pre, bkg, ft) = (&self.pre_edge, &self.autobk, &self.transform);
        let finite = |value: Option<f64>| value.is_none_or(f64::is_finite);
        if !self.energy_shift.is_finite()
            || ![pre.e0, pre.pre1, pre.pre2, pre.norm1, pre.norm2]
                .into_iter()
                .all(finite)
        {
            return invalid("energies must be finite");
        }
        if let (Some(low), Some(high)) = (pre.pre1, pre.pre2)
            && low >= high
        {
            return invalid("the pre-edge range must increase");
        }
        if let (Some(low), Some(high)) = (pre.norm1, pre.norm2)
            && low >= high
        {
            return invalid("the post-edge range must increase");
        }
        if pre.step.is_some_and(|step| !is_positive(step)) {
            return invalid("the edge step must be positive");
        }
        if pre.nnorm.is_some_and(|order| order > MAX_NNORM) {
            return invalid("the post-edge polynomial order must be at most 5");
        }
        if !(-MAX_VICTOREEN..=MAX_VICTOREEN).contains(&pre.nvict) {
            return invalid("the Victoreen exponent must be between -4 and 4");
        }
        if !(MIN_RBKG..=MAX_RBKG).contains(&bkg.rbkg) {
            return invalid("Rbkg must be between 0.1 and 5 Å");
        }
        if !(0..=MAX_K_WEIGHT).contains(&bkg.kweight) || !(0..=MAX_K_WEIGHT).contains(&ft.kweight) {
            return invalid("k weights must be between 0 and 3");
        }
        let k_end = bkg.kmax.unwrap_or(f64::INFINITY);
        if !bkg.kmin.is_finite() || bkg.kmin < 0.0 || bkg.kmin >= k_end {
            return invalid("the background k range must increase from zero or above");
        }
        if !ft.kmin.is_finite() || ft.kmin < 0.0 || !ft.kmax.is_finite() || ft.kmin >= ft.kmax {
            return invalid("the k range must increase from zero or above");
        }
        let widths = [bkg.dk, ft.dk, ft.dk2];
        if !widths
            .iter()
            .all(|width| width.is_finite() && *width >= 0.0)
        {
            return invalid("window widths cannot be negative");
        }
        if !is_positive(ft.rmax_out) {
            return invalid("the output R range must be positive");
        }
        if !(bkg.clamp_lo.is_finite() && bkg.clamp_hi.is_finite())
            || bkg.clamp_lo < 0.0
            || bkg.clamp_hi < 0.0
        {
            return invalid("background clamps cannot be negative");
        }
        check_transform(bkg.kstep, bkg.nfft)?;
        check_transform(ft.kstep, ft.nfft)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Exafs {
    pub k: Vec<f64>,
    pub chi: Vec<f64>,
    /// `k^weight * chi` with the transform's weight.
    pub chi_weighted: Vec<f64>,
    pub background: Vec<f64>,
    pub r: Vec<f64>,
    pub chi_r_magnitude: Vec<f64>,
}

/// Each stage needs the one before it, so a stage that cannot run leaves the
/// later ones empty and records why in `issues`.
#[derive(Debug, Clone, PartialEq)]
pub struct XasReduction {
    pub energy: Vec<f64>,
    pub mu: Vec<f64>,
    pub edge: Option<PreEdge>,
    pub exafs: Option<Exafs>,
    pub issues: Vec<String>,
}

pub fn reduce(
    energy: &[f64],
    mu: &[f64],
    params: &XasProcessing,
) -> Result<XasReduction, XasError> {
    let shifted: Vec<f64> = energy.iter().map(|e| e + params.energy_shift).collect();
    let (energy, mu) = prepare_spectrum(&shifted, mu)?;
    let mut issues = Vec::new();
    let dropped = shifted.len() - energy.len();
    if dropped > 0 {
        let rows = if dropped == 1 { "row" } else { "rows" };
        issues.push(format!(
            "Dropped {dropped} {rows} with a repeated energy or an unreadable signal."
        ));
    }
    let edge = pre_edge(&energy, &mu, &params.pre_edge)
        .map_err(|error| issues.push(format!("Normalization: {error}")))
        .ok();
    if let (Some(e0), Some(first), Some(last)) = (params.pre_edge.e0, energy.first(), energy.last())
        && !(*first..=*last).contains(&e0)
    {
        issues.push(format!("E0 = {e0:.1} eV is outside the scan."));
    }
    if let (Some(edge), Some(last)) = (&edge, energy.last())
        && params
            .pre_edge
            .norm1
            .is_some_and(|norm1| edge.e0 + norm1 >= *last)
    {
        issues.push("The post-edge range starts past the end of the scan.".to_owned());
    }
    let exafs = edge.as_ref().and_then(|edge| {
        let reach = energy.last().map_or(0.0, |last| {
            energy_to_k(std::slice::from_ref(last), edge.e0)[0]
        });
        if reach < MIN_EXAFS_K {
            issues.push(format!(
                "The scan ends at k = {reach:.1} 1/Å, too short for EXAFS."
            ));
            return None;
        }
        if params.transform.kmin >= reach {
            issues.push(format!(
                "The Fourier transform starts at k = {:.1} 1/Å, past the end of the data.",
                params.transform.kmin
            ));
        }
        extract(&energy, &mu, edge, params)
            .map_err(|error| issues.push(format!("EXAFS: {error}")))
            .ok()
    });
    Ok(XasReduction {
        energy,
        mu,
        edge,
        exafs,
        issues,
    })
}

fn extract(
    energy: &[f64],
    mu: &[f64],
    edge: &PreEdge,
    params: &XasProcessing,
) -> Result<Exafs, XasError> {
    let background = autobk(energy, mu, edge.e0, edge.edge_step, &params.autobk)?;
    let transform = xftf(&background.k, &background.chi, &params.transform)?;
    let chi_weighted = k_weighted(&background.k, &background.chi, params.transform.kweight)?;
    Ok(Exafs {
        k: background.k,
        chi: background.chi,
        chi_weighted,
        background: background.background,
        r: transform.r,
        chi_r_magnitude: transform.magnitude,
    })
}
