---
title: MCP styling
description: Change display properties, legends and themes through MCP.
---

## Change display without rebuilding the plot

Use an exact target from preflight `creation.series_defaults[].target` or execution
`value.bindings[].target`, or discover
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
  coupling follows the rules in
  [Position a plot's legend precisely](#position-a-plots-legend-precisely).

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
no edge width is set. `marker_every` thins markers to every n-th source point,
starting with the first; the line still passes through every point. A chart
that owns semantic colors rejects a single series
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
   When every match has the same reason, as in an unfiltered search, `value.reason`
   states it once instead of `value.reasons`.
   Resolve results to explicit stable IDs; `plotium_resources_inspect` reads
   descriptors using `ids` and `expected_revision`.
3. `plotium_properties_list` with `{}` lists supported IDs and value types. Discover
   curve addresses with `plotium_properties_inspect`:

   ```json
   {"ids":["CANVAS_UUID/OBJECT_ID"],"key":"series.line.stroke_width","expected_project_context":"PROJECT_CONTEXT","expected_revision":7}
   ```

   The response's `value.readings` contains one row per applicable curve. When
   every reading shares its schema or default, `value.schema` and
   `value.default_value` hold it once instead. Copy
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

A batch holds 1–200 writes and accepts the properties `plotium_properties_list` returns.
Planning refuses the whole
batch, and nothing is committed, when any write's target is missing, its
property does not apply to that target, its value is invalid, or the same key is
written twice to one target. A write whose target already holds the value is
reported as skipped while the others apply. `plotium_execute` commits every
write as one **Undo** step; `value.writes` lists each write that was skipped,
with its index, key, outcome and `skip_reason`.

Property-write responses are short. The preflight of `properties.set`,
`properties.set_many` and `properties.reset` returns `required_authority`,
`estimated_changes`, `summary` (`compatible`, `skipped`, `failed`) and, in
`targets`, only the targets that will not be written. The execute result adds
`summary` (`succeeded`, `skipped`, `failed`, and `by_reason` counts of
`skip_reason`); `targets` and a batch's `value.writes` list only what did not
succeed. When every write applies, both lists are empty.
`plotium_preview_plan` still renders the complete plan.

## Values the user set by hand

When the user changes a figure value in Plotium — in the property panels, by
scrolling over a control or by dragging a legend — Plotium records it as set by
hand. `plotium_get_state` lists these values in `hand_edits`, and
`properties.inspect` marks each one with `"hand_set": true`.

`properties.set`, `properties.set_many` and `properties.reset` leave hand-set
values alone: the target is reported as skipped with `skip_reason: "hand_set"`,
in preflight and in the result, and the other targets still apply. A legend's
placement and its dragged position count as one value. Pass
`"override_hand_edits": true` in `parameters` only when the user asked for that
change; the new value then counts as the user's.

Values you write without the override are not recorded, so a later theme or
write may replace them. Themes keep hand-set values; see the next section.

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
automation revision or Undo history. Discover these rules with `plotium_capabilities_get {"section":"guide_contract"}`.

## Apply a theme or saved style to a canvas

`figure.apply_theme` restyles whole canvases with one of Plotium's built-in
themes, such as `nature`, or with a canvas style saved in Plotium. Set
`theme_id` to a built-in id from
`builtin_themes` in
`plotium_capabilities_get {"section":"operation_constraints","key":"figure.apply_theme"}`, or to the saved style's name. Saved styles are not
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
panel-label and shape styles for new objects. Values the user set by hand are
kept, including figure typography; `diagnostics` adds a note saying how many. A theme sized for journal columns
(today `nature`) never resizes a canvas; for each canvas whose width matches
none of its column presets, the result's `diagnostics` adds a note listing the
preset widths (89 / 136 / 183 mm).
