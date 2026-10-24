//! This connection's boundary, not the core registry's available algorithms.
use plotium_core::automation::{
    DOCUMENT_PAGE_DEFAULT, DOCUMENT_PAGE_MAX, DOCUMENT_RESPONSE_BYTES, document_root_target,
};
use serde_json::{Value, json};

pub(crate) const EDIT_OPERATIONS: [&str; 25] = [
    "analysis.diffusion.plot",
    "plot.field.create",
    "analysis.dosy.build",
    "analysis.regions.extract",
    "analysis.diffusion.fit",
    "processing.edit",
    "processing.apply_scheme",
    "diffusion.set_input",
    "project.save",
    "figure.export",
    "resource.rename",
    "dataset.delete",
    "figure.apply_theme",
    "properties.set",
    "properties.set_many",
    "properties.reset",
    "data.import",
    "data.transform",
    "table.patch",
    "table.refresh",
    "plot.series.bind",
    "plot.create",
    "plot.series.append",
    "figure.edit",
    "plot.series.materialize_default",
];

/// Detail sections served by `plotium_capabilities_get {section,key?}`.
pub(crate) const SECTIONS: [&str; 7] = [
    "tools",
    "operation_constraints",
    "operation_details",
    "scientific_operations",
    "guide_contract",
    "candidate_preview",
    "limits",
];
/// Sections that require selecting entries instead of returning the entire section.
const KEY_REQUIRED: [&str; 3] = [
    "operation_constraints",
    "scientific_operations",
    "operation_details",
];
const KEYED: [&str; 5] = [
    "tools",
    "operation_constraints",
    "operation_details",
    "scientific_operations",
    "limits",
];
/// Operations that name their targets inside parameters and omit ids/targets.
pub(crate) const NO_SELECTOR: [&str; 11] = [
    "data.import",
    "project.save",
    "figure.export",
    "figure.edit",
    "plot.create",
    "plot.series.append",
    "plot.series.bind",
    "plot.series.materialize_default",
    "plot.field.create",
    "analysis.diffusion.plot",
    "properties.set_many",
];
/// Table operations that take exactly one DatasetId in ids.
pub(crate) const SINGLE_DATASET: [&str; 3] = ["table.patch", "table.refresh", "data.transform"];

fn tool_index() -> Vec<Value> {
    crate::protocol::definitions().iter().map(|tool|{
        let (operations,result,targets)=match tool.name.as_ref(){
            "plotium_tasks_get"=>(vec!["tasks.get"],"{task:TaskReceipt}; retained in the application process; task_not_found never authorizes replay","TaskId; callable while busy or after project change"),
            "plotium_tasks_list"=>(vec!["tasks.list"],"{tasks:TaskReceipt[]}; retained in the application process","offset/limit; callable while busy or after project change"),
            "plotium_tasks_cancel"=>(vec!["tasks.cancel"],"{task:TaskReceipt}; task_not_found never authorizes replay","TaskId; callable while busy or after project change; requires reversible_modify"),
            "plotium_analysis_results_inspect"=>(vec!["analysis.results.inspect"],"ToolResult.value: paginated native DOSY/ILT columns, validity, counts and provenance","one result field ID"),
            "plotium_diffusion_inspect"=>(vec!["diffusion.inspect"],"ToolResult.value: {resolution_source,increment_count,input:DiffusionInputReport,field_requirement,executable,acquisition_evidence}","one dataset or field ID"),
            "plotium_processing_inspect"=>(vec!["processing.inspect"],"ToolResult.value: {dataset,axes:[{axis,pipeline,pending_apply}]}","one dataset ID"),
            "plotium_get_state"=>(vec!["state.get"],"{project_context,revision,availability,project:{dataset_count,canvas_count},selection:{items,total,next_offset,canvas:{items:[{target,kind,name,visible}],total,part}},hand_edits:{items:[{target,key,label,value}],total,next_offset},permissions,capability_fingerprint,save_state}; latest state per call, not a frozen selection snapshot","application; project, selection and hand_edits are null while busy"),
            "plotium_capabilities_get"=>(vec!["capabilities.get"],"{} returns {capability_fingerprint,connection_capabilities:index}; {section,key?} returns {capability_fingerprint,section,key?,contract}; immutable contracts, not current authorization","static; no project or revision required; callable while busy"),
            "plotium_resources_search"=>(vec!["resources.search"],"ToolResult.value: FrozenTargetSet","ResourceQuery"),
            "plotium_resources_inspect"=>(vec!["resources.inspect"],"ToolResult.value: ResourceDescriptor[]","explicit resource IDs"),
            "plotium_properties_list"=>(vec!["properties.list"],"{detail,properties:[{id,label,scope,schema,unit,access,applicability}]}; compact by default, keys/scope filter, detail full returns complete definitions","static; no target"),
            "plotium_properties_inspect"=>(vec!["properties.inspect"],"ToolResult.value: {property,canonical_label,tier,aggregate,readings}; ToolResult.targets reports skips","ids expand by property, or exact Series TargetRef[]"),
            "plotium_plan"=>(EDIT_OPERATIONS.to_vec(),"{plan_id,preflight:ToolPlan|property_preflight{required_authority,estimated_changes,summary:{compatible,skipped,failed},targets:non-compatible PlannedTarget[]}|save_preflight|export_preflight,save_id?,export_id?,import?:{status,items},transform?:{status,output},update?:{status,output,before_table_revision,changed}}; native import or table_data_only import/transform; import omits ids/targets, transform requires one DatasetId in ids; prepared is not committed","rename: dataset/canvas/content IDs; properties: ids or exact Series targets; transform: one DatasetId; creation: explicit parent inside parameters, no top-level selector"),
            "plotium_execute"=>(EDIT_OPERATIONS.to_vec(),"ToolResult for edits; properties.set/set_many/reset add summary {succeeded,skipped,failed,by_reason} and keep only non-succeeded targets and value.writes; {save:SaveReceipt} or {export:ExportReceipt} for external writes. Submitted plan is consumed even on failure","one-use plan_id"),
            "plotium_document_inspect"=>(vec!["document.inspect"],"ToolResult.value: self {revision,node:{target,kind,name,visible,ancestors,child_count,...}}; children {revision,target,items,total,next_offset,truncated}","document, canvas, panel, content or exact Series TargetRef"),
            "plotium_table_columns"=>(vec!["table.columns"],"ToolResult.value: {dataset_id,table_id,table_revision,snapshot_fingerprint,fields,columns,total_rows,diagnostics,inference,freshness,analyses,provenance?}","explicit DatasetId and table RevisionId"),
            "plotium_table_read_rows"=>(vec!["table.read_rows"],"ToolResult.value: {dataset_id,table_id,table_revision,revision,column_ids,offset,requested_rows,returned_rows,total_rows,next_offset,end,rows:[{row_id,values,sources?}]} ; typed 64-bit integer values are decimal strings","explicit DatasetId, table RevisionId and ColumnId projection"),
            "plotium_figure_diagnose"=>(vec!["figure.diagnose"],"ToolResult.value: {project_context,basis_revision,canvas,summary:{issues,by_code,by_severity},output,items,coverage,budget,truncated,continuation}; view issues and detail compact by default; output_page_pt; estimates and potential conflicts are labeled; does not modify project audit or dirty state","exact canvas and output.size_mm, optional exact content/Series and overlap_targets; or one-use cursor"),
            "plotium_preview_plan"=>(vec!["render.preview_image"],"MCP PNG image plus ToolResult.value: {plan_id,basis_project_context,basis_revision,candidate:true,target,output,width_px,height_px,mime_type,downsampled?}; no render_revision; downsampled lists [{target,source_points,drawn_points}] for curves thinned to preview pixels; exports draw every point","existing supported plan and one affected whole canvas"),
            "plotium_render_preview"=>(vec!["render.preview_image"],"MCP PNG image content plus ToolResult.value: {target,render_revision,width_px,height_px,mime_type,downsampled?}; downsampled lists [{target,source_points,drawn_points}] for curves thinned to preview pixels; exports draw every point","one exact whole-canvas TargetRef"),
            "plotium_export_status"=>(vec!["figure.export.status"],"{export:ExportReceipt,current_project_context,current_revision,matches_current_project}; non-auditing per-file results","opaque export_id; last 16 batches retained across reconnect/project changes within this desktop process"),
            "plotium_save_status"=>(vec!["project.save.status"],"{save:SaveReceipt,current_project_context,current_revision,current_dirty,current_path,matches_current_project}; no audit mutation","opaque save_id; retained across reconnect/project changes in this desktop process"),
            _=>unreachable!("all connection tools have discovery contracts"),
        };
        let mut entry = json!({"name":tool.name,"operations":operations,
            "result_contract":result,"targets":targets,
            "required_permission":if tool.name=="plotium_execute"{"operation_specific: reversible_modify or per_request_desktop_save/export_authorization"}else if tool.name=="plotium_plan"{"operation_specific: read or reversible_modify"}else if matches!(tool.name.as_ref(),"plotium_preview_plan"|"plotium_tasks_cancel") {"reversible_modify"} else{"read"}});
        // plan and execute accept every editable operation; the index lists them once.
        if matches!(tool.name.as_ref(), "plotium_plan" | "plotium_execute") {
            entry.as_object_mut().expect("local entry").remove("operations");
        }
        entry
    }).collect()
}

fn operation_constraints() -> serde_json::Map<String, Value> {
    let Value::Object(mut constraints) = json!({"figure.apply_theme":theme_constraint(),"plot.field.create":{"schema":plotium_core::automation::scientific_plot_schema(),"preflight_permission":"reversible_modify","execute_permission":"reversible_modify"},"analysis.diffusion.plot":{"schema":plotium_core::automation::scientific_plot_schema(),"preflight_permission":"reversible_modify","execute_permission":"reversible_modify"},"figure.export":{"schema":plotium_core::export::figure_export_schema(),"preflight_permission":"read","execute_permission":"per_request_desktop_export_authorization","result_contract":"plan returns export_id and per-file preflight; execute requests independent desktop consent. Poll plotium_export_status. Each file reports success/failure independently; no automatic retry or Undo for disk effects."},"project.save":{"schema":crate::session::save::schema(),"preflight_permission":"read","execute_permission":"per_request_desktop_save_authorization","result_contract":"plan returns save_id; execute queues a desktop authorization prompt, not a completed save. Poll plotium_save_status. Existing targets require explicit replacement confirmation. Saving is not undoable."},"plot.series.append":{"preflight_permission":"reversible_modify","execute_permission":"reversible_modify","schema":plotium_core::automation::curve_append_schema(),"result_contract":"binding_plan.appended contains frozen new Series targets; existing bindings are retained; 200 total curves and shared binding row/plan/response budgets apply."},"table.patch":{"preflight_permission":"reversible_modify","execute_permission":"reversible_modify","schema":plotium_core::automation::table_patch_schema()},"table.refresh":{"preflight_permission":"reversible_modify","execute_permission":"reversible_modify","schema":plotium_core::automation::table_refresh_schema()},"figure.edit":{"preflight_permission":"reversible_modify","execute_permission":"reversible_modify","schema":plotium_core::automation::figure_edit_schema(),"result_contract":"ToolPlan.figure_plan/result and ToolResult.value contain proposed created/modified exact targets, size_mm, margins/gutter, panel_frames final content_frames and ordered content_edits with requested/effective frames, parent/page coordinates and asset changes. annotation_edits list each annotation op with its plot target, op, plot-local annotation_id and resulting annotation. panel_edits list each panel_stack op with its layout, gap_pt, members_top_to_bottom and merged_panels. content_frame edits only that item, not its panel (in a shared-x stack the siblings reflow to its new height share); image_replace changes only its asset reference. Frozen native assets commit once atomically; one Undo/Redo restores content and IDs. Deletion-only plans report deleted_targets, deletions, remaining_series, parent/group/viewport_link/selection changes, retained_asset_ids and native auto_height changes. Reject duplicates, overlaps, nonempty panels, last Series, default/dynamic bindings and mixing deletion with other edits. Source datasets/assets remain; empty parents remain. Selection-effect changes require new preflight. No automatic reflow."},"plot.create":{"preflight_permission":"reversible_modify","execute_permission":"reversible_modify","schema":plotium_core::automation::plot_create_schema(),"result_contract":"ToolPlan.binding_plan contains creation {parent,canvas,panel,plot,created,size_mm,frame_pt,panel_frame_pt,coordinate_units,series_defaults}, and bindings [{target,source,diagnostics}]. ToolResult.value contains creation {canvas,panel,plot,size_mm,size_preset?,frame_pt,panel_frame_pt,coordinate_units,series_defaults} and bindings [{target,diagnostics:{total_rows,plotted_rows,missing_xy,non_finite_xy,missing_error}}]; ToolResult.targets lists every created target. Preflight IDs are proposed, not committed. One Undo removes only created content; Redo restores the same IDs. After response loss query proposed IDs/current revision before creating again."},"data.transform":{"preflight_permission":"reversible_modify","execute_permission":"reversible_modify","schema":plotium_core::automation::table_transform_schema()},"plot.series.bind":{"preflight_permission":"reversible_modify","execute_permission":"reversible_modify","schema":plotium_core::automation::curve_binding_schema(false)},"plot.series.materialize_default":{"preflight_permission":"reversible_modify","execute_permission":"reversible_modify","schema":plotium_core::automation::curve_binding_schema(true)},"data.import":{"schema":plotium_core::automation::data_import_schema(),"native_from_welcome":true,"preflight_permission":"reversible_modify","execute_permission":"reversible_modify"}})
    else {
        unreachable!("literal object")
    };
    let registry = plotium_core::automation::ToolRegistry::built_in();
    for id in EDIT_OPERATIONS {
        if constraints.contains_key(id) {
            continue;
        }
        let schema = &registry
            .get(id)
            .expect("editable operations are registered")
            .parameter_schema;
        // Scientific edits start background work at preflight, so planning them needs modify.
        let scientific = id.starts_with("processing.")
            || id.starts_with("diffusion.")
            || id.starts_with("analysis.");
        constraints.insert(id.into(), json!({"schema":schema,"preflight_permission":if scientific {"reversible_modify"} else {"read"},"execute_permission":"reversible_modify"}));
    }
    constraints["dataset.delete"]["result_contract"] = json!(
        "Refused at preflight, naming the users, while a plot, projection, derived dataset or report uses a dataset; delete a derived dataset in its own request first. One Undo restores the datasets and their IDs."
    );
    constraints
}

fn sections() -> serde_json::Map<String, Value> {
    let constraints = operation_constraints();
    let details = crate::operation_details::build(&constraints["figure.edit"]);
    let scientific = plotium_core::automation::ToolRegistry::built_in().descriptors()
        .filter(|d|d.id.starts_with("processing.")||d.id.starts_with("diffusion.")||d.id.starts_with("analysis."))
        .map(|d| {
            let mut entry = json!({"descriptor":d,"required_permission":if d.effect==plotium_core::automation::EffectLevel::ReadOnly{"read"}else{"reversible_modify"}});
            if EDIT_OPERATIONS.contains(&d.id.as_str()) {
                entry["descriptor"].as_object_mut().expect("descriptor").remove("parameter_schema");
                entry["input_schema_ref"] = json!({"section":"operation_constraints","key":d.id});
            }
            (d.id.clone(), entry)
        }).collect::<serde_json::Map<_,_>>();
    let tools = tool_index().into_iter().map(|mut tool| {
        let name = tool["name"].as_str().expect("tool names are strings").to_owned();
        let entry = tool.as_object_mut().expect("local entry");
        entry.remove("name");
        (name, json!({"result_contract":entry.remove("result_contract"),"targets":entry.remove("targets")}))
    }).collect::<serde_json::Map<_,_>>();
    let Value::Object(sections) = json!({"tools":tools,"operation_constraints":constraints,"operation_details":details,"scientific_operations":scientific,
        "candidate_preview":plotium_core::automation::plan_preview_contract(),
        "guide_contract": {"targets":"ids: explicit plot object IDs; no Series components",
            "write_policy":"strict_all_targets; existing non-guide property skip behavior is unchanged",
            "permission":"preflight/read permitted; execute requires reversible_modify",
            "coordinates":"origin within max(plot_size - legend_size, 0); x right, y down; not pixels or page fractions",
            "coupling":"position [x,y] sets inside/custom; placement custom retains origin or initializes [1,0]; other placements clear origin; placement Auto/reset and position null/reset clear both origin and placement, retaining layout/title/visibility/fonts/axes",
            "geometry":"properties_inspect guide_geometry uses object_local_pt at scale 1; layout text widths are estimates; fits_plot reports size fit, not clipping or collision diagnostics",
            "position_applicability":"categorical legend content independent of guide visibility; retained custom origins remain readable/resettable without content; not free-positioned colorbars",
            "limits":{"targets":200,"title_utf8_bytes":16384,"response_bytes":262144}},
        "limits":{"figure_diagnostic":plotium_core::automation::figure_diagnostic_limits(),"figure_export":{"outputs":8,"dpi_max":2400,"png_edge":16384,"png_pixels":16777216,"asset_bytes":16777216,"decoded_asset_bytes":67108864,"svg_bytes":8388608,"output_bytes":67108864,"in_flight":1,"receipts":16},"table_update":plotium_core::automation::table_update_limits(),"figure_edit":{"max_edits":200,"max_deletion_targets":200,"deletion_curve_rows":2000000,"max_panels":200,"grid_rows_cols_max":16,"max_images":8,"image_file_bytes":1048576,"image_batch_bytes":4194304,"image_decoded_bytes":4194304,"text_utf8_bytes":16384},"plot_creation":{"max_plots":1,"max_series":200,"max_curve_rows":2000000,"pending_curve_rows_shared_with_bindings":2000000,"response_bytes":256*1024},"table_transform":plotium_core::automation::table_transform_limits(),"curve_binding":{"max_edits":200,"max_curve_rows":2000000,"pending_curve_rows":2000000,"response_bytes":256*1024,"diagnostic_rows":20},"document_page_default":DOCUMENT_PAGE_DEFAULT,"document_page_max":DOCUMENT_PAGE_MAX,"document_response_bytes":DOCUMENT_RESPONSE_BYTES,"pending_plans":32,"image_preview":plotium_core::automation::image_preview_limits(),"table_read":plotium_core::automation::table_read_limits(),"native_import":plotium_core::automation::native_import_limits(),"table_import":plotium_core::automation::table_import_limits()}})
    else {
        unreachable!("literal object")
    };
    sections
}

fn size(value: &Value) -> usize {
    serde_json::to_vec(value).map_or(0, |bytes| bytes.len())
}

fn build_index(sections: &serde_json::Map<String, Value>) -> Value {
    let constraints = &sections["operation_constraints"];
    // Grouped by permission pair: most operations share one of a few pairs.
    let mut operations: Vec<Value> = Vec::new();
    for id in EDIT_OPERATIONS {
        let (preflight, execute) = (
            &constraints[id]["preflight_permission"],
            &constraints[id]["execute_permission"],
        );
        match operations.iter_mut().find(|group| &group["preflight_permission"] == preflight && &group["execute_permission"] == execute) {
            Some(group) => group["operations"].as_array_mut().expect("local group").push(json!(id)),
            None => operations.push(json!({"preflight_permission":preflight,"execute_permission":execute,"operations":[id]})),
        }
    }
    let tools = tool_index()
        .into_iter()
        .map(|mut tool| {
            let entry = tool.as_object_mut().expect("local entry");
            entry.remove("result_contract");
            entry.remove("targets");
            tool
        })
        .collect::<Vec<_>>();
    let scientific = sections["scientific_operations"].as_object().expect("local section").values().map(|entry| {
        let d = &entry["descriptor"];
        json!({"id":d["id"],"title":d["title"],"effect":d["effect"],"required_permission":entry["required_permission"]})
    }).collect::<Vec<_>>();
    let listing = SECTIONS.iter().map(|name| {
        let detail = &sections[*name];
        let mut entry = json!({"bytes":size(detail),"key":if KEY_REQUIRED.contains(name) {"required"} else if KEYED.contains(name) {"optional"} else {"none"}});
        let entries = detail.as_object().expect("sections are objects");
        // Keys repeat ids the index already lists, so only the large schemas show per-key sizes.
        match *name {
            "operation_constraints" => entry["key_bytes"] = entries.iter().map(|(key, value)| (key.clone(), json!(size(value)))).collect::<serde_json::Map<_,_>>().into(),
            "scientific_operations" => entry["keys"] = json!("scientific_operations[].id"),
            "tools" => entry["keys"] = json!("tools[].name"),
            "operation_details" => entry["entries"] = entries.iter().map(|(key, value)| json!({"key":key,"purpose":value["purpose"]})).collect::<Vec<_>>().into(),
            "limits" => entry["keys"] = json!(entries.keys().collect::<Vec<_>>()),
            _ => {}
        }
        (name.to_string(), entry)
    }).collect::<serde_json::Map<_,_>>();
    json!({"tools":tools,"editable_operations":operations,"scientific_operations":scientific,
        "sections":listing,
        "properties":"Property definitions and ids are listed by plotium_properties_list.",
        "property_batch":{"operation":"properties.set_many","writes_min":1,"writes_max":200,"atomic":true,"undo_steps":1,"selectors":"exact target/key/value per write; omit top-level ids/targets","target_source":"plot.create ToolResult.value.bindings[].target and value.creation.plot; preserve full resource and component identity","unsupported_property":"error.details.write_index and key identify the rejected write"},
        "recovery_contract":{"location":"error.recovery or receipt.recovery","unknown":"null plan_consumed/committed and unknown submission/completion are not evidence of non-execution","retryable":"false: follow next_action, never replay the failed request","authorization":"recovery actions and annotations grant no permissions; file requests require fresh exact-path desktop consent"},
        "resource_capabilities":"Core resource capabilities describe data/rendering semantics, not operations exposed by this connection. Only the operations above can be called here.",
        "document_root":document_root_target(),
        "revision_contract":"Committed synchronous execute results include project_context and after_revision. Reuse them for known exact targets in that context; refresh state after Undo, desktop changes, uncertain outcomes or when current permissions/selection are needed. Submitted tasks are not committed; only matching succeeded terminal receipts with end_revision permit reuse. Revision-bound reads and plans reject stale revisions; execute rechecks permission and revision. Queries do not append project run records or change dirty state, edit generation or revision."})
}

/// Built-in themes are fixed in the binary, so they belong to the static
/// capability fingerprint; saved styles change at run time and are listed by a
/// failed preflight instead.
fn theme_constraint() -> Value {
    json!({"preflight_permission":"read","execute_permission":"reversible_modify",
        "schema":{"type":"object","additionalProperties":false,"required":["theme_id"],"properties":{"theme_id":{"type":"string","minLength":1,"description":"A built-in theme id or the name of a canvas style the user saved in Plotium"}}},
        "targets":"top-level ids including at least one CanvasId; other resources are skipped",
        "builtin_themes":plotium_core::theme::Theme::all().iter().map(|theme|json!({"id":theme.id,"name":theme.name})).collect::<Vec<_>>(),
        "saved_styles":"Not listed here. An unknown theme_id fails preflight with error.details.themes listing every built-in theme and saved style currently available. A saved style is read again when the plan executes.",
        "result_contract":"One Undo step restyles each canvas: page background, text and shape colours, and every curve's colour, marker paint, marker size and line width the theme defines. The document's style defaults are replaced too: plot text sizes, axis strokes and ticks (so plots on every canvas change) and the default text, panel-label and shape styles for new objects. Canvases are never resized; ToolResult.diagnostics notes each canvas whose width matches none of a journal theme's column presets (nature: 89 / 136 / 183 mm)."})
}

pub(crate) fn check_property(parameters: &Value) -> Result<(), crate::session::Failure> {
    if crate::session::PROPERTIES
        .iter()
        .any(|id| Some(id.as_str()) == parameters.get("key").and_then(Value::as_str))
    {
        Ok(())
    } else {
        Err(crate::session::Failure::new(
            "unsupported_property",
            "Property is outside the connection's supported display properties",
        ))
    }
}

/// Reject what would only fail at execution, so a typo never consumes a plan.
pub(crate) fn check_theme(
    plan: &plotium_core::automation::ToolPlan,
) -> Result<(), crate::session::Failure> {
    use plotium_core::automation::{TargetCompatibility, TargetSelector};
    let invalid = |message| Err(crate::session::Failure::new("invalid_parameters", message));
    if matches!(plan.request.targets, TargetSelector::Components { .. }) {
        return invalid("Theme application requires canvas ids, not Series targets");
    }
    let id = plan
        .request
        .parameters
        .get("theme_id")
        .and_then(Value::as_str);
    let dir = plotium_core::theme::styles_dir();
    let id = id.unwrap_or_default();
    if let Err(error) = plotium_core::theme::resolve_theme(dir.as_deref(), id) {
        // A broken style file is still listed, so pointing at the listing
        // would send the agent back to the same name.
        if dir
            .as_deref()
            .is_some_and(|dir| plotium_core::theme::user_style_exists(dir, id))
        {
            return Err(crate::session::Failure::new(
                "invalid_parameters",
                format!("saved style \"{}\" cannot be loaded: {error}", id.trim()),
            ));
        }
        let mut themes: Vec<Value> = plotium_core::theme::Theme::all()
            .iter()
            .map(|theme| json!({"id":theme.id,"name":theme.name,"source":"builtin"}))
            .collect();
        if let Some(dir) = &dir {
            themes.extend(
                plotium_core::theme::list_user_styles(dir)
                    .into_iter()
                    .map(|style| json!({"id":style.name,"name":style.name,"source":"saved"})),
            );
        }
        let mut failure = crate::session::Failure::new(
            "invalid_parameters",
            format!("{error}; choose a theme_id from error.details.themes"),
        );
        failure.details = Some(json!({ "themes": themes }));
        return Err(failure);
    }
    if !plan
        .targets
        .iter()
        .any(|target| target.status == TargetCompatibility::Compatible)
    {
        return invalid("Theme application requires at least one canvas id");
    }
    Ok(())
}

struct Discovery {
    index: Value,
    sections: serde_json::Map<String, Value>,
    fingerprint: String,
}

fn discovery() -> &'static Discovery {
    static DISCOVERY: std::sync::OnceLock<Discovery> = std::sync::OnceLock::new();
    DISCOVERY.get_or_init(|| {
        let sections = sections();
        let index = build_index(&sections);
        let fingerprint = fingerprint_of(&contract(&index, &sections));
        Discovery {
            index,
            sections,
            fingerprint,
        }
    })
}

/// Everything an agent may cache under one fingerprint: the index, every
/// detail section, the MCP tool definitions and the property catalog.
fn contract(index: &Value, sections: &serde_json::Map<String, Value>) -> Value {
    json!({"index":index,"sections":sections,"tools":crate::protocol::definitions(),"properties":crate::session::property_catalog(),"property_catalog_compact":crate::property_catalog::respond(json!({})).expect("static catalog")})
}

#[cfg(test)]
pub(crate) fn manifest() -> &'static Value {
    &discovery().index
}

pub(crate) fn fingerprint() -> &'static str {
    &discovery().fingerprint
}

/// Answer `plotium_capabilities_get`: the index, or one detail section entry.
pub(crate) fn respond(arguments: Value) -> Result<Value, crate::session::Failure> {
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Query {
        section: Option<String>,
        key: Option<String>,
        keys: Option<Vec<String>>,
    }
    if arguments.get("keys").is_some_and(|v| !v.is_array()) {
        return Err(
            crate::Failure::new("invalid_parameters", "keys must be an array").at_field("keys"),
        );
    }
    let invalid = |message: String, field: &str, values: Vec<String>| {
        let mut failure = crate::session::Failure::new("invalid_parameters", message);
        failure.details = Some(
            json!({ field: values, "field": if field == "sections" {"section"} else {"key"} }),
        );
        failure
    };
    let query: Query = serde_json::from_value(arguments)
        .map_err(|e| crate::session::Failure::new("invalid_parameters", e.to_string()))?;
    if query.key.is_some() && query.keys.is_some() {
        return Err(
            crate::Failure::new("invalid_parameters", "Supply key or keys, not both")
                .at_field("keys"),
        );
    }
    let discovery = discovery();
    let Some(section) = query.section else {
        if query.key.is_some() || query.keys.is_some() {
            return Err(invalid(
                "key requires section; choose a section from error.details.sections".into(),
                "sections",
                SECTIONS.map(String::from).to_vec(),
            ));
        }
        return Ok(
            json!({"capability_fingerprint":discovery.fingerprint,"connection_capabilities":discovery.index}),
        );
    };
    let Some(detail) = discovery.sections.get(&section) else {
        return Err(invalid(
            format!(
                "Unknown capability section \"{section}\"; choose one from error.details.sections"
            ),
            "sections",
            SECTIONS.map(String::from).to_vec(),
        ));
    };
    let keys = || match detail {
        Value::Object(entries) if KEYED.contains(&section.as_str()) => {
            entries.keys().cloned().collect()
        }
        _ => Vec::new(),
    };
    if let Some(requested) = query.keys {
        if section != "operation_details"
            || requested.is_empty()
            || requested.len() > 32
            || requested
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != requested.len()
        {
            return Err(crate::Failure::new(
                "invalid_parameters",
                "keys is a unique nonempty list of at most 32 operation_details keys",
            )
            .at_field("keys"));
        }
        let mut contracts = serde_json::Map::new();
        for key in &requested {
            let entry = detail.get(key).ok_or_else(|| {
                invalid(format!("Unknown operation detail {key}"), "keys", keys()).at_field("keys")
            })?;
            contracts.insert(key.clone(), entry.clone());
        }
        return Ok(
            json!({"capability_fingerprint":discovery.fingerprint,"section":section,"keys":requested,"contract":contracts}),
        );
    }
    let mut contract = match &query.key {
        None if KEY_REQUIRED.contains(&section.as_str()) => {
            return Err(invalid(
                format!("Section \"{section}\" requires a key; choose one from error.details.keys"),
                "keys",
                keys(),
            ));
        }
        None => detail.clone(),
        Some(key) => match detail.get(key) {
            Some(entry) if KEYED.contains(&section.as_str()) => entry.clone(),
            _ => {
                return Err(invalid(
                    format!(
                        "Unknown key \"{key}\" for section \"{section}\"; valid keys are in error.details.keys"
                    ),
                    "keys",
                    keys(),
                ));
            }
        },
    };
    if section == "tools"
        && let Some(key) = &query.key
        && let Some(tool) = crate::protocol::definitions()
            .into_iter()
            .find(|tool| tool.name == *key)
    {
        contract["input_schema"] = json!(tool.input_schema);
    }
    let mut response = json!({"capability_fingerprint":discovery.fingerprint,"section":section,"contract":contract});
    if let Some(key) = query.key {
        response["key"] = json!(key);
    }
    Ok(response)
}

fn fingerprint_of(contract: &Value) -> String {
    use sha2::{Digest, Sha256};
    // JSON object order is not part of the contract, including with preserve_order.
    fn canonical(value: &Value) -> Value {
        match value {
            Value::Object(object) => {
                let sorted: std::collections::BTreeMap<_, _> = object.iter().collect();
                Value::Object(
                    sorted
                        .into_iter()
                        .map(|(k, v)| (k.clone(), canonical(v)))
                        .collect(),
                )
            }
            Value::Array(items) => Value::Array(items.iter().map(canonical).collect()),
            other => other.clone(),
        }
    }
    let bytes = serde_json::to_vec(&canonical(contract)).expect("JSON contract is serializable");
    format!("sha256:{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
#[path = "capabilities_tests.rs"]
mod tests;
