//! Borrowed final-spectrum diagnostics supplied by the NMR library.

pub use nmr::processing::background::{BackgroundReport, Status};

pub fn diagnose(values: &[f32], rows: usize, cols: usize) -> Result<BackgroundReport, String> {
    nmr::processing::background::diagnose_f32(
        values,
        &[rows, cols],
        None,
        nmr::processing::background::Representation::PhaseSensitive,
        &mut nmr::ExecutionContext::default(),
    )
    .map_err(|error| error.to_string())
}
