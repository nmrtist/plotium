use super::*;
use crate::protein::{AssignmentEdit, ChainId, Edit, MoleculeId, ResidueId};
use crate::state::{DatasetId, Peak3DId, PlotiumApp};
use serde::Deserialize;
use serde_json::{Value, json};

pub const EDIT: &str = "protein.edit";
pub const ASSIGN: &str = "protein.assignments.edit";
pub const INSPECT: &str = "protein.inspect";
pub const EXPORT: &str = "protein.assignments.export";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AssignmentParams {
    dataset: DatasetId,
    peak: Peak3DId,
    edit: AssignmentEdit,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Inspect {
    molecule: Option<MoleculeId>,
    chain: Option<ChainId>,
    #[serde(default)]
    view: InspectView,
    dataset: Option<DatasetId>,
    residue: Option<ResidueId>,
    #[serde(default)]
    offset: usize,
    limit: Option<usize>,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "snake_case")]
enum InspectView {
    #[default]
    Overview,
    Chains,
    Residues,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Export {
    molecule: MoleculeId,
    #[serde(default)]
    confirmed_only: bool,
}

pub(super) fn validate(tool: &str, value: Value) -> Result<(), AutomationError> {
    use super::registry::parse;
    match tool {
        EDIT => parse::<Edit>(tool, value).map(drop),
        ASSIGN => parse::<AssignmentParams>(tool, value).map(drop),
        INSPECT => parse::<Inspect>(tool, value).map(drop),
        EXPORT => parse::<Export>(tool, value).map(drop),
        _ => Err(AutomationError::Execution(
            "Unknown protein operation".into(),
        )),
    }
}

pub(super) fn execute(
    app: &mut PlotiumApp,
    plan: &ToolPlan,
) -> Result<ToolResult, AutomationError> {
    use super::registry::parse;
    let tool = plan.request.tool_id.as_str();
    let params = plan.request.parameters.clone();
    let fail = AutomationError::Execution;
    let value = match tool {
        EDIT => {
            app.edit_protein(parse(tool, params)?).map_err(fail)?;
            json!({"molecules": app.doc.protein.molecules, "bindings": app.doc.protein.bindings})
        }
        ASSIGN => {
            let p: AssignmentParams = parse(tool, params)?;
            app.edit_assignment(p.dataset, p.peak, p.edit)
                .map_err(fail)?;
            json!({"assignment": app.doc.protein.assignment(p.dataset, p.peak)})
        }
        INSPECT => inspect(app, parse(tool, params)?).map_err(fail)?,
        EXPORT => {
            let p: Export = parse(tool, params)?;
            app.export_protein(p.molecule, p.confirmed_only)
                .map_err(fail)?
        }
        _ => return Err(fail("Unknown protein operation".into())),
    };
    let mut result =
        super::tool_executors::readonly_value(tool, plan.frozen_targets.revision, value);
    result.after_revision = DocumentRevision(app.doc.automation_revision);
    if result.after_revision != plan.frozen_targets.revision {
        result.modified.clone_from(&plan.frozen_targets.targets);
    }
    Ok(result)
}

fn inspect(app: &PlotiumApp, p: Inspect) -> Result<Value, String> {
    let state = &app.doc.protein;
    let molecule = p.molecule.map(|id| state.molecule(id)).transpose()?;
    let limit = p.limit.unwrap_or(100).min(250);
    let offset = p.offset;
    let next = |total: usize| {
        let end = offset.saturating_add(limit);
        (limit > 0 && end < total).then_some(end)
    };
    // Names and FASTA titles are user input; these labels are only previews.
    let label = |text: &str| text.chars().take(64).collect::<String>();
    match p.view {
        InspectView::Chains | InspectView::Residues => {
            if p.dataset.is_some() || p.residue.is_some() {
                return Err("Dataset and residue filters apply to the overview".into());
            }
            let m = molecule.ok_or("Select a molecule for sequence lookup")?;
            if matches!(p.view, InspectView::Chains) {
                if p.chain.is_some() {
                    return Err("Use the residues view to select a chain".into());
                }
                let chains = m.chains.iter().skip(offset).take(limit).map(|c| {
                    json!({"id":c.id,"name":label(&c.name),"title":label(&c.title),"start":c.start,"residue_count":c.residues.len()})
                }).collect::<Vec<_>>();
                return Ok(
                    json!({"molecule":m.id,"chains":chains,"total":m.chains.len(),"next_offset":next(m.chains.len())}),
                );
            }
            let id = p.chain.ok_or("Select a chain for residue lookup")?;
            let c = m
                .chains
                .iter()
                .find(|c| c.id == id)
                .ok_or_else(|| format!("Unknown chain {} in molecule {}", id.0, m.id.0))?;
            let residues = c.residues.iter().enumerate().skip(offset).take(limit).map(|(i,r)| {
                json!({"id":r.id,"code":r.code,"number":i64::from(c.start) + i as i64})
            }).collect::<Vec<_>>();
            Ok(
                json!({"molecule":m.id,"chain":c.id,"residues":residues,"total":c.residues.len(),"next_offset":next(c.residues.len())}),
            )
        }
        InspectView::Overview => {
            if p.chain.is_some() {
                return Err("Use the residues view to select a chain".into());
            }
            if let Some(d) = p.dataset {
                crate::protein::volume(&app.doc, d)?;
            }
            let matches = state
                .assignments
                .iter()
                .filter(|a| {
                    p.dataset.is_none_or(|d| a.dataset == d)
                        && p.molecule
                            .is_none_or(|m| state.bindings.get(&a.dataset) == Some(&m))
                        && p.residue.is_none_or(|r| {
                            a.candidates
                                .iter()
                                .any(|c| c.targets.iter().flatten().any(|t| t.residue == r))
                        })
                })
                .collect::<Vec<_>>();
            let rows = matches.iter().skip(offset).take(limit).collect::<Vec<_>>();
            let matching_molecules = state
                .molecules
                .iter()
                .filter(|m| {
                    p.molecule.is_none_or(|id| m.id == id)
                        && p.dataset
                            .is_none_or(|d| state.bindings.get(&d) == Some(&m.id))
                        && p.residue.is_none_or(|id| {
                            m.chains
                                .iter()
                                .any(|c| c.residues.iter().any(|r| r.id == id))
                        })
                })
                .collect::<Vec<_>>();
            let molecules = matching_molecules
                .iter()
                .skip(offset)
                .take(limit)
                .map(|m| json!({"id":m.id,"name":label(&m.name),"chain_count":m.chains.len()}))
                .collect::<Vec<_>>();
            let molecule_ids: std::collections::HashSet<_> =
                matching_molecules.iter().map(|m| m.id).collect();
            let bindings = state
                .bindings
                .iter()
                .filter(|(d, m)| p.dataset.is_none_or(|id| **d == id) && molecule_ids.contains(m))
                .collect::<Vec<_>>();
            let page_bindings = bindings
                .iter()
                .skip(offset)
                .take(limit)
                .copied()
                .collect::<std::collections::BTreeMap<_, _>>();
            Ok(
                json!({"molecules":molecules,"bindings":page_bindings,"assignments":rows,
                "total":matches.len(),"molecule_total":matching_molecules.len(),"binding_total":bindings.len(),
                "next_offset":next(matches.len().max(matching_molecules.len()).max(bindings.len()))}),
            )
        }
    }
}

pub(super) fn descriptors() -> Vec<ToolDescriptor> {
    let target_schema = json!({
        "type":["object","null"],
        "properties": {
            "residue":{"type":"string"},
            "members":{"type":"array","minItems":1,"uniqueItems":true,"items":{"type":"string"}},
            "group_meaning":{"enum":["unspecified","one_of","shared"],"default":"unspecified",
                "description":"For multiple atoms: one_of means one unidentified member; shared means all members share the observed resonance. Unspecified groups may be saved but not confirmed. Single atoms use unspecified."}
        },
        "required":["residue","members"]
    });
    [
        (EDIT, "Edit protein sequence metadata and spectrum bindings", true,
         json!({"operation":{"enum":["import","rename","label_chain","delete","bind","clear_spectrum_assignments"]},"source":{"type":"string"},"fasta":{"type":"string"},"name":{"type":"string"},"chains":{"type":"array"},"molecule":{"type":["string","null"]},"chain":{"type":"string"},"label":{"type":"object"},"dataset":{"type":"string"}}), vec!["operation"]),
        (ASSIGN, "Edit or confirm a manual 3D peak assignment", true,
         json!({"dataset":{"type":"string"},"peak":{"type":"integer","minimum":0},"edit":{"type":"object","properties":{"operation":{"enum":["add","update","delete","confirm","withdraw","clear"]},"targets":{"type":"array","minItems":3,"maxItems":3,"items":target_schema},"candidate":{"type":"string"},"note":{"type":"string"}},"required":["operation"]}}),vec!["dataset","peak","edit"]),
        (INSPECT, "List protein summaries and associated peaks, or page chains and residues", false,
         json!({"view":{"enum":["overview","chains","residues"],"default":"overview"},"molecule":{"type":"string","description":"Required for chains and residues"},"chain":{"type":"string","description":"Required for residues"},"dataset":{"type":"string"},"residue":{"type":"string"},"offset":{"type":"integer","minimum":0},"limit":{"type":"integer","minimum":0}}),vec![]),
        (EXPORT, "Export a molecule's manual assignments with acquisition and processing provenance", false,
         json!({"molecule":{"type":"string"},"confirmed_only":{"type":"boolean"}}),vec!["molecule"]),
    ].into_iter().map(|(id,title,edit,properties,required)| ToolDescriptor {
        id:id.into(),version:1,title:title.into(),description:title.into(),
        parameter_schema:json!({"type":"object","additionalProperties":false,"properties":properties,"required":required}),
        result_schema:json!({"type":"object"}),target_kinds:vec![ResourceKindId::new(KIND_DOCUMENT)],
        required_capabilities:vec![],effect:if edit {EffectLevel::Reversible} else {EffectLevel::ReadOnly},
        undoable:edit,deterministic:!edit,task_kind:"synchronous".into()
    }).collect()
}
