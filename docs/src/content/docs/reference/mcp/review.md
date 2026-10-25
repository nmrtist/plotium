---
title: MCP preview, diagnostics and recovery
description: Render previews, diagnose figure geometry and recover from errors.
---

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
opening a file or rendering SVG text. A curve with more points than the preview
can show is drawn as a min/max envelope of four point pairs per pixel column, at
least 2,048 pairs across the plot (markers: one per pixel), and `downsampled`
lists each such curve as `{target, source_points, drawn_points}`; it is omitted
when nothing was thinned. A zoomed-in curve keeps every visible point.
Exports always draw every point. The MCP client must deliver image content
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
Limits are also available with
`plotium_capabilities_get {"section":"limits","key":"image_preview"}`.

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
| `properties.set` / `properties.reset` / `properties.set_many` | Document and object display properties, including their native multi-target batch; not canvas, app preference or dataset processing properties, chart type or stack changes |
| `figure.edit` local edits | `content_frame`, `image_replace`, `text_set`, `annotation_create`/`annotation_set`/`annotation_delete` (text and reference lines), including native batches |
| `figure.edit` deletion-only batch | `content_delete`, originally empty `panel_delete`, explicit-column `series_delete`; all existing refusal rules remain |
| Other operations, including save/export | `unsupported_operation`; their normal execution contract is unchanged |

To let the user choose a style, such as a palette, line widths, legend
placement or label layout, plan two or three alternatives, preview each and show
the images side by side. Execute only the plan the user picks. The others are
never committed: they then fail with `stale_revision`, but count toward the 32
pending plans until submitted or the connection resets.

Affected canvases must use ordinary line/scatter plots, native text, shapes and
raster content; previews do not launch scientific computations. A cross-canvas
property plan applies its complete batch to the disposable native context; request
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
  "checks": ["bounds","clipping","text_overflow","content_overlap","legend_occlusion","data_shape","layout_aesthetics","intent"],
  "detail": "full",
  "limit": 100
}
```

Copy complete content or Series targets into optional `targets` to narrow the scan.
A Series target also includes its owning plot frame and guides as context.
Omission scans the canvas; it never uses GUI selection. `content_overlap` additionally
requires explicit whole-content `overlap_targets` (at least two). It compares visible,
page-clipped frames, not opaque pixels. `view` accepts `issues` (default) for
findings, `layout` for measured geometry rows instead, or `both`. `detail` is
`compact` (default) or `full`. Compact issue rows keep `target`, `component`,
`code`, `severity`, `message`, `measurement_basis` and any `suggestions`; their
`evidence` keeps only `other_target`, `other_component`, `point_range` and
`interpretation`, and coverage rows of checks that completed are left out.
`full` returns all evidence geometry and every coverage row. Coverage rows that
report a limit are always returned.

`output.size_mm` must be positive and preserve the canvas aspect ratio (0.001 mm
height tolerance). Like native export, it uniformly scales the existing page without
reflow. Every initial query recalculates geometry. Results use `output_page_pt`:
72 points per inch, top-left origin, X right, Y down. `output` includes native size
and scale; text, stroke widths and bounds scale together. Panel-local frames are
converted through the native page-frame service. Plot data uses the actual plot
clip; a panel border is **not** a general content clip. Native raster frame and
`clip_children` clipping are reported separately.

`ToolResult.value` contains `project_context`, `basis_revision`, `canvas`, `summary`,
`output`, `items`, `coverage`, `truncated` and `continuation`. In the default
compact detail, `output` has `size_mm` and `size_pt`, and `coverage` reports its
status, scan counts and unavailable or unsupported items. `detail: "full"` adds the
page coordinate description to `output`, the requested, supported and unsupported
check lists to `coverage`, and `budget`, `preview_request` and `read_policy`. `summary`
counts the issues on this page (`issues`, `by_code`, `by_severity`); add the
counts of every page of a continued scan. Each issue has a full native `target`,
an owner-relative `component`, `code`, `severity` and explanation; with
`detail: "full"` its evidence also carries bounds, clip and intersection geometry. Pass `target` to `plotium_document_inspect`
with `view: "self"`; names do not identify objects. `guide.legend` belongs to its
plot, not a new resource. A curve conflict points to the existing Series and its
owning plot's legend through `evidence.other_target` / `other_component`.

- `out_of_canvas`, `partially_clipped`, `fully_clipped` and
  `curve_primitives_clipped` / `shape_clipped` are informational facts, including intentional manual
  range clipping. Curve evidence applies to the reported native run's point range.
- `text_container_overflow` is a warning based on native width/line-spacing
  **estimates**, not exact glyph ink. Text does not automatically wrap or shrink.
- `content_frame_overlap` is informational; deliberate overlap is allowed.
- `axis_ticks_sparse` is a warning: at the output size an axis with shown tick
  labels draws fewer than two ticks, so its scale cannot be read. Enlarge the
  panel or lower `object.figure.typography.tick_pt`.
- `right_axis_untitled` is a warning: curves use the right y axis but it shows
  no title. Set `object.axes.y2_label` (or turn `object.axes.y2_show_label`
  back on).
- `legend_occlusion_potential` is a warning, not a claim that data is invisible.
  The scan visits projected line segments and marker shapes after effective
  clipping, retaining dash phase across batches. It does not use a whole-curve
  bounding box or sample points. Stroke envelopes and the legend layout rectangle
  are conservative; rounded corners, translucency and exact antialiasing can differ.

The `data_shape`, `layout_aesthetics` and `intent` checks report issues with severity
`hint`: the figure could show its data more clearly, nothing is wrong. They are
included when `checks` is omitted. Hints never edit the document. Each carries
`suggestions`, a list of `{action, property, target, reason}`; `property` names a
property to set on `target`, or both are null. Lengths are measured at the
requested `output.size_mm`; the thresholds are provisional. A query narrowed to
Series reports Series hints for those Series only.

| Code | Fires when |
| --- | --- |
| `small_values_invisible` | A bar's value is under 1 % of the value-axis span, or it prints under 0.5 mm, beside bars at least 5 mm long |
| `bars_too_narrow` | A bar prints narrower than 0.6 mm |
| `few_samples_hidden` | Every group of a box or violin Series, or of a bar Series with repeated rows per x, has fewer than 10 values, or a bar Series of one row per x has `series.replicates` of 1–9; and the plot has no scatter Series |
| `too_many_categories` | The category axis draws its labels but drops some, or neighbouring labels collide |
| `log_scale_candidate` | A plot without bars, boxes or violins has only positive line or scatter values on a linear value axis, spanning three or more decades |
| `bar_baseline_not_zero` | The value-axis range leaves out the bars' baseline |
| `labels_overlap` | Two text boxes on the canvas overlap by more than 0.2 mm in both directions; evidence names both and a direction to move the second. Entries of one legend and tick labels of one axis are not compared with each other |
| `text_over_data` | An annotation or value label overlaps a bar, box, violin, line or marker |
| `intent_mismatch` | The chart form does not suit the plot's `object.intent.purpose` (see below) |
| `panels_misaligned` | Two frames in one column (left edges within 1 mm) or row (bottom edges within 1 mm) have plot areas whose left or bottom edges differ by more than 0.5 mm, or the gaps between three or more frames in a row or column differ by more than 0.5 mm |

Hints beyond 50 of one kind, and checks that hit a budget, add an `unavailable`
coverage row.

The `intent` check weighs each plot built from explicit columns against the
purpose its author stated; `detail` is never reported. A plot without a purpose
gets a `not_applicable` coverage row, `no_purpose_stated`: ask the user what it
should show. Table chart types get `chart_type_not_covered`. `intent_mismatch`
evidence keeps the reason in `interpretation` and, with `detail: "full"`, adds
the purpose, the claim and the chart features it read. Its suggestions name
chart forms Plotium can draw; relay them and let the user choose.

| Purpose | Reported when |
| --- | --- |
| `compare` | Lines join categories |
| `trend` | The category axis holds categories, or the plot draws bars |
| `distribution` | On a category axis, bars or lines without a box, violin or scatter Series |
| `relationship` | The category axis holds categories, or the plot draws bars, boxes or violins |
| `composition` | The plot draws lines or points but no bars |
| `change` | Two or more bar Series sit side by side, or positive bar values span tenfold or more; or, without bars, boxes or violins, positive left-axis line or scatter values span tenfold or more on a linear axis |

Native guide layout includes estimated text widths. Font availability is checked
using native export font discovery; glyph coverage and cross-backend ink bounds
remain unverified. Raster viewport geometry uses header dimensions, integer crop,
rotation and fit; successful decoding and alpha-pixel coverage require the preview.
Non-line/scatter internal geometry, error-bar clipping/occlusion, derived scientific
annotations and opaque-pixel occlusion are not covered. Native rectangle, ellipse, line and arrow extrema use the actual page clip. Their frame/guide
observations remain available; coverage states the limitation. Beyond the hints
above, this tool does not judge aesthetics, scientific correctness or statistical
significance.

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

Call `plotium_render_preview` with the canvas, context and `basis_revision` to
obtain the native PNG, then inspect it. Convert a returned
X coordinate to pixels using `x * width_px / output.size_pt[0]`; use height
for Y. Check its `render_revision` against `basis_revision`. Editor zoom does not
change these units. Direct core `figure.diagnose` returns one bounded batch without
a retained cursor; the MCP query provides continuation. No automatic fix is applied.

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

## Error codes

Errors include stable codes: `permission_denied`, `expired_plan`,
`stale_revision`, `busy`, `invalid_parameters`, `unsupported_operation`, `unsupported_property`,
`project_changed`, `unavailable`, `connection_lost`, `plan_limit`, `request_cancelled`, `connection_stopped`, `response_too_large` and `execution_failed`.
Invalid capability sections or keys point back to static discovery, with valid
candidates in `error.details`. Invalid plan parameters point to that operation's
`operation_constraints` instead of asking for project state. Input errors include
`error.details.field` at the decoded request boundary, such as `plan_id`,
`parameters`, or `parameters.writes[0].key`. Stale revisions and changed projects
still require reading state and replanning. This guidance never permits replay.
Nested source and target errors report their full path, such as
`parameters.series[0].source.y`.

Errors carry `error.recovery`; asynchronous save, export and task receipts carry
`recovery`. `retryable: false` means do not repeat the failed call: follow the
reported next action. A `busy` execute can already have consumed its plan.
For stale revisions, read state and review a fresh plan. An expired plan may
already have run; inspect its outcome before deciding on a new operation.
The AI connections (MCP) settings page shows recent operations and errors without
opening itself in response to background activity.
