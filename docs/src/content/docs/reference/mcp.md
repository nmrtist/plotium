---
title: MCP reference
description: Manual configuration, tool calls and limits for local Plotium MCP clients.
---

For connection setup and everyday tasks, start with [AI client connections](/guides/automation/#local-agent-connections-mcp). This reference is for custom clients and tool-level troubleshooting.

## Manual configuration and diagnostics

On Windows, Plotium finds standalone client executables on `PATH`. For Codex
installed with npm, it resolves the native executable inside the npm package,
using prefixes on `PATH` and the default `%APPDATA%\npm` prefix. Both x64 and
ARM64 installations are supported. For a custom npm prefix, add it to `PATH` and
restart Plotium, or use **Choose executable…** to select the package's
`codex.exe`. Adding a client requires the Codex CLI to be installed.

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

After upgrading Plotium, restart its MCP connection in your AI client to load
the updated tools. If connector installation fails, open
**AI connections (MCP)** to read the error and press **Retry**.

## Permissions and supported operations

Edits use Plotium's normal **Undo** and **Redo**. **Read only** prevents further
edit execution, including pending plans. Table reads and property preflight remain
available; import, native binding and creation and transform/patch/refresh preflight require Read and modify.
The selected permission persists until you change it.

Project queries and plan creation require both `expected_revision` and
`expected_project_context`, using the `project_context` from `plotium_get_state`.
The examples below use `PROJECT_CONTEXT` as a placeholder for that value.
Every project open, new, close or recovery creates a new context, even when
revision numbers match. Switching clears plans and rejects queued work;
`busy` during a transition means wait until it finishes or is cancelled.
On `project_changed`, reread state and replan with newly discovered targets.
Never reuse old targets or plans. Executions bind the context of their saved plan.

Project edits, scientific computations and data transformations retain durable run
provenance and mark the project as unsaved. Ordinary queries, including state,
resource/property/table/document browsing, scientific result inspection, previews
and diagnostics, do not append project audit records or change dirty state, edit
generation or automation revision. Query-only workflows also leave the project
unchanged; workflows containing modifying operations retain the complete run,
including query inputs needed to reproduce those operations. Handshakes, tool listing, authenticated status, five-second heartbeats,
the static property catalog and save coordination (planning, submission and
`plotium_save_status`), and export planning/submission/status do not add audit records.

The connection supports native acquisition import and CSV/TSV data-only import, native table queries, canvas previews, figure geometry diagnostics,
document structure, native plot creation and page/panel/text/local-image authoring, explicit numeric curve bindings, supported table transforms, cell patches, explicit saved-recipe refreshes and resource queries, renaming datasets,
canvases and canvas objects, applying built-in themes or saved canvas styles to canvases, and these display properties only:

| Property ID | Target | Value |
| --- | --- | --- |
| `series.line.color` | Object + Series | `#RRGGBB` |
| `series.line.stroke_width` | Object + Series | 0.05–10 pt |
| `series.line.style` | Object + Series | `solid`, `dashed`, `dotted`, `dash_dot` |
| `series.line.marker` | Object + Series | `none`, `circle`, `square`, `diamond`, `triangle_up`, `triangle_down`, `star`, `pentagon`, `hexagon`, `plus`, `cross` |
| `series.line.marker_size` | Object + Series | 0.1–20 pt diameter |
| `series.line.marker_fill / series.line.marker_edge_color` | Object + Series | `#RRGGBB` |
| `series.line.marker_edge_width` | Object + Series | 0–5 pt; 0 draws no outline (0.5 pt while hollow) |
| `series.line.marker_hollow` | Object + Series | boolean |
| `series.bar.width` | Object + Series | 0.05–1, fraction of the bar's slot (bar curves; default 0.8) |
| `series.bar.edge_color` | Object + Series | `#RRGGBB` bar outline (bar curves; unset, a near-black shade of the fill's hue that follows color edits) |
| `series.bar.edge_width` | Object + Series | 0–5 pt bar outline; 0 draws none (bar curves; default 0.5 pt, or the theme's supporting-line width) |
| `series.y_axis` | Object + Series | `left`, `right` (line and scatter curves; bars stay left) |
| `series.points.spread` | Object + Series | 0–1 spread of replicate points across their mark (point and line curves; default 0) |
| `series.value_labels` | Object + Series | boolean; write each bar's or point's value (bar, point and line curves) |
| `object.chart.orientation` | Object | `vertical`, `horizontal` (explicit-column plots) |
| `series.visible` | Object + Series | boolean |
| `series.label` | Object + Series | string |
| `object.axes.x_label / object.axes.y_label` | Object | string |
| `object.axes.frame` | Object | `open` (left and bottom), `box` (all four sides) |
| `object.axes.x_scale / object.axes.y_scale` | Object | `linear`, `log` (line, scatter and stick plots; a manual range must stay above zero) |
| `object.axes.x_display_unit / object.axes.y_display_unit` | Object | compatible code from property inspection |
| `object.axes.x_range / object.axes.y_range` | Object | auto or manual range object |
| `object.axes.y2_label` | Object | string; right y axis title |
| `object.axes.y2_show_tick_labels / object.axes.y2_show_label` | Object | boolean |
| `object.axes.y2_scale` | Object | `linear`, `log` |
| `object.axes.y2_display_unit` | Object | compatible code from property inspection |
| `object.axes.y2_range` | Object | auto or manual range object |
| `object.figure.typography.tick_pt / object.figure.typography.label_pt / object.figure.typography.legend_pt` | Object | 1–72 pt |
| `object.guides.visibility` | Object | `auto`, `show`, `hide` |
| `object.guides.placement` | Object | `auto`, `inside`, `outside_right`, `outside_bottom`, `custom` |
| `object.guides.layout` | Object | `auto`, `vertical`, `horizontal` |
| `object.guides.title` | Object | string |
| `object.guides.position` | Object | `[x,y]` in [0,1], or `null` |
| `series.contour.line_width` | Object + Series | 0.05–10 pt |
| `series.contour.count` | Object + Series | 1–256 |
| `document.figure.typography.tick_pt / document.figure.typography.label_pt` | document | 1–72 pt |
| `document.figure.typography.axis_line_pt` | document | 0.05–10 pt; axis lines and tick marks |
| `document.figure.typography.tick_length_pt` | document | 0–20 pt |
| `document.figure.typography.tick_direction` | document | `out`, `in` |
| `document.figure.typography.minor_ticks` | document | 0–9 minor ticks per major interval; 0 hides them |
| `document.figure.typography.typeface` | document | `arial`, `helvetica`, `times_new_roman`; text inside every plot |
| `document.figure.typography.font_weight` | document | `regular`, `bold`; text inside every plot |

The `object.axes.y2_*` properties apply only while some curve of the plot has
`series.y_axis` `right`; on other plots they are not applicable.

Scientific processing and task interfaces are described below. Preferences, new format parsers, automatic scientific-result plotting and built-in chat are outside this interface.

### Tool annotations and client approval

Every tool advertises MCP `annotations` in `tools/list` and in
`connection_capabilities.tools`, independently of the current **Access** setting.
`readOnlyHint` describes environment changes; `destructiveHint` distinguishes
potentially destructive writes from additive ones; `idempotentHint` describes
whether repeating identical arguments has additional effects; `openWorldHint`
describes interaction with external entities, including local input/output files.
Destructive and idempotent hints are meaningful only for non-read-only tools.
See the [MCP annotation definitions](https://modelcontextprotocol.io/specification/2025-11-25/schema#toolannotations).

Queries, previews and receipt inspection are read-only and operate on native
project/application state. Diagnosis is also read-only, but continuation cursors
are single-use. `plotium_plan` is not read-only: each successful call retains a new
executable plan and can read external data, schema, processing-scheme or image
files and prepare computation. It does not commit the candidate or write
save/export outputs. `plotium_execute` covers edits, deletion, scientific tasks
and save/export requests, so the entire tool advertises potentially destructive,
non-idempotent, open-world behavior. Task cancellation is a potentially destructive
write to task state; repeating cancellation for the same retained task has no
additional effect.

A client's policy may use trusted-server hints when deciding whether to ask for
approval; clients can still ask, and untrusted annotations must not authorize
calls. Client approval, Plotium **Access**, and desktop file consent are separate
decisions. Annotations grant no permission: revision/context checks, revocation
and one-use plans still apply. Every save/export request retains its own desktop
confirmation and exact file/overwrite scope, including in **Read only**.
Never replay a call after disconnection or an unknown outcome, even when marked
idempotent; inspect state or retained receipts instead. After upgrading Plotium
and its connector, reconnect the client to refresh the tool catalog.

## Read state and discover capabilities on demand

`plotium_get_state {}` reads the latest dynamic state. It returns
`project_context`, `revision`, `availability`, `permissions`, `save_state`,
`capability_fingerprint`, project counts and a bounded selection page. It does not
include the full capability catalog or project blueprint. Discover exact targets
with the revision-bound `plotium_document_inspect` pages or
`plotium_resources_search`, then inspect explicit IDs with
`plotium_resources_inspect`. Copy complete returned targets rather than names or
collection positions.

When idle, `selection` is `{items,total,next_offset}`. The default page contains
at most 16 stable resource references. Pass `selection_offset` (default 0) and
`selection_limit` (1–50) to read further pages, for example:

```json
{"selection_offset":16,"selection_limit":50}
```

Every call reads the current selection; pages are not a frozen snapshot. GUI
selection can change without changing the document revision. Do not combine pages
as an atomic selection or use selection as execution authority. While busy,
`project` and `selection` are `null`; availability, permissions, the capability
fingerprint and save receipt IDs remain readable.

Call `plotium_capabilities_get {}` to obtain
`{capability_fingerprint,connection_capabilities}`. The fingerprint is SHA-256 of
the complete static discovery contract with canonical JSON object ordering. Cache
that contract under its returned fingerprint; fetch it again when the state
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

## Import native data from the welcome page

`plotium_get_state {}` works immediately after opening Plotium, including while
background work, recovery or saving blocks project access. It adds
`availability` entries for `project_read`, `project_modify`
and `native_import`; each has `available` and a nullable `{code,message}` reason.
You can import native data or tables directly from the welcome page. When idle,
`project` contains dataset and canvas counts, even before importing data. While busy,
`project` and `selection` are both `null`. Check `availability` before planning
an operation. Queries add no project audit entry or unsaved changes.
Connection capabilities remain discoverable through `plotium_capabilities_get {}` in every state.

Enable **Read and modify**, copy `project_context` and `revision` from state,
then call `plotium_plan`:

```json
{
  "expected_project_context": "PROJECT_CONTEXT",
  "expected_revision": 0,
  "tool_id": "data.import",
  "parameters": {"paths": ["C:/Data/experiment.jdf"]}
}
```

Use actual absolute paths. Native import accepts explicit data files and atomic
acquisition directories supported by the desktop loader, such as Bruker acquisition
or processed-data directories. It reads required companions without recursive batch
discovery or scanning adjacent experiments. ZIP archives, project files and new
formats are outside this operation. Optional `sampling_declaration` uses the core
import contract. Discover the shared schema in
`connection_capabilities.operation_constraints["data.import"].schema`.

Preflight runs in the background and returns per-input format, dimensions,
warnings, failures and proposed dataset/canvas IDs in `import.items[].metadata`.
It creates no resources or document edits. Call `plotium_execute {"plan_id":"PLAN_ID"}`
to commit frozen data; changing or deleting the source after preflight does not
affect execution. Success adds native datasets and default canvases to the current
project, preserving its context and selection.

Results report `succeeded`, `partial` or `failed`. All successful inputs commit
atomically in one Undo step. Undo leaves an empty project when the batch was its
first content; Redo restores the same resource IDs. If every input fails or the
import is cancelled, no resources, Undo entry or unsaved changes are added.
Successful imports retain scientific provenance. Permissions, context and revision
are checked again at submission. Plans are single-use even when execution is refused. After
`project_changed`, read state and preflight again.

Native import allows 16 inputs per batch, one preflight in flight and four retained
prepared imports, shared with table import. Conservative native frozen-payload
reservations total at most 512 MiB across retained plans and the active preflight;
oversized batches fail with `import_budget_exceeded`. Preflight responses are
limited to 256 KiB. Scientific processing and DOSY use the separate shared scientific operations.
CSV/TSV `mode: "table_data_only"` retains its existing data-only behavior and limits.

## Discover structure without relying on names

Start with `plotium_get_state {}` for current context, revision and permissions.
Call `plotium_capabilities_get {}` when you need static contracts. Its
`connection_capabilities` describes this connection's tools, input schemas, result
contracts, supported operations, property definitions and limits. Standard MCP
`tools/list` also lists tool input schemas. Resource descriptors describe **core** data and
rendering capabilities; a capability listed on a resource
does not make that operation callable through this connection. Only
`project.save`, `figure.export`, `resource.rename`, `figure.apply_theme`, `properties.set`, `properties.set_many`, `properties.reset` and the restricted
`data.import` / `data.transform` modes, `table.patch`, `table.refresh`, `figure.edit`, `plot.create` and `plot.series.append` / `plot.series.bind` / `plot.series.materialize_default` below can be planned here. Read-only access allows table reads
and existing property preflight, but refuses import/binding/creation/figure-edit/transform/patch/refresh preflight and all reversible edit execution. Saving and exporting use separate per-request desktop authorization, also in Read only mode.

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

`plotium_properties_list {}` returns the supported definitions:
`schema`, `unit` (empty for dimensionless values), `scope`, `access` and static
`applicability`. It contains no current target values. Colour schemas state
their input form as `"format": "#rrggbb"`. Unit-code schemas list every
registered unit code; the codes a given axis accepts are the ones
`plotium_properties_inspect` returns for that target, which are compatible with
its coordinate unit. Use
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
the input or budget. Use the separate cell-patch and explicit refresh operations below for subsequent updates.


## Patch cells, then explicitly refresh derived tables

Use `table.patch` with exactly one DatasetId in top-level `ids`, no `targets`,
and the current table revision. Copy RowIds and ColumnIds from table reads;
page offsets, row numbers and names are not cell identities. For example:

```json
{
  "tool_id": "table.patch",
  "expected_project_context": "PROJECT_CONTEXT",
  "expected_revision": 12,
  "ids": ["DATASET_UUID"],
  "parameters": {
    "table_revision": "TABLE_REVISION_UUID",
    "edits": [{
      "row_id": "RETURNED_ROW_UUID",
      "column_id": "RETURNED_COLUMN_UUID",
      "value": {"kind": "float64", "value": 2.5}
    }]
  }
}
```

Values use the tagged `kind`/`value` representation returned by table reads.
Use decimal strings for Int64 and nanosecond temporal values, a JSON number or
`"NaN"`/`"+Inf"`/`"-Inf"` for Float64, and `{"kind":"null"}` for a missing cell.
Types and nullability must match exactly; a Float64 patch cannot silently widen
an Int64 column. Categorical values address existing level indices. Extension
values and live calculated region tables are not editable through this operation.

A batch accepts 1–1000 distinct cells and at most 64 KiB of parameters. An absent
row/column, duplicate cell, invalid type or stale revision rejects the entire
patch. It cannot insert/delete rows, change schema, reimport files or write to the
original CSV/TSV. Preparation produces `update: {status:"prepared", output,
before_table_revision, changed}`; nothing is applied until execution. A no-op
returns `changed:false` without advancing the document revision or adding Undo.

Table metadata, row pages and Series sources include
`freshness: {status, refreshable, reason, sources, mcp_refreshable, mcp_refresh_reason}`.
`refreshable` reports native refresh capability; `mcp_refreshable` additionally checks
the connection's supported recipe subset, not the current permission mode. Dataset
resource metadata exposes these as `table_refreshable`, `table_mcp_refreshable` and
`table_mcp_refresh_reason`, alongside `table_freshness` and its reason. `source` means no saved derived recipe; `current` means recorded input
revisions still match; `stale` means a direct or upstream input changed;
`unavailable` means sources or a supported recipe are missing. The data browser
marks affected relational tables **Out of date**, and their sheets explain why.
Stored results remain readable and drawable, with this explicit freshness state.
Editing a table invalidates its stored curve-fit analyses; Undo restores them.
A stale derived table's analyses describe its old result, not current source data.

Refresh only when requested. Plan `table.refresh` with `ids:["DERIVED_DATASET_UUID"]`
and `parameters:{"table_revision":"CURRENT_DERIVED_REVISION"}`. It reruns the
persisted single-source `snapshot_read`/`project`/`filter`/`stable_sort`/`computed_column`
recipe, with the same expression and complexity limits as data-only transforms,
using current source snapshots and the existing reference executor/lineage path.
Other native recipes, including Pivot, are rejected by MCP refresh with a reason;
the native GUI/core refresh capabilities are unchanged. Before adoption, the output
must retain TableId and the column schema, including ColumnIds, types and units;
every output RowId must belong to the current input. A mismatch rejects the whole
refresh without changing tables, plot bindings or Undo. Refresh stale upstream tables first;
missing sources or recipes report a reason instead of recomputing unrelated data.
Existing manual cell edits on a derived table are rebased by stable RowId; missing
or ambiguous identities block the whole refresh, never fall back to row position.
Repeated refreshes retain those edits. `include_provenance:true` returns both the
current `operation` and the effective `refresh_recipe` through edit/rebase history.

Optional `memory_limit_bytes` resolves the explicit value, then the saved recipe
budget, then 512 MiB; valid range is 1–536870912. This remains the executor's
materialized-result estimate, not a peak-memory limit. Patches, transforms and
refreshes share one background preflight, four prepared candidates and a 512 MiB
reservation limit; each patch reserves 64 KiB. Prepared summaries are at most
128 KiB and complete responses at most 256 KiB. Discover schemas and limits in
`operation_constraints` and `limits.table_update`.

Both operations require **Read and modify** for preparation and execution.
Permission, document/table revisions and current inputs are rechecked before a
candidate can be adopted. Cancellation, project changes, connection stop or stale
inputs discard uncommitted work. Execute consumes the plan even when rejected.
After a lost response, inspect actual state; do not replay the submitted operation.

Each patch or refresh is one independent Undo/Redo step. DatasetId, TableId,
ColumnId, existing RowId and native plot/Series identities remain intact; filters
can change which existing rows are present. Revisions and source mappings change,
while authored colors, axis ranges and panel frames stay untouched. Freshness is
computed from saved provenance and current revisions, so Undo/Redo restores the
corresponding freshness as well as values. Native sheet editing and refresh use
the same typed revision-adoption service.

## Add a derived curve to an existing plot

Use `plot.series.append` when a newly calculated table should appear beside the
existing curves, without replacing a plot. In the GUI, select the explicit plot,
open **Data → Add curve**, choose the source table, X/Y and optional Y-error
columns, then press **Append curve**. The old default-field **Add series** control
keeps its existing behavior for non-explicit plots.

For MCP, omit top-level `ids`/`targets` and provide the exact whole plot target:

```json
{
  "expected_project_context": "<current context>",
  "expected_revision": 23,
  "tool_id": "plot.series.append",
  "parameters": {
    "plot": {"resource": {"id": "<canvas-id>/<content-id>", "kind": "plotium.canvas.object", "parent_id": "<canvas-id>", "local_id": "<content-id>"}},
    "series": [{"kind": "line", "source": {
      "dataset_id": "<derived-dataset-id>", "field_id": 0,
      "table_id": "<derived-table-id>", "table_revision": "<derived-revision>",
      "x_column_id": "<x-column-id>", "y_column_id": "<signal-x2-column-id>",
      "y_error_column_id": null
    }}]
  }
}
```

Copy targets, FieldId and all table/column IDs from discovery; labels are not
identity. The target must already have explicit coordinates. A batch adds
1–200 selected line/scatter/bar/box/violin curves, with at most 200 total curves in that plot.
Existing bars regroup to share their slots with appended bars.
The shared binding validator checks every source revision, complete curve data,
errors and units before one atomic commit. Compatible source units convert to
the plot's existing coordinate units; incompatible batches fail without adding
any curve. The binding service's 2,000,000 curve-row and pending-plan limits and
256 KiB response limit also apply, including existing curves in the affected plot.

`preflight.binding_plan.appended` contains the proposed new Series targets;
`bindings` includes the unchanged existing and newly appended bindings. Execute
adopts those frozen IDs. One Undo removes the additions; Redo restores them.
Retired IDs are not reused by a different append after Undo. Existing Series
styles, coordinate meaning, authored axis ranges, plot/Panel identity and frames
remain unchanged. Preparation and execution require Read and modify; context,
revisions and permission use the same one-use-plan lifecycle as binding edits.
After response loss, inspect the proposed targets before making another plan.

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
`project.save` requires separate desktop approval; SVG/PDF/PNG exports require their own desktop approval. Use `plot.create` below to create a new plot.

## Create a native single plot from an imported table

Ask the agent: “Import this CSV, use time in s and current/error in mA, and create
one editable line plot with symmetric Y error bars. Preview it, then change only
that curve's line width to 1.5 pt and preview again.” Supply an absolute input path
and explicit units. The agent can discover the complete import and creation
schemas through `plotium_capabilities_get` and the `plotium_plan` tool schema.

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
identity. `kind` is `line`, `scatter`, `bar`, `box` or `violin`; explicitly set `y_error_column_id` to a
ColumnId or `null`. A `bar` curve draws filled bars from zero with optional
error whiskers, filled in the curve color with a thin outline. Whiskers use the
outline's color and width, and their caps span 45% of the bar. A bar too small
for its outline on paper (narrower than 4 or shorter than 2 outline widths, such
as a near-zero value) is drawn as a plain fill, keeping its whiskers. On a numeric
x, a slot is the typical spacing between positions, narrowed only where
neighbours sit closer. Its x column may be numeric or a
Utf8 text column; text values become categories in table row order, and equal
texts share one slot. Bar curves that share a slot sit side by side within it; a
bar alone in its slot keeps the full width, so one bar curve per category gives
centred bars in their own colors. A `box` or `violin` curve summarises the raw y
samples that share each x (Tukey box: median, IQR and 1.5 × IQR whiskers with
outliers; violin: Gaussian kernel density with a quartile bar), takes no y error
column, and shares slots with bars. `line` and `scatter` curves may use a text x
too and overlay the slots: the k-th line or scatter curve sits on the k-th bar,
box or violin curve's position, so raw points line up with their bar.
`series.points.spread` spreads replicate points that share an x across that
mark (0–1 of its width, in row order), and `series.value_labels` writes each
bar's or point's value past its end (or whisker) in tick-label size.
`object.chart.orientation` `horizontal` turns an explicit plot on its side:
categories run down the vertical axis, first at the top, and values along the
horizontal one; error bars, labels and axis titles turn with it, and the view
resets. All series in one plot share a text or a numeric x. Plots with bars,
boxes or violins have no logarithmic axes. Optional `y_axis`
is `left` (default) or `right` for a line or scatter curve; `right` on a bar,
box or violin is rejected. Any right curve gives the plot a right y axis with its own automatic
range, scale and title, which starts as the first right curve's column name and
unit; that curve also fixes the right axis's unit. Reference lines, annotations
and arrows stay in left-axis coordinates. Multiple entries create independent native Series in this
one plot, not separate plots. Source extraction and all numeric, missing-value,
error and unit rules are the same as explicit curve binding above. Table-wide
X/Y defaults and other plots are untouched; no source table is copied.

The exact `parent` determines which objects are new:

- Document root: create canvas, single panel and plot. Optional
  `canvas: {name, size_mm: [width, height]}` sets the new canvas (10–1000 mm per
  dimension), or `canvas: {name, size_preset}` uses a built-in page size such as
  `nature-1col` (89 mm), `nature-1p5col` (136 mm) or `nature-2col` (183 mm) with
  its default height and margins (0 mm for journal columns, 15 mm for paper,
  10 mm for slides); the result reports the applied `size_preset`. Omission freezes the native default size and
  plot name.
- Existing canvas without a panel or plot: create panel and plot using native
  GUI empty-page placement and the canvas's existing layout settings/margins.
  Do not supply `canvas` options.
- Existing canvas with content: supply `panel_frame_pt: [x,y,width,height]` to
  create another panel and plot at that exact page-relative pt frame. Existing
  panels, curves and loose content stay in place; overlap is allowed.
- Its sole empty, visible, unlocked free-layout panel: create only the plot,
  preserving the panel's page position and size. Omit `canvas`, `panel_name` and `panel_frame_pt`.

`panel_name` optionally names a newly created panel; otherwise it uses the plot
name. Names must be nonblank and at most 1024 UTF-8 bytes. `panel_frame_pt`
sets only a newly created panel: X/Y are finite page pt, width/height are finite
and positive. Omission retains the original empty-page native placement contract;
an already plotted canvas requires the explicit frame. Existing content is never
retiled or rebuilt into a replacement plot.

`preflight.binding_plan.creation` returns proposed complete targets in
`canvas`, `panel`, `plot` and `created`, plus `size_mm`, `panel_frame_pt`
(page coordinates), `frame_pt` (panel-local content coordinates),
`coordinate_units` and `series_defaults` (kind and line width in pt).
`binding_plan.bindings` returns each exact owner+SeriesId target, actual source
mapping/revision and full-row diagnostics. Preflight creates nothing. Execute
the one-use plan; `ToolResult.value.creation` returns the same `canvas`,
`panel`, `plot`, `size_mm`, any applied `size_preset`, `frame_pt`,
`panel_frame_pt`, `coordinate_units` and `series_defaults`, and
`value.bindings` returns each series `target` with its row counts
(`total_rows`, `plotted_rows`, `missing_xy`, `non_finite_xy`, `missing_error`).
The parent, source mappings and sample affected row IDs stay in the preflight;
`ToolResult.targets` lists every created target, and the result's
`after_revision` is the committed revision.

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

## Author page layout, text and local images atomically

Plan `figure.edit` with `parameters: {canvas: EXACT_CANVAS_TARGET, edits: [...]}`;
omit top-level `ids`/`targets`. Discover the complete tagged edit schema in
`connection_capabilities.operation_constraints["figure.edit"].schema`.
One batch changes one canvas, in supplied edit order, with one Undo/Redo:

| `op` | Required fields and meaning |
| --- | --- |
| `page` | At least one of `size_mm: [width,height]` or `size_preset`, `margin_mm: [top,right,bottom,left]`, `gutter_mm`; lengths in mm. A journal `size_preset` sets the column width and keeps the height of a page with content; paper and slide presets set both dimensions. Presets also replace the page margins (0 mm journal, 15 mm paper, 10 mm slides) unless `margin_mm` is given; the result reports `size_preset` |
| `panel_frame` | `target: EXACT_PANEL_TARGET`, `frame_pt: [x,y,width,height]` in page pt |
| `content_frame` | `target: EXACT_CONTENT_TARGET`, complete `frame_pt: [x,y,width,height]` in parent-local pt; changes only this item |
| `image_replace` | `target: EXACT_RASTER_TARGET`, `path`; replaces only its asset reference with frozen local PNG/JPEG |
| `content_delete` | `target: EXACT_CONTENT_TARGET`; removes this content, retaining its empty parent |
| `panel_delete` | `target: EXACT_PANEL_TARGET`; requires an originally empty panel |
| `series_delete` | `target: EXACT_SERIES_TARGET`; removes a non-final explicit Series |
| `panel_label` | `target`, `text`, `visible`; writes a native manual panel label, preserving its position/font |
| `panel_stack` | `panel: EXACT_PANEL_TARGET`, `shared_x: true\|false`; optional `gap_pt` (0–144) and `merge: [EXACT_PANEL_TARGET,...]`, both only with `shared_x: true`. Stacks the panel's plots on one shared x axis or returns it to free layout |
| `align` | `panels: [EXACT_PANEL_TARGET,...]`, `edge: left\|center\|right\|top\|middle\|bottom`; at least two panels |
| `distribute` | `panels`, `axis: horizontal\|vertical`; at least three panels |
| `grid` | `panels`, `rows`, `cols`; native Frame-basis cells in supplied target order |
| `text_create` | `parent: EXACT_CANVAS_OR_PANEL_TARGET`, `name`, `frame_pt`, `text`, `font_pt` |
| `text_set` | `target: EXACT_TEXT_TARGET`, `text`; optional `font_pt` |
| `image_create` | `parent`, `name`, `frame_pt`, `path: "/absolute/local/image.png"` (PNG or JPEG) |
| `annotation_create` | `target: EXACT_PLOT_TARGET`, `text`, `at: [x,y]` in the plot's data coordinates, `font_pt`; optional `kind: "text"` |
| `annotation_create` (reference line) | `target: EXACT_PLOT_TARGET`, `kind: "reference_line"`, `orientation: vertical\|horizontal`, `value`; optional `label`, `font_pt`, `line: {color, width_pt, style}` |
| `annotation_create` (arrow) | `target: EXACT_PLOT_TARGET`, `kind: "arrow"`, tail `at: [x,y]`, head `to: [x,y]`; optional `head: none\|open\|filled\|double`, `line_width_pt`, `color: "#rrggbb"` |
| `annotation_set` | `target: EXACT_PLOT_TARGET`, `annotation_id`; at least one of `text`, `at`, `font_pt` (text), `orientation`, `value`, `label`, `font_pt`, `line` (reference line) or `at`, `to`, `head`, `line_width_pt`, `color` (arrow) |
| `annotation_delete` | `target: EXACT_PLOT_TARGET`, `annotation_id` |

For example, after creating two plots, set their page and selected-panel grid:

```json
{
  "tool_id": "figure.edit",
  "expected_project_context": "PROJECT_CONTEXT",
  "expected_revision": 3,
  "parameters": {
    "canvas": {"resource":{"id":"CANVAS_UUID","kind":"plotium.canvas"}},
    "edits": [
      {"op":"page","size_mm":[180,90],"margin_mm":[4,4,15,4],"gutter_mm":5},
      {"op":"grid","panels":[{"resource":{"id":"CANVAS_UUID/panels/PANEL_A_UUID","kind":"plotium.canvas.panel","parent_id":"CANVAS_UUID","local_id":"PANEL_A_UUID"}},{"resource":{"id":"CANVAS_UUID/panels/PANEL_B_UUID","kind":"plotium.canvas.panel","parent_id":"CANVAS_UUID","local_id":"PANEL_B_UUID"}}],"rows":1,"cols":2}
    ]
  }
}
```

Replace all identity placeholders with discovered complete targets, not names. The resulting frames are (4,4,83.5,71) and (92.5,4,83.5,71) mm.
A frame uses a top-left origin, X rightwards and Y downwards; 1 pt = 1/72 inch.
Panel frames use the existing **Object Geometry/drag authoring** semantics:
moving preserves child-local coordinates; resizing scales all child-local frames
proportionally, without changing curve IDs, bindings, styles, axis ranges or fonts.
This differs from the free Panel inspector's container-only dimensions.
New text/image frames are parent-local pt, or page pt for a canvas parent.

`panel_stack` with `shared_x: true` needs at least two plots. `plot.create`
places each new plot in its own Panel, so list the other plots' Panels in
`merge`: they are merged into `panel` and their plots are stacked top to bottom
in their page order, like **Stack with Shared X** on the desktop. Without
`merge` the plots keep the Panel's content order. The stack fills the Panel's
inner width;
their heights keep the proportions of their current frames, so change one with
`content_frame` to give it a larger share. The gap defaults to 0 pt when the
stack is turned on. Only the bottom plot draws x tick labels and the x title,
only the top plot draws a figure title, inner boundaries touch with inward x
ticks, and the plot areas share left and right edges. Entering the stack writes
one x range covering every member's data to each member (in its own coordinate
unit), so members must already share an x scale; every member draws the bottom
member's x direction without storing it. The plots share one x viewport, and
later x range, x scale, x display unit and x thousands-separator edits on any
member apply to all of them in one Undo step; the write is rejected when any
member cannot take the value, and `applied` lists the member edits. `shared_x: false` returns the Panel to free layout and keeps
the frames. The result lists each operation in `panel_edits` with `layout`,
`gap_pt`, `members_top_to_bottom` and `merged_panels`. `plotium_document_inspect` reports every
Panel's `layout` (`free`, `vertical_stack`, `horizontal_stack`, `grid` or
`shared_x_stack`); a stack adds `shared_x_stack: {members_top_to_bottom,
gap_pt}`. `plotium_figure_diagnose` and plan previews use the stacked geometry.

Plot annotations are anchored in data coordinates, not page pt: the label is
centred above its `at` point and follows the plot's axis ranges, zoom and pan,
in the canvas and in every export. An anchor outside the visible range is
clipped with the plot. `annotation_id` is local to the plot and never reused
after deletion; read it from `annotation_edits` in the result or from the plot's
`annotations` list in `plotium_document_inspect`. `at` uses the plot's source
units reported in `coordinate_units`, not a display unit chosen for the tick
labels. Annotation text permits 1024 UTF-8 bytes (nonblank), and a plot holds at
most 256 annotations. The plot and its parent panel must be unlocked; Auto height
may stay on.

A reference line is a plot annotation of `kind: "reference_line"`: a vertical
line at x = `value` or a horizontal line at y = `value`, in the same source
units as `at`, that spans the whole plot area. It follows the axis ranges, zoom
and pan, and disappears when `value` lies outside the visible range. `line`
sets `color` (`#rrggbb`), `width_pt` (0.1–20) and `style` (`solid`, `dashed`,
`dotted` or `dash_dot`); a new line defaults to `#000000`, 0.75 pt (or the
theme's supporting-line width, 0.3 pt under Nature), dashed, and
omitted `line` fields keep their value on `annotation_set`. The optional
`label` is drawn in the line colour near the top of a vertical line or the right
end of a horizontal one. It uses the tick label size until `font_pt` sets an
explicit size, which then stays until changed. An empty `label` removes it. Reference lines and text
annotations share one id sequence and the 256-per-plot budget, and
`plotium_document_inspect` lists both under `annotations`, each with its `kind`.
Fields of the other kind are rejected rather than ignored.

```json
{"op":"annotation_create","target":EXACT_PLOT_TARGET,"kind":"reference_line",
 "orientation":"horizontal","value":0,"label":"baseline",
 "line":{"color":"#7f7f7f","width_pt":0.5,"style":"dotted"}}
```

An arrow annotation (`kind: "arrow"`) draws a straight line from its tail `at` to
its head `to`, both in the same data coordinates, so it follows the axes and is
clipped to the plot area like a label. `head` defaults to `filled` (`double`
fills both ends, `none` draws a plain line); `line_width_pt` (0.1–20 pt) defaults to 1 pt, or the
theme's supporting-line width (0.3 pt under Nature); `color` defaults to black. The two ends must differ. An arrow carries no
text: label it with a separate text annotation, for example at its tail. Arrows
share the `annotation_id` sequence and 256-per-plot budget with the other kinds,
are listed under `annotations` with `kind: "arrow"`, and reject fields of the
other kinds.

Align uses the selected bounding edges/centres. Distribution equalizes centres
between fixed endpoints, not edge gaps. Grid composes the existing native cell
algorithm with the authoring resize setter for an explicit subset; it does not
claim an existing GUI subset-grid button. Grid uses the current page margins and
gutter, but does not change saved grid preferences or simplify axes. Page size
and margin edits alone never rearrange content. Non-target panels and loose
content remain in place, including any resulting overlap or clipping.

Panel layout-operation targets must be distinct, on this canvas, unlocked and
use free or shared-x stack layout; new text and images need a free-layout parent;
locked children reject a panel edit. Hidden explicit targets remain addressable. Turn off native Auto height before
exact page/layout authoring.
Dimensions must be finite/positive; page size is 10–1000 mm, margins/gutter are
nonnegative and must leave positive space. Grid is 1–16 rows/columns with enough
cells for every target; each cell must reach the native 1 pt minimum. Text permits 16384 UTF-8 bytes and 1–72 pt fonts; names
and manual labels permit 1024 bytes (nonblank). A batch permits 1–200 edits and
at most 200 panels per layout operation.

Image preflight reads only explicitly named absolute regular files: at most 8
image inputs (creation and replacement combined; duplicates still count), 1 MiB encoded per file, 4 MiB encoded per batch, and 4 MiB decoded RGBA8
per image. PNG/JPEG probing and decoding reuse native image I/O; bytes are frozen
in the plan and embedded in the native asset store at commit. Identical encoded
assets are shared, not linked to the external path. Missing/damaged assets or
unsupported formats reject the entire batch with `invalid_parameters` and a
specific asset error. There is no directory scan, network fetch or source write.

`preflight.figure_plan` reports proposed `created`/`modified` targets and `result`
with page dimensions, margins/gutter and exact `panel_frames`. Execute returns
that result in `ToolResult.value`. Preflight commits nothing; invalid targets,
geometry, assets or a stale revision leave no partial objects. Both planning and
execution require Read and modify. Plans are single-use, including failed
execution; the 32-plan and 256 KiB response limits apply. One Undo restores the
whole batch; Redo restores the same native content and asset IDs. After a lost
execute response, inspect the proposed targets before making another create call.

Use the ordinary GUI Object Geometry, text and image inspectors to continue
editing. Preview the actual canvas PNG after layout changes; native text frames
can clip long text, and layout never silently shrinks fonts or resolves collisions.

## Move existing content or replace an image in place

To widen an existing caption or replace one of two images sharing an asset, first
inspect the exact content with `plotium_document_inspect`, `view: "self"`. Copy
its complete target, not its name, selection or collection position. Use the
current context/revision and plan `figure.edit` with edits such as:

```json
[
  {"op":"content_frame","target":{"resource":{"id":"CANVAS_UUID/CONTENT_ID","kind":"plotium.canvas.object","parent_id":"CANVAS_UUID","local_id":"CONTENT_ID"}},"frame_pt":[12,18,140,30]},
  {"op":"image_replace","target":{"resource":{"id":"CANVAS_UUID/IMAGE_ID","kind":"plotium.canvas.object","parent_id":"CANVAS_UUID","local_id":"IMAGE_ID"}},"path":"/absolute/local/replacement.png"}
]
```

Place this array in `parameters.edits` alongside the discovered whole-canvas
`parameters.canvas`. Omit top-level `ids`/`targets`. The two targets above may be
different items; to move and replace the same image, repeat that image's target.
Existing text/font edits still use `text_set`.

`content_frame` supports native plot, text, raster and existing rect/ellipse/line/
arrow content, without creating or reparenting anything. It sets a **complete**
frame, not a relative delta. Coordinates use the parent's local pt, or page pt
for loose content: top-left origin, X rightwards, Y downwards. `parent` identifies
the actual canvas/panel; `page_frame` uses the native parent-origin translation.
Positions may extend beyond the panel or page and are not clamped or snapped.
All components and converted bounds must be finite, and width/height must be at
least 1 pt. This numeric minimum does not change the GUI drag minimum.

Only the target item changes (in a shared-x stack its siblings reflow to the
new share): its parent and siblings otherwise keep their frames/order;
plot bindings, Series identities, sources, units, manual axis ranges and styles,
and text contents/fonts remain intact. The target and its parent must be unlocked;
a parent must use Free or Shared X Stack layout and Auto height must be off. In a
shared-x stack only the height counts: it sets the item's share of the stack, and
the stack reflows. A locked sibling does
not block this edit. `geometry.frame_editable` and `frame_edit_error` report these
native eligibility conditions, independent of connection modify permission.

For a raster with `preserve_aspect: true`, dimensions preserve the **existing
frame ratio**, not the replacement's pixel ratio. The native aspect rule chooses
the width/height scale with the larger absolute departure from 1 (width wins a
tie), bounded by the 1 pt minimum. Requested X/Y remain the top-left anchor.
Preflight reports `requested_frame_pt` and `effective_frame_pt`; they may differ.
`geometry` exposes `min_size_pt`, `preserve_aspect` and `aspect_ratio`.
`fit` independently controls contain/cover/stretch rendering within the frame.

`image_replace` sets only the target's AssetId. It retains ContentId, parent,
frame, z-order, name, visibility and all native image attributes:
`page_index`, crop, fit, quarter-turn rotation, opacity, interpolation and
preserve_aspect. Other references to the old asset remain unchanged. A retained
crop/fit can make the new image look different; settings are never reset to hide
that effect. PNG/JPEG replacement requires the existing `page_index` to be 0;
other pages reject the batch instead of silently changing page index. Replacement
requires the target/parent unlocked, but not Free layout or Auto height off.
`image.replaceable` and `replace_error` disclose that eligibility.

Creation and replacement share the image budgets above, byte freezing, encoded
asset deduplication, embedding and Undo lifecycle. Changing or deleting the source
after preflight cannot alter the candidate or Redo. There is no source write or
asset garbage collection. Bad files, unsupported formats, wrong/stale targets,
locked targets and invalid geometry reject the whole batch, not just that edit.

`preflight.figure_plan.result.content_edits` reports each new edit in input order:
`edit_index`, `op`, `target`, `parent`, `frame_space`, `before_frame_pt`,
`effective_frame_pt`, `page_frame` and `geometry`; frame edits add
`requested_frame_pt`, replacement adds `asset_change` with `before`/`after`
AssetIds, `format` and `pixel_size`. These are the state at that edit's step.
`result.content_frames` gives the final geometry for each distinct targeted
content after all edits, including any later explicit parent-panel resize.
Execute returns the same result in `ToolResult.value`. Both operations reuse the
same permission/context/revision checks and one-use plans as other figure edits;
one Undo restores the batch's frames and asset references, Redo restores the
same identities and frozen image. Inspect/render the committed state afterwards.

## Delete exact content or selected Series

Ask the agent: “Delete this duplicate note and this redundant curve, keeping the
other curve with the same name.” Discover the complete canvas/content targets
and plot-owned Series targets first. Do not identify them by name, row index,
visible trace number or the current GUI selection.

Use a deletion-only `figure.edit` batch on one canvas:

```json
{"tool_id":"figure.edit","expected_project_context":"PROJECT_CONTEXT","expected_revision":12,"parameters":{
  "canvas":{"resource":{"id":"CANVAS_UUID","kind":"plotium.canvas"}},
  "edits":[
    {"op":"content_delete","target":{"resource":{"id":"CANVAS_UUID/CONTENT_ID","kind":"plotium.canvas.object","parent_id":"CANVAS_UUID","local_id":"CONTENT_ID"}}},
    {"op":"series_delete","target":{"resource":{"id":"CANVAS_UUID/PLOT_ID","kind":"plotium.canvas.object","parent_id":"CANVAS_UUID","local_id":"PLOT_ID"},"component":{"kind":"series","id":2}}}
  ]
}}
```

`content_delete` removes one whole native plot, text, raster or existing
rect/ellipse/line/arrow shape. Deleting a plot does not delete its source Dataset.
`panel_delete` takes a complete Panel target and requires it to be empty in the
original state. Deleting all its children in the same batch does not make a
nonempty panel eligible. Content deletion retains the empty parent panel and
does not reflow siblings.

`series_delete` removes only the exact SeriesId inside its owning plot. It
supports line/scatter plots with explicit column bindings and an established
coordinate contract, including bindings to derived tables. The last Series
cannot be removed: use an explicit `content_delete` to remove the whole plot.
Table-default bindings and dynamic multi-channel displays are rejected; this
operation never materializes, rebinds or edits a Dataset. Remaining Series keep
their identities, sources, order, encodings, coordinate units and manual ranges,
including when the first Series is deleted.

Duplicate targets, a whole plot plus any of its Series, cross-canvas targets,
locked targets/parents, and mixing deletion with non-deletion edits reject the
whole batch. Hidden exact targets remain eligible. A locked sibling is not a
reason to reject an unrelated deletion. Native automatic page height remains
enabled if it was enabled; its derived height is reported, not silently disabled.

Read `preflight.figure_plan.deleted` and `result.deleted_targets` before executing.
`result.deletions` includes parent/order information and all owned Series removed
with a plot. `parent_changes`, `group_changes`, `viewport_link_changes` and
`selection_change` list necessary relationship updates; group/link changes retain
surviving members or remove a relationship that has fewer than two members.
`remaining_series` reports the surviving order and active Series identity, `retained_asset_ids` lists the
assets referenced by removed raster content, and `auto_height` reports derived
before/after heights when enabled. These are metadata, not a candidate image.
Execute returns the same deletion manifest in `ToolResult.value`.

Planning commits nothing. One successful execute is one atomic Undo/Redo;
invalid batches leave no partial deletion. Undo restores exact identities,
parents, object/panel order, groups, links and necessary selection. Identity
allocator high-water marks never move backwards. No original file, Dataset or
asset is deleted. Runtime Undo retains embedded assets; saving uses the existing
reachable-asset policy, and reopening a project does not restore Undo history.

The existing modify permission, project-context/revision checks and one-use
plan rules apply. A selection change that alters the reported deletion effect
also requires a new preflight, even though GUI selection does not advance the
revision. Up to 200 explicit deletion targets, 2,000,000 remaining curve-row
visits per Series candidate batch, 32 shared pending plans and the complete
256 KiB response budget apply. Oversized manifests fail rather than truncate.
Target/contract failures return `invalid_parameters` with the reason;
`stale_revision`, `project_changed`, `permission_denied`, `expired_plan`,
`plan_limit` and `response_too_large` retain their existing meanings.

After an unknown execute outcome, rediscover the current context, revision and
exact targets before planning again; do not replay deletion automatically. View
the committed PNG, then use native Undo/Redo and save/reopen to check the result.

## Diagnose figure geometry at an output size

Ask: “At 180 × 90 mm, report out-of-page content, clipped geometry, text overflow,
intentional overlap between these two captions, and possible line/scatter occlusion
by legends. Do not edit anything. Inspect the returned targets and native PNG.”

After discovering the canvas, call `plotium_figure_diagnose` directly (no edit plan):

```json
{
  "expected_project_context": "PROJECT_CONTEXT",
  "expected_revision": 7,
  "canvas": {"resource":{"id":"CANVAS_UUID","kind":"plotium.canvas"}},
  "output": {"size_mm":[180,90]},
  "checks": ["bounds","clipping","text_overflow","content_overlap","legend_occlusion"],
  "view": "both",
  "limit": 100
}
```

Copy complete content or Series targets into optional `targets` to narrow the scan.
A Series target also includes its owning plot frame and guides as context.
Omission scans the canvas; it never uses GUI selection. `content_overlap` additionally
requires explicit whole-content `overlap_targets` (at least two). It compares visible,
page-clipped frames, not opaque pixels. `view` accepts `layout`, `issues` or `both`;
coverage rows are always returned.

`output.size_mm` must be positive and preserve the canvas aspect ratio (0.001 mm
height tolerance). Like native export, it uniformly scales the existing page without
reflow. Every initial query recalculates geometry. Results use `output_page_pt`:
72 points per inch, top-left origin, X right, Y down. `output` includes native size
and scale; text, stroke widths and bounds scale together. Panel-local frames are
converted through the native page-frame service. Plot data uses the actual plot
clip; a panel border is **not** a general content clip. Native raster frame and
`clip_children` clipping are reported separately.

`ToolResult.value` contains `project_context`, `basis_revision`, `canvas`, `output`,
`items`, `coverage`, `budget`, `truncated` and `continuation`. Each issue has a full
native `target`, an owner-relative `component`, `code`, `severity`, explanation and
bounds/clip/intersection evidence. Pass `target` to `plotium_document_inspect`
with `view: "self"`; names do not identify objects. `guide.legend` belongs to its
plot, not a new resource. A curve conflict points to the existing Series and its
owning plot's legend through `evidence.other_target` / `other_component`.

- `out_of_canvas`, `partially_clipped`, `fully_clipped` and
  `curve_primitives_clipped` / `shape_clipped` are informational facts, including intentional manual
  range clipping. Curve evidence applies to the reported native run's point range.
- `text_container_overflow` is a warning based on native width/line-spacing
  **estimates**, not exact glyph ink. Text does not automatically wrap or shrink.
- `content_frame_overlap` is informational; deliberate overlap is allowed.
- `right_axis_untitled` is a warning: curves use the right y axis but it shows
  no title. Set `object.axes.y2_label` (or turn `object.axes.y2_show_label`
  back on).
- `legend_occlusion_potential` is a warning, not a claim that data is invisible.
  The scan visits projected line segments and marker shapes after effective
  clipping, retaining dash phase across batches. It does not use a whole-curve
  bounding box or sample points. Stroke envelopes and the legend layout rectangle
  are conservative; rounded corners, translucency and exact antialiasing can differ.

Native guide layout includes estimated text widths. Font availability is checked
using native export font discovery; glyph coverage and cross-backend ink bounds
remain unverified. Raster viewport geometry uses header dimensions, integer crop,
rotation and fit; successful decoding and alpha-pixel coverage require the preview.
Non-line/scatter internal geometry, error-bar clipping/occlusion, derived scientific
annotations and opaque-pixel occlusion are not covered. Native rectangle, ellipse, line and arrow extrema use the actual page clip. Their frame/guide
observations remain available; coverage states the limitation. This tool does not
judge aesthetics, scientific correctness or statistical significance.

**Read coverage even when no issues are returned.** Per-target statuses distinguish
`complete`, `unsupported`, `unavailable` and `not_applicable` (for example hidden,
empty rendered data or absent legend). Overall `partial` means more pages remain;
`limited` means completed traversal with unsupported/unavailable parts. Estimates
remain estimates even with completed traversal. Counts describe scanned objects,
source points and overlap pairs, never a percentage of pixels. Missing assets/fonts,
invalid sources and excluded geometry are reported, not treated as a clean figure.

Follow a non-null `continuation` using only these arguments:

```json
{"expected_project_context":"PROJECT_CONTEXT","expected_revision":7,"cursor":"RETURNED_CURSOR"}
```

The one-use cursor binds the original target, output, checks and page configuration
and expires after 300 idle seconds. A new initial query replaces the previous scan.
Finish all pages before claiming coverage; an empty issue page can still be truncated.
Only one diagnostic batch and one unread snapshot are retained per desktop connection.
The native stdio facade keeps one diagnostic transport between pages and closes it
when stdio ends; other tools retain their one-call, never-replayed transport.
Limits are 200 explicit targets, 200 overlap targets / 19,900 pairs, 200 objects and
1,000,000 source points per batch, 4,096 rendered runs and polygons per plot, 1 MiB text,
64 MiB estimated snapshot payload and 16 MiB encoded-asset probes. Pages default to
100 rows (maximum 200), with 96 KiB page values and 256 KiB JSON-RPC responses.
These are payload/work budgets, not a total process-memory bound.
Snapshot/text overflow returns `diagnostic_budget_exceeded`; narrow the target.
Batch/row limits return `truncated: true` and a cursor without sampling.

Context, revision, busy barriers and connection access are rechecked before delivery.
Edits, project changes, cancellation, connection shutdown and transport closure
invalidate affected work; expired/consumed cursors return `expired_cursor`.
After `stale_revision` or `project_changed`, read state and start again, never combine
old and new pages. Read-only access suffices. Reads change no object, binding, manual
range, geometry, Undo history, automation revision, edit generation or dirty state.
They do not append project audit records. Geometry, issues, cursors and PNGs are
not persisted as diagnostic copies.

Use `preview_request` to obtain the native PNG, then inspect it. Convert a returned
X coordinate to pixels using `x * width_px / output.size_pt[0]`; use height
for Y. Check its `render_revision` against `basis_revision`. Editor zoom does not
change these units. Direct core `figure.diagnose` returns one bounded batch without
a retained cursor; the MCP query provides continuation. No automatic fix is applied.

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
Read-only connections permit previews. Previews do not append project run records
or change dirty state, edit generation or automation revision.

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

## Preview a plan before committing it

After `plotium_plan`, call `plotium_preview_plan` with the **same** plan ID,
context and revision, and one affected whole-canvas target:

```json
{
  "plan_id": "RETURNED_PLAN_ID",
  "expected_project_context": "PROJECT_CONTEXT",
  "expected_revision": 7,
  "target": {"resource":{"id":"CANVAS_UUID","kind":"plotium.canvas"}},
  "max_edge": 1440
}
```

The response contains an actual MCP PNG image block. `ToolResult.value` contains
`plan_id`, `basis_project_context`, `basis_revision`, `candidate: true`, `target`,
`width_px`, `height_px`, `mime_type` and `output` (`size_mm`, `size_pt`, `max_edge`).
There is no `render_revision`: this image has not been committed. The read's
`before_revision` and `after_revision` both identify the baseline. Read the image,
then explicitly call `plotium_execute` with the original `plan_id`. Finally read
`plotium_render_preview` and fresh `plotium_figure_diagnose` results. Candidate
preview does not introduce candidate diagnostics or an automatic repair loop.
Keep diagnostic `measurement_basis`, `coverage` and `unsupported` records:
estimated text bounds are not glyph ink, and a potential legend conflict is not
proof of occlusion.

| Existing plan | Candidate support |
| --- | --- |
| `properties.set` / `properties.reset` | The five `object.guides.*` properties above, including their native multi-target batch |
| `figure.edit` local edits | `content_frame`, `image_replace`, `text_set`, `annotation_create`/`annotation_set`/`annotation_delete` (text and reference lines), including native batches |
| `figure.edit` deletion-only batch | `content_delete`, originally empty `panel_delete`, explicit-column `series_delete`; all existing refusal rules remain |
| Other operations, including save/export | `unsupported_operation`; their normal execution contract is unchanged |

Affected canvases must use ordinary line/scatter plots, native text, shapes and
raster content; previews do not launch scientific computations. A cross-canvas
guide plan applies its complete batch to the disposable native context; request
each affected canvas separately. It is still one plan and one Undo step.
Automatic axes, guide dependencies and derived page height use the native action
and renderer, not a second layout model. With identical baseline, settings and
fonts, candidate content and geometry match execution. Cross-machine font or
renderer differences are not a pixel-equivalence guarantee.

Candidate reads require **Read and modify**, checked at capture and background
delivery alongside context, revision, idle state and the still-pending plan.
Revocation and regrant during rendering also prevent delivery. Preview neither
consumes the plan nor extends its lifetime or grants execution permission.
Ordinary edit plans have no wall-clock TTL; they share the 32-plan session cap
and are cleared on context/connection changes. Submitted plans are single-use,
including failed submissions. `expired_plan` means absent/consumed;
`stale_revision`, `project_changed`, `permission_denied`, `busy`,
`request_cancelled` and the image errors above describe other refusals.
After a human edit or project change, rediscover and replan. Never replay an
outcome-unknown execute after a disconnect.

Preview borrows frozen inputs and image bytes, without rereading a replacement
path, allocating planned identities or executing then undoing the live document.
It does not change authored content, data, bindings, selection, history, automation
revision, persistent assets, edit generation or dirty state. It adds no project
audit record and persists no PNG bytes. No candidate file or cache is created. Candidate and
committed previews share the single rendering slot, cancellation and image budgets
above. Candidate construction additionally rejects a native snapshot estimate over
64 MiB (canvas geometry, source field dimensions at 64 bytes/element, metadata and
frozen assets); this estimate is not a process-memory ceiling. Cancelled project
work retains the slot until the bounded renderer exits.

## Position a plot's legend precisely

Guide properties use `ids:[PLOT_OBJECT_ID]`, not Series components. Discover the
object by its full canvas/object identity; plot names need not be unique. Use
`properties.set` with one key/value, review its plan, then execute the returned
one-use plan ID:

```json
{
  "expected_project_context": "PROJECT_CONTEXT",
  "expected_revision": 7,
  "tool_id": "properties.set",
  "ids": ["CANVAS_UUID/OBJECT_ID"],
  "parameters": {"key":"object.guides.position","value":[0,1]}
}
```

`position` accepts exactly two finite numbers in [0,1], or `null`. It places the
legend box's top-left origin within the space in which that complete box can
move inside the axes' plotting rectangle. x increases rightward, y downward:
`origin = plot_origin + [x,y] * max(plot_size - legend_size, 0)`. These are not
screen pixels, page fractions or data coordinates. [0,0] is top-left; [1,1]
aligns bottom-right when the legend fits. The renderer recomputes this mapping
when the output size, legend content or layout changes.

The native properties and their necessary coupling are:

- `visibility`: `auto`, `show`, `hide`. Auto shows a categorical legend with at
  least two native entries; Show permits one; Hide hides guides, not data.
- `placement`: `auto`, `inside`, `outside_right`, `outside_bottom`, `custom`.
  Custom retains an existing origin or starts at [1,0], and sets Inside.
  Every other placement clears the custom origin.
- `position`: [x,y] sets Inside/custom. `null`, position reset, placement Auto
  and placement reset clear both origin and placement overrides, matching GUI
  double-click. A read of `null` means no custom origin; consult placement for
  the actual mode, which may be outside.
- `layout`: `auto`, `vertical`, `horizontal`, retaining native layout behavior.
- `title`: text; blank/whitespace clears its override using native normalization.

Visibility/layout/title reset only their own overrides. Position changes and
resets retain these fields, font size, bindings, curve styles and manual axis
ranges. Guides still use native entry merging; six curves need not produce six
entries when their native legend identities merge. The four existing properties
retain their color-scale behavior. Free two-dimensional positioning applies to
categorical legend content, independent of guide visibility. Retained origins
without content remain readable/resettable, but new coordinates require legend
content; they do not reposition a colorbar independently.

Guide inspection adds `guide_geometry` to applicable readings. It returns
`coordinate_space:"object_local_pt"`, `scale:1`, `plot_rect`, nullable
`legend_rect`/`color_scale_rect` (x/y/width/height), `entry_count`, nullable
`available_travel_pt`, `effective_position`, `fits_plot` (two size-fit booleans),
and `not_rendered_reason` (`hidden`, `no_entries`, `auto_single_entry`, or null).
`measurement_basis:"renderer_layout_text_width_estimate"` names the same layout
boxes used by rendering, not exact glyph bounds. Object/parent visibility and
clipping are not evaluated here. Effective position is the
resolved origin relative to available travel, without clamping outside placements;
a zero-travel axis reports zero. Guide-hidden or automatically omitted legends return null geometry
and the reason, while the property value retains the configured origin.

An oversized legend is not silently shrunk. Its unavailable travel axes are zero,
`fits_plot` reports false, and the effective position can differ from the requested
origin. Size fit does not promise absence of clipping or curve overlap; inspect
the native PNG and final exports. This observation is limited to guide layout,
not a whole-figure diagnostic.

Guide writes validate every explicit target before committing; an existing but
inapplicable target rejects the whole batch with `invalid_parameters`, unlike
the unchanged skip policy of older properties. Duplicate IDs are processed once.
One request changes one property across at most 200 IDs with one Undo/Redo step;
titles are limited to 16,384 UTF-8 bytes, structured responses to 256 KiB
(`response_too_large`), and the existing shared limit is 32 pending plans.
Malformed/out-of-range values fail with `invalid_parameters`. Inspection reports
inapplicable targets. Read-only connections can inspect/preflight but execute
requires reversible-modify permission. Context/revision changes, revocation,
busy and expired-plan failures retain the normal single-use contract. Inspection
does not add project audit records or change dirty state, edit generation,
automation revision or Undo history. Discover these rules in `connection_capabilities.guide_contract`.

## Change display without rebuilding the plot

Use the exact target returned in `creation.series_defaults[].target`, or discover
it with `plotium_document_inspect`. The owner resource and stable SeriesId travel
together: same-name curves remain separate targets. `plotium_properties_list`
lists the allowed properties; `plotium_properties_inspect` supplies current values,
defaults, target-specific unit choices and range input schemas.

Read `plotium_get_state` first. Call `plotium_plan` with its `project_context`
and current revision, replacing the placeholders below with discovered values:

```json
{
  "expected_project_context": "PROJECT_CONTEXT",
  "expected_revision": 7,
  "tool_id": "properties.set",
  "targets": [{
    "resource": {"id":"CANVAS_UUID/OBJECT_ID","kind":"plotium.canvas.object",
      "parent_id":"CANVAS_UUID","local_id":"OBJECT_ID"},
    "component": {"kind":"series","id":1}
  }],
  "parameters": {"key": "series.line.color", "value": "#0072B2"}
}
```

Review the preflight, then pass only its returned `plan_id` to `plotium_execute`.
This changes only that curve's color, including its error bars, not its width or
source columns. Axis and local font properties instead take `ids:[PLOT_OBJECT_ID]`.

One request changes one key across its targets, atomically, with one Undo step.
An existing but inapplicable target is reported as skipped; a missing or forged
identity rejects the request. Invalid colors, non-finite values and invalid
ranges are refused at preflight. Execution rechecks revision and permission;
a rejected execute consumes its plan. Re-read state before planning again.

Range values distinguish these operations:

- `{"mode":"auto"}` clears only that axis's manual range.
- `{"mode":"manual","min":0,"max":10,"unit":"mA"}` sets finite increasing
  bounds in the stated compatible unit. `unit:null` is only for unknown coordinate
  units. A known unit cannot be introduced onto an unknown-unit plot.
- Resetting **all** axis settings is a separate native operation, not a side
  effect of Auto. Axis reset clears only the named override; guide origin/placement
  coupling follows the rules above.

Display units use compatible codes returned by property inspection, such as `s` → `ms` or `mA` → `A`.
The plot retains R4's coordinate-unit contract: source data, manual range storage,
shifts and error amplitudes keep their meaning. Ticks and automatically derived
unit text change; authored axis titles stay verbatim. Rebinding/reordering curves
and removing the first curve do not redefine coordinates. Numeric range editing
is unavailable on hidden/categorical axes; a retained range can still be cleared.
In inspection, `unit_code.codes` lists the target's compatible choices; `axis_range.input_schema` describes
the range value and its `view`/`unit` describe the current display window.

Marker size is a diameter in points, independent after explicitly set. Without
an override, native point curves retain their existing width-based radius; the
existing stroke-width API retains this behavior. Reset restores the native
marker/size, and an explicit empty legend label omits that legend entry. Stroke
style applies to line curves; scatter curves are skipped for that property.
Markers apply to line/scatter curves, not sticks, bars, boxes or violins. Bar
width and outline width apply to bar, box and violin curves; the outline color to
bar curves. Marker fill and edge color
apply to closed markers and follow the curve color until set; reset returns
them to it. A hollow marker draws no fill and keeps an outline, 0.5 pt when
no edge width is set. A chart that owns semantic colors rejects a single series
color, marker fill or marker edge color override as inapplicable.

Plot-local tick, axis-title and legend sizes inherit document typography until
set. Reset removes only that override; changing one plot's font does not style
other plots. Font/title measurements use the native renderer. These edits do not
retile pages or change panel/content positions and sizes. GUI edits use the same
catalog controls in Line, Axes, and Legend & scales; plot-local units, ranges and
font sizes appear under Advanced. Document-level font properties
retain their existing project-wide scope.

After editing, get a fresh revision and request `plotium_render_preview` for the
returned canvas. Inspect its PNG and `render_revision`, then use GUI Undo/Redo to
check the same native objects. MCP `project.save` requires separate desktop
authorization; exporting requires its own desktop authorization.

## Inspect and edit an exact curve

An example agent task is: “Find my line plot, inspect its stroke width, preflight
changing it to 1.5 pt, then apply the saved plan.” The tool sequence is:

1. `plotium_get_state` with `{}` returns `project_context`, `revision`, project overview, selection
   and connection permissions.
2. `plotium_resources_search` with `expected_project_context`, `expected_revision` and a `query` such as
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
and the same target. Component addressing supports only the listed display
properties; processing parameters use the separate `processing.edit` operation. `resource.rename` requires
`ids`.

Tool responses include structured content and its JSON text representation.
Project reads and preflight require the current `expected_revision`; fetch the
state again after edits or Undo. Execution rechecks the saved revision and current
permission. Plans are single-use, including failed submissions, and at most 32
can be pending. Restarting the connection clears them. Requests cancelled before
desktop execution are skipped without changing the project. Cancellation cannot
roll back an edit that has already started its atomic commit; use **Undo**.

Errors include stable codes: `permission_denied`, `expired_plan`,
`stale_revision`, `busy`, `invalid_parameters`, `unsupported_operation`, `unsupported_property`,
`project_changed`, `unavailable`, `connection_lost`, `plan_limit`, `request_cancelled`, `connection_stopped`, `response_too_large` and `execution_failed`.
Errors carry `error.recovery`; asynchronous save, export and task receipts carry
`recovery`. `retryable: false` means do not repeat the failed call: follow the
reported next action. A `busy` execute can already have consumed its plan.
For stale revisions, read state and review a fresh plan. An expired plan may
already have run; inspect its outcome before deciding on a new operation.
The AI connections (MCP) settings page shows recent operations and errors without
opening itself in response to background activity.

## Recovery actions and uncertain outcomes

`recovery` describes this request, not permission to execute another one:

| Field | Meaning |
| --- | --- |
| `plan_consumed` | `true` / `false` when observed; `null` when unknown. A consumed plan cannot be reused. |
| `submission` | `submitted`, `not_submitted` or `unknown`; accepted background work is not completed work. |
| `committed` | `true` / `false` when known; `null` when effects or durable completion are uncertain. |
| `completion` | `pending`, `succeeded`, `failed`, `partial` or `unknown`. |
| `receipt` | Status tool and exact ID in `arguments`; `retained` is `true` when found, `false` when missing, or `null` when unknown. A null `receipt` means no known receipt query. |
| `next_action` | One next step: `call_tool` with exact arguments, `desktop_file_confirmation`, `check_desktop_connection`, `inspect_outcome`, or `none`. |
| `replay_allowed` | Always `false`; recovery never authorizes replay or automatic resubmission. |

For `call_tool`, make only the named read. For desktop confirmation, review the
pending request's exact paths and overwrite scope in Plotium, then query its
receipt. For `check_desktop_connection`, check AI connections (MCP) and access
mode in the desktop, then read state. For `inspect_outcome`, compare the retained
receipt, saved preflight targets/paths and current state; missing evidence needs
human review before any new scientific computation or file request.

- **Busy:** planning fails before submission. Execution may consume the plan
  before the busy check; finish the desktop operation, read state and review a
  fresh plan. Never resubmit the consumed ID.
- **Stale revision / project switch / revoked permission:** refresh state and
  access in the desktop. After a switch, rediscover targets in the new context.
  Existing plan IDs and file permissions do not transfer to the new project.
- **Save refused or cancelled before installation:** the consumed plan's receipt is terminal with
  `phase: not_installed`, `committed: false` and `completion: failed`. Closing the
  confirmation is refusal. Inspect state and ask whether saving is still wanted;
  a newly intended save requires a fresh plan and desktop confirmation.
  Cancellation after a worker starts must be resolved from its final phase;
  it does not undo installed output.
- **Pending save/task:** accepted, queued, running or saving is `pending`, not
  success. Use the receipt query; do not submit a replacement task or save.
  `installed_unconfirmed` stays unknown even after a save worker terminates.
- **Export:** the batch succeeds only when completed and every file succeeded.
  Mixed terminal results are `partial`; inspect each exact output path and error.
  A failed file can have been installed before a durability error, so failure
  alone does not prove that its path is unchanged. Never retry the entire batch
  automatically, including its successful files.
- **Disconnect / expired plan / unknown or evicted receipt:** preserve unknown
  consumption, submission and commit state. Reconnect and query a known receipt
  ID in the same desktop process. If it is absent, inspect saved targets/paths;
  absence does not prove non-execution and does not authorize recomputation or
  overwrite. Receipts are not persisted across desktop restarts.

These actions retain server permissions, revision/context checks, one-use plans
and fresh per-request desktop file consent. Tool annotations are neither
authorization nor a reason to retry. Reconnect clients and refresh their tool
catalog after updating the desktop and connector together.

## Style several curves in one plan

`properties.set` writes one value. To give each curve its own colour and legend
label, plan `properties.set_many` instead: each write names an exact target, one
supported key and a value. Omit top-level `ids` and `targets`; series writes use
the complete Series target from inspection, and object-level writes such as axis
titles or legend settings use the plot object's resource target without a
component.

```json
{
  "expected_project_context": "PROJECT_CONTEXT",
  "expected_revision": 7,
  "tool_id": "properties.set_many",
  "parameters": {"writes": [
    {"target": SERIES_0_TARGET, "key": "series.line.color", "value": "#0072B2"},
    {"target": SERIES_0_TARGET, "key": "series.label", "value": "Sample 1"},
    {"target": SERIES_1_TARGET, "key": "series.line.color", "value": "#D55E00"},
    {"target": SERIES_1_TARGET, "key": "series.label", "value": "Sample 2"},
    {"target": {"resource": PLOT_OBJECT_RESOURCE}, "key": "object.axes.x_label", "value": "Temperature (°C)"}
  ]}
}
```

After `plot.create` executes, use its full stable targets directly. For example,
with the decoded creation `ToolResult` in `created` and the original state in
`state`, prepare this `plotium_plan` argument object:

```js
const request = {
  expected_project_context: state.project_context,
  expected_revision: created.after_revision,
  tool_id: "properties.set_many",
  parameters: { writes: [
    ...created.value.bindings.map((binding, index) => ({
      target: binding.target,
      key: "series.label",
      value: `Sample ${index + 1}`
    })),
    { target: created.value.creation.plot,
      key: "object.axes.x_label", value: "Temperature (°C)" }
  ] }
};
```

Copy each whole `target`, including owner resource `kind`, `id`, `parent_id`,
`local_id` and component identity when present. A bare SeriesId, name or array
index is not a selector. Keep the total within 200 writes. Preflight this object,
review it and execute its new `plan_id` once. `unsupported_operation` means the
operation is unavailable; `unsupported_property` identifies an unsupported key
in `error.details.write_index` (zero based) and `key`. Query
`plotium_properties_list` and correct the whole batch before planning again.

A batch holds 1–200 writes and accepts the display properties listed above.
`preflight.targets` has one row per write, in order. Planning refuses the whole
batch, and nothing is committed, when any write's target is missing, its
property does not apply to that target, its value is invalid, or the same key is
written twice to one target. A write whose target already holds the value is
reported as skipped while the others apply. `plotium_execute` commits every
write as one **Undo** step; `value.writes` reports each write's index, key and
outcome.

## Apply a theme or saved style to a canvas

`figure.apply_theme` restyles whole canvases with one of Plotium's built-in
themes, such as `nature`, or with a canvas style saved in Plotium. Set
`theme_id` to a built-in id from
`operation_constraints["figure.apply_theme"].builtin_themes` in
`plotium_capabilities_get`, or to the saved style's name. Saved styles are not
part of the static capabilities: an unknown `theme_id` fails preflight with
`invalid_parameters`, and `error.details.themes` lists every built-in theme and
saved style currently available. Plan it with the canvas ids in top-level `ids`:

```json
{
  "expected_project_context": "PROJECT_CONTEXT",
  "expected_revision": 7,
  "ids": ["CANVAS_ID"],
  "tool_id": "figure.apply_theme",
  "parameters": {"theme_id": "nature"}
}
```

Preflight needs only read access; executing requires Read and modify.
Preflight rejects an unknown `theme_id`, Series `targets`, and ids that include
no canvas; other ids that are not canvases are skipped. A saved style is read
again when the plan executes. Execution changes each canvas's background, text
and shape colours, and every curve's colour, marker paint, marker size and line
width the theme defines, as one **Undo** step. Curve and marker colours set by
hand are kept when a theme is re-applied; resetting a curve's colour returns it
to theme colours.
The document's style defaults are also replaced: plot text sizes, axis strokes
and ticks, so plots on other canvases change with them, and the default text,
panel-label and shape styles for new objects. A theme sized for journal columns
(today `nature`) never resizes a canvas; for each canvas whose width matches
none of its column presets, the result's `diagnostics` adds a note listing the
preset widths (89 / 136 / 183 mm).

## Save the current desktop project

Saving uses the same native snapshot, background worker, backups and completion
path as **Save project**. It saves the current desktop state, not a separate CLI
project. First read `plotium_get_state`; `save_state` reports the current path,
persisted archive revision, dirty flag, edit generation and up to 16 retained
`receipt_ids`. Query each ID with `plotium_save_status` for the complete receipt
and recovery action. An ID alone does not indicate submission success or completion.

Plan a first save or Save As with no `ids` or `targets`:

```json
{
  "expected_project_context": "PROJECT_CONTEXT",
  "expected_revision": 46,
  "tool_id": "project.save",
  "parameters": {"path": "/absolute/path/figure.plotium"}
}
```

Omit `path` to save the current file; an untitled project requires a path.
The parent directory must exist. Targets must be regular, non-symlink `.plotium`
files or absent paths. Preflight resolves the parent directory and freezes the
observed target. `include_view_snapshots` optionally overrides the project's
current native save profile. The plan returns `save_id`, the resolved path and
`overwrite_required`; it grants no file-write permission.

Submit the one-use plan with `plotium_execute {"plan_id":"PLAN_ID"}`. The result
contains `save.status: "awaiting_authorization"`. In Plotium, review the exact
path and press **Allow save**. If a file already exists, first check **Replace the
existing file at this exact path**. **Deny** leaves the target untouched.
This permission is independent of **Read and modify**, applies to one save only,
and cannot be supplied as a tool argument. A changed target requires a fresh
plan and approval. One external save may be pending at a time.

Serialization uses an exclusively created private sibling temporary, not a fixed
`<target>.tmp`. For an existing target, its frozen metadata is checked again after
backup preparation, immediately before replacement. A detected change refuses
installation and preserves the later target. Permission admission and this file
check are separate: the check plus replacement is not an inter-process
compare-and-swap. A non-cooperating write after the check, or one that preserves
the checked metadata, may go undetected; avoid concurrent saves to the same path.

Poll `plotium_save_status {"save_id":"SAVE_ID"}`; this read needs no document
revision or context, works while saving, and does not dirty the project.
`awaiting_authorization`, `queued` and `saving` are incomplete states with null
`phase`. Only `completed` gives a final phase:

| Phase | File state and next action |
| --- | --- |
| `not_installed` | This save did not install its snapshot at the target. Resolve the reported denial/failure, inspect current state, then obtain fresh authorization if another save is needed. |
| `installed_unconfirmed` | The snapshot replaced/created the target, but backup finalization or durable sync failed. Keep the desktop project open; inspect the target and storage before an explicitly authorized retry. Do not assume the old file remains. |
| `committed` | The captured snapshot completed the native durable commit. Later edits may still be unsaved. |

On Windows, a partial replacement failure can remove or move the old target even
when the new snapshot is `not_installed`. Plotium keeps the serialized candidate
and reports its exact recovery path. It restores a known original backup only
without overwriting an existing target; otherwise it retains and reports that
backup and any older staged backup. If Windows moved the original without
returning its location, the error says so. Keep the project open and inspect the
reported files before retrying; this failure does not clear dirty state.

The receipt records original `project_context`, `snapshot.document_revision`,
`snapshot.edit_generation`, `persisted_revision`, exact path, `state_applied`,
`dirty_after_completion`, error and recovery message. The status response also
reports current context/path/revision/dirty and `matches_current_project`.
The snapshot is captured after desktop approval and before background work.
New edits after capture remain dirty; a committed save does not mean every
current edit is on disk. An installed-but-unconfirmed
save keeps dirty and records the installed baseline for native recovery.

Switching projects or stopping the connection revokes pending write permission.
The worker only holds its original snapshot and path, checks authorization at
installation, and cannot adopt a completion into a different project, even at
an equal document revision. Once the worker has atomically admitted installation, stopping cannot guarantee
cancellation of that original file write; inspect its receipt. Revocation before
admission prevents installation. Saving creates no Undo step and Undo does not delete or restore files.

The desktop retains the latest 16 receipts across reconnects and project changes
until application exit. After a lost response, query the original `save_id`
before doing anything else; do not replay the submitted call. After application
restart or `unknown_save`, inspect the target and freshly opened project instead
of assuming success or failure. Stable stdio configuration continues to work;
reopening/restarting still requires fresh state, target discovery and plans.
Connection recovery is not evidence of a successful save.


## Export a captured canvas as SVG, PDF and PNG

Use `figure.export` with one exact whole-canvas TargetRef and no top-level
`ids`/`targets`. The native exporters preserve the canvas background, complete
page, curves, absolute Y-error amplitudes, native text and embedded images.
There is no trimming, theme application or project modification. Keep the
editable `.plotium` project as well as these delivery formats.

```json
{
  "expected_project_context": "PROJECT_CONTEXT",
  "expected_revision": 48,
  "tool_id": "figure.export",
  "parameters": {
    "canvas": {"resource":{"id":"CANVAS_ID","kind":"plotium.canvas"}},
    "size_mm": [180,90],
    "dpi": 300,
    "outputs": [
      {"format":"svg","path":"/absolute/path/figure.svg"},
      {"format":"pdf","path":"/absolute/path/figure.pdf"},
      {"format":"png","path":"/absolute/path/figure.png"}
    ]
  }
}
```

`size_mm` defaults to the native page size. An explicit size uniformly scales
the whole figure and must retain its aspect ratio (0.001 mm height tolerance);
it does not resize the editable page. `dpi` defaults to 300, accepts 1–2400 and
only controls PNG. Each PNG axis is rounded to the nearest integer from the
native physical points / 72 × DPI × uniform size scale, with a minimum of one
pixel. A 180 × 90 mm page at 300 DPI is **2126 × 1063 pixels**. SVG/PDF retain
physical page dimensions. Text uses the native generic sans-serif family;
a machine without usable system fonts returns a font error. SVG font appearance
on another machine depends on that machine's available sans-serif fonts.

There are 1–8 outputs per batch. Each absolute path needs its matching lowercase
extension and an existing parent directory; targets must be regular non-symlink
files or absent. Duplicate resolved output paths are refused. Preflight returns
`export_id`, physical/pixel size and an error or resolved target for each file.
A bad directory does not hide the other files' results. `overwrite` defaults to
false: an existing target reports `overwrite_not_requested` and remains untouched.
Set `overwrite:true` only when replacement is intended; this still grants no
write permission.

Submit the one-use plan through `plotium_execute`. Valid files await Plotium's
**Allow AI to export this figure?** prompt, listing the exact canvas, revision,
size and paths. Review it and press **Allow export**. Existing files additionally
require the unchecked **Replace the listed existing output files** checkbox.
**Deny** installs nothing. **Read and modify** is not export authorization;
Read only may request fresh, separate export consent. All-invalid batches finish
with per-file errors without asking for consent.

Poll `plotium_export_status {"export_id":"EXPORT_ID"}` without context/revision.
It is non-auditing, works while rendering, and returns `export`, current context,
current revision and `matches_current_project`. Batch status is
`awaiting_authorization`, `rendering` or `completed`. Each `files[]` entry has
format, exact path, overwrite requirement, `status`, `error_code`, `message`
and installed byte count. File status is `pending`, `succeeded`, `failed` or
`unknown` (worker failed; inspect that path before deciding what to do).
Completion does not imply every file succeeded. Missing/corrupt images, fonts,
changed targets, permission denial and rendering/budget errors are reported
without replacing that output. Successful files in the same batch remain.

Rendering runs off-thread on the canvas/assets captured after desktop consent,
bound to the original `project_context`, `canvas` and `render_revision`. Later
edits do not change that snapshot. Project transition, connection Stop or access
change revokes pending installation permission. Each file's atomic permission
admission precedes its installation: revocation before admission blocks it;
an already admitted file may finish and must be checked in the receipt. Old
completion never changes the new project's document, save path, dirty state or
status. Unique owned sibling staging and atomic no-clobber creation preserve
unapproved files. Existing-file metadata is checked just before replacement;
this is not a cross-process compare-and-swap, so avoid concurrent writers.
Export success means installed output, not the native save's durable-commit
phase; Undo cannot remove or restore exported files.

Limits: one running batch, 32 shared pending plans, 4096 bytes per path, PNG
16,384 pixels per edge and 16,777,216 pixels total; referenced embedded assets
16 MiB, decoded asset RGBA 64 MiB, SVG 8 MiB and encoded output 64 MiB per file.
No external asset lookup or placeholder substitution occurs. Limits and the
complete schema are discoverable in `connection_capabilities`.

The last 16 export receipts survive reconnects and project changes in this
desktop process, not application exit. After response loss query the original
`export_id`; do not resubmit the export. After restart or `unknown_export`,
inspect the exact output files first. Reopen the editable project, fetch fresh
state and targets, then make a new plan only for a newly intended operation.


## Scientific operations and tasks

`plotium_processing_inspect`, `plotium_diffusion_inspect` and
`plotium_analysis_results_inspect` are read-only queries with explicit `ids`,
`expected_project_context` and `expected_revision`. Result inspection accepts
`offset` and `limit` (1–100); use smaller pages for large ILT grids. Discovery's
`scientific_operations` comes from the core descriptors, including parameter
schemas, capabilities, permissions and execution modes.

Scientific edits use `plotium_plan` / `plotium_execute`:

| Operation | Parameters and target |
| --- | --- |
| `processing.edit` | One dataset; `edits` uses `replace`, `enable`, `add`, `remove` or `move`; existing steps use stable StepIds |
| `processing.apply_scheme` | `path`, optional `compatible_only`; preflight freezes scheme bytes, fingerprint and compatible target configurations |
| `diffusion.set_input` | One dataset; complete `input`; invalid input errors include a structured validation report |
| `analysis.regions.extract` | One region-series field; `regions` references RegionIds or explicit bounds and metrics; optional explicit `input` |
| `analysis.diffusion.fit` | One native table; stable `response_column` ColumnId, optional native `options`; the x column must use `s/m²` |
| `analysis.dosy.build` | One diffusion source field; `method` is `mono_exp` or `ilt`; optional `input` and ILT `params` |

An explicit region is
`{"source":"explicit","bounds":[0.8,1.2],"metric":"Height","name":"Signal"}`;
reference an existing region with `{"source":"existing","id":0}`. Other metrics
are `Area`, `Mean`, `Max` and `Min`. Extraction creates an independent native table
with source evidence, region-to-column correspondence and diffusion increment
bindings. It does not create a plot.

Except for input updates, these edits return `ToolResult.value.task`. Preflight
allocates `task_id`; `queued` means registered. Query with
`plotium_tasks_get({task_id})` or `plotium_tasks_list({offset,limit})`; cancel with
`plotium_tasks_cancel({task_id})` (modify permission required). States are
`queued / running / committing / succeeded / failed / cancelled / stale`.
Receipts include the original plan_id, source project, start/end revisions,
source field versions, phase, completed columns, failure code, resource references
and terminal summary. Unknown work totals are `null`. Receipt queries remain
available while busy and after a project switch.

Receipts belong to the application process. Disconnecting does not cancel
submitted work; reconnect in the same process to query it. All active tasks and
the most recent 128 terminal receipts are retained; process restart is outside
this retention contract. `task_not_found` means unknown or evicted and must never
trigger automatic recomputation. Repeating a consumed scientific plan returns
its retained receipt. Cancellation is idempotent, prevents writes before commit,
and returns the existing state after termination.

Frozen inputs run in the existing ComputeService. Before one atomic undoable
commit, the main thread checks project, document revision, field versions,
generation, cancellation and caller permission. Any document edit during
computation makes the task stale. Undo restores configuration, results and linked
tables; Redo reuses computed results. Background completion does not open or close
panels. Synchronous workflows retain `processing.apply_scheme` and wait for
its commit before dependent nodes run; other background scientific operations
require single-step calls and terminal receipts.

Diffusion inspection distinguishes executable input, missing parameters, evidence
requiring confirmation and invalid input. Issues carry codes, parameter paths,
expected units, actual values, evidence and remedies. b values bind IncrementIds
and use `s/m²`; D uses `m²/s`. Original units and conversion evidence remain
available. Resolution uses explicit input, target result provenance, then stored
default/acquisition information. JEOL validation retains full program identity,
gradient shape, timing and vendor coefficient checks.

Mono-exponential diagnostics include D, I₀, D standard error, effective points,
R², residuals and rejection reasons. ILT returns its D grid, λ, nonnegative
solution, reconstruction residuals and solver termination rather than
mono-exponential confidence estimates. `empty` means no accepted scientific
result. Unavailable numbers are `null` with a reason. Result inspection pages
column diagnostics; provenance retains algorithm version, actual parameters and
input fingerprint.

Completed decay-fit tasks return a stable `plotium.analysis` resource in the task's `produced` list. Pass its ID directly to `plotium_analysis_results_inspect` to read that analysis. A task status of `succeeded` means the calculation and result commit completed; check `converged`, `termination` and `notices` separately for scientific convergence. The outer `converged` follows the attempt with the lowest finite cost. Attempts that exhaust the iteration budget without meeting a convergence criterion or terminate with trust-region collapse remain unconverged, and their estimates are retained with a warning for diagnosis.

## Plot native scientific results

Use `plotium_plan` and `plotium_execute` with either operation below. Both
require `{source: ResourceRef, name: string}` in `parameters` and omit top-level
`ids` and `targets`. Discover the complete source reference and schema first.

| Operation | Source and native output |
| --- | --- |
| `plot.field.create` | One native field, including mono-exponential `nmr.dosy_map` or ILT `nmr.ilt_map`; creates a canvas pinned to that field and its native encoding. |
| `analysis.diffusion.plot` | One diffusion-fit analysis with one response dataset; creates a frozen table and canvas with b–intensity scatter, stored fitted curve, residual panel, D standard error, R² and effective point count. |

Preflight returns `preflight.scientific_plot.summary` with proposed `canvas`,
`source`, `produced` and scientific `evidence`; execute returns the same summary
in `value`. Plotting does not recompute scientific results. Stale or empty DOSY
results and stale, unconverged or multiple-response fits must be corrected first.
Missing D uncertainty is labelled unavailable. Field preparation permits at most
4,000,000 elements and 200 series, including a default 128-row ILT of a
20,480-point spectrum; fit plots permit 20,000 rows. Plot points,
contour vertices and heatmap cells share the 2,000,000 pending-element budget
with explicit curve rows. Contour plots count generated vertices toward this
budget; heatmaps count cells.
One Undo removes created resources; Redo restores the same identities and data.
Changing the owner's display mode does not change a field plot's source.
Frozen fit tables retain source-table lineage; the automation run retains the
selected analysis, row selection, extraction evidence and diffusion input.

Use `value.canvas` with `plotium_figure_diagnose` and `plotium_render_preview`,
then inspect the returned PNG. An image alone does not validate the science:

1. Start at the welcome page with `plotium_get_state`; import the acquisition,
   inspect processing and wait for processing tasks to succeed.
2. Inspect diffusion units, increment bindings, missing parameters and evidence.
   Supply confirmed input only when supported by the experiment.
3. State the region-selection rule, bounds, metric and overlap policy before
   extraction. Wait for extraction, then fit the returned response ColumnId.
4. Inspect the exact analysis: validity, D in `m²/s`, standard error, effective
   points, R², residuals, rejected rows and provenance. Create its native plot.
   Independently compute and inspect mono-exponential or ILT DOSY; check accepted
   and rejected columns, algorithm settings, input fingerprint and numerical
   agreement with the region fit. ILT supplies solver diagnostics instead of
   mono-exponential confidence estimates.
5. Plot the DOSY result field. Check resource identities and state, geometry
   diagnostics and both PNGs for correct axes, curves, contours, residuals and
   readable labels. Report scientific or visual failures explicitly.
