use super::*;
use plotium_analysis::fit_model::{FitDataset, FitModelDefinition, FitOptions, fit_model};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AfmFitTarget {
    pub canvas: CanvasId,
    pub object: ObjectId,
    pub source: DatasetId,
    pub branch: AfmForceBranch,
}

#[derive(Clone)]
pub struct AfmFitDraft {
    pub target: AfmFitTarget,
    pub model: String,
    pub options: FitOptions,
    pub overlay: bool,
    pub error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AfmFitParameter {
    pub name: String,
    pub value: f64,
    pub standard_error: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AfmForceFit {
    pub source: DatasetId,
    pub branch: AfmForceBranch,
    pub model: FitModelDefinition,
    pub options: FitOptions,
    pub shown: bool,
    pub converged: bool,
    pub parameters: Vec<AfmFitParameter>,
    pub r_squared: Option<f64>,
    pub notices: Vec<String>,
    pub x_name: String,
    pub x_unit: String,
    pub y_name: String,
    pub y_unit: String,
    /// Each row stores X, observation, and prediction, in acquisition order.
    pub rows: Vec<[f64; 3]>,
}

impl AfmForceFit {
    pub fn range(&self) -> AxisRange {
        self.rows.iter().fold(
            AxisRange {
                min: f64::INFINITY,
                max: f64::NEG_INFINITY,
            },
            |range, [x, _, _]| AxisRange {
                min: range.min.min(*x),
                max: range.max.max(*x),
            },
        )
    }
}

impl PlotiumApp {
    pub(crate) fn apply_afm_force_fits(
        &mut self,
        target: AfmFitTarget,
        fits: &[Arc<AfmForceFit>],
    ) -> Result<(), crate::actions::ActionApplyError> {
        let plot = self
            .doc
            .canvas_index(target.canvas)
            .and_then(|ci| self.doc.canvases[ci].object_mut(target.object))
            .and_then(CanvasObject::plot_mut)
            .ok_or_else(|| {
                crate::actions::ActionApplyError::StaleTarget(format!("AFM plot {}", target.object))
            })?;
        plot.set_afm_force_fits(fits.to_vec());
        Ok(())
    }
    pub fn afm_fit_target(
        &self,
        source: DatasetId,
        branch: AfmForceBranch,
    ) -> Option<AfmFitTarget> {
        let on_page = |ci: usize| {
            let canvas = self.doc.canvases.get(ci)?;
            let valid = |object| {
                canvas.object(object).is_some_and(|item| item.visible)
                    && self.afm_force_plot_dataset(ci, object) == Some(source)
            };
            let object = canvas
                .active_plot_object_id()
                .filter(|&id| valid(id))
                .or_else(|| {
                    canvas
                        .objects
                        .iter()
                        .map(|item| item.id)
                        .find(|&id| valid(id))
                })?;
            Some(AfmFitTarget {
                canvas: canvas.resource_id,
                object,
                source,
                branch,
            })
        };
        self.session
            .active_canvas
            .and_then(on_page)
            .or_else(|| (0..self.doc.canvases.len()).find_map(on_page))
    }

    pub fn afm_fit_plot(&self, target: AfmFitTarget) -> Option<&PlotObject> {
        let ci = self.doc.canvas_index(target.canvas)?;
        (self.afm_force_plot_dataset(ci, target.object)? == target.source).then_some(())?;
        self.doc.canvases[ci].object(target.object)?.plot()
    }

    pub fn afm_fit_result(&self, target: AfmFitTarget) -> Option<&Arc<AfmForceFit>> {
        let plot = self.afm_fit_plot(target)?;
        let source = plot.primary_dataset()?;
        plot.afm_force_fits()
            .iter()
            .find(|fit| fit.source == source && fit.branch == target.branch)
    }

    pub fn afm_fit_selection(&self, target: AfmFitTarget) -> Option<AxisRange> {
        let selection = self.session.ui.analysis_selection.as_ref()?;
        (selection.canvas == target.canvas
            && selection.object == target.object
            && self.afm_fit_plot(target)?.primary_dataset() == Some(selection.dataset))
        .then_some(selection.x_range)
    }

    pub fn run_afm_fit(
        &mut self,
        target: AfmFitTarget,
        model_id: &str,
        options: FitOptions,
        shown: bool,
    ) -> Result<(), String> {
        let plot = self
            .afm_fit_plot(target)
            .ok_or("Select a force curve before fitting.")?;
        let source = plot
            .primary_dataset()
            .ok_or("The force curve source is unavailable.")?;
        let afm = self
            .doc
            .dataset_by_id(source)
            .and_then(Dataset::as_afm)
            .ok_or("The AFM source is unavailable.")?;
        let context = afm
            .force_curve_context()
            .ok_or("The AFM source has no force curves.")?;
        let figure = afm
            .field_catalog
            .id_for_key("afm.force_curve")
            .and_then(|field| afm.force_figure(field))
            .ok_or("The force curve is unavailable.")?;
        let range = self.afm_fit_selection(target);
        let branch = figure
            .series
            .iter()
            .find(|s| s.name == target.branch.name())
            .ok_or("The selected AFM branch is unavailable.")?;
        let (x, y): (Vec<_>, Vec<_>) = branch
            .points
            .iter()
            .copied()
            .filter(|[x, _]| range.is_none_or(|r| *x >= r.min && *x <= r.max))
            .map(|[x, y]| (x, y))
            .unzip();
        if x.len() < 3 {
            return Err(
                "Select a range with at least three AFM samples, or clear the range.".into(),
            );
        }
        if x.iter().chain(&y).any(|v| !v.is_finite()) || x.iter().all(|v| *v == x[0]) {
            return Err("Choose an AFM range with finite samples and distinct X values.".into());
        }
        let model = super::app_impl_analysis::resolve_table_fit_model(model_id, false)?;
        let ([input], [response]) = (
            model.independent_variables.as_slice(),
            model.responses.as_slice(),
        ) else {
            return Err("Choose a model with one independent variable and one response.".into());
        };
        let data = FitDataset {
            id: target.branch.name().into(),
            inputs: BTreeMap::from([(input.id.clone(), x.clone())]),
            responses: BTreeMap::from([(response.id.clone(), y)]),
            sigmas: BTreeMap::new(),
            constants: BTreeMap::new(),
        };
        let result = fit_model(model, vec![data], &[], options)
            .map_err(|error| format!("AFM fit failed: {error}"))?;
        let rows = result
            .points
            .iter()
            .map(|p| {
                x.get(p.row)
                    .map(|&v| [v, p.observed, p.predicted])
                    .filter(|row| {
                        row.iter().all(|v| v.is_finite()) && (row[1] - row[2]).is_finite()
                    })
                    .ok_or("The AFM fit returned a non-finite prediction.")
            })
            .collect::<Result<Vec<_>, _>>()?;
        if result.parameters.iter().any(|p| !p.value.is_finite()) {
            return Err("The AFM fit returned a non-finite parameter.".into());
        }
        let parameters = result
            .parameters
            .iter()
            .map(|p| AfmFitParameter {
                name: p.parameter.clone(),
                value: p.value,
                standard_error: p.standard_error.is_finite().then_some(p.standard_error),
            })
            .collect();
        let mut after = plot.afm_force_fits().to_vec();
        // Refit only replaces this source and branch.
        after.retain(|fit| !(fit.source == source && fit.branch == target.branch));
        after.push(Arc::new(AfmForceFit {
            source,
            branch: target.branch,
            model: result.model,
            options: result.options,
            shown,
            converged: result.converged,
            parameters,
            r_squared: result
                .statistics
                .r_squared
                .is_finite()
                .then_some(result.statistics.r_squared),
            notices: result.notices,
            x_name: context.x.name.into(),
            x_unit: context.x.unit.into(),
            y_name: context.y.name.into(),
            y_unit: context.y.unit.into(),
            rows,
        }));
        self.change_afm_fits(target, after)?;
        self.session.status = if result.converged {
            format!(
                "Fitted {} ({} points).",
                target.branch.name(),
                result.points.len()
            )
        } else {
            "AFM fit did not converge. Inspect the estimates before using them.".into()
        };
        Ok(())
    }

    pub fn toggle_afm_fit_overlay(&mut self, target: AfmFitTarget) -> Result<(), String> {
        let fit = self.afm_fit_result(target).ok_or("Run AFM Fit first.")?;
        let mut after = self
            .afm_fit_plot(target)
            .ok_or("The source plot is unavailable.")?
            .afm_force_fits()
            .to_vec();
        for stored in &mut after {
            if Arc::ptr_eq(stored, fit) {
                let stored = Arc::make_mut(stored);
                stored.shown = !stored.shown;
            }
        }
        self.change_afm_fits(target, after)
    }

    fn change_afm_fits(
        &mut self,
        target: AfmFitTarget,
        after: Vec<Arc<AfmForceFit>>,
    ) -> Result<(), String> {
        let before = self
            .afm_fit_plot(target)
            .ok_or("The source plot is unavailable.")?
            .afm_force_fits()
            .to_vec();
        self.try_execute_action(crate::actions::Action::SetAfmForceFits {
            target,
            before,
            after,
        })
        .map_err(|error| error.to_string())
    }
}
