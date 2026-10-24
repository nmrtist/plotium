use crate::{Failure, PendingRequest};
use rmcp::{ErrorData, RoleServer, ServerHandler, model::*, service::RequestContext};
use serde_json::{Value, json};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub(crate) struct Handler {
    pub send: mpsc::Sender<PendingRequest>,
    pub cancel: CancellationToken,
    pub wake: Arc<dyn Fn() + Send + Sync>,
}
impl ServerHandler for Handler {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("Plotium", env!("CARGO_PKG_VERSION")))
            .with_instructions("Known operations can fetch their static detail directly; otherwise browse the plotium_capabilities_get index. Read plotium_get_state first. Supply its project_context as expected_project_context on project queries and planning. On project_changed, reread state and replan; never reuse targets or plans. For known exact targets, committed synchronous results provide project_context and after_revision for the next call. Refresh state after desktop edits or Undo. Never replay a tool call after a disconnection.")
    }
    async fn list_tools(
        &self,
        _: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        Ok(ListToolsResult {
            tools: definitions(),
            ..Default::default()
        })
    }
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        if !definitions().iter().any(|tool| tool.name == request.name) {
            return Err(ErrorData::invalid_params("Unknown tool", None));
        }
        let arguments = Value::Object(request.arguments.unwrap_or_default());
        if request.name == "plotium_properties_list" {
            return Ok(match crate::property_catalog::respond(arguments.clone()) {
                Ok(value) => CallToolResult::structured(value),
                Err(error) => CallToolResult::structured_error(
                    error
                        .for_request("plotium_properties_list", &arguments)
                        .value(),
                ),
            }
            .into());
        }
        let (response, mut receive) = oneshot::channel();
        let image_request = matches!(
            request.name.as_ref(),
            "plotium_render_preview" | "plotium_preview_plan"
        );
        let diagnostic_request = request.name == "plotium_figure_diagnose";
        let request_name = request.name.to_string();
        let request_arguments = arguments.clone();
        let pending = PendingRequest {
            name: request.name.into_owned(),
            arguments,
            response,
            cancellation: context.ct.clone(),
            transport: Some(context.peer.clone()),
        };
        if self.send.try_send(pending).is_err() {
            return Ok(CallToolResult::structured_error(
                Failure::new("busy", "Desktop queue is full or stopped")
                    .for_request(&request_name, &request_arguments)
                    .not_submitted()
                    .value(),
            )
            .into());
        }
        (self.wake)();
        let outcome = tokio::select! {
            biased;
            _ = context.ct.cancelled() => {
                receive.close();
                Err(Failure::new("request_cancelled", "Request cancelled before completion"))
            },
            _ = self.cancel.cancelled() => Err(Failure::new("connection_stopped", "Connection stopped")),
            result = &mut receive => result.unwrap_or_else(|_| Err(Failure::new("connection_stopped", "Desktop disconnected"))),
        };
        Ok(match outcome {
            Ok(value) if image_request => image_result(value, &context.id),
            Ok(value) if diagnostic_request => diagnostic_result(value, &context.id),
            Ok(value) => CallToolResult::structured(value),
            Err(error) => CallToolResult::structured_error(
                error
                    .for_request_if_missing(&request_name, &request_arguments)
                    .value(),
            ),
        }
        .into())
    }
}
pub(crate) fn definitions() -> Vec<Tool> {
    let revision = json!({"type": "integer", "minimum": 0});
    let ids = json!({"type": "array", "items": {"type": "string"}, "minItems": 1});
    let targets = json!({"type":"array", "minItems":1, "items": {
        "type":"object", "additionalProperties":false, "required":["resource","component"],
        "properties": {
            "resource": {"type":"object", "additionalProperties":false, "required":["id","kind"], "properties": {
                "id":{"type":"string"}, "kind":{"type":"string"},
                "parent_id":{"type":["string","null"]}, "local_id":{"type":["string","null"]}
            }},
            "component":{"type":"object", "additionalProperties":false, "required":["kind","id"], "properties": {
                "kind":{"const":"series"}, "id":{"type":"integer","minimum":0,"maximum":u64::MAX}
            }}
        }
    }});
    let key = json!({"type": "string", "enum": crate::session::PROPERTIES.iter().map(|id| id.as_str()).collect::<Vec<_>>()});
    let registry = plotium_core::automation::ToolRegistry::built_in();
    let mut query = registry
        .get("resources.search")
        .and_then(|d| d.parameter_schema.get("properties"))
        .and_then(|p| p.get("query"))
        .cloned()
        .unwrap_or(json!({"type": "object"}));
    query["description"] = json!(
        "ResourceQuery fields: kinds (resource kind IDs), capabilities (capability IDs), name_contains, units (strings), metadata (string map), lineage_source, offset, limit. Omitted filters match all resources. Plot object kind: plotium.canvas.object; document ID: document."
    );
    let mut document = plotium_core::automation::document_inspect_schema();
    document["properties"]["expected_revision"] = revision.clone();
    document["required"] = json!(["expected_revision", "target", "view"]);
    document
        .as_object_mut()
        .expect("local document schema")
        .remove("allOf");
    let mut tools: Vec<Tool> = [
        ("plotium_get_state", "Read latest availability, revision, project counts, a bounded selection page (selection.canvas: what the user selected on the active page, and the legend or label they clicked as part, the thing they mean by \"this\"), a bounded page of values the user set by hand (hand_edits: keep them; property writes skip them unless override_hand_edits is true because the user asked), permissions and capability fingerprint, including while busy. Each call observes current state; pages are not a frozen selection snapshot. Use plotium_capabilities_get for static contracts. Queries do not modify project state.", json!({"selection_offset":{"type":"integer","minimum":0},"selection_limit":{"type":"integer","minimum":1,"maximum":50,"default":16},"hand_edits_offset":{"type":"integer","minimum":0},"hand_edits_limit":{"type":"integer","minimum":1,"maximum":50,"default":16}}), json!([])),
        ("plotium_capabilities_get", "Read a compact index of static tool, operation and contract summaries with a stable fingerprint; sections lists each detail section, its keys and sizes. Fetch detail directly with {section,key?}; operation_details accepts keys for batching, such as an operation schema with {section:\"operation_constraints\",key:<tool_id>}. Callable while busy; cached capabilities and annotations never authorize execution or retry. Read current permissions from plotium_get_state.", json!({"section":{"enum":crate::capabilities::SECTIONS},"key":{"type":"string","minLength":1},"keys":{"type":"array","minItems":1,"maxItems":32,"uniqueItems":true,"items":{"type":"string"},"description":"Only operation_details; mutually exclusive with key."}}), json!([])),
        ("plotium_resources_search", "Search current project resources using ResourceQuery.", json!({"expected_revision": revision, "query": query}), json!(["expected_revision", "query"])),
        ("plotium_resources_inspect", "Read descriptors for explicit stable resource IDs.", json!({"expected_revision": revision, "ids": ids}), json!(["expected_revision", "ids"])),
        ("plotium_properties_list", "Browse supported display properties in compact form, or fetch full definitions with keys and detail full. Optional scope filters use catalog scope names. Complex schemas marked requires_full need full detail. For 1-200 exact target/key/value writes use properties.set_many through plotium_plan, atomically with one Undo. Reuse full targets returned by plot.create. Does not create a run record.", crate::property_catalog::schema(), json!([])),
        ("plotium_properties_inspect", "Read property values, defaults, bounds and applicability. Supply exactly one nonempty ids or targets array.", json!({"expected_revision": revision, "ids": ids, "targets": targets, "key": key}), json!(["expected_revision", "key"])),
        ("plotium_plan", "Preflight a supported edit on resource IDs or exact series targets. Returns a one-use opaque plan_id; at most 32 pending plans. Use ids or targets exclusively; only properties.set/reset accept targets. Operations naming their own targets omit both; table operations require exactly one dataset id.", json!({"expected_revision": revision, "ids": ids, "targets": targets, "tool_id": {"enum": crate::capabilities::EDIT_OPERATIONS}, "parameters": {"type": "object", "description": "Operation arguments. Fetch the exact schema with plotium_capabilities_get {section:\"operation_constraints\",key:<tool_id>}. resource.rename {name}; dataset.delete {}; figure.apply_theme {theme_id}; properties.set {key,value,override_hand_edits?}; properties.reset {key,override_hand_edits?}; properties.set_many {writes:[{target,key,value}]}; data.import {paths,...}; data.transform {mode:table_data_only,name,plan}; table.patch {table_revision,edits}; table.refresh {table_revision}; plot.create {parent,name,series}; plot.series.append {plot,series}; plot.series.bind/materialize_default {edits}; figure.edit {canvas,edits}; project.save {path}; figure.export {canvas,outputs}. Property writes skip values the user set by hand unless override_hand_edits is true because the user asked."}}), json!(["expected_revision", "tool_id", "parameters"])),
        ("plotium_execute", "Submit a saved plan once. Edits require reversible-modify permission. project.save queues separate per-request desktop file authorization; poll plotium_save_status for completion. figure.export requests separate desktop authorization; poll plotium_export_status for per-file completion. External writes are not undoable.", json!({"plan_id": {"type": "string"}}), json!(["plan_id"])),
    ].into_iter().map(|(name, description, properties, required)| {
        let schema = json!({"type": "object", "properties": properties, "required": required, "additionalProperties": false});
        Tool::new(name, description, schema.as_object().expect("local schema is an object").clone())
    }).collect();
    tools.push(Tool::new("plotium_document_inspect", "Inspect bounded document structure and source bindings by full stable target. view self returns details; children pages direct children. Pagination is only valid with view children. Never uses selection.", document.as_object().expect("local schema is an object").clone()));
    let mut diagnostic = plotium_core::automation::figure_diagnostic_schema();
    diagnostic["properties"]["expected_revision"] = json!({"type":"integer","minimum":0});
    diagnostic["properties"]["cursor"] = json!({"type":"string","format":"uuid"});
    diagnostic["required"] = json!(["expected_revision"]);
    tools.push(Tool::new("plotium_figure_diagnose", "Read native figure geometry and potential issues at explicit physical size. The data_shape and layout_aesthetics checks add severity hint issues with suggestions: the figure could be clearer, nothing is wrong. Defaults return compact issue rows only; pass view both for layout geometry and detail full for evidence. summary counts the issues on each page by code and severity. Coverage is mandatory: no issues is not proof of full coverage. Initial queries require canvas and output; continuation accepts only cursor, context and revision. Follow one-use continuation with the same context/revision; a new initial query replaces the previous scan. Queries do not append project audit records or change dirty state, edit generation or revision.", diagnostic.as_object().expect("local schema").clone()));
    tools.push(Tool::new("plotium_preview_plan", "Read an uncommitted candidate PNG for one existing local edit plan. Does not execute or consume the plan. Requires current context, revision, reversible-modify permission and valid plan at capture and delivery. Use plotium_execute explicitly afterwards.", plotium_core::automation::plan_preview_schema().as_object().expect("local schema").clone()));
    let mut preview = plotium_core::automation::image_preview_schema();
    preview["properties"]["expected_revision"] = json!({"type":"integer","minimum":0});
    preview["required"] = json!(["expected_revision", "target"]);
    tools.push(Tool::new("plotium_render_preview", "Read a whole-canvas PNG image with full target identity, frozen render_revision and pixel dimensions. Curves denser than the preview pixels are thinned and listed in downsampled; exports keep every point. No file I/O. Rejects missing assets and budget overflow.", preview.as_object().expect("local schema is an object").clone()));
    for (name, rows) in [
        ("plotium_table_columns", false),
        ("plotium_table_read_rows", true),
    ] {
        let mut schema = plotium_core::automation::table_read_schema(rows);
        schema["properties"]["expected_revision"] = json!({"type":"integer","minimum":0});
        schema["required"]
            .as_array_mut()
            .expect("local required fields")
            .push(json!("expected_revision"));
        tools.push(Tool::new(name,"Read native table data at explicit document and table revisions; no selection fallback.",schema.as_object().expect("local schema").clone()));
    }
    for (name, id) in [
        (
            "plotium_analysis_results_inspect",
            plotium_core::automation::ANALYSIS_RESULTS_INSPECT,
        ),
        (
            "plotium_diffusion_inspect",
            plotium_core::automation::DIFFUSION_INSPECT,
        ),
        (
            "plotium_processing_inspect",
            plotium_core::automation::PROCESSING_INSPECT,
        ),
    ] {
        let mut schema = registry
            .get(id)
            .expect("registered scientific inspection")
            .parameter_schema
            .clone();
        schema["properties"]["expected_revision"] = revision.clone();
        schema["properties"]["ids"] =
            json!({"type":"array","minItems":1,"maxItems":1,"items":{"type":"string"}});
        schema["required"] = json!(["expected_revision", "ids"]);
        tools.push(Tool::new(
            name,
            "Inspect scientific inputs or processing pipelines using the shared core operation.",
            schema.as_object().expect("local scientific schema").clone(),
        ));
    }
    for name in [
        "plotium_tasks_get",
        "plotium_tasks_list",
        "plotium_tasks_cancel",
    ] {
        let schema = if name == "plotium_tasks_list" {
            json!({"type":"object","additionalProperties":false,"properties":{"offset":{"type":"integer","minimum":0},"limit":{"type":"integer","minimum":1,"maximum":128}}})
        } else {
            json!({"type":"object","additionalProperties":false,"required":["task_id"],"properties":{"task_id":{"type":"string"}}})
        };
        tools.push(Tool::new(name, "Query or cancel retained scientific tasks. Disconnect does not cancel work; unknown or evicted tasks must never be replayed automatically.", schema.as_object().expect("local task schema").clone()));
    }
    tools.push(Tool::new("plotium_save_status", "Read a retained desktop save receipt after submission or reconnect, without an audit mutation. Queued/running is not success. Never replay an unknown save.", json!({"type":"object","additionalProperties":false,"required":["save_id"],"properties":{"save_id":{"type":"string"}}}).as_object().expect("local schema").clone()));
    tools.push(Tool::new("plotium_export_status", "Read retained per-file native export receipts without an audit mutation, including after reconnect. Never replay unknown exports.", json!({"type":"object","additionalProperties":false,"required":["export_id"],"properties":{"export_id":{"type":"string"}}}).as_object().expect("local schema").clone()));
    for tool in &mut tools {
        tool.annotations = Some(crate::tool_effects::annotations(&tool.name));
        if !matches!(
            tool.name.as_ref(),
            "plotium_get_state"
                | "plotium_capabilities_get"
                | "plotium_execute"
                | "plotium_properties_list"
                | "plotium_save_status"
                | "plotium_export_status"
                | "plotium_tasks_get"
                | "plotium_tasks_list"
                | "plotium_tasks_cancel"
        ) {
            let schema = std::sync::Arc::make_mut(&mut tool.input_schema);
            schema
                .get_mut("properties")
                .and_then(Value::as_object_mut)
                .expect("local properties")
                .insert(
                    "expected_project_context".into(),
                    json!({"type":"string","format":"uuid"}),
                );
            schema
                .get_mut("required")
                .and_then(Value::as_array_mut)
                .expect("local requirements")
                .push(json!("expected_project_context"));
        }
    }
    tools
}

fn diagnostic_result(value: Value, request_id: &RequestId) -> CallToolResult {
    let result = CallToolResult::structured(value);
    let envelope = JsonRpcResponse {
        jsonrpc: JsonRpcVersion2_0,
        id: request_id.clone(),
        result: &result,
    };
    match serde_json::to_vec(&envelope){
        Ok(bytes) if bytes.len()<=plotium_core::automation::DOCUMENT_RESPONSE_BYTES=>result,
        _=>CallToolResult::structured_error(Failure::new("response_too_large","MCP diagnostic response exceeds 256 KiB; use a smaller request ID and start a new scan").value()),
    }
}

pub(crate) fn image_result(mut value: Value, request_id: &RequestId) -> CallToolResult {
    let data = value
        .get_mut("value")
        .and_then(Value::as_object_mut)
        .and_then(|v| v.remove("png_base64"));
    let Some(Value::String(data)) = data else {
        return CallToolResult::structured_error(
            Failure::new("execution_failed", "Preview produced no PNG image").value(),
        );
    };
    let mut result = CallToolResult::structured(value);
    result.content.push(ContentBlock::image(data, "image/png"));
    let envelope = JsonRpcResponse {
        jsonrpc: JsonRpcVersion2_0,
        id: request_id.clone(),
        result: &result,
    };
    match serde_json::to_vec(&envelope) {
        Ok(bytes) if bytes.len() <= plotium_core::automation::PREVIEW_RESPONSE_BYTES => result,
        Ok(_) => CallToolResult::structured_error(
            Failure::new(
                "preview_budget_exceeded",
                "MCP preview response exceeds 6 MiB; request a smaller max_edge",
            )
            .value(),
        ),
        Err(error) => CallToolResult::structured_error(
            Failure::new(
                "execution_failed",
                format!("Could not encode preview response: {error}"),
            )
            .value(),
        ),
    }
}

#[cfg(test)]
#[path = "protocol_tests.rs"]
mod tests;
