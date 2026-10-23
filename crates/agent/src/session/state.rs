use super::*;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StateQuery {
    #[serde(default)]
    selection_offset: usize,
    #[serde(default = "default_selection_limit")]
    selection_limit: usize,
    #[serde(default)]
    hand_edits_offset: usize,
    #[serde(default = "default_selection_limit")]
    hand_edits_limit: usize,
}

fn default_selection_limit() -> usize {
    16
}

impl Session {
    pub(super) fn get_state(
        &self,
        app: &PlotiumApp,
        arguments: Value,
        blocked: bool,
    ) -> Result<Value, Failure> {
        let query: StateQuery = decode(arguments)?;
        let limit = query.selection_limit;
        if !(1..=50).contains(&limit) {
            return Err(Failure::new(
                "invalid_parameters",
                "selection_limit must be 1..=50",
            ));
        }
        if !(1..=50).contains(&query.hand_edits_limit) {
            return Err(Failure::new(
                "invalid_parameters",
                "hand_edits_limit must be 1..=50",
            ));
        }
        let busy = if blocked {
            Some(Failure::new(
                "busy",
                "Resolve recovery or save operation and retry",
            ))
        } else {
            ensure_project_ready(app).err()
        };
        let read_reason = busy.clone();
        let modify_reason = read_reason
            .clone()
            .or_else(|| ensure_table_edit_finished(app).err())
            .or_else(|| {
                (!self.allow_modify)
                    .then(|| Failure::new("permission_denied", "Switch to Read and modify"))
            });
        let (project, selection, hand_edits) = if busy.is_some() {
            (Value::Null, Value::Null, Value::Null)
        } else {
            let selected = ProjectResourceProvider::new(app).current_selection();
            let items: Vec<_> = selected
                .iter()
                .skip(query.selection_offset)
                .take(limit)
                .collect();
            let end = query.selection_offset.saturating_add(items.len());
            let next_offset = (end < selected.len()).then_some(end);
            (
                json!({"dataset_count":app.doc.datasets.len(),"canvas_count":app.doc.canvases.len()}),
                json!({"items":items,"total":selected.len(),"next_offset":next_offset}),
                plotium_core::automation::hand_edit_readings(
                    app,
                    query.hand_edits_offset,
                    query.hand_edits_limit,
                ),
            )
        };
        let availability = |reason: Option<Failure>| json!({"available":reason.is_none(),"reason":reason.map(|e|json!({"code":e.code,"message":e.message}))});
        Ok(
            json!({"project_context":app.session.project_context,"revision":app.doc.automation_revision,
            "availability":{"project_read":availability(read_reason),"project_modify":availability(modify_reason.clone()),"native_import":availability(modify_reason)},
            "project":project,"selection":selection,"hand_edits":hand_edits,"capability_fingerprint":crate::capabilities::fingerprint(),
            "permissions":{"allow_reversible_modify":self.allow_modify,"save":"per_request_desktop_authorization","export":"per_request_desktop_authorization"},
            "save_state":{"current_path":app.doc.project_path,"persisted_revision":app.doc.project_revision,"dirty":app.doc.dirty,"edit_generation":app.doc.edit_generation,
                "receipt_ids":app.session.external_saves.receipts().map(|r| &r.save_id).collect::<Vec<_>>()}}),
        )
    }
}
