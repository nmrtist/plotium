use super::*;

impl Session {
    pub(super) fn get_state(&self, app: &mut PlotiumApp, blocked: bool) -> Result<Value, Failure> {
        let busy = if blocked {
            Some(Failure::new(
                "busy",
                "Resolve recovery or save operation and retry",
            ))
        } else {
            ensure_not_busy(app).err()
        };
        let read_reason = busy.clone();
        let modify_reason = read_reason.clone().or_else(|| {
            (!self.allow_modify)
                .then(|| Failure::new("permission_denied", "Switch to Read and modify"))
        });
        let (project, selection) = if busy.is_some() {
            (Value::Null, Value::Null)
        } else {
            let selection = json!(ProjectResourceProvider::new(app).current_selection());
            let revision = DocumentRevision(app.doc.automation_revision);
            let plan = plan_tool(
                app,
                request("project.get_blueprint", json!({}), vec![], revision),
            )?;
            (
                execute_tool(app, plan, ExecutionAuthority::Read)?.value,
                selection,
            )
        };
        let availability = |reason: Option<Failure>| json!({"available":reason.is_none(),"reason":reason.map(|e|json!({"code":e.code,"message":e.message}))});
        Ok(
            json!({"project_context":app.session.project_context,"revision":app.doc.automation_revision,
            "availability":{"project_read":availability(read_reason),"project_modify":availability(modify_reason.clone()),"native_import":availability(modify_reason)},
            "project":project,"selection":selection,"connection_capabilities":crate::capabilities::manifest(self.allow_modify),
            "permissions":{"allow_reversible_modify":self.allow_modify,"save":"per_request_desktop_authorization","export":"per_request_desktop_authorization"},
            "save_state":{"current_path":app.doc.project_path,"persisted_revision":app.doc.project_revision,"dirty":app.doc.dirty,"edit_generation":app.doc.edit_generation,"receipts":app.session.external_saves.receipts().collect::<Vec<_>>()}}),
        )
    }
}
