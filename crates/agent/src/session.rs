//! Desktop-thread capability boundary. No project state crosses the bridge.
use std::collections::HashMap;

use plotium_core::{
    automation::*,
    properties::{self, PropertyId},
    state::PlotiumApp,
};
use serde::Deserialize;
use serde_json::{Value, json};

pub const PROPERTIES: [PropertyId; 5] = [
    properties::line::STROKE_WIDTH,
    properties::contour::LINE_WIDTH,
    properties::contour::COUNT,
    properties::typography::TICK_PT,
    properties::typography::LABEL_PT,
];

#[derive(Debug)]
pub struct Failure {
    pub code: &'static str,
    pub message: String,
}
impl Failure {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
    pub fn value(&self) -> Value {
        json!({"error": {"code": self.code, "message": self.message,
            "retryable": self.code == "busy"}})
    }
}
impl From<AutomationError> for Failure {
    fn from(error: AutomationError) -> Self {
        let code = match error {
            AutomationError::StaleRevision { .. } => "stale_revision",
            AutomationError::InsufficientAuthority { .. } => "permission_denied",
            AutomationError::UnknownTool(_) | AutomationError::ToolVersion { .. } => {
                "unsupported_operation"
            }
            AutomationError::InvalidParameters { .. } | AutomationError::InvalidSelector(_) => {
                "invalid_parameters"
            }
            _ => "execution_failed",
        };
        Self::new(code, error.to_string())
    }
}

#[derive(Default)]
pub struct Session {
    pub allow_modify: bool,
    plans: HashMap<String, ToolPlan>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Search {
    expected_revision: DocumentRevision,
    query: ResourceQuery,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Inspect {
    expected_revision: DocumentRevision,
    ids: Vec<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InspectProperty {
    expected_revision: DocumentRevision,
    ids: Vec<String>,
    key: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Plan {
    tool_id: String,
    parameters: Value,
    ids: Vec<String>,
    expected_revision: DocumentRevision,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Execute {
    plan_id: String,
}
fn decode<T: serde::de::DeserializeOwned>(value: Value) -> Result<T, Failure> {
    serde_json::from_value(value).map_err(|e| Failure::new("invalid_parameters", e.to_string()))
}
fn check_property(parameters: &Value) -> Result<(), Failure> {
    if PROPERTIES
        .iter()
        .any(|id| Some(id.as_str()) == parameters.get("key").and_then(Value::as_str))
    {
        Ok(())
    } else {
        Err(Failure::new(
            "unsupported_operation",
            "Property is outside the connection's supported display properties",
        ))
    }
}
fn request(
    tool: &str,
    parameters: Value,
    ids: Vec<String>,
    revision: DocumentRevision,
) -> ToolRequest {
    ToolRequest {
        tool_id: tool.into(),
        tool_version: 1,
        parameters,
        targets: TargetSelector::Explicit { ids },
        expected_revision: revision,
        caller: CallerType::Agent,
    }
}
pub fn property_catalog() -> Value {
    json!({"properties": PROPERTIES.iter().filter_map(|id| properties::definition(*id)).map(|d|
        json!({"id": d.id.as_str(), "label": d.canonical_label, "scope": format!("{:?}", d.scope_kind)})
    ).collect::<Vec<_>>()})
}
impl Session {
    pub fn call(
        &mut self,
        app: &mut PlotiumApp,
        name: &str,
        arguments: Value,
    ) -> Result<Value, Failure> {
        self.call_when_available(app, name, arguments, false)
    }

    /// The shell supplies recovery/save barriers that live outside core state.
    pub fn call_when_available(
        &mut self,
        app: &mut PlotiumApp,
        name: &str,
        arguments: Value,
        blocked: bool,
    ) -> Result<Value, Failure> {
        let barrier = || {
            if blocked {
                Err(Failure::new(
                    "busy",
                    "Resolve recovery or save operation and retry",
                ))
            } else {
                ensure_idle(app)
            }
        };
        if name == "plotium_execute" {
            let args: Execute = decode(arguments)?;
            // Consume even on authorization, busy or revision failure.
            let plan = self.plans.remove(&args.plan_id).ok_or_else(|| {
                Failure::new(
                    "expired_plan",
                    "Plan expired or was already submitted; preflight again",
                )
            })?;
            barrier()?;
            let authority = if self.allow_modify {
                ExecutionAuthority::ReversibleModify
            } else {
                ExecutionAuthority::Read
            };
            return Ok(json!(execute_tool(app, plan, authority)?));
        }
        barrier()?;
        if name == "plotium_get_state" {
            let _: Empty = decode(arguments)?;
            let revision = DocumentRevision(app.doc.automation_revision);
            let selection = ProjectResourceProvider::new(app).current_selection();
            let result = execute_tool(
                app,
                plan_tool(
                    app,
                    request("project.get_blueprint", json!({}), vec![], revision),
                )?,
                ExecutionAuthority::Read,
            )?;
            return Ok(
                json!({"revision": app.doc.automation_revision, "project": result.value,
                "selection": selection, "permissions": {"allow_reversible_modify": self.allow_modify}}),
            );
        }
        if name == "plotium_plan" {
            let args: Plan = decode(arguments)?;
            if !matches!(
                args.tool_id.as_str(),
                "resource.rename" | TOOL_SET | TOOL_RESET
            ) {
                return Err(Failure::new(
                    "unsupported_operation",
                    "Only rename and supported display properties can be planned",
                ));
            }
            if args.ids.is_empty() {
                return Err(Failure::new(
                    "invalid_parameters",
                    "Supply explicit stable target IDs",
                ));
            }
            if args.tool_id != "resource.rename" {
                check_property(&args.parameters)?;
            }
            let plan = plan_tool(
                app,
                request(
                    &args.tool_id,
                    args.parameters,
                    args.ids,
                    args.expected_revision,
                ),
            )?;
            if args.tool_id == "resource.rename"
                && plan.frozen_targets.targets.iter().any(|r| {
                    !matches!(
                        r.kind.0.as_str(),
                        KIND_DATASET | KIND_CANVAS | KIND_CANVAS_OBJECT
                    )
                })
            {
                return Err(Failure::new(
                    "unsupported_operation",
                    "Only datasets, canvases and canvas objects may be renamed",
                ));
            }
            if self.plans.len() >= 32 {
                return Err(Failure::new(
                    "plan_limit",
                    "32 plans are pending; execute a plan or restart the connection",
                ));
            }
            let id = uuid::Uuid::new_v4().to_string();
            let result = json!({"plan_id": id, "preflight": plan});
            self.plans.insert(id, plan);
            return Ok(result);
        }
        let (tool, parameters, ids, revision) = match name {
            "plotium_resources_search" => {
                let args: Search = decode(arguments)?;
                (
                    "resources.search",
                    json!({"query": args.query}),
                    vec![],
                    args.expected_revision,
                )
            }
            "plotium_resources_inspect" => {
                let args: Inspect = decode(arguments)?;
                (
                    "resources.inspect",
                    json!({}),
                    args.ids,
                    args.expected_revision,
                )
            }
            "plotium_properties_inspect" => {
                let args: InspectProperty = decode(arguments)?;
                let parameters = json!({"key": args.key});
                check_property(&parameters)?;
                (TOOL_INSPECT, parameters, args.ids, args.expected_revision)
            }
            _ => return Err(Failure::new("unsupported_operation", "Unknown tool")),
        };
        if tool != "resources.search" && ids.is_empty() {
            return Err(Failure::new(
                "invalid_parameters",
                "Supply explicit stable target IDs",
            ));
        }
        let plan = plan_tool(app, request(tool, parameters, ids, revision))?;
        Ok(json!(execute_tool(app, plan, ExecutionAuthority::Read)?))
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Empty {}

pub fn ensure_idle(app: &PlotiumApp) -> Result<(), Failure> {
    let ui = &app.session.ui;
    if app.compute_busy()
        || app.session.data_imports.is_pending()
        || app.session.line_fit_job.is_some()
        || app.session.xps_fit_job.is_some()
        || app.session.symmetry_audit_job.is_some()
        || app.session.table_transform_job.is_some()
        || app.session.table_refresh_job.is_some()
        || ui.project_transition.is_some()
        || ui.project_save_in_progress
        || ui.interaction.is_active()
        || ui.property_gesture.is_some()
        || ui.processing_session.is_some()
        || ui.wheel_zoom.is_some()
        || ui.wheel_property.is_some()
        || ui.canvas_size_edit.is_some()
        || ui.page_layout_edit.is_some()
        || ui.inspector_edit.is_some()
        || ui.series_presentation_edit.is_some()
        || ui.axis_overrides_before.is_some()
        || ui.figure_typography_before.is_some()
        || ui.caption_edit_before.is_some()
        || ui.note_edit_before.is_some()
        || ui.region_edit_before.is_some()
        || ui.phase_edit_active
        || ui.rename.is_some()
        || ui.text_edit.is_some()
        || ui.panel_note_edit.is_some()
        || ui.panel_note_inline_edit.is_some()
        || ui.property_text_edits.iter().any(|edit| edit.editing)
    {
        return Err(Failure::new(
            "busy",
            "Finish the current edit or project operation and retry",
        ));
    }
    Ok(())
}
