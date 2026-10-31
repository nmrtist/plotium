//! `processing.edit` for XRD settings and per-region XPS recipes.
use super::processing_edit::{PipelineEdit, invalid};
use super::*;
use crate::actions::DatasetProcessingState;
use crate::state::{Dataset, PlotiumApp, XasDataset, XpsDataset};
use plotium_io::xps::XpsRegionId;
use plotium_processing::xas::XasProcessing;
use plotium_processing::xps::{XpsProcessingRecipe, XpsProcessingStep, XpsStepKind};
use plotium_processing::xrd::XrdProcessing;
use plotium_processing::{StepId, StepSource};
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub(super) fn has_pending_edit(app: &PlotiumApp, dataset: &Dataset) -> bool {
    app.session
        .ui
        .proc_pending
        .as_ref()
        .is_some_and(|(id, _)| *id == dataset.resource_id())
}

/// A paused desktop edit restores its own snapshot on Discard, so an XRD, XAS or XPS
/// recipe must not change underneath it.
pub(super) fn reject_pending(app: &PlotiumApp, dataset: &Dataset) -> Result<(), AutomationError> {
    let recipe = matches!(dataset, Dataset::Xrd(_) | Dataset::Xps(_) | Dataset::Xas(_));
    if recipe && has_pending_edit(app, dataset) {
        return Err(AutomationError::Execution(
            "Apply or discard the paused processing edit in Plotium first.".into(),
        ));
    }
    Ok(())
}

pub(super) fn prepare_xrd(edits: Vec<PipelineEdit>) -> Result<Value, AutomationError> {
    let mut settings = None;
    for edit in edits {
        let PipelineEdit::SetXrd { processing } = edit else {
            return Err(invalid("XRD datasets are edited with set_xrd."));
        };
        settings = Some(XrdProcessing::from(processing));
    }
    let settings = settings.ok_or_else(|| invalid("Supply an XRD edit."))?;
    plotium_processing::xrd::validate(settings).map_err(|e| invalid(e.to_string()))?;
    Ok(json!({"xrd":settings}))
}

pub(super) fn prepare_xas(
    xas: &XasDataset,
    edits: Vec<PipelineEdit>,
) -> Result<Value, AutomationError> {
    let (mut signal, mut processing) = (xas.signal, xas.params);
    for edit in edits {
        match edit {
            PipelineEdit::SetXas {
                signal: next,
                processing: settings,
            } => {
                if next.is_none() && xas.signal.is_some() {
                    return Err(invalid("A scan with a signal must keep one."));
                }
                signal = next;
                let mut settings = XasProcessing::from(*settings);
                let shift = settings.energy_shift;
                let e0_unchanged = settings.pre_edge.e0 == processing.pre_edge.e0;
                settings.energy_shift = processing.energy_shift;
                processing = if e0_unchanged {
                    settings.with_energy_shift(shift)
                } else {
                    XasProcessing {
                        energy_shift: shift,
                        ..settings
                    }
                };
            }
            PipelineEdit::ApplyXasPreset { preset } => {
                processing = preset.apply(&processing, xas.k_end());
            }
            _ => {
                return Err(invalid(
                    "XAS datasets are edited with set_xas or apply_xas_preset.",
                ));
            }
        }
    }
    processing.validate().map_err(|e| invalid(e.to_string()))?;
    if let Some(signal) = signal {
        crate::state::signal_mu(&xas.scan, signal).map_err(|e| invalid(e.to_string()))?;
    }
    Ok(json!({"xas":{"signal":signal,"processing":processing}}))
}

type RegionRecipes = BTreeMap<XpsRegionId, XpsProcessingRecipe>;

fn xps_kind(kind: Value) -> Result<XpsStepKind, AutomationError> {
    serde_json::from_value(kind)
        .map_err(|e| invalid(format!("XPS steps are Window, Smooth or Normalize: {e}")))
}

fn region_steps<'a>(
    recipes: &'a mut RegionRecipes,
    axis: &str,
) -> Result<&'a mut Vec<XpsProcessingStep>, AutomationError> {
    axis.parse()
        .ok()
        .and_then(|id| recipes.get_mut(&XpsRegionId(id)))
        .map(|recipe| &mut recipe.steps)
        .ok_or_else(|| invalid("Axis must be an XPS region ID for this dataset."))
}

fn locate(
    recipes: &mut RegionRecipes,
    step: StepId,
) -> Result<(&mut Vec<XpsProcessingStep>, usize), AutomationError> {
    recipes
        .values_mut()
        .find_map(|recipe| {
            let index = recipe.steps.iter().position(|s| s.id == step)?;
            Some((&mut recipe.steps, index))
        })
        .ok_or_else(|| invalid(format!("Unknown StepId {}.", step.get())))
}

fn insertion(
    steps: &[XpsProcessingStep],
    before: Option<StepId>,
) -> Result<usize, AutomationError> {
    before.map_or(Ok(steps.len()), |id| {
        steps
            .iter()
            .position(|s| s.id == id)
            .ok_or_else(|| invalid("The insertion anchor must exist in the same region."))
    })
}

pub(super) fn prepare_xps(
    dataset: &Dataset,
    xps: &XpsDataset,
    edits: Vec<PipelineEdit>,
) -> Result<Value, AutomationError> {
    let mut recipes = xps.region_recipes.clone();
    let mut next_id = xps.next_step_id;
    for edit in edits {
        match edit {
            PipelineEdit::Add { axis, kind, before } => {
                let kind = xps_kind(kind)?;
                let steps = region_steps(&mut recipes, &axis)?;
                let position = insertion(steps, before)?;
                let id = StepId::new(next_id);
                next_id = next_id
                    .checked_add(1)
                    .ok_or_else(|| invalid("StepId space exhausted."))?;
                steps.insert(
                    position,
                    XpsProcessingStep {
                        id,
                        kind,
                        enabled: true,
                        source: StepSource::User,
                    },
                );
            }
            PipelineEdit::Replace { step, kind } => {
                let kind = xps_kind(kind)?;
                let (steps, index) = locate(&mut recipes, step)?;
                steps[index].kind = kind;
                steps[index].source = StepSource::User;
            }
            PipelineEdit::Enable { step, enabled } => {
                let (steps, index) = locate(&mut recipes, step)?;
                steps[index].enabled = enabled;
            }
            PipelineEdit::Remove { step } => {
                let (steps, index) = locate(&mut recipes, step)?;
                steps.remove(index);
            }
            PipelineEdit::Move { step, before } => {
                let (steps, index) = locate(&mut recipes, step)?;
                if before == Some(step) {
                    continue;
                }
                let value = steps.remove(index);
                let position = insertion(steps, before)?;
                steps.insert(position, value);
            }
            PipelineEdit::SetXrd { .. }
            | PipelineEdit::SetXas { .. }
            | PipelineEdit::ApplyXasPreset { .. } => {
                return Err(invalid(
                    "set_xrd, set_xas and apply_xas_preset apply only to their dataset kind.",
                ));
            }
        }
    }
    let frozen = json!({"xps":{"region_recipes":recipes,"next_step_id":next_id}});
    super::processing_edit::state_from_frozen(dataset, &frozen)?
        .apply_to(&mut dataset.clone())
        .map_err(|e| invalid(e.to_string()))?;
    Ok(frozen)
}

/// The XRD, XAS or XPS recipe frozen in preflight, or `None` for an NMR freeze.
pub(super) fn state_from_frozen(
    dataset: &Dataset,
    frozen: &Value,
) -> Result<Option<DatasetProcessingState>, AutomationError> {
    if let Some(value) = frozen.get("xrd") {
        let settings: XrdProcessing =
            serde_json::from_value(value.clone()).map_err(|e| invalid(e.to_string()))?;
        return match dataset {
            Dataset::Xrd(_) => Ok(Some(DatasetProcessingState::Xrd(settings))),
            _ => Err(invalid(
                "Frozen XRD settings do not belong to this dataset.",
            )),
        };
    }
    if let Some(value) = frozen.get("xas") {
        let parse_error = |e: serde_json::Error| invalid(e.to_string());
        let signal = serde_json::from_value(value["signal"].clone()).map_err(parse_error)?;
        let processing: XasProcessing =
            serde_json::from_value(value["processing"].clone()).map_err(parse_error)?;
        return match dataset {
            Dataset::Xas(_) => Ok(Some(DatasetProcessingState::Xas {
                signal,
                processing: Box::new(processing),
            })),
            _ => Err(invalid(
                "Frozen XAS settings do not belong to this dataset.",
            )),
        };
    }
    let Some(value) = frozen.get("xps") else {
        return Ok(None);
    };
    let recipes: RegionRecipes = serde_json::from_value(value["region_recipes"].clone())
        .map_err(|e| invalid(e.to_string()))?;
    let next_id = value["next_step_id"]
        .as_u64()
        .ok_or_else(|| invalid("Missing frozen XPS step allocator."))?;
    match DatasetProcessingState::from_dataset(dataset) {
        DatasetProcessingState::Xps {
            active_region,
            measurement_shifts,
            fit_workspaces,
            fits,
            ..
        } => Ok(Some(DatasetProcessingState::Xps {
            active_region,
            measurement_shifts,
            region_recipes: recipes,
            fit_workspaces,
            fits,
            next_step_id: next_id,
        })),
        _ => Err(invalid("Frozen XPS recipes do not belong to this dataset.")),
    }
}
