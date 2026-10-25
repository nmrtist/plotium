---
title: Legends, lines and markers
description: Place and title legends, set line widths and marker styles, and write superscripts and subscripts.
---

## Legends and colour scales

Use **Legend & scales** in the Object inspector for any plot.

- With **Visibility** on **Auto**, Plotium shows a categorical legend when the
  plot has two or more named entries and shows a continuous colour scale for a
  heatmap. **Show** also permits a one-entry legend; **Hide** removes the legend
  or colour scale from that plot.
- **Position** on **Auto** places a legend inside the plot in whichever corner
  or edge midpoint covers the fewest data points and lines, preferring the
  roomier spot and then the upper right. When every such spot covers data,
  Plotium extends **Auto** axis ranges on the legend's side, by up to half their
  span, just far enough to clear one; manual ranges, zoomed views and
  aspect-locked plots are never changed.
- **Inside** keeps the legend in the upper right, and **Outside right** or
  **Outside bottom** reserve space beside the plot.
- **Layout** switches a categorical legend between a vertical list and a
  horizontal row; **Title** is available under the section's advanced controls.

Legends draw no border and no background, on screen and in every export
format; only their entries and title are drawn. Legend text size and colour are
document-wide settings in
[Figure typography](/guides/themes-and-styles/#figure-typography).

### Place a legend by hand

With **Select** active, click a legend or colour scale to open its
**Legend & scales** settings. Drag a categorical legend to place it freely; the
custom position survives resizing and is used by every export format. A clicked
legend or plot label is outlined; the arrow keys then move it by 0.5 pt
(`Shift`: 5 pt). Double-click a legend or colour scale to restore its automatic
placement.

## Line width

**Line** in the Object inspector sets the **Stroke width** of the selected
plot's line series, in points. New line series are 0.5 pt; drag or type any
value from 0.05 to 10 pt, or take one from **Presets** — *Fine* 0.50 pt,
*Medium* 0.75 pt, *Bold* 1.25 pt. The section appears only when something in the
selection is drawn as a line, and its header counts the series it is about to
change. Select several plots and it edits them together; select nothing and it
follows the page's active plot.

When the selected series do not all carry the same width the control shows an
em dash and the word *mixed* rather than presenting one of them as the setting;
setting the row is what makes them agree. A selected series that is drawn as
something else — a contour, say — is reported as skipped in the Operation
history, and the rest still take the value. A width that differs from the one
Plotium would choose for this data is marked with a dot, and the reset button
beside it re-derives that default.

## Markers

**Advanced** under **Line** holds each curve's marker colours, apart from its
line colour.

- **Marker fill** sets the inside of closed markers (circle, square, diamond,
  triangles, star, pentagon, hexagon) and **Marker edge color** their outline;
  both follow the curve colour until set.
- **Marker edge width** draws that outline, from 0 (no outline, the default) to
  5 pt.
- **Hollow marker** leaves closed markers unfilled and keeps their outline,
  0.5 pt when the edge width is 0; the fill colour returns when it is turned
  off.
- **Marker every N points** draws a marker on the first point and every N-th
  point after it, for dense data such as hundreds of cycles; the line still runs
  through every point.

Plus and cross markers stay in the curve colour. The legend swatch shows the
same marker. Charts whose colours carry meaning keep their marker colours.

## Superscripts and subscripts

Axis titles, legend entries and text use true superscripts and subscripts: type
`cm^{-2}` or `H_{2}O`. Characters such as `⁻²` and `₂` are drawn the same way.
EMF output falls back to those Unicode characters. NMR isotope numbers are set
as superscripts automatically.

## With an AI client

You can ask an AI client connected to Plotium to do this for you, for example:

> On the left panel only, move the legend to the bottom-left corner, keep it
> vertical and title it Samples. Show me a preview before you apply it.

> Find the curve named Current and change only that curve's line width to
> 1.5 pt.

> Give each curve in this plot its own colour and legend text: Control, 1 mM,
> 5 mM. Show me two or three palettes to choose from.

Plotium changes only the plot or curve you name, keeps values you set by hand
(such as a legend you dragged), and applies the change as one Undo step. When
you ask for options, it previews each and applies only the one you pick. See
[Access, review and undo](/ai/control-and-review/).
