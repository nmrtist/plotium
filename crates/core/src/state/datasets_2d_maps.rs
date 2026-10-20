//! Synchronous DOSY/ILT map builders on a pseudo-2D dataset. The desktop app
//! goes through the async compute service instead; these stay for headless use
//! and tests, and must keep the same numerical semantics as the async path.

use super::*;
use sha2::{Digest, Sha256};
use std::sync::Arc;

pub(crate) const DIFFUSION_MAP_ALGORITHM_VERSION: u32 = 1;
pub(crate) const ILT_MAP_ALGORITHM_VERSION: u32 = 1;
pub(crate) const MONO_EXP_SNR_FRAC: f64 = 0.05;

pub(crate) fn dosy_data_fingerprint(
    stack: &plotium_processing::StackSpectrum,
    input: &crate::DiffusionInput,
) -> String {
    let mut hash = Sha256::new();
    hash.update((stack.traces.len() as u64).to_le_bytes());
    hash.update((stack.ppm.len() as u64).to_le_bytes());
    for value in &stack.ppm {
        hash.update(value.to_bits().to_le_bytes());
    }
    for trace in &stack.traces {
        for value in trace {
            hash.update(value.re.to_bits().to_le_bytes());
            hash.update(value.im.to_bits().to_le_bytes());
        }
    }
    // Include identities and all model/source evidence, as well as final b values.
    // This struct contains no maps, so serde produces a deterministic field order.
    match serde_json::to_vec(input) {
        Ok(bytes) => hash.update(bytes),
        Err(error) => hash.update(format!("Invalid diffusion input: {error}")),
    }
    format!("{:x}", hash.finalize())
}

pub(crate) fn mono_exp_provenance(
    stack: &plotium_processing::StackSpectrum,
    encoding: &crate::DiffusionInput,
) -> DosyResultProvenance {
    DosyResultProvenance {
        source_fields: Vec::new(),
        columns: Vec::new(),
        algorithm: "diffusion_map".into(),
        version: DIFFUSION_MAP_ALGORITHM_VERSION,
        input: DosyInvocation::MonoExp {
            snr_frac: MONO_EXP_SNR_FRAC,
        },
        data_fingerprint: dosy_data_fingerprint(stack, encoding),
        encoding: Some(encoding.clone()),
    }
}

pub(crate) fn ilt_provenance(
    stack: &plotium_processing::StackSpectrum,
    encoding: &crate::DiffusionInput,
    params: IltParams,
) -> DosyResultProvenance {
    DosyResultProvenance {
        source_fields: Vec::new(),
        columns: Vec::new(),
        algorithm: "ilt_map".into(),
        version: ILT_MAP_ALGORITHM_VERSION,
        input: DosyInvocation::Ilt { params },
        data_fingerprint: dosy_data_fingerprint(stack, encoding),
        encoding: Some(encoding.clone()),
    }
}

impl Nmr2DDataset {
    pub fn suggested_diffusion_input(&self) -> Result<crate::DiffusionInput, String> {
        crate::diffusion_input::automatic_input(&self.data)
    }

    pub fn resolve_diffusion_input(
        &self,
        explicit: Option<&crate::DiffusionInput>,
    ) -> Result<crate::DiffusionInput, String> {
        let mut input = self.diffusion_input_candidate(explicit).0?;
        let count = match &self.processed {
            Processed2D::Stack(stack) => stack.traces.len(),
            _ => self.data.rows,
        };
        input.b_values(count)?;
        input.increments.sort_by_key(|entry| entry.id);
        Ok(input)
    }

    /// Resolve the lifecycle copy once; inspection and execution share this
    /// priority even when the selected input is invalid or needs confirmation.
    pub(crate) fn diffusion_input_candidate(
        &self,
        explicit: Option<&crate::DiffusionInput>,
    ) -> (Result<crate::DiffusionInput, String>, &'static str) {
        let provenance = match self.dosy_method {
            DosyMethod::MonoExp => self.dosy_provenance.as_ref(),
            DosyMethod::Ilt(_) => self.ilt_provenance.as_ref(),
        };
        if let Some(input) = explicit {
            (Ok(input.clone()), "explicit_input")
        } else if let Some(input) = provenance.and_then(|p| p.encoding.as_ref()) {
            (Ok(input.clone()), "result_provenance")
        } else if let Some(input) = &self.diffusion_input {
            (Ok(input.clone()), "stored_default")
        } else {
            (self.suggested_diffusion_input(), "acquisition")
        }
    }

    pub(crate) fn invalidate_dosy_results(&mut self, reason: &str) {
        self.dosy_map = None;
        self.dosy_provenance = None;
        self.ilt_map = None;
        self.ilt_provenance = None;
        self.dosy_figure = None;
        self.ilt_figure = None;
        self.dosy_provenance_warning =
            (self.display == PseudoDisplay::DosyMap).then(|| match self.dosy_method {
                DosyMethod::MonoExp => format!(
                    "{reason}, so Plotium is showing the stack instead. Build the per-column DOSY \
                     map to replace it."
                ),
                DosyMethod::Ilt(_) => format!(
                    "{reason}, so Plotium is showing the stack instead. Build the ILT DOSY map to \
                     replace it."
                ),
            });
    }

    pub fn dosy_input_error(&self) -> Option<&'static str> {
        match &self.processed {
            Processed2D::Stack(stack)
                if stack.direct.unit == Some(nmr::axis::AxisUnit::Ppm)
                    && stack.direct_domain == plotium_io::Domain::Frequency =>
            {
                None
            }
            _ => Some("DOSY maps require a stack of frequency-domain spectra calibrated in ppm."),
        }
    }

    /// Fit every column to build a DOSY map. Only meaningful for diffusion
    /// datasets.
    pub fn build_dosy_map(&mut self) -> bool {
        if self.dosy_input_error().is_some() {
            return false;
        }
        let Ok(input) = self.resolve_diffusion_input(None) else {
            return false;
        };
        let Processed2D::Stack(stack) = &self.processed else {
            return false;
        };
        let Ok(b) = input.b_values(stack.traces.len()) else {
            return false;
        };
        let Some(map) = plotium_analysis::diffusion::diffusion_map_b_cancellable(
            &**stack,
            &b,
            MONO_EXP_SNR_FRAC,
            &|| false,
        ) else {
            return false;
        };
        let any = map.d.iter().any(|d| d.is_finite());
        self.dosy_figure = Some(Arc::new(build_dosy_figure(
            &map,
            &self.data.direct.nucleus,
            &stack.source,
        )));
        let mut provenance = mono_exp_provenance(stack, &input);
        provenance.columns = map.diagnostics.clone();
        self.dosy_map = Some(map);
        self.dosy_provenance = Some(provenance);
        self.dosy_provenance_warning = None;
        if any {
            self.dosy_method = DosyMethod::MonoExp;
            self.display = PseudoDisplay::DosyMap;
        }
        any
    }
    /// Build a full ILT/CONTIN DOSY map (a regularized inversion). Requires
    /// diffusion metadata and a gradient-encoded ruler; each gradient value is
    /// converted to a Stejskal–Tanner b-factor before inversion.
    pub fn build_ilt_map(&mut self, params: IltParams) -> bool {
        if self.dosy_input_error().is_some() {
            return false;
        }
        let Ok(input) = self.resolve_diffusion_input(None) else {
            return false;
        };
        let Processed2D::Stack(stack) = &self.processed else {
            return false;
        };
        let Ok(b_factors) = input.b_values(stack.traces.len()) else {
            return false;
        };
        let d_grid = log_grid(params.d_min, params.d_max, params.n_grid);
        let result = ilt_map(&**stack, &b_factors, &d_grid, params.lambda);
        let any = result.amp.iter().flatten().any(|&a| a > 0.0);
        self.ilt_figure = Some(Arc::new(build_ilt_figure(
            &result,
            &self.data.direct.nucleus,
            &stack.source,
        )));
        self.dosy_method = DosyMethod::Ilt(params);
        let mut provenance = ilt_provenance(stack, &input, params);
        provenance.columns = result.diagnostics.clone();
        self.ilt_map = Some(result);
        self.ilt_provenance = Some(provenance);
        self.dosy_provenance_warning = None;
        if any {
            self.display = PseudoDisplay::DosyMap;
        }
        any
    }
}

impl PlotiumApp {
    pub fn set_diffusion_input(&mut self, dataset: usize, input: Option<crate::DiffusionInput>) {
        let Some(id) = self.doc.datasets.get(dataset).map(Dataset::resource_id) else {
            return;
        };
        match self.diffusion_input_action(id, input) {
            Ok(action) => self.execute_action(action),
            Err(error) => self.session.status = error,
        }
    }
}
