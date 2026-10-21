//! This connection's boundary, not the core registry's available algorithms.
use plotium_core::automation::{
    DOCUMENT_PAGE_DEFAULT, DOCUMENT_PAGE_MAX, DOCUMENT_RESPONSE_BYTES, document_root_target,
};
use serde_json::{Value, json};

pub(crate) const EDIT_OPERATIONS: [&str; 23] = [
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

pub(crate) fn manifest(allow_modify: bool) -> Value {
    let tools=crate::protocol::definitions().iter().map(|tool|{
        let (operations,result,targets)=match tool.name.as_ref(){
            "plotium_tasks_get"=>(vec!["tasks.get"],"{task:TaskReceipt}; retained in the application process; task_not_found never authorizes replay","TaskId; callable while busy or after project change"),
            "plotium_tasks_list"=>(vec!["tasks.list"],"{tasks:TaskReceipt[]}; retained in the application process","offset/limit; callable while busy or after project change"),
            "plotium_tasks_cancel"=>(vec!["tasks.cancel"],"{task:TaskReceipt}; task_not_found never authorizes replay","TaskId; callable while busy or after project change; requires reversible_modify"),
            "plotium_analysis_results_inspect"=>(vec!["analysis.results.inspect"],"ToolResult.value: paginated native DOSY/ILT columns, validity, counts and provenance","one result field ID"),
            "plotium_diffusion_inspect"=>(vec!["diffusion.inspect"],"ToolResult.value: {resolution_source,increment_count,input:DiffusionInputReport,field_requirement,executable,acquisition_evidence}","one dataset or field ID"),
            "plotium_processing_inspect"=>(vec!["processing.inspect"],"ToolResult.value: {dataset,axes:[{axis,pipeline,pending_apply}]}","one dataset ID"),
            "plotium_get_state"=>(vec!["project.get_blueprint"],"{project_context,revision,availability:{project_read,project_modify,native_import},project,selection,permissions,connection_capabilities,save_state}","application; project and selection are null while busy"),
            "plotium_resources_search"=>(vec!["resources.search"],"ToolResult.value: FrozenTargetSet","ResourceQuery"),
            "plotium_resources_inspect"=>(vec!["resources.inspect"],"ToolResult.value: ResourceDescriptor[]","explicit resource IDs"),
            "plotium_properties_list"=>(vec!["properties.list"],"{properties:[{id,label,scope,schema,access,applicability}]}","static; no target"),
            "plotium_properties_inspect"=>(vec!["properties.inspect"],"ToolResult.value: {property,canonical_label,tier,aggregate,readings}; ToolResult.targets reports skips","ids expand by property, or exact Series TargetRef[]"),
            "plotium_plan"=>(EDIT_OPERATIONS.to_vec(),"{plan_id,preflight:ToolPlan|save_preflight|export_preflight,save_id?,export_id?,import?:{status,items},transform?:{status,output},update?:{status,output,before_table_revision,changed}}; native import or table_data_only import/transform; import omits ids/targets, transform requires one DatasetId in ids; prepared is not committed","rename: dataset/canvas/content IDs; properties: ids or exact Series targets; transform: one DatasetId; creation: explicit parent inside parameters, no top-level selector"),
            "plotium_execute"=>(EDIT_OPERATIONS.to_vec(),"ToolResult for edits; {save:SaveReceipt} or {export:ExportReceipt} for external writes. Submitted plan is consumed even on failure","one-use plan_id"),
            "plotium_document_inspect"=>(vec!["document.inspect"],"ToolResult.value: self {revision,node:{target,kind,name,visible,ancestors,child_count,...}}; children {revision,target,items,total,next_offset,truncated}","document, canvas, panel, content or exact Series TargetRef"),
            "plotium_table_columns"=>(vec!["table.columns"],"ToolResult.value: {dataset_id,table_id,table_revision,snapshot_fingerprint,fields,columns,total_rows,diagnostics,inference,freshness,analyses,provenance?}","explicit DatasetId and table RevisionId"),
            "plotium_table_read_rows"=>(vec!["table.read_rows"],"ToolResult.value: {dataset_id,table_id,table_revision,revision,column_ids,offset,requested_rows,returned_rows,total_rows,next_offset,end,rows:[{row_id,values,sources?}]} ; typed 64-bit integer values are decimal strings","explicit DatasetId, table RevisionId and ColumnId projection"),
            "plotium_figure_diagnose"=>(vec!["figure.diagnose"],"ToolResult.value: {project_context,basis_revision,canvas,output,items,coverage,budget,truncated,continuation}; output_page_pt; estimates and potential conflicts are labeled; does not modify project audit or dirty state","exact canvas and output.size_mm, optional exact content/Series and overlap_targets; or one-use cursor"),
            "plotium_preview_plan"=>(vec!["render.preview_image"],"MCP PNG image plus ToolResult.value: {plan_id,basis_project_context,basis_revision,candidate:true,target,output,width_px,height_px,mime_type}; no render_revision","existing supported plan and one affected whole canvas"),
            "plotium_render_preview"=>(vec!["render.preview_image"],"MCP PNG image content plus ToolResult.value: {target,render_revision,width_px,height_px,mime_type}","one exact whole-canvas TargetRef"),
            "plotium_export_status"=>(vec!["figure.export.status"],"{export:ExportReceipt,current_project_context,current_revision,matches_current_project}; non-auditing per-file results","opaque export_id; last 16 batches retained across reconnect/project changes within this desktop process"),
            "plotium_save_status"=>(vec!["project.save.status"],"{save:SaveReceipt,current_project_context,current_revision,current_dirty,current_path,matches_current_project}; no audit mutation","opaque save_id; retained across reconnect/project changes in this desktop process"),
            _=>unreachable!("all connection tools have discovery contracts"),
        };
        json!({"name":tool.name,"operations":operations,"input_schema":tool.input_schema,"annotations":tool.annotations,
            "result_contract":result,"targets":targets,
            "required_permission":if tool.name=="plotium_execute"{"operation_specific: reversible_modify or per_request_desktop_save/export_authorization"}else if matches!(tool.name.as_ref(),"plotium_preview_plan"|"plotium_tasks_cancel") {"reversible_modify"} else{"read"},
            "currently_permitted":!matches!(tool.name.as_ref(),"plotium_preview_plan"|"plotium_tasks_cancel") || allow_modify})
    }).collect::<Vec<_>>();
    json!({"tools":tools,"editable_operations":EDIT_OPERATIONS,
        "operation_constraints":{"plot.field.create":{"schema":plotium_core::automation::scientific_plot_schema(),"currently_permitted":allow_modify},"analysis.diffusion.plot":{"schema":plotium_core::automation::scientific_plot_schema(),"currently_permitted":allow_modify},"figure.export":{"schema":plotium_core::export::figure_export_schema(),"preflight_permission":"read","execute_permission":"per_request_desktop_export_authorization","currently_permitted":true,"result_contract":"plan returns export_id and per-file preflight; execute requests independent desktop consent. Poll plotium_export_status. Each file reports success/failure independently; no automatic retry or Undo for disk effects."},"project.save":{"schema":crate::session::save::schema(),"preflight_permission":"read","execute_permission":"per_request_desktop_save_authorization","currently_permitted":true,"result_contract":"plan returns save_id; execute queues a desktop authorization prompt, not a completed save. Poll plotium_save_status. Existing targets require explicit replacement confirmation. Saving is not undoable."},"plot.series.append":{"preflight_permission":"reversible_modify","execute_permission":"reversible_modify","currently_permitted":allow_modify,"schema":plotium_core::automation::curve_append_schema(),"result_contract":"binding_plan.appended contains frozen new Series targets; existing bindings are retained; 200 total curves and shared binding row/plan/response budgets apply."},"table.patch":{"preflight_permission":"reversible_modify","execute_permission":"reversible_modify","currently_permitted":allow_modify,"schema":plotium_core::automation::table_patch_schema()},"table.refresh":{"preflight_permission":"reversible_modify","execute_permission":"reversible_modify","currently_permitted":allow_modify,"schema":plotium_core::automation::table_refresh_schema()},"figure.edit":{"preflight_permission":"reversible_modify","execute_permission":"reversible_modify","currently_permitted":allow_modify,"schema":plotium_core::automation::figure_edit_schema(),"result_contract":"ToolPlan.figure_plan/result and ToolResult.value contain proposed created/modified exact targets, size_mm, margins/gutter, panel_frames final content_frames and ordered content_edits with requested/effective frames, parent/page coordinates and asset changes. content_frame edits only that item, not its panel; image_replace changes only its asset reference. Frozen native assets commit once atomically; one Undo/Redo restores content and IDs. Deletion-only plans report deleted_targets, deletions, remaining_series, parent/group/viewport_link/selection changes, retained_asset_ids and native auto_height changes. Reject duplicates, overlaps, nonempty panels, last Series, default/dynamic bindings and mixing deletion with other edits. Source datasets/assets remain; empty parents remain. Selection-effect changes require new preflight. No automatic reflow."},"plot.create":{"preflight_permission":"reversible_modify","execute_permission":"reversible_modify","currently_permitted":allow_modify,"schema":plotium_core::automation::plot_create_schema(),"result_contract":"ToolPlan.binding_plan and ToolResult.value contain creation {parent,canvas,panel,plot,created,size_mm,frame_pt,panel_frame_pt,coordinate_units,series_defaults}, and bindings [{target,source,diagnostics}]. Preflight IDs are proposed, not committed. One Undo removes only created content; Redo restores the same IDs. After response loss query proposed IDs/current revision before creating again."},"data.transform":{"preflight_permission":"reversible_modify","execute_permission":"reversible_modify","currently_permitted":allow_modify,"schema":plotium_core::automation::table_transform_schema()},"plot.series.bind":{"preflight_permission":"reversible_modify","execute_permission":"reversible_modify","currently_permitted":allow_modify,"schema":plotium_core::automation::curve_binding_schema(false)},"plot.series.materialize_default":{"preflight_permission":"reversible_modify","execute_permission":"reversible_modify","currently_permitted":allow_modify,"schema":plotium_core::automation::curve_binding_schema(true)},"data.import":{"schema":plotium_core::automation::data_import_schema(),"native_from_welcome":true,"preflight_permission":"reversible_modify","execute_permission":"reversible_modify","currently_permitted":allow_modify}},
        "scientific_operations":plotium_core::automation::ToolRegistry::built_in().descriptors().filter(|d|d.id.starts_with("processing.")||d.id.starts_with("diffusion.")||d.id.starts_with("analysis.")).map(|d|json!({"descriptor":d,"currently_permitted":d.effect==plotium_core::automation::EffectLevel::ReadOnly||allow_modify})).collect::<Vec<_>>(),
        "candidate_preview":plotium_core::automation::plan_preview_contract(),
        "guide_contract": {"targets":"ids: explicit plot object IDs; no Series components",
            "write_policy":"strict_all_targets; existing non-guide property skip behavior is unchanged",
            "permission":"preflight/read permitted; execute requires reversible_modify",
            "coordinates":"origin within max(plot_size - legend_size, 0); x right, y down; not pixels or page fractions",
            "coupling":"position [x,y] sets inside/custom; placement custom retains origin or initializes [1,0]; other placements clear origin; placement Auto/reset and position null/reset clear both origin and placement, retaining layout/title/visibility/fonts/axes",
            "geometry":"properties_inspect guide_geometry uses object_local_pt at scale 1; layout text widths are estimates; fits_plot reports size fit, not clipping or collision diagnostics",
            "position_applicability":"categorical legend content independent of guide visibility; retained custom origins remain readable/resettable without content; not free-positioned colorbars",
            "limits":{"targets":200,"title_utf8_bytes":16384,"response_bytes":262144}},
        "supported_properties":crate::session::PROPERTIES.iter().map(|id|id.as_str()).collect::<Vec<_>>(),
        "resource_capabilities":"Core resource capabilities describe data/rendering semantics, not operations exposed by this connection. Only the operations above can be called here.",
        "document_root":document_root_target(),
        "limits":{"figure_diagnostic":plotium_core::automation::figure_diagnostic_limits(),"figure_export":{"outputs":8,"dpi_max":2400,"png_edge":16384,"png_pixels":16777216,"asset_bytes":16777216,"decoded_asset_bytes":67108864,"svg_bytes":8388608,"output_bytes":67108864,"in_flight":1,"receipts":16},"table_update":plotium_core::automation::table_update_limits(),"figure_edit":{"max_edits":200,"max_deletion_targets":200,"deletion_curve_rows":2000000,"max_panels":200,"grid_rows_cols_max":16,"max_images":8,"image_file_bytes":1048576,"image_batch_bytes":4194304,"image_decoded_bytes":4194304,"text_utf8_bytes":16384},"plot_creation":{"max_plots":1,"max_series":200,"max_curve_rows":2000000,"pending_curve_rows_shared_with_bindings":2000000,"response_bytes":256*1024},"table_transform":plotium_core::automation::table_transform_limits(),"curve_binding":{"max_edits":200,"max_curve_rows":2000000,"pending_curve_rows":2000000,"response_bytes":256*1024,"diagnostic_rows":20},"document_page_default":DOCUMENT_PAGE_DEFAULT,"document_page_max":DOCUMENT_PAGE_MAX,"document_response_bytes":DOCUMENT_RESPONSE_BYTES,"pending_plans":32,"image_preview":plotium_core::automation::image_preview_limits(),"table_read":plotium_core::automation::table_read_limits(),"native_import":plotium_core::automation::native_import_limits(),"table_import":plotium_core::automation::table_import_limits()},
        "revision_contract":"Fetch get_state after edits or Undo. Revision-bound reads and plans reject stale revisions; execute rechecks permission and revision. Queries do not append project run records or change dirty state, edit generation or revision."})
}
