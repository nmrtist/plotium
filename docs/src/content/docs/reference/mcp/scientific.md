---
title: MCP scientific operations
description: Run processing, diffusion fitting and DOSY through MCP, and plot the results.
---

These operations are the ones the desktop runs for
[processing](/guides/processing/) and [DOSY](/guides/dosy/). The command line
runs them too, with [`plotium-cli tool run`](/reference/cli/#single-step-scientific-operations).

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
