use plotium_core::automation::{
    CallerType, DocumentRevision, ExecutionAuthority, TargetSelector, ToolRequest, execute_tool,
    plan_tool,
};
use plotium_core::state::{ActivitySourceId, PlotiumApp};

pub(super) struct Capture {
    source: ActivitySourceId,
    paused: bool,
}

impl Capture {
    pub(super) fn new(app: &mut PlotiumApp, paused: bool) -> Self {
        let capture = Self {
            source: ActivitySourceId::default(),
            paused,
        };
        capture.refresh(app);
        if paused && let Some(canvas) = app.doc.canvases.first() {
            let request = ToolRequest {
                tool_id: "resource.rename".into(),
                tool_version: 1,
                parameters: serde_json::json!({"name": "Updated figure"}),
                targets: TargetSelector::Explicit {
                    ids: vec![canvas.resource_id.to_string()],
                },
                expected_revision: DocumentRevision(app.doc.automation_revision),
                caller: CallerType::Agent,
            };
            let previous = app.session.background_activity.execution_source;
            app.session.background_activity.execution_source = Some(capture.source);
            let result = plan_tool(app, request)
                .and_then(|plan| execute_tool(app, plan, ExecutionAuthority::ReversibleModify));
            app.session.background_activity.execution_source = previous;
            if let Err(error) = result {
                log::error!("Could not prepare paused-results capture: {error}");
                app.session.status = format!("Could not prepare paused-results capture: {error}");
            }
        }
        capture
    }

    pub(super) fn refresh(&self, app: &mut PlotiumApp) {
        // Synthetic scenes have no live AI connection to keep the source bound.
        app.session
            .result_follow
            .synchronize(app.session.project_context, Some(self.source));
        app.session.result_follow.resume();
        if self.paused {
            app.session
                .result_follow
                .pause("You navigated or edited the workspace.");
        }
    }
}
