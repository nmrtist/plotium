use super::*;

pub fn freeze_targets(
    provider: &impl ResourceProvider,
    selector: &TargetSelector,
) -> Result<FrozenTargetSet, AutomationError> {
    match selector {
        TargetSelector::Components { targets } => {
            if targets.is_empty() {
                return Err(AutomationError::InvalidSelector(
                    "component targets must not be empty".into(),
                ));
            }
            let mut components = Vec::new();
            let mut resources = Vec::new();
            for target in targets {
                let valid = provider
                    .inspect(&target.resource.id)
                    .is_some_and(|descriptor| descriptor.resource == target.resource)
                    && provider.component_exists(target);
                if !valid {
                    return Err(AutomationError::InvalidSelector(format!(
                        "invalid component target {}",
                        target.describe()
                    )));
                }
                if !components.contains(target) {
                    components.push(target.clone());
                }
                if !resources.contains(&target.resource) {
                    resources.push(target.resource.clone());
                }
            }
            Ok(FrozenTargetSet {
                revision: provider.revision(),
                total_matches: components.len(),
                component_targets: components,
                targets: resources,
                reasons: Vec::new(),
                truncated: false,
            })
        }
        TargetSelector::Explicit { ids } => {
            let mut targets = Vec::new();
            let mut reasons = Vec::new();
            for id in ids {
                let descriptor = provider.inspect(id).ok_or_else(|| {
                    AutomationError::InvalidSelector(format!("resource {id} does not exist"))
                })?;
                reasons.push(SelectionReason {
                    resource_id: id.clone(),
                    reason: "explicit stable id".to_owned(),
                });
                targets.push(descriptor.resource);
            }
            Ok(FrozenTargetSet {
                component_targets: Vec::new(),
                revision: provider.revision(),
                total_matches: targets.len(),
                targets,
                reasons,
                truncated: false,
            })
        }
        TargetSelector::CurrentSelection => {
            let targets = provider.current_selection();
            let reasons = targets
                .iter()
                .map(|target| SelectionReason {
                    resource_id: target.id.clone(),
                    reason: "current GUI selection".to_owned(),
                })
                .collect();
            Ok(FrozenTargetSet {
                component_targets: Vec::new(),
                revision: provider.revision(),
                total_matches: targets.len(),
                targets,
                reasons,
                truncated: false,
            })
        }
        TargetSelector::Query { query } => Ok(search_resources(provider, query)),
        TargetSelector::WorkflowInput { name } => Err(AutomationError::InvalidSelector(format!(
            "workflow input '{name}' must be bound by the workflow executor"
        ))),
        TargetSelector::NodeOutput { node, port } => Err(AutomationError::InvalidSelector(
            format!("node output {node}.{port} must be bound by the workflow executor"),
        )),
    }
}
