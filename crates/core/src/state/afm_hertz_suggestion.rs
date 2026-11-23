use super::{AfmFitTarget, AfmHertzDraft, AfmHertzReference, AfmHertzWindow, Arc, PlotiumApp};
use plotium_analysis::afm_hertz_suggestion::{HertzSuggestion, suggest_hertz};

#[derive(Clone)]
pub(super) struct AfmHertzProposal {
    target: AfmFitTarget,
    pixel: [usize; 2],
    data: Arc<plotium_io::AfmData>,
    candidate: HertzSuggestion,
}

impl AfmHertzDraft {
    pub fn hertz_suggestion_radius(&self) -> Result<f64, &'static str> {
        if self.linearized
            || self.window != AfmHertzWindow::RelativeZ
            || self.reference != AfmHertzReference::Zero
        {
            return Err(
                "Choose Manual Contact Hertz, Relative Z (nm), and Off (corrected zero) to use suggestions.",
            );
        }
        self.radius()
    }
}

impl PlotiumApp {
    pub fn suggest_afm_hertz(&mut self) -> Result<(), String> {
        let draft = self
            .session
            .ui
            .afm_hertz_task
            .as_mut()
            .ok_or("Open Hertz Fit first.")?;
        draft.suggestion = None;
        let radius = draft.hertz_suggestion_radius()?;
        let target = draft.target;
        let (afm, spring) = self.afm_hertz_source(target)?;
        let force_increment = afm
            .force_increment_nn()
            .ok_or("Use an AFM curve with a valid force scale for suggestions.")?;
        let points = self.afm_hertz_points(target)?;
        let candidate = suggest_hertz(&points, spring, radius, force_increment)
            .map_err(|error| error.to_string())?;
        let proposal = AfmHertzProposal {
            target,
            pixel: afm.selected_pixel,
            data: afm.data.clone(),
            candidate,
        };
        self.session
            .ui
            .afm_hertz_task
            .as_mut()
            .ok_or("Open Hertz Fit first.")?
            .suggestion = Some(proposal);
        Ok(())
    }

    pub fn afm_hertz_suggestion(&self) -> Option<&HertzSuggestion> {
        let draft = self.session.ui.afm_hertz_task.as_ref()?;
        let proposal = draft.suggestion.as_ref()?;
        let (afm, spring) = self.afm_hertz_source(draft.target).ok()?;
        (proposal.target == draft.target
            && proposal.pixel == afm.selected_pixel
            && Arc::ptr_eq(&proposal.data, &afm.data)
            && proposal.candidate.input.spring_constant_n_per_m == spring
            && proposal.candidate.input.radius_nm == draft.hertz_suggestion_radius().ok()?)
        .then_some(&proposal.candidate)
    }

    pub fn apply_afm_hertz_suggestion(&mut self) -> Result<(), String> {
        let input = self
            .afm_hertz_suggestion()
            .ok_or("Generate a suggestion for the current curve and probe radius first.")?
            .input;
        let draft = self
            .session
            .ui
            .afm_hertz_task
            .as_mut()
            .ok_or("Open Hertz Fit first.")?;
        draft.contact_z_nm = input.contact_z_nm.to_string();
        draft.baseline_z_nm = input.baseline_z_nm.map(|bound| bound.to_string());
        draft.fit_z_nm = input.fit_z_nm.map(|bound| bound.to_string());
        draft.suggestion = None;
        draft.result = None;
        draft.error = None;
        Ok(())
    }
}
