---
title: MCP page layout and deletion
description: Arrange pages, panels, text and images, and delete exact content through MCP.
---

## Author page layout, text and local images atomically

Plan `figure.edit` with `parameters: {canvas: EXACT_CANVAS_TARGET, edits: [...]}`;
omit top-level `ids`/`targets`. Discover the complete tagged edit schema with
`plotium_capabilities_get {"section":"operation_constraints","key":"figure.edit"}`.
One batch changes one canvas, in supplied edit order, with one Undo/Redo:

| `op` | Required fields and meaning |
| --- | --- |
| `page` | At least one of `size_mm: [width,height]` or `size_preset`, `margin_mm: [top,right,bottom,left]`, `gutter_mm`; lengths in mm. A journal `size_preset` sets the column width and keeps the height of a page with content; paper and slide presets set both dimensions. Presets also replace the page margins (0 mm journal, 15 mm paper, 10 mm slides) unless `margin_mm` is given; the result reports `size_preset` |
| `panel_frame` | `target: EXACT_PANEL_TARGET`, `frame_pt: [x,y,width,height]` in page pt |
| `content_frame` | `target: EXACT_CONTENT_TARGET`, complete `frame_pt: [x,y,width,height]` in parent-local pt; changes only this item |
| `image_replace` | `target: EXACT_RASTER_TARGET`, `path`; replaces only its asset reference with frozen local PNG/JPEG/SVG |
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
| `image_create` | `parent`, `name`, `frame_pt`, `path: "/absolute/local/image.png"` (PNG, JPEG, or SVG) |
| `annotation_create` | `target: EXACT_PLOT_TARGET`, `text`, `at: [x,y]` in the plot's data coordinates, `font_pt`; optional `kind: "text"`, `color: "#rrggbb"` |
| `annotation_create` (reference line) | `target: EXACT_PLOT_TARGET`, `kind: "reference_line"`, `orientation: vertical\|horizontal`, `value`; optional `label`, `font_pt`, `line: {color, width_pt, style}` |
| `annotation_create` (arrow) | `target: EXACT_PLOT_TARGET`, `kind: "arrow"`, tail `at: [x,y]`, head `to: [x,y]`; optional `head: none\|open\|filled\|double`, `line_width_pt`, `color: "#rrggbb"` |
| `annotation_set` | `target: EXACT_PLOT_TARGET`, `annotation_id`; at least one of `text`, `at`, `font_pt`, `color`, `offset_pt`, `placement` (text), `orientation`, `value`, `label`, `font_pt`, `line`, `placement` (reference line) or `at`, `to`, `head`, `line_width_pt`, `color` (arrow); optional `override_hand_edits` |
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
one x range covering every member's data (in its own coordinate unit) and the
top member's x major step (in the unit it displays) to each member, so members
must already share an x scale; every member draws the bottom
member's x direction without storing it. The plots share one x viewport, and
later x range, x scale, x display unit, x thousands-separator and x major step
edits on any
member apply to all of them in one Undo step; the write is rejected when any
member cannot take the value, and `applied` lists the member edits. A batch that
requests conflicting shared x settings is rejected without changes; equivalent
ranges expressed in compatible coordinate units are accepted.
`shared_x: false` returns the Panel to free layout and keeps
the frames. The result lists each operation in `panel_edits` with `layout`,
`gap_pt`, `members_top_to_bottom` and `merged_panels`. `plotium_document_inspect` reports every
Panel's `layout` (`free`, `vertical_stack`, `horizontal_stack`, `grid` or
`shared_x_stack`); a stack adds `shared_x_stack: {members_top_to_bottom,
gap_pt}`. `plotium_figure_diagnose` and plan previews use the stacked geometry.

Plot annotations are anchored in data coordinates, not page pt: the label is
centred above its `at` point and follows the plot's axis ranges, zoom and pan,
in the canvas and in every export. When that spot covers data, the legend, a
value label or another label, the text moves to the nearest clear spot within
about three text heights; an arrow pointing at it does not count.
`offset_pt: [dx, dy]` (pt, x right, y up) pins the text there instead, with
`[0, 0]` exactly at `at`, and `placement: "auto"` returns it to automatic
placement. `color` (`#rrggbb`) sets the text colour, black by default. A label
the user dragged on the canvas keeps its place through edits of `text` and
`font_pt`; a new `at`, `offset_pt` or `placement` on it is rejected unless
`override_hand_edits: true` says the user asked. Each text in
`annotations` reports `placement` (`auto`, `fixed` or `hand`) and the drawn
`offset_pt`, and `plotium_get_state` lists dragged labels under `hand_edits`
with key `annotation.label_position`. An anchor outside the visible range is
clipped with the plot. `annotation_id` is local to the plot and never reused
after deletion; read it from `annotation_edits` in the result or from the plot's
`annotations` list in `plotium_document_inspect`. `at` uses the plot's source
units reported in `coordinate_units`, not a display unit chosen for the tick
labels. `\n` in `text` starts a new line.
Annotation text permits 1024 UTF-8 bytes (nonblank), and a plot holds at
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
end of a horizontal one, or further along the line or on its other side when
that spot covers data, the legend or another label; inspection reports
`label_placement` and the drawn `label_position: {along, side}`, and
`placement: "auto"` returns a dragged label to automatic placement under the
same `override_hand_edits` rule. It uses the tick label size until `font_pt` sets an
explicit size, which then stays until changed. An empty `label` removes it. Reference lines and text
annotations share one id sequence and the 256-per-plot budget, and
`plotium_document_inspect` lists both under `annotations`, each with its `kind`.
Fields of the other kind are rejected rather than ignored; a reference line
takes its colour in `line.color`, not `color`.

An arrow or reference line the user moved on the canvas reports
`hand_moved: true` and is listed under `hand_edits` with key
`annotation.position`. Restyling it is allowed, but a new `at` or `to` on such
an arrow, or a new `value` or `orientation` on such a line, is rejected unless
`override_hand_edits: true` says the user asked; the moved position then
becomes the agent's again.

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
per PNG/JPEG image. SVG is normalized at preflight (text to outlines, external
references dropped) and stays vector in SVG, PDF, and bitmap output. Probing and
decoding reuse native image I/O; bytes are frozen
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
that effect. Replacement requires the existing `page_index` to be 0;
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
