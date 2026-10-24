use super::*;

pub(super) fn annotate(app: &PlotiumApp, name: &str, value: &mut Value) {
    // Receipt revisions describe their own lifecycle, possibly in another project.
    // A submitted task's after_revision is not evidence of a committed edit.
    if name == "plotium_execute"
        && value["tool_id"].is_string()
        && value["before_revision"].is_number()
        && value["after_revision"] == json!(app.doc.automation_revision)
        && value.pointer("/value/task").is_none()
        && value.get("save").is_none()
        && value.get("export").is_none()
    {
        value["project_context"] = json!(app.session.project_context);
    }
}

#[cfg(test)]
#[path = "result_context_tests.rs"]
mod tests;
