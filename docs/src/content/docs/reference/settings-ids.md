---
title: Setting IDs
description: Stable identifiers for settings used by the Automation window, workflows, the command line and AI clients.
---

Every setting in the Object inspector, the processing steps and Preferences has
a stable ID such as `series.contour.count`. You need these IDs only when you
name a setting outside the panels: in the Automation window's property tools, in
a workflow file, on [the command line](/reference/cli/) or from your own
[MCP client](/reference/mcp/). An AI client finds them by itself.

The complete list, with value types and ranges, comes from
`plotium-cli tool properties` or the MCP tool `plotium_properties_list`. The
tables below cover the settings people use most. The command palette also
matches a setting by its ID.

## Reading and writing settings

The Automation window and workflows reach settings through three tools:

| Tool | What it does |
| --- | --- |
| **Inspect a property** | Reads the current value, the default, and the range the setting accepts |
| **Set a property** | Writes a value |
| **Reset a property** | Re-derives the value from the current data, as the panel's reset button does |

Name the setting by its ID in the tool's parameters:
`{"key": "series.contour.count", "value": 12}` for **Set a property**, and
`{"key": "series.contour.count"}` for the other two. Which resource to target
follows from the setting: a contour or line setting lives on a plot object, an
apodization setting on a dataset, figure typography on the document (listed as
**Plotium document**), a page size on a canvas, and a preference on the
application (listed as **Plotium application**).

## Figures

These IDs address the plot and figure settings described in the
[Figures](/guides/plots-and-charts/) guides. Plot settings (`object.*`) apply to
one plot, series settings (`series.*`) to one curve, and typography settings
(`document.figure.typography.*`) to every plot in the document.

### Axes

| Setting | ID |
| --- | --- |
| X / Y title | `object.axes.x_label`, `object.axes.y_label` |
| X / Y range | `object.axes.x_range`, `object.axes.y_range` |
| X / Y display unit | `object.axes.x_display_unit`, `object.axes.y_display_unit` |
| X / Y tick labels shown | `object.axes.x_show_tick_labels`, `object.axes.y_show_tick_labels` |
| X / Y title shown | `object.axes.x_show_label`, `object.axes.y_show_label` |
| X / Y scale | `object.axes.x_scale`, `object.axes.y_scale` |
| Equal x/y scale (1:1) | `object.axes.equal_scale` |
| Frame | `object.axes.frame` |
| X / Y major tick step | `object.axes.x_major_step`, `object.axes.y_major_step` |
| X / Y thousands separator | `object.axes.x_thousands_separator`, `object.axes.y_thousands_separator` |
| X / Y break | `object.axes.x_break`, `object.axes.y_break` |
| X / Y lower share | `object.axes.x_break_lower_fraction`, `object.axes.y_break_lower_fraction` |
| Right Y title, range, display unit | `object.axes.y2_label`, `object.axes.y2_range`, `object.axes.y2_display_unit` |
| Right Y tick labels / title shown | `object.axes.y2_show_tick_labels`, `object.axes.y2_show_label` |
| Right Y scale, major tick step | `object.axes.y2_scale`, `object.axes.y2_major_step` |
| Curve on the right y axis | `series.y_axis` |

An axis break is written as `{"mode":"manual","min":…,"max":…,"unit":…}`, or
`{"mode":"auto"}` for no break. Figure diagnosis reports data points that a
break hides.

### Legends, lines and markers

| Setting | ID |
| --- | --- |
| Legend visibility, position, layout, title | `object.guides.visibility`, `object.guides.position`, `object.guides.layout`, `object.guides.title` |
| Legend placement | `object.guides.placement` |
| Legend text | `series.label` |
| Curve visible | `series.visible` |
| Line colour, stroke width, line style | `series.line.color`, `series.line.stroke_width`, `series.line.style` |
| Marker and marker size | `series.line.marker`, `series.line.marker_size` |
| Marker fill, edge colour, edge width | `series.line.marker_fill`, `series.line.marker_edge_color`, `series.line.marker_edge_width` |
| Hollow marker, marker every N points | `series.line.marker_hollow`, `series.line.marker_every` |
| Bar width, edge colour, edge width | `series.bar.width`, `series.bar.edge_color`, `series.bar.edge_width` |
| Error bars show, replicates | `series.error_bars` (`unspecified`, `sd`, `sem`, `ci95`), `series.replicates` |

### Typography

| Setting | ID |
| --- | --- |
| Font, font weight | `document.figure.typography.typeface`, `document.figure.typography.font_weight` |
| Tick-label, axis-title, figure-title and legend sizes | `document.figure.typography.tick_pt`, `document.figure.typography.label_pt`, `document.figure.typography.title_pt`, `document.figure.typography.legend_pt` |
| Legend text colour | `document.figure.typography.legend_color` |
| Axis line width, tick length, tick direction | `document.figure.typography.axis_line_pt`, `document.figure.typography.tick_length_pt`, `document.figure.typography.tick_direction` |
| Minor ticks | `document.figure.typography.minor_ticks` |
| Text sizes for one plot | `object.figure.typography.tick_pt`, `object.figure.typography.label_pt`, `object.figure.typography.legend_pt` |

### Charts and colour maps

| Setting | ID |
| --- | --- |
| Chart type | `object.chart.type_id` |
| Auto bins, bins | `object.chart.bins.auto`, `object.chart.bins.count` |
| Stacked bars | `object.chart.stacked` |
| Colormap | `object.chart.colormap` |
| Transpose heatmap | `object.chart.transpose` |
| Purpose, claim | `object.intent.purpose`, `object.intent.claim` |
| Heatmap colour range, range centre | `series.heatmap.range_span`, `series.heatmap.range_center` |

### Contour levels

| Setting in the Object inspector | id | Accepts |
| --- | --- | --- |
| **Lowest level** | `series.contour.base.magnitude` | a number above 0; what it measures depends on the anchor |
| **Anchor** | `series.contour.base.policy` | `absolute`, `noise_floor`, `background_scale`, `fraction_of_range` |
| **Levels** | `series.contour.count` | 1 to 256 |
| **Level ratio** | `series.contour.ratio` | above 1, at most 10 |
| **Negative contours** | `series.contour.negative.enabled` | `true` or `false` |
| **Positive colour** | `series.contour.positive_color` | `"#rrggbb"` |
| **Negative colour** | `series.contour.negative_color` | `"#rrggbb"` |
| **Line width** | `series.contour.line_width` | 0.05 to 10 pt; default 0.5 |

[Contour levels](/guides/contour-levels/) explains what each contour setting
does and which data each anchor needs.

## Processing settings

| Setting on an apodization step | id | Accepts |
| --- | --- | --- |
| **Window** | `dataset.processing.apodization.kind` | `none`, `cosine_bell`, `exponential`, `gaussian` |
| **LB** | `dataset.processing.apodization.lb_hz` | −10000 to 10000 |
| **GB** | `dataset.processing.apodization.gb_hz` | greater than 0, at most 10000 |

[Processing pipeline](/guides/processing/) explains each row.

## Application preferences

Preferences take the same three tools on **Plotium application**. For
`settings.appearance.accent.color`, **Set a property** accepts `"#rrggbb"` and
pins the canvas accent to that colour; **Reset a property** clears it, and the
accent follows the theme again.

## Batch results

### What a checked target expands to

The resource you check is rarely the thing that is written — the setting decides
what inside it is. A plot object expands to its series, a dataset to its
processing steps, and the document to itself. Preflight and the result list show
one row per component, so a plot holding three series produces three rows,
naming the object and the series inside it.

A component the setting does not reach is listed as **Skipped** with the reason,
and the rest still apply: a heatmap drawn under a contour, a zero-fill step when
the setting belongs to apodization, or a **GB** asked of a step whose window is
*Exponential*. A resource with nothing to address at all — a text box, a dataset
with no pipeline — is skipped the same way.

Result rows and the run record name the component beside the resource:

```json
{
  "resource": { "id": "…", "kind": "plotium.dataset" },
  "component": { "kind": "processing_step", "id": 3 }
}
```

Series components read `{"kind": "series", "id": 2}`. Both ids are local to the
resource they travel with, so a step id means nothing next to a different
dataset.

### Why a target was skipped

Every skipped row carries a sentence written to be read. Rows the write itself
passed over also carry `skip_reason`, a stable token a workflow can branch on
without matching on wording:

| `skip_reason` | Means |
| --- | --- |
| `already_at_value` | The target already held that value, so nothing was written |
| `not_applicable` | The setting does not apply to this target |
| `target_missing` | The address no longer names anything in the document |

Rows already ruled out at preflight carry the sentence alone. They are never
same-value no-ops, so the absence of the token is itself the distinction.

A **Set a property** whose every target already stores the requested value and
override state writes nothing: each target is reported as skipped, the document revision does not move,
and nothing is added to the undo stack. Setting an inherited value as an explicit
override still records that author choice.

A value outside the range is refused at **Preflight**, before you confirm, and
names both the value you gave and the limit that rejected it. Past that point
either every listed component takes the value or none does, and what is written
collapses into one **Undo automation** step.

**Inspect a property** returns its readings to whoever ran it: run it as a
workflow step and the values land in the run record, which is also what
[the command line](/reference/cli/) writes to its manifest file. In the
Automation window the numbers appear under **Result value (JSON)**, below the
per-component rows.

## Readings and schemas

Each reading names its `target` and carries the current `value`, a
`default_value` where the setting has one, `modified` — whether the value
differs from that default — an `availability`, and the `schema` that bounds it.

A setting the current state does not allow you to write is still read back:
`availability` is `"disabled"` and `disabled_reason` names what has to change
first, such as switching the phase mode to *Manual* before φ0 can be set.

Schemas are tagged by `type`: `bool`, `text`, `int`, `stepped_int`, `float`,
`enum`, `color`, `unit_code`, or `axis_range`.

- `int` and `stepped_int` carry `min` and `max`, plus a `unit` where the setting
  has one. `stepped_int` also carries the `step` its values must land on — a
  Savitzky-Golay window, for instance, runs from 3 to 201 in steps of 2.
- `float` carries `min`, `max` and `exclusive_min`, and where a setting refuses
  particular values, `excluded` (one value) or `excluded_magnitude` (everything
  at or below that magnitude). Its `display` — `linear`, `degrees` or `log10` —
  is how the panel shows the number; the `unit` and `log` fields beside it
  restate the same thing.
- `enum` lists its variants, each with a stable id and a label.

Bounds and values are always in the setting's own units, whatever `display`
says: a phase whose `display` is `degrees` is radians on the wire.
