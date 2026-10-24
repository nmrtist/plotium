//! Recovery describes observed outcomes; it never grants permission to replay.
use super::*;

#[derive(Clone, Debug)]
pub struct Failure {
    pub code: &'static str,
    pub message: String,
    pub details: Option<Value>,
    pub(super) recovery: Option<Value>,
}
impl Failure {
    pub(crate) fn at_field(mut self, field: &str) -> Self {
        if self.details.is_none() {
            self.details = Some(json!({}));
        }
        if let Some(details) = self.details.as_mut().and_then(Value::as_object_mut) {
            details.insert("field".into(), json!(field));
        }
        self
    }
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            details: None,
            recovery: None,
            message: message.into(),
        }
    }
    pub fn value(&self) -> Value {
        json!({"error": {"code": self.code, "message": self.message,
            "retryable": false, "details":self.details,
            "recovery":self.recovery.clone().unwrap_or_else(|| for_error(self.code))}})
    }
}
impl From<AutomationError> for Failure {
    fn from(error: AutomationError) -> Self {
        Self::from(&error)
    }
}
impl From<&AutomationError> for Failure {
    fn from(error: &AutomationError) -> Self {
        if let AutomationError::DiffusionInput { report } = error {
            let mut failure = Self::new("invalid_diffusion_input", error.to_string());
            failure.details = Some(json!({"diffusion_input":report}));
            return failure;
        }
        let code = match error {
            AutomationError::StaleRevision { .. } | AutomationError::StaleTableRevision { .. } => {
                "stale_revision"
            }
            AutomationError::TableTransformCancelled => "request_cancelled",
            AutomationError::NativeImportBudget(_) => "import_budget_exceeded",
            AutomationError::TableTransformBudget(_) => "transform_budget_exceeded",
            AutomationError::DiagnosticCancelled => "request_cancelled",
            AutomationError::DiagnosticBudget(_) => "diagnostic_budget_exceeded",
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
        let mut failure = Self::new(code, error.to_string());
        if let AutomationError::InvalidParameters { tool_id, .. } = error {
            failure.details = Some(json!({"operation":tool_id}));
        }
        failure
    }
}

fn recovery(action: Value) -> Value {
    json!({"plan_consumed":null,"submission":"unknown","committed":null,
        "completion":"unknown","receipt":null,"next_action":action,"replay_allowed":false})
}

fn state() -> Value {
    json!({"kind":"call_tool","tool":"plotium_get_state","arguments":{}})
}

pub(super) fn for_error(code: &str) -> Value {
    let action = match code {
        "unsupported_property" => {
            json!({"kind":"call_tool","tool":"plotium_properties_list","arguments":{}})
        }
        "permission_denied" | "unavailable" | "connection_stopped" => {
            json!({"kind":"check_desktop_connection"})
        }
        "unknown_save" | "unknown_export" | "task_not_found" | "expired_plan" => {
            json!({"kind":"inspect_outcome","automatic_resubmission":false})
        }
        _ => state(),
    };
    recovery(action)
}

impl Failure {
    pub(crate) fn for_request(mut self, name: &str, arguments: &Value) -> Self {
        if name == "plotium_plan"
            && self.code == "invalid_parameters"
            && self.details.as_ref().is_some_and(|details| {
                details["field"].is_null()
                    && details["operation"].is_string()
                    && details.get("operation") == arguments.get("tool_id")
            })
        {
            self = self.at_field("parameters");
        }
        let mut value = for_error(self.code);
        if matches!(self.code, "invalid_parameters" | "unsupported_operation") {
            let field = self.details.as_ref().and_then(|d| d["field"].as_str());
            let query = if name == "plotium_capabilities_get" {
                Some(json!({}))
            } else if name == "plotium_plan"
                && !matches!(
                    field,
                    Some("expected_project_context" | "expected_revision")
                )
            {
                Some(
                    arguments
                        .get("tool_id")
                        .and_then(Value::as_str)
                        .filter(|id| crate::capabilities::EDIT_OPERATIONS.contains(id))
                        .map_or(
                            json!({}),
                            |id| json!({"section":"operation_constraints","key":id}),
                        ),
                )
            } else {
                None
            };
            if let Some(arguments) = query {
                value["next_action"] = json!({"kind":"call_tool","tool":"plotium_capabilities_get","arguments":arguments});
            } else if name == "plotium_properties_list" {
                value["next_action"] =
                    json!({"kind":"call_tool","tool":"plotium_properties_list","arguments":{}});
            }
        }
        if name == "plotium_execute" {
            value["plan_id"] = arguments.get("plan_id").cloned().unwrap_or(Value::Null);
        }
        for (tool, key) in [
            ("plotium_save_status", "save_id"),
            ("plotium_export_status", "export_id"),
            ("plotium_tasks_get", "task_id"),
            ("plotium_tasks_cancel", "task_id"),
        ] {
            if name == tool
                && let Some(id) = arguments.get(key)
            {
                let retained = matches!(
                    self.code,
                    "unknown_save" | "unknown_export" | "task_not_found"
                )
                .then_some(false);
                value["receipt"] = json!({"tool":if tool == "plotium_tasks_cancel" { "plotium_tasks_get" } else { tool },"arguments":{key:id},"retained":retained});
            }
        }
        self.recovery = Some(value);
        if name == "plotium_plan" {
            // Preflight can retain candidates, but never submits or commits execution.
            self.not_submitted()
        } else {
            self
        }
    }

    pub(crate) fn for_request_if_missing(self, name: &str, arguments: &Value) -> Self {
        if self.recovery.is_some() {
            self
        } else {
            self.for_request(name, arguments)
        }
    }

    pub(crate) fn not_submitted(mut self) -> Self {
        let mut value = self.recovery.take().unwrap_or_else(|| for_error(self.code));
        value["plan_consumed"] = json!(false);
        value["submission"] = json!("not_submitted");
        value["committed"] = json!(false);
        value["completion"] = json!("failed");
        self.recovery = Some(value);
        self
    }
}

fn receipt(kind: &str, value: &mut Value) {
    let (tool, key) = match kind {
        "save" => ("plotium_save_status", "save_id"),
        "export" => ("plotium_export_status", "export_id"),
        _ => ("plotium_tasks_get", "task_id"),
    };
    let query = json!({"tool":tool,"arguments":{key:value[key]},"retained":true});
    let status = value["status"].as_str().unwrap_or("");
    let terminal = match kind {
        "save" | "export" => status == "completed",
        _ => matches!(status, "succeeded" | "failed" | "cancelled" | "stale"),
    };
    let mut result = recovery(if terminal {
        json!({"kind":"inspect_outcome","automatic_resubmission":false})
    } else if status == "awaiting_authorization" {
        json!({"kind":"desktop_file_confirmation","receipt":query})
    } else {
        json!({"kind":"call_tool","tool":tool,"arguments":{key:value[key]}})
    });
    result["receipt"] = query;
    result["plan_consumed"] = if kind == "task" && !value["plan_id"].is_string() {
        Value::Null
    } else {
        json!(true)
    };
    result["submission"] = json!("submitted");
    if !terminal {
        result["completion"] = json!("pending");
        // Running file workers may already have installed output.
        if status == "awaiting_authorization" || status == "queued" {
            result["committed"] = json!(false);
        }
    } else if kind == "save" {
        match value["phase"].as_str() {
            Some("committed") => {
                result["committed"] = json!(true);
                result["completion"] = json!("succeeded");
            }
            Some("not_installed") => {
                result["committed"] = json!(false);
                result["completion"] = json!("failed");
            }
            _ => {}
        }
    } else if kind == "export" {
        if let Some(files) = value["files"].as_array() {
            let succeeded = files.iter().filter(|f| f["status"] == "succeeded").count();
            let failed = files.iter().filter(|f| f["status"] == "failed").count();
            result["completion"] = json!(if !files.is_empty() && succeeded == files.len() {
                "succeeded"
            } else if succeeded > 0 && succeeded + failed == files.len() {
                "partial"
            } else if failed == files.len() && !files.is_empty() {
                "failed"
            } else {
                "unknown"
            });
            // A failed file may have been installed before a durability error.
            if !files.is_empty() && succeeded == files.len() {
                result["committed"] = json!(true);
            }
        }
    } else {
        result["completion"] = json!(if status == "succeeded" {
            "succeeded"
        } else {
            "failed"
        });
        result["committed"] = json!(status == "succeeded");
    }
    if result["completion"] == "succeeded" {
        result["next_action"] = json!({"kind":"none"});
    }
    value["recovery"] = result;
}

pub(super) fn annotate(value: &mut Value) {
    for kind in ["save", "export", "task"] {
        if value[kind].is_object() {
            receipt(kind, &mut value[kind]);
        }
    }
    if value["value"].is_object() {
        annotate(&mut value["value"]);
    }
    if let Some(tasks) = value.get_mut("tasks").and_then(Value::as_array_mut) {
        for task in tasks {
            receipt("task", task);
        }
    }
}

#[cfg(test)]
#[path = "recovery_tests.rs"]
mod tests;
