use crate::signal;

pub const MAX_SNIP_ITERATIONS: u16 = 200;
pub const MAX_SAVGOL_WINDOW: u16 = 101;
pub const MAX_SAVGOL_ORDER: u8 = 6;
pub const CU_KALPHA1_ANGSTROM: f64 = 1.540593;
pub const MAX_WAVELENGTH_ANGSTROM: f64 = 10.0;
pub const MIN_WAVELENGTH_ANGSTROM: f64 = 1e-3;

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct XrdProcessing {
    pub background: Option<SnipBackground>,
    pub smoothing: Option<SavitzkyGolay>,
    pub normalization: XrdNormalization,
    #[serde(default)]
    pub axis: XrdAxis,
    /// Replaces the wavelength the file declares, or supplies a missing one.
    #[serde(default)]
    pub wavelength_angstrom: Option<f64>,
}

impl Default for XrdProcessing {
    fn default() -> Self {
        Self {
            background: None,
            smoothing: None,
            normalization: XrdNormalization::None,
            axis: XrdAxis::TwoTheta,
            wavelength_angstrom: None,
        }
    }
}

/// The ruler a pattern is displayed on. The stored pattern stays in 2theta, so
/// processing does not depend on this choice.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum XrdAxis {
    #[default]
    TwoTheta,
    Q,
    D,
    CuKAlpha,
}

impl XrdAxis {
    pub const ALL: [Self; 4] = [Self::TwoTheta, Self::Q, Self::D, Self::CuKAlpha];

    pub fn label(self) -> &'static str {
        match self {
            Self::TwoTheta => "2theta (deg)",
            Self::Q => "Q (Å⁻¹)",
            Self::D => "d (Å)",
            Self::CuKAlpha => "2theta, Cu Kα (deg)",
        }
    }

    pub fn menu_label(self) -> &'static str {
        match self {
            Self::TwoTheta => "2theta",
            Self::Q => "Q",
            Self::D => "d spacing",
            Self::CuKAlpha => "2theta at Cu K-alpha",
        }
    }

    pub fn unit(self) -> &'static str {
        match self {
            Self::TwoTheta | Self::CuKAlpha => "deg",
            Self::Q => "Å⁻¹",
            Self::D => "Å",
        }
    }

    pub fn needs_wavelength(self) -> bool {
        self != Self::TwoTheta
    }

    /// The 2theta of a reflection at `position` on this axis, the inverse of
    /// [`Self::position`].
    pub fn two_theta(self, position: f64, wavelength: f64) -> Option<f64> {
        let sine = match self {
            Self::TwoTheta => return Some(position),
            Self::Q => position * wavelength / (4.0 * std::f64::consts::PI),
            Self::D => wavelength / (2.0 * position),
            Self::CuKAlpha => {
                (position / 2.0).to_radians().sin() * wavelength / CU_KALPHA1_ANGSTROM
            }
        };
        ((0.0..=1.0).contains(&sine)).then(|| 2.0 * sine.asin().to_degrees())
    }

    /// The position of a reflection at `two_theta_deg`, or `None` when Cu K-alpha
    /// could not reach it. `wavelength` is in Å.
    pub fn position(self, two_theta_deg: f64, wavelength: f64) -> Option<f64> {
        let sine = (two_theta_deg / 2.0).to_radians().sin();
        match self {
            Self::TwoTheta => Some(two_theta_deg),
            Self::Q => Some(4.0 * std::f64::consts::PI * sine / wavelength),
            Self::D => (sine > 0.0).then(|| wavelength / (2.0 * sine)),
            Self::CuKAlpha => {
                let scaled = sine * CU_KALPHA1_ANGSTROM / wavelength;
                (scaled <= 1.0).then(|| 2.0 * scaled.asin().to_degrees())
            }
        }
    }
}

/// `pattern` on `axis`, increasing along x. d runs opposite to 2theta.
pub fn on_axis(
    two_theta_deg: &[f64],
    intensity: &[f64],
    axis: XrdAxis,
    wavelength: f64,
) -> (Vec<f64>, Vec<f64>) {
    let mut points: Vec<(f64, f64)> = two_theta_deg
        .iter()
        .zip(intensity)
        .filter_map(|(angle, value)| Some((axis.position(*angle, wavelength)?, *value)))
        .collect();
    if axis == XrdAxis::D {
        points.reverse();
    }
    points.into_iter().unzip()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SnipBackground {
    pub iterations: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SavitzkyGolay {
    pub window: u16,
    pub polynomial_order: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum XrdNormalization {
    None,
    Maximum,
    Area,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProcessedXrd {
    pub intensity: Vec<f64>,
    pub background: Option<Vec<f64>>,
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum XrdProcessingError {
    #[error("2theta and intensity lengths differ")]
    LengthMismatch,
    #[error("SNIP background iterations must be between 1 and {MAX_SNIP_ITERATIONS}")]
    InvalidBackground,
    #[error(
        "Savitzky-Golay window must be odd, between 3 and {MAX_SAVGOL_WINDOW}, and longer than a polynomial order no greater than {MAX_SAVGOL_ORDER}"
    )]
    InvalidSmoothing,
    #[error("the wavelength must be between 0.001 and {MAX_WAVELENGTH_ANGSTROM} Å")]
    InvalidWavelength,
    #[error("enter the wavelength to show this pattern on Q, d or Cu K-alpha 2theta")]
    MissingWavelength,
    #[error("XRD processing produced a non-finite numeric result")]
    NonFiniteResult,
}

pub fn process(
    two_theta_deg: &[f64],
    intensity: &[f64],
    params: XrdProcessing,
) -> Result<ProcessedXrd, XrdProcessingError> {
    if two_theta_deg.len() != intensity.len() {
        return Err(XrdProcessingError::LengthMismatch);
    }
    validate(params)?;
    let (mut values, background) = match params.background {
        Some(settings) => {
            let background = signal::snip_background(intensity, settings.iterations);
            let corrected = intensity
                .iter()
                .zip(&background)
                .map(|(observed, baseline)| (observed - baseline).max(0.0))
                .collect();
            (corrected, Some(background))
        }
        None => (intensity.to_vec(), None),
    };
    if let Some(settings) = params.smoothing {
        values = savitzky_golay(&values, settings)?;
    }
    normalize(two_theta_deg, &mut values, params.normalization);
    if !values.iter().all(|value| value.is_finite())
        || background
            .as_ref()
            .is_some_and(|values| !values.iter().all(|value| value.is_finite()))
    {
        return Err(XrdProcessingError::NonFiniteResult);
    }
    Ok(ProcessedXrd {
        intensity: values,
        background,
    })
}

pub fn validate(params: XrdProcessing) -> Result<(), XrdProcessingError> {
    if params
        .wavelength_angstrom
        .is_some_and(|value| !(MIN_WAVELENGTH_ANGSTROM..=MAX_WAVELENGTH_ANGSTROM).contains(&value))
    {
        return Err(XrdProcessingError::InvalidWavelength);
    }
    if params.background.is_some_and(|settings| {
        settings.iterations == 0 || settings.iterations > MAX_SNIP_ITERATIONS
    }) {
        return Err(XrdProcessingError::InvalidBackground);
    }
    if params.smoothing.is_some_and(|settings| {
        settings.window < 3
            || settings.window > MAX_SAVGOL_WINDOW
            || settings.window % 2 == 0
            || settings.polynomial_order > MAX_SAVGOL_ORDER
            || settings.polynomial_order >= settings.window as u8
    }) {
        return Err(XrdProcessingError::InvalidSmoothing);
    }
    Ok(())
}

fn savitzky_golay(input: &[f64], settings: SavitzkyGolay) -> Result<Vec<f64>, XrdProcessingError> {
    let window = usize::from(settings.window);
    if input.len() < window {
        return Ok(input.to_vec());
    }
    signal::savitzky_golay(
        input,
        window,
        usize::from(settings.polynomial_order),
        signal::SavitzkyGolayEdges::RepeatEndpoints,
    )
    .ok_or(XrdProcessingError::InvalidSmoothing)
}

fn normalize(x: &[f64], values: &mut [f64], method: XrdNormalization) {
    let divisor = match method {
        XrdNormalization::None => return,
        XrdNormalization::Maximum => signal::maximum(values),
        XrdNormalization::Area => signal::trapezoid_area(x, values),
    };
    if divisor.is_finite() && divisor > 0.0 {
        values.iter_mut().for_each(|value| *value /= divisor);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maximum_normalization_scales_peak_to_one() {
        let result = process(
            &[1.0, 2.0, 3.0],
            &[2.0, 4.0, 1.0],
            XrdProcessing {
                normalization: XrdNormalization::Maximum,
                ..XrdProcessing::default()
            },
        )
        .unwrap();
        assert_eq!(result.intensity, vec![0.5, 1.0, 0.25]);
    }

    #[test]
    fn smoothing_preserves_a_quadratic_interior() {
        let values = vec![4.0, 1.0, 0.0, 1.0, 4.0];
        let result = savitzky_golay(
            &values,
            SavitzkyGolay {
                window: 5,
                polynomial_order: 2,
            },
        )
        .unwrap();
        assert!(result[2].abs() < 1e-10);
    }

    #[test]
    fn rejects_zero_background_iterations() {
        let error = process(
            &[1.0, 2.0, 3.0],
            &[2.0, 4.0, 1.0],
            XrdProcessing {
                background: Some(SnipBackground { iterations: 0 }),
                ..XrdProcessing::default()
            },
        )
        .unwrap_err();

        assert_eq!(error, XrdProcessingError::InvalidBackground);
    }

    #[test]
    fn rejects_processing_parameters_above_supported_limits() {
        let background = process(
            &[1.0, 2.0, 3.0],
            &[2.0, 4.0, 1.0],
            XrdProcessing {
                background: Some(SnipBackground {
                    iterations: MAX_SNIP_ITERATIONS + 1,
                }),
                ..XrdProcessing::default()
            },
        )
        .unwrap_err();
        assert_eq!(background, XrdProcessingError::InvalidBackground);

        let smoothing = process(
            &[1.0, 2.0, 3.0],
            &[2.0, 4.0, 1.0],
            XrdProcessing {
                smoothing: Some(SavitzkyGolay {
                    window: MAX_SAVGOL_WINDOW + 2,
                    polynomial_order: 3,
                }),
                ..XrdProcessing::default()
            },
        )
        .unwrap_err();
        assert_eq!(smoothing, XrdProcessingError::InvalidSmoothing);
    }

    #[test]
    fn rejects_non_finite_results() {
        let error = process(
            &[1.0, 2.0, 3.0],
            &[2.0, f64::INFINITY, 1.0],
            XrdProcessing::default(),
        )
        .unwrap_err();

        assert_eq!(error, XrdProcessingError::NonFiniteResult);
    }
}

#[cfg(test)]
mod axis_tests {
    use super::*;

    /// LaB6 (100), a = 4.15689 Å, at a synchrotron wavelength.
    const D_100: f64 = 4.15689;
    const WAVELENGTH: f64 = 0.7;

    fn two_theta() -> f64 {
        2.0 * (WAVELENGTH / (2.0 * D_100)).asin().to_degrees()
    }

    #[test]
    fn positions_follow_bragg() {
        let angle = two_theta();
        let d = XrdAxis::D.position(angle, WAVELENGTH).unwrap();
        assert!((d - D_100).abs() < 1e-9);
        let q = XrdAxis::Q.position(angle, WAVELENGTH).unwrap();
        assert!((q - 2.0 * std::f64::consts::PI / D_100).abs() < 1e-9);
        let cu = XrdAxis::CuKAlpha.position(angle, WAVELENGTH).unwrap();
        let expected = 2.0 * (CU_KALPHA1_ANGSTROM / (2.0 * D_100)).asin().to_degrees();
        assert!((cu - expected).abs() < 1e-9);
    }

    #[test]
    fn cu_k_alpha_drops_reflections_it_cannot_reach() {
        assert_eq!(XrdAxis::CuKAlpha.position(90.0, 0.5), None);
    }

    #[test]
    fn d_spacing_runs_upward_with_its_intensities() {
        let (x, y) = on_axis(&[10.0, 20.0, 30.0], &[1.0, 2.0, 3.0], XrdAxis::D, 1.0);
        assert!(x.windows(2).all(|pair| pair[1] > pair[0]));
        assert_eq!(y, vec![3.0, 2.0, 1.0]);
    }

    #[test]
    fn a_bad_wavelength_is_rejected() {
        for wavelength in [0.0, 1e-300, 1e300] {
            let params = XrdProcessing {
                wavelength_angstrom: Some(wavelength),
                ..XrdProcessing::default()
            };
            assert_eq!(validate(params), Err(XrdProcessingError::InvalidWavelength));
        }
    }

    #[test]
    fn two_theta_inverts_every_axis() {
        for axis in XrdAxis::ALL {
            let position = axis.position(20.0, WAVELENGTH).unwrap();
            let angle = axis.two_theta(position, WAVELENGTH).unwrap();
            assert!((angle - 20.0).abs() < 1e-9, "{axis:?}");
        }
    }
}
