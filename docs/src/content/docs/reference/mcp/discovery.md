---
title: MCP state and discovery
description: Read the current state, discover capabilities and find exact targets.
---

## Read state and discover capabilities on demand

`plotium_get_state {}` reads the latest dynamic state. It returns
`project_context`, `revision`, `availability`, `permissions`, `save_state`,
`capability_fingerprint`, project counts and a bounded selection page. It does not
include the full capability catalog or project blueprint. Discover exact targets
with the revision-bound `plotium_document_inspect` pages or
`plotium_resources_search`, then inspect explicit IDs with
`plotium_resources_inspect`. Copy complete returned targets rather than names or
collection positions.

Two short forms save copying. A resource reference may omit `parent_id` and
`local_id`; Plotium derives them from a nested ID such as `CANVAS_UUID/OBJECT_ID`.
A Series target may be written `{"plot":"CANVAS_UUID/OBJECT_ID","series":0}` in
plan and property-inspection targets and in curve-binding and `properties.set_many`
writes. Both expand to the full target before planning; plans, results and audit
records keep the full form.

When idle, `selection` is `{items,total,next_offset}`. The default page contains
at most 16 stable resource references. Pass `selection_offset` (default 0) and
`selection_limit` (1–50) to read further pages, for example:

```json
{"selection_offset":16,"selection_limit":50}
```

`selection.canvas` reports what the user selected on the active page:
`{items,total,part}`. `items` holds at most `selection_limit` summaries
(`target`, `kind`, `name`, `visible`) of the selected panels and items. `part`
is the legend, label or annotation the user clicked inside the lead plot, or `null`. A
legend reads `{target,part:"legend",property:"object.guides.position",value}`,
where a `null` value means automatic placement. A label reads
`{target,part:"label",kind,annotation_id,edit:"figure.edit annotation_set",…}`
with its current placement; an arrow or a reference line itself reads
`{target,part:"annotation",kind,annotation_id,edit,…}` with its position and
`hand_moved`. When the user says "this", edit that target or part.

Every call reads the current selection; pages are not a frozen snapshot. GUI
selection can change without changing the document revision. Do not combine pages
as an atomic selection or use selection as execution authority.

`hand_edits` lists the figure values the user set by hand in Plotium, as
`{items,total,next_offset}`. Each item has the `target`, the property `key`, its
`label` and current `value`. Pass `hand_edits_offset` and `hand_edits_limit`
(1–50, default 16) for further pages. Keep these values when you continue
editing; see [Values the user set by hand](/reference/mcp/styling/#values-the-user-set-by-hand).

While busy, `project`, `selection` and `hand_edits` are `null`; availability,
permissions, the capability fingerprint and save receipt IDs remain readable.

Call `plotium_capabilities_get {}` to obtain a compact index,
`{capability_fingerprint,connection_capabilities}`. The index lists each tool's
operations and required permission, the editable operations grouped by preflight
and execute permission, the scientific operations, the recovery, revision and
property-batch contracts, the document root and `sections`. Tool descriptions,
input schemas and annotations are in `tools/list`; property definitions are in
`plotium_properties_list`.

`sections` names each detail section with its size in bytes and whether it takes a
`key`. `operation_constraints` and `operation_details` list each key with a
one-line purpose, so you can match a task to an operation before fetching it. Fetch one detail with `{"section":...,"key":...}`, which returns
`{capability_fingerprint,section,key,contract}`:

| Section | Key | Contract |
| --- | --- | --- |
| `tools` | optional tool name | result contract and target rules; a keyed request also includes the tool input schema |
| `operation_constraints` | required operation id | schema, permissions and result contract of one editable operation |
| `operation_details` | required operation/suboperation key, or `keys` | focused figure-edit schema, permissions, result contract and example |
| `scientific_operations` | required operation id | descriptor and required permission; editable operations link to their canonical input schema |
| `guide_contract` | none | legend and colorbar placement rules |
| `candidate_preview` | none | uncommitted plan preview contract |
| `limits` | optional limit name | connection budgets |

For example, read the `figure.edit` schema with:

```json
{"section":"operation_constraints","key":"figure.edit"}
```

Known operations can request their detail directly without first reading the index.
For a page edit, use `{"section":"operation_details","key":"figure.edit/page"}`.
For several edits, pass `keys` instead of `key`, for example
`{"section":"operation_details","keys":["figure.edit/panel_label","figure.edit/annotation_create.arrow"]}`.
The response has `keys` and a `contract` map keyed by the requested names. This
section accepts 1–32 unique keys per call; other sections accept only `key`.
Each detail holds one exact edit schema with only the restrictions and result
fields that apply to that edit. Replace example targets with
full stable references from document inspection. Use `operation_constraints` for
the complete mixed-edit schema. Editable scientific descriptors expose
`input_schema_ref: {section,key}` instead of duplicating that schema.

An unknown section or key fails with `invalid_parameters`; `error.details.sections`
or `error.details.keys` lists the valid values.

The fingerprint is SHA-256 of the complete static discovery contract (index, every
section, the MCP tool definitions and the property catalog) with canonical JSON
object ordering. Cache the index and fetched sections under the returned
fingerprint; fetch it again when the state
fingerprint differs. If state changes before discovery completes, use the
fingerprint returned with the discovered contract. Project edits, project changes,
selection changes and permission changes do not change static capabilities.

Static discovery includes required permissions, not `currently_permitted` flags.
Always read current `permissions` and `availability` from state; execution still
checks server permissions, context, revision and single-use plans. Fingerprints,
cached schemas and annotations grant no permissions and never authorize retry.
Save and export still require fresh desktop confirmation for each exact request.
Update the desktop and connector together and reconnect clients to refresh the
tool catalog after upgrading.

The advertised inputs for planning, property inspection, document inspection and
figure diagnostics list their fields directly so clients can display complete
signatures. Plotium still validates conditional rules: use exactly one nonempty
`ids` or `targets` selector where required, omit both for operations with embedded
targets, and supply one dataset ID for table operations. Only `properties.set`
and `properties.reset` accept Series targets. Document pagination applies only
to `view: "children"`; diagnostic continuation accepts only `cursor`, context and
revision, whereas an initial diagnostic request requires `canvas` and `output`.

Committed synchronous `plotium_execute` results include `project_context`,
`before_revision` and `after_revision`. For known exact targets in that context,
use the returned context and `after_revision` for the next query or plan without
an extra state read. This does not refresh selection, hand edits, availability or
permissions. Read state initially, after desktop edits or Undo/Redo, when those
dynamic values are needed, and on stale/context errors or uncertain outcomes.
Execution still rechecks context, revision and permissions.

Plans for figure edits, curve append/bind/expansion, scientific plots, renames,
themes and dataset deletion return `plan_id`, `project_context`, `basis_revision`
and a `preflight` without the echoed request or frozen target set.
`preflight.summary` counts compatible, skipped and failed targets, and
`preflight.targets` lists only the targets that will be skipped or fail. Their
execution results carry `summary` counts of succeeded, skipped and failed
targets, list only skipped or failed rows in `targets`, and report changed
resources in `produced` and `modified`.

`plotium_apply` takes the same inputs as `plotium_plan` and plans and commits one
reversible edit in a single call, returning the `plotium_execute` result. It
accepts `properties.set`, `properties.set_many`, `properties.reset`,
`figure.edit` without deletions, `plot.create`, `plot.series.append`,
`plot.series.bind`, `plot.series.materialize_default`, `resource.rename` and
`figure.apply_theme`, and requires Read and modify. Preflight, revision and
permission checks, Undo and run records are those of the two-step form, and no
plan is retained. Saving, export, deletions, imports, table and scientific
operations still use `plotium_plan` and `plotium_execute`, as does previewing a
candidate. A rejected apply commits nothing. After a disconnection, read state:
a revision equal to the request's `expected_revision` means the edit was not
committed; otherwise inspect the targets. Never replay the call.

A queued or submitted scientific task is not a committed edit; its immediate
`after_revision` must not be reused. Only a terminal `succeeded` receipt in the
known project context with `end_revision` permits reuse. Check scientific
validity/convergence separately: successful task completion alone does not prove
a valid fit. Failed, cancelled, stale, missing or uncertain receipts require the
existing recovery flow and never authorize automatic replay. Save/export receipts
describe file operations and do not provide a new document revision.

## Discover structure without relying on names

Resource descriptors describe **core** data and rendering capabilities; a
capability listed on a resource does not make that operation callable through
this connection. The capability index lists the operations you can plan.

Use the returned revision to list the document's canvases:

```json
{
  "expected_project_context": "PROJECT_CONTEXT",
  "expected_revision": 7,
  "target": {"resource":{"id":"document","kind":"plotium.document"}},
  "view": "children",
  "offset": 0,
  "limit": 50
}
```

Call `plotium_document_inspect` with this request. Copy a complete
`value.items[].target` into the next request to descend from canvas to panel,
content, and Series. A canvas lists its panels followed by loose content;
parented content appears only under its panel. `view: "self"`, with pagination
arguments omitted, reads one target's details, `ancestors` and `child_count`.
The content kinds are plot, text, shape and raster image; image pixels and data
rows are not returned. Frames use points: a panel is page-relative, while content
inside it is panel-local. Content details include its exact `parent`, `frame_space`,
converted `page_frame`, and `geometry` constraints/editability. Raster details retain
`asset_id` and add `image` attributes and replacement eligibility. Canvas dimensions are in millimetres.

Series discovery uses the plot's current display binding, like property addressing.
Inactive channel or display-mode alternatives remain stored but are not listed or
addressable until reactivated. Authored hidden Series in the active binding are
still listed with `visible: false`. After switching channels or display modes,
fetch the current revision and discover the active targets again.

`encoding` names the native encoding family: `line` includes both line and scatter
curves; `plot.create` reports the authored subtype in `series_defaults[].kind`.

A Series detail includes its complete property-compatible target, encoding,
visibility, explicit label, and `source`: dataset and field resource references,
owner-local `field_id`, optional `item_id`, units and binding status. Use the
returned resource IDs with `plotium_resources_inspect` for field/column
descriptors. A missing field retains its `field_id` but has no resolvable field
reference. A source can report `resolved`, `missing_dataset`, `missing_field`,
`missing_item`, `incompatible_encoding`, `unbound` or `missing_column`.

For tables, `binding_mode: "table_default"` reports the shared `x_column` and
all `series_columns` Y/error bindings. One logical Series may expand to several
rendered table traces; those traces do not gain separate Series IDs here.
Explicit sources report `binding_mode: "table_explicit"`, TableId, current table revision,
`x_column`, `y_column` and optional `error_column`, without reporting defaults as their source.
`coordinate_units` reports the plot coordinate contract. Names, including identical curve names, are display
text, never identity. Panels use `CANVAS_UUID/panels/PANEL_ID`; content and Series
retain the existing addresses shown below.

Children queries default to 50 entries, accept 1–200, and return `total`,
`next_offset` and `truncated`. Follow `next_offset` with the **same revision and
target**, including when the 256 KiB structured-response budget shortens a page.
A detail or single entry that exceeds that budget returns `response_too_large`.
There is no recursive whole-project dump. Revision changes reject continuation;
read state and restart the listing. Missing targets and forged identities return
`invalid_parameters`, without using the current selection.

`plotium_properties_list {}` browses every supported property, one compact row
each: `id`, `label`, value `type`, and where they apply `min`/`max`, `unit`,
`format`, enum `values`, the curve `encoding` and required field capabilities
(`requires`). An ID starting with `document.` has document scope; every other ID
has object scope, and `series.` IDs target a Series. Rows marked `requires_full`
(nested inputs and display-unit codes) need full detail. Fetch complete
definitions for the properties you need in one call:

```json
{"keys":["series.line.stroke_width","series.line.color"],"detail":"full"}
```

Use `scope` with a scope name from the catalog, or `prefix` with the start of a
property ID such as `series.line` or `object.axes`, to filter the list. A prefix
that matches nothing fails with `error.details.prefixes`. `keys` must be
nonempty and unique; unsupported keys, unknown scopes and key/scope conflicts are
errors, rather than silently omitting requested properties. `detail: "full"`
without filters returns the complete catalog. Both forms are static, callable
while busy, and contain no current target values. Full details preserve complex
input constraints, colour format `#rrggbb` and every registered unit code.
Use `plotium_properties_inspect` for target-specific values, defaults, bounds and
applicability; an axis accepts only codes compatible with its coordinate unit.
Static applicability does not grant connection write permission.
