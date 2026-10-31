---
title: Plots and chart types
description: Turn a table into a plot, choose its columns and chart type, and combine several datasets in one plot.
---

Every imported dataset gets its own page with a plot. For spectra and other
instrument data, the plot is ready as soon as the import finishes. A plain table
needs one more step: you tell Plotium which columns to draw.

## Plot a table

When you [import a CSV, TSV, or XLSX table](/guides/importing-data/#import-a-table), Plotium opens it in the sheet and
creates a page with an empty line plot for it. To draw the data:

1. Select the plot on the page.
2. In the Object inspector, open **Data → Curve columns**.
3. Choose the **X** column and the **Y** column. Only numeric columns are
   listed. Plotium starts with the first numeric column as X and the second as
   Y.
4. Optionally choose a **Y error** column to draw error bars (see
   [Error bars](#error-bars)).
5. Press **Apply curve binding**.

Plotium then reports how many rows were plotted and how many were left out
because a value was missing or not finite.

To draw more columns in the same plot, open **Data → Add curve**, choose the
source table, its **X**, **Y**, and optional **Y error** columns, pick **Line**
or **Scatter**, and press **Append curve**. The source can be another table in
the project, so a derived table can join the plot it came from. A curve whose
units cannot convert to the plot's existing units is refused.

To change the columns of an existing curve later, open **Curve columns** again,
choose new columns, and press **Apply curve binding**. Two plots can draw the
same table with different columns; changing one plot does not affect the other.

Tables that Plotium creates itself, such as region integrals or fit results,
already know their x column and value columns and are plotted right away. If
such a plot draws several default curves, **Curve columns** first offers
**Expand default curves**: choose which curve keeps its identity, then edit each
curve's columns separately.

## Chart types

Select a plot made from a table and pick a chart in the **Chart type** gallery
of the Object inspector. **Chart** on the Ribbon's **Figure** tab jumps there
directly: it selects the table's plot and opens the inspector.

Picking a chart is picking the question it answers: to follow values along an
axis, use **Line**; to compare quantities across categories, **Bar** or
**Grouped bars**; to show how one column's values are distributed,
**Histogram**; to compare distributions across several columns at a glance,
**Box** or **Violin**; to show parts of a whole, **Pie**; and to see an
entire matrix of values at once, **Heatmap** or **Surface 3D**.

- **Line** — one point series per column against the x axis, with fitted
  curves overlaid when present.
- **Bar** — one chosen column as filled bars on the numeric x axis.
- **Grouped bars** — every column side by side, one group per row; check
  **Stacked** to pile the columns instead (positive values stack upward,
  negative downward).
- **Histogram** — the value distribution of one chosen column. Bins follow the
  Freedman–Diaconis rule; uncheck **Auto bins** to set a fixed **Bins** count
  between 1 and 512. The count you inherit is the one the automatic rule just
  produced, so unchecking the box leaves the histogram as it was.
- **Box** — one box per column: median, quartile box, 1.5 × IQR whiskers, and
  individual outlier points.
- **Violin** — one kernel-density silhouette per column with an inner quartile
  bar (columns need at least two distinct values; degenerate columns fall back
  to raw points).
- **Heatmap** — the whole table as a colormapped matrix with rows reading
  top-to-bottom; pick the colormap in the inspector. An x column that only
  rises or only falls gives a numeric axis, and so do column names that differ
  only in such a number, like `300 K`, `t=10 min`, or `scan 12`; uneven steps
  keep their true spacing. Anything else keeps one slot per row or column. Turn on **Transpose** to put the x column across and the columns
  up, as in an in-situ map of 2θ against time. To set which values the colours
  span, see [Heatmap colour range](/guides/heatmap-range/).
- **Pie** — one chosen column as wedges, one per row, labeled with
  percentages. Negative and missing rows are excluded.
- **Surface 3D** — the table as a height field in a fixed orthographic view;
  adjust the azimuth/elevation angles and the colormap in the inspector.

Grouped bars, Box, Violin, Heatmap, and Surface 3D use all columns at once;
Bar, Histogram, and Pie read the single column selected next to the gallery.
Every chart renders identically on screen and in SVG/PDF/EMF export.

**Purpose** and **Claim**, under **Chart › Advanced**, record what the plot
should show (for example a trend or a comparison between groups) and the one
statement it supports. They never change the drawing. An AI client that checks
the figure compares the chart with them and suggests better-fitting forms
without switching on its own.

## Error bars

Error bars come from a column that holds the uncertainty of another column.
Choose it as **Y error** in **Curve columns** or **Add curve**. Plotium checks
that the value column and its uncertainty have compatible units. Line, Bar and
unstacked Grouped-bar charts draw them as vertical error bars and widen the
y-axis range so the bars stay fully visible. Under **Line** in the Object
inspector, **Error bars show** records whether the bars are SD, SEM or a 95% CI,
and **Replicates (n)** how many measurements each value summarises; an AI client
asks you for both.

Tables produced by Plotium's analysis tools already carry their
uncertainties, and an imported table keeps whichever its file describes — a
symmetric ±σ, separate lower and upper errors, or a confidence interval with its
level. To mark an uncertainty column in a CSV or TSV you write yourself, see
[Error-bar columns](/guides/importing-data/#error-bar-columns).

## Several datasets in one plot

A single plot can display several 1D datasets — superimposed, or stacked with
adjustable vertical spacing and 3D shear. 2D datasets combine as a color
overlay. Use **Add series…** under **Data** in the Object inspector to add
another dataset to the selected plot.

To show related plots one above the other on a common x axis instead, use a
[Shared X Stack](/guides/multi-panel-figures/#stack-plots-on-a-shared-x-axis).

## Waterfall plots of a series

A table with one spectrum per column, such as an in-situ scan series, stacks as
a waterfall. With the chart type on **Line**, set **Mode** to **Offset**
in the **Stack** section of the Object Inspector, then adjust **Vertical
spacing** and **3D shear**. The same options apply to a stack of several
datasets.

- **Gradient** colours the traces along the sequence through a colormap.
- **Fill under traces**, under **Advanced**, hides the parts of each trace that
  sit behind the one in front.
- **Side scale**, under **Advanced**, labels the trace baselines on the right,
  in place of the legend.

The sequence comes from the trace names when they differ only in a number, like
`300 K` or `t=10 min`, and these also title the scale. Other names use the trace
order. Switch **Type** to **Heatmap** to show the same table as a map.

## Align traces

To align line series along x, select the plot and choose **Align Traces…** in
the **Overlay** group on the **Analyze** tab. The command is available when the
plot contains at least two visible line series with the same x-axis unit. You
can open the same dialog with **Align traces…** under **Data** in the Object
inspector.

Choose a reference series, then align by **Trace start** (the first finite
plotted sample) or by a peak within an x window. For peak alignment,
**Positive** finds upward peaks, **Negative** finds downward peaks, and
**Magnitude** compares their prominence. Review the **Anchor**, **Delta**, and
**Result** columns before selecting **Apply**. Hidden series, incompatible
series, and series without a usable anchor are listed as **Skipped** and remain
unchanged.

Alignment preserves the plot's current zoom. To adjust one line manually, edit
its **X shift** under **Data**. Automatic and manual shifts are undoable plot
settings and do not affect source data or data exports.

## With an AI client

You can ask an AI client connected to Plotium to do this for you, for example:

> Import the CSV at /Users/me/data/experiment.csv. Use time as X and current as
> Y, with units s and mA. Create an editable line plot and show me a preview.

> Delete this duplicate note and this exact curve, but keep the other curve with
> the same name. Show me what will be deleted first.

Before creating a plot, the AI checks with you what it should show and records
the answer as the plot's **Purpose** and **Claim**. Units are taken only from
what you state, and only the items you name are deleted; source data stays, and
each change is one Undo step. Check the preview before you agree. See
[Access, review and undo](/ai/control-and-review/).

## Next steps

- [Axes and ticks](/guides/axes/): ranges, logarithmic and broken axes, a
  second y axis.
- [Legends, lines and markers](/guides/legends-and-lines/).
- [Typography, themes and styles](/guides/themes-and-styles/): fonts, text
  sizes, and journal themes.
