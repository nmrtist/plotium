use super::{
    AutobkParams, PreEdge, PreEdgeParams, XasError, XftfParams, autobk, energy_to_k, k_weighted,
    pre_edge, prepare_spectrum, xftf,
};

/// Below this photoelectron wavenumber a scan has no usable EXAFS region, as
/// in XANES-only measurements.
const MIN_EXAFS_K: f64 = 4.0;

#[derive(Debug, Clone, Copy, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct XasProcessing {
    /// Added to the energy axis before any other step, in eV.
    pub energy_shift: f64,
    pub pre_edge: PreEdgeParams,
    pub autobk: AutobkParams,
    pub transform: XftfParams,
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
    let exafs = edge.as_ref().and_then(|edge| {
        let reach = energy.last().map_or(0.0, |last| {
            energy_to_k(std::slice::from_ref(last), edge.e0)[0]
        });
        if reach < MIN_EXAFS_K {
            issues.push(format!(
                "The scan ends at k = {reach:.1} Å⁻¹, too short for EXAFS."
            ));
            return None;
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
