//! Desktop-thread capability boundary. No project state crosses the bridge.
use std::collections::HashMap;
mod table;
mod transform;

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

#[derive(Clone, Debug)]
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
        Self::from(&error)
    }
}
impl From<&AutomationError> for Failure {
    fn from(error: &AutomationError) -> Self {
        let code = match error {
            AutomationError::StaleRevision { .. } | AutomationError::StaleTableRevision { .. } => {
                "stale_revision"
            }
            AutomationError::TableTransformCancelled => "request_cancelled",
            AutomationError::TableTransformBudget(_) => "transform_budget_exceeded",
            AutomationError::PreviewCancelled => "request_cancelled",
            AutomationError::PreviewBudget(_) => "preview_budget_exceeded",
            AutomationError::PreviewAsset(_) => "preview_asset_unavailable",
            AutomationError::ResponseTooLarge => "response_too_large",
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
    context: Option<plotium_core::state::ProjectContextId>,
    plans: HashMap<String, ToolPlan>,
    imports: HashMap<String, PreparedTableImport>,
    transforms: HashMap<String, PreparedTableTransform>,
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
    ids: Option<Vec<String>>,
    targets: Option<Vec<TargetRef>>,
    key: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Plan {
    tool_id: String,
    parameters: Value,
    ids: Option<Vec<String>>,
    targets: Option<Vec<TargetRef>>,
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
fn selector(
    ids: Option<Vec<String>>,
    targets: Option<Vec<TargetRef>>,
) -> Result<TargetSelector, Failure> {
    match (ids, targets) {
        (Some(ids), None) if !ids.is_empty() => Ok(TargetSelector::Explicit { ids }),
        (None, Some(targets))
            if !targets.is_empty()
                && targets
                    .iter()
                    .all(|target| matches!(target.component, Some(ComponentRef::Series(_)))) =>
        {
            Ok(TargetSelector::Components { targets })
        }
        _ => Err(Failure::new(
            "invalid_parameters",
            "Supply exactly one nonempty ids or targets array; targets require Series components",
        )),
    }
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
    json!({"properties": PROPERTIES.iter().filter_map(|id| property_definition_description(*id)).collect::<Vec<_>>()})
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
        mut arguments: Value,
        blocked: bool,
    ) -> Result<Value, Failure> {
        let changed = self.synchronize_context(app);
        if changed && name == "plotium_execute" {
            return Err(project_changed());
        }
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
            if let Some((context, _)) = args.plan_id.split_once(':')
                && context != app.session.project_context.to_string()
            {
                return Err(project_changed());
            }
            // Consume even on authorization, busy or revision failure.
            let plan = self.plans.remove(&args.plan_id).ok_or_else(|| {
                Failure::new(
                    "expired_plan",
                    "Plan expired or was already submitted; preflight again",
                )
            })?;
            let prepared = self.imports.remove(&args.plan_id);
            let transform = self.transforms.remove(&args.plan_id);
            barrier()?;
            let authority = if self.allow_modify {
                ExecutionAuthority::ReversibleModify
            } else {
                ExecutionAuthority::Read
            };
            return Ok(json!(if let Some(prepared) = transform {
                execute_prepared_table_transform(app, plan, prepared, authority)?
            } else if let Some(prepared) = prepared {
                execute_prepared_table_import(app, plan, prepared, authority)?
            } else {
                execute_tool(app, plan, authority)?
            }));
        }
        barrier()?;
        if matches!(name, "plotium_table_columns" | "plotium_table_read_rows") {
            return self.table_read(app, name, arguments);
        }
        if name == "plotium_plan"
            && arguments.get("tool_id").and_then(Value::as_str) == Some("data.import")
        {
            let (plan, params) = self.prepare_import_plan(app, arguments, blocked)?;
            let prepared = PreparedTableImport::prepare(params, || false)?;
            return self.finish_import_plan(app, plan, prepared, blocked);
        }
        if name == "plotium_plan"
            && arguments.get("tool_id").and_then(Value::as_str) == Some("data.transform")
        {
            let (plan, snapshot) = self.prepare_transform_plan(app, arguments, blocked)?;
            let prepared = snapshot.prepare(&std::sync::atomic::AtomicBool::new(false))?;
            return self.finish_transform_plan(app, plan, prepared, blocked);
        }
        if name == "plotium_render_preview" {
            let (snapshot, run) = self.prepare_preview(app, arguments, blocked)?;
            let outcome = snapshot.render();
            run.finish(app, &outcome)?;
            return Ok(json!(outcome?));
        }
        if !matches!(name, "plotium_get_state" | "plotium_properties_list") {
            check_context(app, &mut arguments)?;
        }
        if matches!(name, "plotium_plan" | "plotium_properties_inspect") {
            for key in ["ids", "targets"] {
                if arguments.get(key).is_some_and(|value| !value.is_array()) {
                    return Err(Failure::new(
                        "invalid_parameters",
                        "ids and targets must be arrays",
                    ));
                }
            }
        }
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
                json!({"project_context": app.session.project_context, "revision": app.doc.automation_revision, "project": result.value,
                "selection": selection, "connection_capabilities": crate::capabilities::manifest(self.allow_modify), "permissions": {"allow_reversible_modify": self.allow_modify}}),
            );
        }
        if name == "plotium_plan" {
            let args: Plan = decode(arguments)?;
            if !crate::capabilities::EDIT_OPERATIONS.contains(&args.tool_id.as_str()) {
                return Err(Failure::new(
                    "unsupported_operation",
                    "Discover supported operations in connection_capabilities.editable_operations",
                ));
            }
            if is_native_curve_tool(&args.tool_id) {
                if !self.allow_modify {
                    return Err(Failure::new(
                        "permission_denied",
                        "Switch to Read and modify before planning native curves",
                    ));
                }
                if args.ids.is_some() || args.targets.is_some() {
                    return Err(Failure::new(
                        "invalid_parameters",
                        "Native curve edits carry their own exact targets or parent",
                    ));
                }
                if self.plans.len() >= 32 {
                    return Err(Failure::new(
                        "plan_limit",
                        "Execute a pending plan before preparing another",
                    ));
                }
                let plan = plan_tool(
                    app,
                    request(
                        &args.tool_id,
                        args.parameters,
                        vec![],
                        args.expected_revision,
                    ),
                )?;
                let pending_rows = self
                    .plans
                    .values()
                    .filter_map(|p| p.binding_plan.as_ref())
                    .map(|p| p.curve_rows())
                    .sum::<usize>();
                if pending_rows + plan.binding_plan.as_ref().map_or(0, |p| p.curve_rows())
                    > 2_000_000
                {
                    return Err(Failure::new(
                        "plan_limit",
                        "Pending curve plans exceed 2,000,000 curve rows; execute a pending plan first",
                    ));
                }
                let id = format!("{}:{}", app.session.project_context, uuid::Uuid::new_v4());
                let value = json!({"plan_id":id,"preflight":plan});
                if serde_json::to_vec(&value)
                    .map_err(|e| Failure::new("execution_failed", e.to_string()))?
                    .len()
                    > DOCUMENT_RESPONSE_BYTES
                {
                    return Err(Failure::new(
                        "response_too_large",
                        "Use fewer curves in the native curve batch",
                    ));
                }
                self.plans.insert(id, plan);
                return Ok(value);
            }
            let targets = selector(args.ids, args.targets)?;
            if args.tool_id == "resource.rename"
                && matches!(targets, TargetSelector::Components { .. })
            {
                return Err(Failure::new(
                    "invalid_parameters",
                    "Rename requires resource ids",
                ));
            }
            if args.tool_id != "resource.rename" {
                check_property(&args.parameters)?;
            }
            let mut request = request(
                &args.tool_id,
                args.parameters,
                vec![],
                args.expected_revision,
            );
            request.targets = targets;
            let plan = plan_tool(app, request)?;
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
            let id = format!("{}:{}", app.session.project_context, uuid::Uuid::new_v4());
            let result = json!({"plan_id": id, "preflight": plan});
            self.plans.insert(id, plan);
            return Ok(result);
        }
        if name == "plotium_document_inspect" {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Query {
                expected_revision: DocumentRevision,
                #[serde(flatten)]
                inspection: DocumentInspectParams,
            }
            let query: Query = decode(arguments)?;
            let target = &query.inspection.target;
            let mut req = request(
                DOCUMENT_INSPECT,
                json!(query.inspection),
                vec![],
                query.expected_revision,
            );
            req.targets = if target.component.is_some() {
                TargetSelector::Components {
                    targets: vec![target.clone()],
                }
            } else {
                TargetSelector::Explicit {
                    ids: vec![target.resource.id.clone()],
                }
            };
            return Ok(json!(execute_tool(
                app,
                plan_tool(app, req)?,
                ExecutionAuthority::Read
            )?));
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
                let mut request = request(TOOL_INSPECT, parameters, vec![], args.expected_revision);
                request.targets = selector(args.ids, args.targets)?;
                let plan = plan_tool(app, request)?;
                return Ok(json!(execute_tool(app, plan, ExecutionAuthority::Read)?));
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
    if !app.session.project_present {
        return Err(Failure::new(
            "no_project",
            "Open a project first / 请先打开项目",
        ));
    }
    Ok(())
}

impl Session {
    pub(crate) fn prepare_preview(
        &self,
        app: &mut PlotiumApp,
        mut arguments: Value,
        blocked: bool,
    ) -> Result<(ImagePreviewSnapshot, PreparedToolRun), Failure> {
        check_context(app, &mut arguments)?;
        if blocked {
            return Err(Failure::new(
                "busy",
                "Resolve recovery or save operation and retry",
            ));
        }
        ensure_idle(app)?;
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Args {
            expected_revision: DocumentRevision,
            target: TargetRef,
            #[serde(default = "preview_edge")]
            max_edge: u32,
        }
        fn preview_edge() -> u32 {
            PREVIEW_DEFAULT_EDGE
        }
        let args: Args = decode(arguments)?;
        let plan = plan_tool(
            app,
            request(
                IMAGE_PREVIEW,
                json!(ImagePreviewParams {
                    target: args.target.clone(),
                    max_edge: args.max_edge
                }),
                vec![args.target.resource.id],
                args.expected_revision,
            ),
        )?;
        let run = PreparedToolRun::begin(app, &plan);
        match ImagePreviewSnapshot::capture(app, &plan) {
            Ok(snapshot) => Ok((snapshot, run)),
            Err(error) => {
                let failure = Failure::from(&error);
                run.finish(app, &Err(error))?;
                Err(failure)
            }
        }
    }
}

fn project_changed() -> Failure {
    Failure::new(
        "project_changed",
        "Project changed; read plotium_get_state and replan with the new project_context. Never reuse old targets or plans.",
    )
}
fn check_context(app: &PlotiumApp, arguments: &mut Value) -> Result<(), Failure> {
    let context = arguments
        .as_object_mut()
        .and_then(|a| a.remove("expected_project_context"))
        .ok_or_else(|| {
            Failure::new(
                "invalid_parameters",
                "expected_project_context is required; read plotium_get_state first",
            )
        })?;
    let expected: plotium_core::state::ProjectContextId = decode(context)?;
    if expected != app.session.project_context {
        return Err(project_changed());
    }
    Ok(())
}
impl Session {
    pub fn synchronize_context(&mut self, app: &PlotiumApp) -> bool {
        let changed = self
            .context
            .is_some_and(|id| id != app.session.project_context);
        if changed || app.session.ui.project_transition.is_some() {
            self.plans.clear();
            self.imports.clear();
            self.transforms.clear();
        }
        self.context = Some(app.session.project_context);
        changed
    }
}
