use super::{AfmFitTarget, Arc, PlotiumApp};
use plotium_analysis::afm_hertz_suggestion::{HertzSuggestion, suggest_hertz};

#[derive(Clone)]
pub(super) struct AfmHertzProposal {
    target: AfmFitTarget,
    pixel: [usize; 2],
    data: Arc<plotium_io::AfmData>,
    candidate: HertzSuggestion,
}

impl PlotiumApp {
    pub fn suggest_afm_hertz(&mut self) -> Result<(), String> {
        let draft = self
            .session
            .ui
            .afm_hertz_task
            .as_mut()
            .ok_or("Open Manual Hertz first.")?;
        draft.suggestion = None;
        if !draft.spring_confirmed {
            return Err("Confirm the file spring constant first.".into());
        }
        let radius = draft.radius()?;
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
            .ok_or("Open Manual Hertz first.")?
            .suggestion = Some(proposal);
        Ok(())
    }

    pub fn afm_hertz_suggestion(&self) -> Option<&HertzSuggestion> {
        let draft = self.session.ui.afm_hertz_task.as_ref()?;
        let proposal = draft.suggestion.as_ref()?;
        let (afm, spring) = self.afm_hertz_source(draft.target).ok()?;
        (draft.spring_confirmed
            && proposal.target == draft.target
            && proposal.pixel == afm.selected_pixel
            && Arc::ptr_eq(&proposal.data, &afm.data)
            && proposal.candidate.input.spring_constant_n_per_m == spring
            && proposal.candidate.input.radius_nm == draft.radius().ok()?)
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
            .ok_or("Open Manual Hertz first.")?;
        draft.contact_z_nm = input.contact_z_nm.to_string();
        draft.baseline_z_nm = input.baseline_z_nm.map(|bound| bound.to_string());
        draft.fit_z_nm = input.fit_z_nm.map(|bound| bound.to_string());
        draft.suggestion = None;
        draft.result = None;
        draft.error = None;
        Ok(())
    }
}
