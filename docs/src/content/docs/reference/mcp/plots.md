---
title: MCP plots and curves
description: Create native plots and add or rebind curves through MCP.
---

## Create a native single plot from an imported table

Ask the agent: “Import this CSV, use time in s and current/error in mA, and create
one editable line plot with symmetric Y error bars. Preview it, then change only
that curve's line width to 1.5 pt and preview again.” Supply an absolute input path
and explicit units. The agent can discover the complete import and creation
schemas through the `operation_constraints` section of `plotium_capabilities_get`.

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
identity. `kind` is `line`, `scatter`, `bar`, `box` or `violin`; `y_error_column_id` is a
ColumnId, `null` or omitted for no error bars. A source can instead name columns:
`{"dataset_id":"DATASET_UUID","x":"time","y":"signal","y_error":"sd"}`. `x`, `y`
and optional `y_error` are exact, case-sensitive column names or ColumnIds;
Plotium fills in the default field, table and current table revision. A missing
or repeated name fails with `invalid_parameters`, the path in
`error.details.field` and the table's columns in `error.details.candidates`.
The plan response lists each source resolved this way in `resolved_sources`.
The same source forms apply to `plot.series.append` and `plot.series.bind`. A `bar` curve draws filled bars from zero with optional
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
error and unit rules are the same as explicit curve binding below. Table-wide
X/Y defaults and other plots are untouched; no source table is copied.

Optional `intent: {purpose, claim}` records what the plot is meant to show.
`purpose` is `compare`, `trend`, `distribution`, `relationship`, `composition`,
`change` (ratio or fold difference) or `detail` (zoom or inset); `claim` is one
sentence of at most 500 characters. Ask the user for both before creating a
plot instead of guessing; if they leave it open, propose your reading and let
them confirm or edit it. The intent never changes the drawing. It is saved with
the project, edited later through `object.intent.purpose` (`unspecified` clears
it) and `object.intent.claim`, and shown under **Chart › Advanced** in the Object inspector.

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

The MCP plan returns `plan_id`, `project_context` and `basis_revision`.
`preflight` includes `required_authority`, `created_target_count`, exceptional
`targets` and `outputs`. `preflight.binding_plan.creation` contains the exact
`parent`, proposed complete `canvas`, `panel`, `plot` and `created` targets
(including all Series), plus `size_mm`, any `size_preset`, `panel_frame_pt`
(page coordinates), `frame_pt` (panel-local coordinates), `coordinate_units`
and `series_defaults` (kind and line width in pt). Keep these frozen identities
when checking an uncertain execution outcome.

`preflight.binding_plan.binding_count` counts curves, and `row_counts` sums
`total_rows` and `plotted_rows` across them. `binding_plan.bindings` includes only
curves with missing/non-finite data or missing errors, with each complete target
and full diagnostics including sample `affected_rows`. An empty list means no
such issues, not zero bindings. The response omits echoed requests, frozen-target
duplicates, source mappings and successful-target explanations; the stored plan
still retains the complete validated sources and candidate.

Planning creates nothing. Executing the single-use plan returns
`value.creation` with the same canvas/panel/plot and layout, and `value.bindings`
with **every** Series target and its `total_rows`, `plotted_rows`, `missing_xy`,
`non_finite_xy` and `missing_error` counts. Defaults are disclosed only in
preflight. `produced` lists newly created resources; `summary` counts target
outcomes, while `targets` retains only exceptional rows. Diagnostics and
verification remain available. `project_context` and `after_revision` identify
the committed result.

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
and layout controls retain plot/Series identities and source bindings. Known exact
targets may reuse committed synchronous results; refresh state after desktop edits
or Undo/Redo.

## Add a derived curve to an existing plot

Use `plot.series.append` when a newly calculated table should appear beside the
existing curves, without replacing a plot.

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
symmetric Y-error columns. The table's shared default bindings are unchanged.

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
`y_error_column_id` is a ColumnId, or null or omitted to remove the error binding.
Review `preflight.binding_plan.bindings`, including source mappings and row
diagnostics, then submit its one-use `plan_id` with `plotium_execute`.

A default table Series can represent several displayed curves. First use
`plot.series.materialize_default` with
`parameters: {"edits":[{"target":EXACT_SERIES_TARGET,
"retain_y_column_id":"EXISTING_Y_COLUMN_UUID",
"table_revision":"TABLE_REVISION_UUID"}]}`. The SeriesId of the retained Y
column is kept; the other existing curves receive new owner-local SeriesIds, returned in the plan and
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
`project.save` requires separate desktop approval; SVG/PDF/PNG exports require their own desktop approval. Use `plot.create` above to create a new plot.
