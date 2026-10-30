use crate::signal;

pub const MAX_SNIP_ITERATIONS: u16 = 200;
pub const MAX_SAVGOL_WINDOW: u16 = 101;
pub const MAX_SAVGOL_ORDER: u8 = 6;

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct XrdProcessing {
    pub background: Option<SnipBackground>,
    pub smoothing: Option<SavitzkyGolay>,
    pub normalization: XrdNormalization,
}

impl Default for XrdProcessing {
    fn default() -> Self {
        Self {
            background: None,
            smoothing: None,
            normalization: XrdNormalization::None,
        }
    }
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
