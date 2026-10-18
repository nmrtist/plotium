---
title: MCP reference
description: Manual configuration, tool calls and limits for local Plotium MCP clients.
---

For connection setup and everyday tasks, start with [AI client connections](/guides/automation/#local-agent-connections-mcp). This reference is for custom clients and tool-level troubleshooting.

## Manual configuration and diagnostics

In **Preferences → AI connections (MCP)**, expand **Manual configuration and diagnostics**
and press **Copy stable stdio configuration**. The copied JSON contains the actual
absolute connector path; the path below is a placeholder:

```json
{"mcpServers":{"plotium":{"command":"/ABSOLUTE/USER/DATA/bin/plotium-mcp","args":[]}}}
```

Use the copied path for your platform (`plotium-mcp.exe` on Windows). Plotium
copies the connector to its user data directory when enabled and updates it on
application upgrades. Keep `plotium-mcp` beside the desktop executable in a release
installation. The official CLI registration forms are:

```sh
codex mcp add plotium -- /ABSOLUTE/USER/DATA/bin/plotium-mcp
claude mcp add --transport stdio --scope user plotium -- /ABSOLUTE/USER/DATA/bin/plotium-mcp
```

See [Codex MCP](https://learn.chatgpt.com/docs/extend/mcp?surface=cli),
[Claude Code MCP](https://code.claude.com/docs/en/mcp) and the
[MCP Bundle manifest](https://github.com/modelcontextprotocol/mcpb/blob/main/MANIFEST.md).
Temporary HTTP diagnostics include a secret token and expire on service restart;
they are an advanced diagnostic entry, not the saved client configuration.
Discovery is private to the current user and local machine. Remote and cross-environment
WSL access are outside this interface.

## Permissions and supported operations

Edits use Plotium's normal **Undo** and **Redo**. **Read only** prevents further
edit execution, including pending plans. Table reads and property preflight remain
available; import, native binding and creation and transform preflight require Read and modify.
The selected permission persists until you change it.

Project queries and plan creation require both `expected_revision` and
`expected_project_context`, using the `project_context` from `plotium_get_state`.
The examples below use `PROJECT_CONTEXT` as a placeholder for that value.
Every project open, new, close or recovery creates a new context, even when
revision numbers match. Switching clears plans and rejects queued work;
`busy` during a transition means wait until it finishes or is cancelled.
On `project_changed`, reread state and replan with newly discovered targets.
Never reuse old targets or plans. Executions bind the context of their saved plan.

Real agent project reads and executions save run records and mark the project as
unsaved; handshakes, tool listing, authenticated status, five-second heartbeats and
the static property catalog do not.

The connection supports CSV/TSV data-only import, native table queries, canvas previews,
document structure, native single-plot creation, explicit numeric curve bindings, supported table transforms and resource queries, renaming datasets,
canvases and canvas objects, and these display properties only:

| Property ID | Target |
| --- | --- |
| `series.line.stroke_width` | Plot object (its line series) |
| `series.contour.line_width` | Plot object (its contour series) |
| `series.contour.count` | Plot object (its contour series) |
| `document.figure.typography.tick_pt` | `document` |
| `document.figure.typography.label_pt` | `document` |

Processing parameters, preferences, other import formats, other computation, exports, built-in chat
are outside this interface.

## Discover structure without relying on names

Start with `plotium_get_state {}`. Its `connection_capabilities` describes this
connection's eleven tools, input schemas, result contracts, supported operations,
property IDs, limits and current execution permission. Standard MCP `tools/list`
also lists their input schemas. Resource descriptors describe **core** data and
rendering capabilities; a capability such as `figure.export` or `data.transform`
does not make that operation callable through this connection. Only
`resource.rename`, `properties.set`, `properties.reset` and the restricted
`data.import` / `data.transform` modes, `plot.create` and `plot.series.bind` / `plot.series.materialize_default` below can be planned here. Read-only access allows table reads
and existing property preflight, but refuses import/binding/creation/transform preflight and all edit execution.

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
inside it is panel-local. Canvas dimensions are in millimetres.

Series discovery uses the plot's current display binding, like property addressing.
Inactive channel or display-mode alternatives remain stored but are not listed or
addressable until reactivated. Authored hidden Series in the active binding are
still listed with `visible: false`. After switching channels or display modes,
fetch the current revision and discover the active targets again.

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

`plotium_properties_list {}` returns exactly the five supported definitions:
`schema`, `unit` (empty for dimensionless values), `scope`, `access` and static
`applicability`. It contains no current target values. Use
`plotium_properties_inspect` to resolve values, defaults, bounds and applicability
for explicit targets. Property applicability and connection write permission
are separate facts.

## Import CSV/TSV data without creating a plot

Ask the agent to import an explicitly named UTF-8 CSV or TSV, inspect its columns,
and read the whole table in pages. Start with `plotium_get_state {}`, then call
`plotium_plan`:

```json
{
  "expected_project_context": "PROJECT_CONTEXT",
  "expected_revision": 7,
  "tool_id": "data.import",
  "parameters": {
    "mode": "table_data_only",
    "paths": ["/absolute/path/measurement.csv"],
    "header": "present"
  }
}
```

Do not supply `ids` or `targets` for import. `header` accepts `auto` (default),
`present` or `absent`; ambiguous auto detection fails. The `.csv` or `.tsv`
extension selects comma or tab; `format: "csv" | "tsv"` overrides it explicitly.
Only named regular files are read. There is no directory discovery, adjacent
sidecar lookup, source writeback, implicit plotting or X/Y binding.

Preflight runs in the background and returns only when preparation finishes.
`import.items` reports each file as `prepared` or `failed`, with proposed native
IDs, metadata and diagnostics. No dataset exists yet. After reviewing the result,
call `plotium_execute {"plan_id":"RETURNED_PLAN_ID"}`. Execution uses those frozen
bytes and typed snapshots, even if a source file changed after preflight. It
rechecks revision and permission before one atomic commit of all prepared items.
`value.status` is `succeeded`, `partial` or `failed`; inspect every item and its
created IDs. Failed files leave no partial dataset. One Undo removes the batch's
successful items; Redo restores the same DatasetId, FieldId, ColumnId and RowId.
Existing GUI/CLI import entry points retain their original behavior.

Metadata includes DatasetId, TableId, table RevisionId, field identities, stable
ColumnIds, logical types, nullable flags, units, total rows and diagnostics.
Fields and columns are distinct native identities. Missing cells remain null;
mixed numeric/text columns preserve text. Integer-looking tokens outside signed
64-bit range remain text rather than being rounded. Units are never inferred
from numbers or header spelling. Column `type_source` is `file`, `user`,
`inferred` or `unknown`; `unit_source` is `file`, `user` or `unknown`.

Optional `schema` is an ordered native TableSchema with typed ColumnSchema IDs,
logical types, nullable flags and optional UnitSpec declarations. Alternatively,
`schema_path` names a v1 PlotiumDelimitedSchema JSON file explicitly. Its header
and order must match exactly; declarations are strict, with no inference fallback
for invalid values. Supported declared types are null, boolean, int64, float64,
utf8, date and timestamp. Uniqueness/business-key constraints and uncertainty
relations are not accepted by this import. Plain `_sigma` column names do not
assign scientific uncertainty meanings. New native IDs are allocated per import;
file/user schema IDs describe the source, not an existing project target.

Limits are 16 files, 16 MiB per file, 64 MiB selected input bytes per batch,
256 columns and 1,000,000 rows per file. Explicit metadata files are limited to
256 KiB. The prepared item summary is limited to 128 KiB and the complete
preflight response to 256 KiB; reduce the batch/schema if either is exceeded.
At most four prepared imports and 32 total plans are retained per
connection; one import preflight runs at a time. Limits bound inputs, not total
process memory. Cancellation, a project transition, permission revocation or an
intervening edit prevents a pending preflight from becoming an executable plan.
Queued or running work is not completion. Failed/stale submissions consume their
plan; fetch state and preflight again instead of replaying a create request.

## Read a complete table in bounded pages

Copy DatasetId and table RevisionId from import metadata or the dataset resource
descriptor. Call `plotium_table_columns` with those IDs and `expected_revision` to
inspect metadata, then `plotium_table_read_rows`:

```json
{
  "expected_project_context": "PROJECT_CONTEXT",
  "expected_revision": 8,
  "dataset_id": "DATASET_UUID",
  "table_revision": "TABLE_REVISION_UUID",
  "offset": 0,
  "limit": 200
}
```

Replace the placeholders with returned IDs. Optional `column_ids` projects
columns by stable ColumnId, in the supplied order; omitted or empty selects all
columns in schema order. Rows follow immutable native snapshot order. The result
includes `column_ids`, `rows: [{row_id, values}]`, `total_rows`, `requested_rows`,
`returned_rows`, `next_offset` and `end`. Values are tagged by `kind`; null has no
value. Int64, time, timestamp and duration values use decimal strings to avoid
JSON number rounding. Date values are days since the Unix epoch; time, timestamp
and duration values use nanoseconds. Non-finite floats are explicit `NaN`,
`+Inf` or `-Inf` strings, not null.

Follow `next_offset` with the same dataset, document revision, table revision
and projection until `end: true` and `next_offset: null`. The default page is
200 rows, maximum 1000 rows and 20000 projected cells; the complete structured
ToolResult is limited to 256 KiB. The byte limit can shorten a page, never a cell;
use the actual returned count and continuation. A single oversized row returns
`response_too_large`. An offset equal to total_rows returns an empty end page;
an offset beyond it fails. Invalid limits, duplicate/absent columns and oversized
requests fail rather than being silently clamped.

Both revisions must still match on every read. Human edits, including Undo/Redo,
invalidate old document revisions: read state and restart rather than mixing
pages. Invalid IDs never use the current selection. Read-only connections can
query existing tables but cannot import them. Limits and the import permission
condition appear in `connection_capabilities`.

## Derive a table without changing its source

Ask the agent: “Calculate `signal_x2 = y_left * 2`, exclude rows whose
`x_left` equals zero, sort X descending, then bind the result to this existing
curve.” Table processing creates one native table, not a plot. Rebinding uses
[the existing curve-binding operation](#rebind-an-existing-curve-without-changing-another-plot).

1. Read `plotium_get_state`, then `plotium_table_columns` with the DatasetId
   and table revision. Copy the actual ColumnIds, `table_id`, `table_revision`
   and `snapshot_fingerprint`; names and `source_column_id` are not identities.
2. Discover the native plan schema in
   `connection_capabilities.operation_constraints["data.transform"].schema`.
   Supply one DatasetId in `ids` and `mode: "table_data_only"`. The leaf must
   pin exactly that dataset's current snapshot. Supply fresh UUIDs for
   `operation_id` and each computed column's `id`.
3. Call `plotium_plan`, inspect `transform.status: "prepared"` and its
   `output`, then submit the returned `plan_id` with `plotium_execute`.
   Preparation does not insert data. A successful execution returns
   `value.status: "succeeded"`, `value.output` and one `produced` dataset.
   The prepared output identity is retained when committed.
4. Read the complete output with `plotium_table_read_rows`. Bind its actual
   DatasetId, FieldId, TableId, revision and X/Y ColumnIds to an existing curve;
   use `plotium_render_preview` to inspect the native result. To create a new plot,
   pass the same output identities to [`plot.create`](#create-a-native-single-plot-from-an-imported-table)
   instead, then preview the returned canvas. Processing and plot creation remain
   separate, individually undoable operations.

This complete native plan appends a Float64 column. Replace every placeholder
with a discovered identity or a fresh UUID. This example declares unknown units
with `unit: null`; when `y_left` has an explicit unit, copy its **whole UnitSpec**
to the output column for this dimensionless multiplication.

```json
{
  "expected_project_context": "PROJECT_CONTEXT",
  "expected_revision": 7,
  "tool_id": "data.transform",
  "ids": ["DATASET_UUID"],
  "parameters": {
    "mode": "table_data_only",
    "name": "Scaled signal",
    "memory_limit_bytes": 16777216,
    "plan": {
      "version": 1,
      "operation_id": "NEW_OPERATION_UUID",
      "root": {
        "op": "computed_column",
        "input": {
          "op": "snapshot_read",
          "table": "TABLE_UUID",
          "revision": "TABLE_REVISION_UUID",
          "fingerprint": "SNAPSHOT_FINGERPRINT"
        },
        "column": {
          "id": "NEW_COLUMN_UUID", "name": "signal_x2",
          "logical_type": {"kind": "float64"}, "nullable": true, "unit": null
        },
        "expression": {
          "expr": "call", "function": "multiply.v1",
          "args": [
            {"expr": "cast", "input": {"expr": "column", "column": "Y_COLUMN_UUID"},
             "target": {"kind": "float64"}, "failure": "error"},
            {"expr": "literal", "value": {"type": "float64",
              "value": {"kind": "finite", "value": 2.0}}}
          ]
        }
      }
    }
  }
}
```

Supported relation nodes are `snapshot_read`, `project` (ordered ColumnIds),
`filter` (Boolean predicate), `stable_sort` (ordered keys with `column`,
`direction: ascending|descending`, `nulls: first|last`) and `computed_column`
(explicit column schema and expression). Wrap an earlier node in `input` to
compose them. For the task above, wrap the computed node in a filter using
`not.v1(eq.v1(column(X_COLUMN_UUID), literal(int64, 0)))`, then sort X descending,
then project X, Y and the new ColumnId. The schema defines the exact JSON forms.

Expressions use `column`, `literal`, `cast` or versioned `call`. Available
functions are `is_null.v1`, `is_finite.v1`, `eq.v1`, `not.v1`, `and.v1`, `or.v1`,
`add.v1`, `subtract.v1`, `multiply.v1` and `divide.v1`; no arbitrary function,
SQL or script is accepted. Arithmetic requires Float64 operands. Casts are
explicit and may fail; `failure: error` rejects the transform, while `null`
produces a missing value. Native Int64-to-Float64 casts can lose precision.
Computed outputs support Boolean, Int64, Float64 and Utf8. Units are explicitly
declared, not inferred or converted by arithmetic.

Filter keeps only true predicates; false and null are excluded. Sorting preserves
RowIds, resolves equal keys by RowId and places nulls independently of direction.
Non-null float order is `-Inf < finite < +Inf < NaN`. Projected columns retain
their IDs, types and units; computed columns retain the new IDs in the recipe.
Source values, table defaults, canvases and existing styles are unchanged.

Output metadata includes the frozen native recipe, backend, input revisions and
fingerprints, column lineage and row-mapping contract. To inspect it later, call
`plotium_table_columns` with `include_provenance: true`. The same flag on
`plotium_table_read_rows` returns per-row `sources`: source DatasetId (when
recorded), TableId, revision, fingerprint, original RowId and zero-based
`input_position`. This decodes the stored native mapping for the returned page;
it does not follow the latest source revision. Imported rows have no derived
sources. Other relational lineage shapes are explicitly rejected for row-source
paging; ordinary reads without the flag retain their existing behavior.

Preparation runs off the desktop thread. Planning and execution require **Read
and modify**. Permission, document revision and source snapshot are rechecked
when preparation finishes and immediately before commit. Each execution consumes
the plan, including a failed submission. Cancellation, stopping the connection,
project transitions and stale inputs discard uncommitted results. An already
started atomic commit is undone with normal Undo. One successful transform is
one Undo step; Redo restores the same table and column identities.

There is one transform preflight in flight per connection. Parameters are limited
to 64 KiB, 64 relation/expression nodes and depth 16 (JSON nesting at most 48).
The memory budget defaults to and is capped at 512 MiB; it is the reference
executor's **materialized-result estimate**, checked after evaluation, not a
peak-process-memory limit. At most four prepared transforms share a 512 MiB
reservation budget, charged at each request's declared budget. Complete
responses are limited to 256 KiB; the prepared output summary is limited to
128 KiB, leaving room for the request and plan envelope. Limits are discoverable in
`connection_capabilities.limits.table_transform`. Budget failure returns
`transform_budget_exceeded`; cancellation returns `request_cancelled`. Neither
leaves a derived table or a successful execution result. Plan again after fixing
the input or budget. Refresh and source-data patching are separate operations
and are not exposed by this interface.


## Rebind an existing curve without changing another plot

Two plots can use the same native table with different numeric X, Y and optional
symmetric Y-error columns. In the GUI, select a plot and expand **Data → Curve
columns** for its Series. Choose the columns and press **Apply curve binding**.
The table's shared default bindings are unchanged.

For MCP, discover the exact Series target with `plotium_document_inspect` and
copy the DatasetId, TableId, FieldId, table revision and ColumnIds from
`plotium_table_columns`. Use project ColumnIds, not `source_column_id`, names,
trace positions or a guessed field resource address. Call `plotium_plan`:

```json
{
  "expected_project_context": "PROJECT_CONTEXT",
  "expected_revision": 8,
  "tool_id": "plot.series.bind",
  "parameters": {
    "edits": [{
      "target": {
        "resource": {"id":"CANVAS_UUID/OBJECT_ID","kind":"plotium.canvas.object",
          "parent_id":"CANVAS_UUID","local_id":"OBJECT_ID"},
        "component": {"kind":"series","id":0}
      },
      "source": {
        "dataset_id":"DATASET_UUID", "field_id":0,
        "table_id":"TABLE_UUID", "table_revision":"TABLE_REVISION_UUID",
        "x_column_id":"X_COLUMN_UUID", "y_column_id":"Y_COLUMN_UUID",
        "y_error_column_id":null
      }
    }]
  }
}
```

Replace every placeholder, including the numeric IDs, with discovered values.
Do not add top-level `ids` or `targets`: each edit carries its complete target.
`y_error_column_id` is a ColumnId or explicit null to remove the error binding.
Review `preflight.binding_plan.bindings`, including source mappings and row
diagnostics, then submit its one-use `plan_id` with `plotium_execute`.

A default table Series can represent several displayed curves. First use
`plot.series.materialize_default` with
`parameters: {"edits":[{"target":EXACT_SERIES_TARGET,
"retain_y_column_id":"EXISTING_Y_COLUMN_UUID",
"table_revision":"TABLE_REVISION_UUID"}]}`. In the GUI, use **Expand default
curves** and choose the Y column that keeps the original SeriesId. The other
existing curves receive new owner-local SeriesIds, returned in the plan and
execution result. This expands existing content; it does not create a plot.
Fit overlays and normalized/offset multi-curve defaults cannot be expanded
losslessly and are rejected. Default drawing remains available without expansion.

Only Int64 and Float64 columns are accepted. Integers that cannot be represented
exactly in f64 curve coordinates are rejected. X/Y nulls and non-finite values
create gaps, never zero values or connecting lines across a gap. A null error
keeps the valid point without its error bar; zero error is valid. Negative or
non-finite errors reject the entire batch, including on rows with missing X/Y.
Errors are non-negative absolute amplitudes; their names do not assign standard
deviation, standard error or confidence-interval meaning. Diagnostics count all
rows and include up to 20 affected RowIds per curve. No row sampling is used for
binding validation or native curve extraction.

Compatible units are converted into the plot's persisted coordinate units.
Error amplitudes use scale only, not affine offsets. Known and unknown units
cannot be combined. Manual ranges and shifts retain their coordinate meaning
through rebinding, deletion of the first curve and reordering. An incompatible
binding is rejected rather than reinterpreting those numbers. Authored labels,
colors, line/point encoding, visibility, panel placement and axis settings stay
with their existing targets. Explicit plots retain line/point encodings rather
than switching to unrelated table chart types.

A batch accepts 1–200 edits and at most 2,000,000 curve-row visits across the
affected plots. Pending MCP curve plans share that same row budget; complete
responses are limited to 256 KiB. All targets, columns, units, rows and revisions
are checked before one atomic commit and one Undo. There is no partial success
or skipped invalid target. Read-only connections reject binding preflight and
execution. Failed or stale execution consumes the plan; discover current state
and plan again. GUI save/reopen preserves the native bindings and units; MCP
save and export are not exposed; use `plot.create` below to create a new plot.

## Create a native single plot from an imported table

Ask the agent: “Import this CSV, use time in s and current/error in mA, and create
one editable line plot with symmetric Y error bars. Preview it, then change only
that curve's line width to 1.5 pt and preview again.” Supply an absolute input path
and explicit units. The agent can discover the complete import and creation
schemas through `plotium_get_state` and the `plotium_plan` tool schema.

After data-only import, read the actual DatasetId, FieldId, TableId, table revision
and ColumnIds from import results / table metadata. Plan `plot.create` without
top-level `ids` or `targets`:

```json
{
  "expected_project_context": "PROJECT_CONTEXT",
  "expected_revision": 1,
  "tool_id": "plot.create",
  "parameters": {
    "parent": {"resource": {"id": "document", "kind": "plotium.document"}},
    "name": "Current",
    "series": [{
      "kind": "line",
      "source": {
        "dataset_id": "RETURNED_DATASET_UUID",
        "field_id": 0,
        "table_id": "RETURNED_TABLE_UUID",
        "table_revision": "RETURNED_TABLE_REVISION_UUID",
        "x_column_id": "RETURNED_TIME_COLUMN_UUID",
        "y_column_id": "RETURNED_CURRENT_COLUMN_UUID",
        "y_error_column_id": "RETURNED_ERROR_COLUMN_UUID"
      }
    }]
  }
}
```

Replace every identity, including the example FieldId, with the actual returned
identity. `kind` is `line` or `scatter`; explicitly set `y_error_column_id` to a
ColumnId or `null`. Multiple entries create independent native Series in this
one plot, not separate plots. Source extraction and all numeric, missing-value,
error and unit rules are the same as explicit curve binding above. Table-wide
X/Y defaults and other plots are untouched; no source table is copied.

The exact `parent` determines which objects are new:

- Document root: create canvas, single panel and plot. Optional
  `canvas: {name, size_mm: [width, height]}` sets the new canvas (10–1000 mm per
  dimension); omission freezes the native default size and plot name.
- Existing canvas without a panel or plot: create panel and plot using native
  GUI empty-page placement and the canvas's existing layout settings/margins.
  Do not supply `canvas` options.
- Its sole empty, visible, unlocked free-layout panel: create only the plot,
  preserving the panel's page position and size. Omit `canvas` and `panel_name`.

`panel_name` optionally names a newly created panel; otherwise it uses the plot
name. Names must be nonblank and at most 1024 UTF-8 bytes. This operation cannot
create a second panel/plot on an existing plotted canvas or rebuild existing
content. Placement is a native service decision, not an agent-supplied grid,
whitespace or collision algorithm.

`preflight.binding_plan.creation` returns proposed complete targets in
`canvas`, `panel`, `plot` and `created`, plus `size_mm`, `panel_frame_pt`
(page coordinates), `frame_pt` (panel-local content coordinates),
`coordinate_units` and `series_defaults` (kind and line width in pt).
`binding_plan.bindings` returns each exact owner+SeriesId target, actual source
mapping/revision and full-row diagnostics. Preflight creates nothing. Execute
the one-use plan; `ToolResult.value` returns the same identities and native
binding summary, with committed `after_revision` on the result.

The creation is atomic: a bad source, incompatible units, invalid error or
changed parent/revision leaves no new canvas, panel or partial plot. Read-only
connections reject creation preflight and execution. There are 1–200 curves per
plot, at most 2,000,000 curve-row visits, a shared 2,000,000-row pending-plan
budget with binding edits, 32 pending plans and a 256 KiB structured-response
limit. Failed submissions consume their plan too. After a lost execute response,
query current state and the proposed targets before deciding what to do; do not
blindly make a new creation request.

Use returned targets with document inspection and properties; preview the
returned whole-canvas target at the current revision. The result is selectable
and editable in the GUI. One Undo removes the created subtree but keeps existing
parents and imported data; Redo restores the same IDs. Existing native resize
and layout controls retain plot/Series identities and source bindings. Fetch a
fresh revision after any edit or Undo/Redo.

## Read the actual canvas image

Ask the agent: “Preview this canvas and describe its text, curves and local
images.” Use `plotium_document_inspect` to obtain a complete canvas target, then
call `plotium_render_preview` with that target and the current revision:

```json
{
  "expected_project_context": "PROJECT_CONTEXT",
  "expected_revision": 7,
  "target": {"resource":{"id":"CANVAS_UUID","kind":"plotium.canvas"}},
  "max_edge": 1440
}
```

Replace `CANVAS_UUID` with the discovered canvas ID; do not pass a panel,
content item, selection or Series. The response contains a standard MCP PNG
**image content block**, plus `ToolResult.value` metadata: `target`,
`render_revision`, `width_px`, `height_px` and `mime_type`. It does not require
opening a file or rendering SVG text. The MCP client must deliver image content
to its agent; a text-only client cannot consume this preview.

The image covers the whole page with its native layout, background, panel clips,
text, curves and embedded local raster assets. Rendering runs off the desktop
thread from an immutable snapshot. `render_revision` identifies that snapshot;
human edits after capture do not change it. Read state again before editing.
Read-only connections permit previews. Completed reads save a metadata-only run
record and mark the project unsaved without advancing its revision.

`max_edge` defaults to 1440 px and accepts 1–2048; aspect ratio is preserved
with pixel rounding and a minimum of one pixel per axis. Limits are 4,194,304
output pixels, 16 MiB of referenced encoded assets, 64 MiB of header-probed
RGBA8 asset pages, 8 MiB of intermediate SVG, 4 MiB of PNG, and 6 MiB of JSON-RPC
preview response (including the request ID). Shared asset bytes count once;
decoded pages count once per asset/page. These are payload and image-buffer limits, not a total process
memory limit. All referenced assets are validated, including hidden items.
Limits are also available in `connection_capabilities.limits.image_preview`.

Missing or damaged assets and unavailable image pages return
`preview_asset_unavailable`; no placeholder or incomplete success image is
returned. Budget overflow returns `preview_budget_exceeded`, without silent
quality reduction; lower `max_edge` for output-size overflow, or reduce source
assets for asset-budget overflow. Invalid full identities return
`invalid_parameters`, and outdated revisions return `stale_revision`.
Only one preview renders per connection; another returns `busy`. Other existing
tools remain available while rendering. Cancellation or stopping the connection
prevents image delivery; stopping or switching projects discards the old job.

## Inspect and edit an exact curve

An example agent task is: “Find my line plot, inspect its stroke width, preflight
changing it to 1.5 pt, then apply the saved plan.” The tool sequence is:

1. `plotium_get_state` with `{}` returns `revision`, project overview, selection
   and connection permissions.
2. `plotium_resources_search` with `expected_revision` and a `query` such as
   `{"kinds":["plotium.canvas.object"],"name_contains":"Plot"}` locates resources.
   Resolve results to explicit stable IDs; `plotium_resources_inspect` reads
   descriptors using `ids` and `expected_revision`.
3. `plotium_properties_list` with `{}` lists supported IDs and scopes. Discover
   curve addresses with `plotium_properties_inspect`:

   ```json
   {"ids":["CANVAS_UUID/OBJECT_ID"],"key":"series.line.stroke_width","expected_project_context":"PROJECT_CONTEXT","expected_revision":7}
   ```

   The response's `value.readings` contains one row per applicable curve. Copy
   the complete `value.readings[0].target` for the curve you want, for example:

   ```json
   {
     "resource": {
       "id":"CANVAS_UUID/OBJECT_ID",
       "kind":"plotium.canvas.object",
       "parent_id":"CANVAS_UUID",
       "local_id":"OBJECT_ID"
     },
     "component":{"kind":"series","id":0}
   }
   ```

4. Pass that target unchanged to `plotium_plan` to edit exactly that curve:

   ```json
   {
     "tool_id":"properties.set",
     "parameters":{"key":"series.line.stroke_width","value":1.5},
     "targets":[{
       "resource":{
         "id":"CANVAS_UUID/OBJECT_ID",
         "kind":"plotium.canvas.object",
         "parent_id":"CANVAS_UUID",
         "local_id":"OBJECT_ID"
       },
       "component":{"kind":"series","id":0}
     }],
     "expected_project_context": "PROJECT_CONTEXT",
     "expected_revision":7
   }
   ```

5. Review `preflight`, then call `plotium_execute` with only
   `{"plan_id":"RETURNED_PLAN_ID"}`. Plotium immediately displays the change;
   **Undo** restores it and **Redo** reapplies it. Repeating the current value is
   reported as skipped. A batch applies as one undo step.

Property inspection and property-edit planning require exactly one nonempty `ids` or
`targets` array. `ids` expands each resource to all components addressed by the
property; `targets` addresses only the supplied Series components. Series IDs
are local to their plot, so always copy the full target, including its resource.
Duplicate targets are processed once. A missing component or forged resource
identity rejects the entire request; an existing curve to which the property
does not apply is skipped. Inspection returns values, defaults, bounds and
applicability. To inspect just one curve, use `targets` in the inspection request.
To reset it, plan `properties.reset` with `{"key":"series.line.stroke_width"}`
and the same target. Component addressing supports only the five listed display
properties; processing parameters are unavailable. `resource.rename` requires
`ids`.

Tool responses include structured content and its JSON text representation.
Project reads and preflight require the current `expected_revision`; fetch the
state again after edits or Undo. Execution rechecks the saved revision and current
permission. Plans are single-use, including failed submissions, and at most 32
can be pending. Restarting the connection clears them. Requests cancelled before
desktop execution are skipped without changing the project. Cancellation cannot
roll back an edit that has already started its atomic commit; use **Undo**.

Errors include stable codes: `permission_denied`, `expired_plan`,
`stale_revision`, `busy`, `invalid_parameters`, `unsupported_operation`,
`project_changed`, `no_project`, `unavailable`, `connection_lost`, `plan_limit`, `request_cancelled`, `connection_stopped`, `response_too_large` and `execution_failed`.
A `busy` response is retryable after the active edit, recovery confirmation or background project
operation finishes. For stale or expired plans, read state and preflight again.
The AI connections (MCP) settings page shows recent operations and errors without
opening itself in response to background activity.
