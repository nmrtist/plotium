//! Numerical evidence from the existing per-column solvers.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ColumnDiagnostic {
    pub accepted: bool,
    pub reason: Option<String>,
    pub effective_points: usize,
    pub d: Option<f64>,
    pub i0: Option<f64>,
    pub sigma_d: Option<f64>,
    pub r_squared: Option<f64>,
    pub residual_sse: Option<f64>,
    pub residual_rmse: Option<f64>,
    pub unavailable_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub solver_termination: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub solver_iterations: Option<usize>,
}
impl ColumnDiagnostic {
    pub(crate) fn rejected(reason: &str, points: usize) -> Self {
        Self {
            reason: Some(reason.into()),
            effective_points: points,
            ..Self::default()
        }
    }
}
