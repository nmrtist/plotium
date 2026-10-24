---
title: MCP data import and tables
description: Import data, read tables, derive and patch tables through MCP.
---

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
import contract. Discover the shared schema with
`plotium_capabilities_get {"section":"operation_constraints","key":"data.import"}`.

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
oversized batches fail with `import_budget_exceeded`. A call with more than 16
paths fails with a message that gives the number received; import the rest in
further calls. Preflight responses are limited to 256 KiB. Scientific processing and DOSY use the separate shared scientific operations.
CSV, TSV and XLSX files are tables: import them with `mode: "table_data_only"`.
Native import rejects them with that hint.

## Import CSV/TSV/XLSX data without creating a plot

Ask the agent to import an explicitly named UTF-8 CSV or TSV, or an XLSX workbook, inspect its columns,
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
An `.xlsx` file (or `format: "xlsx"`) imports each visible worksheet that has
cells as its own table, named after the worksheet, or after the file when the
workbook holds a single table, the same way the desktop imports it.
`sheets: ["Figure 3", …]` imports only the named worksheets; an unknown name
fails that file and lists the visible worksheets. Worksheets laid out like
Source Data are split into tables as described in
[Importing data](/guides/importing-data/#source-data-worksheets); each item
then reports its `range`. When a table is recognized wrongly, import it again
with `regions: [{"sheet": "Figure 3", "range": "J2:AC4411", "header_rows": 1}]`.
Each region becomes one table, named `name` or the worksheet and range;
`header_rows` counts from the first non-empty row of the range and defaults to
the rows above the first value, and `split_segments: false` keeps stacked
segments in one column. With `regions`, only the regions and any worksheets in
`sheets` are imported. Each worksheet is prepared on its own: one that holds no
table or exceeds a limit, or a region that holds no cells, fails as its own
item and the others still import. A table wider than 256 columns fails and
asks for a narrower region. One file reads at most 8,000,000 worksheet cells,
counted over each worksheet's used range. `header`, `schema` and `schema_path`
apply to CSV/TSV only. If the summary of a batch with XLSX files would exceed
128 KiB, the preflight fails and lists each workbook's worksheets, so you can
import one workbook at a time and name fewer worksheets.
Only named regular files are read. There is no directory discovery, adjacent
sidecar lookup, source writeback, implicit plotting or X/Y binding.

Preflight runs in the background and returns only when preparation finishes.
`import.items` reports each file, or each XLSX worksheet with its `sheet`, as
`prepared` or `failed`, with proposed native IDs, metadata and diagnostics. No
dataset exists yet. After reviewing the result, call
`plotium_execute {"plan_id":"RETURNED_PLAN_ID"}`. Execution uses those frozen
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
query existing tables but cannot import them. Limits appear in the `limits`
capability section and the import permission condition in the index.

## Derive a table without changing its source

Ask the agent: “Calculate `signal_x2 = y_left * 2`, exclude rows whose
`x_left` equals zero, sort X descending, then bind the result to this existing
curve.” Table processing creates one native table, not a plot. Rebinding uses
[the existing curve-binding operation](/reference/mcp/plots/#rebind-an-existing-curve-without-changing-another-plot).

1. Read `plotium_get_state`, then `plotium_table_columns` with the DatasetId
   and table revision. Copy the actual ColumnIds, `table_id`, `table_revision`
   and `snapshot_fingerprint`; names and `source_column_id` are not identities.
2. Discover the native plan schema with
   `plotium_capabilities_get {"section":"operation_constraints","key":"data.transform"}`.
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
   pass the same output identities to [`plot.create`](/reference/mcp/plots/#create-a-native-single-plot-from-an-imported-table)
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
128 KiB, leaving room for the request and plan envelope. Limits are discoverable with
`plotium_capabilities_get {"section":"limits","key":"table_transform"}`. Budget failure returns
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
Before a patch or refresh is committed, its values must remain valid for existing
curve bindings. For example, a column used as Y error cannot contain negative
amplitudes. A rejected update preserves the table, plots and Undo history.

Table metadata, row pages and Series sources include
`freshness: {status, refreshable, reason, sources, mcp_refreshable, mcp_refresh_reason}`.
`refreshable` reports native refresh capability; `mcp_refreshable` additionally checks
the connection's supported recipe subset, not the current permission mode. Dataset
resource metadata exposes these as `table_refreshable`, `table_mcp_refreshable` and
`table_mcp_refresh_reason`, alongside `table_freshness` and its reason. `source` means no saved derived recipe; `current` means recorded input
revisions still match; `stale` means a direct or upstream input changed;
`unavailable` means sources or a supported recipe are missing. The Data browser
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
128 KiB and complete responses at most 256 KiB. Discover schemas and limits in the
`operation_constraints` and `limits` (`table_update`) capability sections.

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

## Delete datasets

`dataset.delete` takes `{}` and DatasetIds in `ids`. Preflight refuses a dataset
that a plot, projection, derived dataset or analysis report still uses and names
the page and panel, dataset or report using it; delete those first. Delete a
derived dataset in its own request before deleting its sources. One Undo restores
every deleted dataset with its original ID and position.
