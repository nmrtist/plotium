---
title: Axes and ticks
description: Set axis titles and ranges, logarithmic and broken axes, a second y axis, and tick marks.
---

Plotium draws axes for print without any setup: clean bottom-and-left axes with
outward ticks, tick precision that follows the displayed range, and tick
density that thins as a panel narrows or shortens (a short panel still keeps
two y ticks wherever they fit). Select one plot and use **Axes** in the Object
inspector to change what you need. Charts without visible axes offer no axis
settings.

## Titles and ranges

Override the X and Y titles or numeric ranges in **Axes**. Leave a title blank,
or keep a range on **Auto**, to use the value derived from the data. A manual
range becomes that axis's full range: zooming and panning stay inside it, and a
double-click on the plot returns to it. Categorical axes have no range
controls.

Axis titles use true superscripts and subscripts; see
[Superscripts and subscripts](/guides/legends-and-lines/#superscripts-and-subscripts).

To hide text on one axis, use the **X text** and **Y text** rows: they toggle
**Tick labels** and **Title** individually, and **Automatic** returns that axis
to showing both. To hide inner axis text across a grid of panels at once, see
[Simplify inner axes](/guides/multi-panel-figures/#simplify-inner-axes).

## Equal x/y scale

**Equal x/y scale (1:1)** uses the same data units per screen unit on both
axes, for example for a Nyquist plot, an AFM image or a 2D NMR spectrum. The
plot area shrinks to that shape inside the frame; the axis ranges are kept. It
needs linear, non-categorical axes and no right y axis, and a plot in a
shared-x stack cannot turn it on. AFM images start with it on; for a true 2D NMR
spectrum Plotium sets its initial value when the spectrum is imported, and you
can change it afterward without changing the import preference.

## Frame

**Frame** in the same **Axes** section chooses *Left and bottom* or *All four
sides*. The four-sided frame adds top and right axis lines with the same stroke
as the other axes; ticks and labels stay on the left and bottom axes. Most plots
start with left and bottom axes; 2D maps such as contour plots and heatmaps
start with all four sides.

## Logarithmic axes

**X scale** and **Y scale**, under the more options of the **Axes** section,
switch a line, scatter or stick plot between *Linear* and *Logarithmic*. A
logarithmic axis labels powers of ten (10¹, 10², …), thins them when the axis
is short, and uses ordinary ticks when fewer than two powers of ten are visible;
minor ticks, when enabled, mark 2–9 × each decade. Its automatic range fits only
the positive data. Zero and negative values have no position: lines break
there, and error bars and sticks that reach below the axis end at its edge. A
manual range on a logarithmic axis must start above zero. Zooming and panning
move by decades. Logarithmic axes are not offered for categorical axes, 2D maps,
bars and distributions, or NMR spectra, whose integral, phase and cursor tools
measure in linear ppm.

## Second y axis

**Y axis** under the advanced controls of **Line** puts a line or scatter curve
on a second y axis at the right of the plot, for a quantity with a different
unit or magnitude. The right axis appears as soon as one curve uses it and goes
away when every curve is back on the left. It fits its own automatic range to
its curves and draws its own ticks, tick labels and title; the title reads
bottom to top like the left one and starts as the first right curve's column
name and unit.

The **Right Y axis** section of the Object inspector, shown only while the plot
has a right axis, sets its title, range, logarithmic scale, tick labels, display
unit and major tick step. Scrolling, dragging and box zoom move both y axes by
the same fraction of their range, a double-click returns both to their full
range, and the **Inspect** cursor reads the pointer on x and on both y axes. One
legend lists every curve. Bars stay on the left axis, and reference lines,
annotations and arrows are placed in left-axis coordinates. In a table plot each
y axis keeps the unit of its first curve; a curve whose unit cannot convert to
it cannot join that axis.

## Axis breaks

**X break** and **Y break**, under the more options of the **Axes** section,
break a linear numeric axis: the values between the two endpoints are left out,
and the axis line opens with a pair of slanted marks. Untick **None** to start
from the middle third of the current range, then edit the endpoints; both must
lie inside the axis range. By default the two pieces keep the same scale;
**X lower share** or **Y lower share** sets how much of the axis length the
lower values get instead.

Each piece gets its own ticks, and a tick right next to the break is left out so
the labels do not collide. Lines that cross the break leave at one side of the
gap and continue on the other; a bar that spans a y break is drawn cut.
Logarithmic, categorical and equal-scale axes, 2D maps and NMR spectra cannot be
broken, nor can the y axis of a plot with a right y axis. In a shared x stack,
an x break applies to every plot.

## Tick positions and number format

To choose where major ticks fall, set **X major tick step** or **Y major tick
step** under **Advanced** in the **Axes** section, or **Right Y major tick
step** in the **Right Y axis** section. Ticks then sit on the multiples of the
step in the axis's display unit — a step of 20 gives 80, 100, 120 — and minor
ticks divide those intervals. 0 returns to automatic ticks. The step does not
apply to logarithmic, category or broken axes, and a step that would draw fewer
than two or more than 100 ticks falls back to automatic ticks. Changing the
display unit clears the step.

To write large tick values as `1,000` rather than `1000`, turn on **X thousands
separator** or **Y thousands separator** under **Advanced** in the same section.
When the axis reaches ±10,000, its labels are then written in full, such as
`15,000`, instead of using a shared ×10ⁿ multiplier. An axis that reaches 10¹⁵
or more, or whose largest value is below 0.001, keeps the multiplier.

## Tick marks and axis lines

Tick marks and axis lines are set once for every plot in the document, in the
advanced controls of the **Figure typography** section (see
[Typography, themes and styles](/guides/themes-and-styles/#figure-typography)):

- **Axis line width** — axis lines and tick marks, 0.05 to 10 pt, default
  1 pt.
- **Tick length** — 0 to 20 pt, default 3 pt.
- **Tick direction** — *Outward*, the default, or *Inward*. Inward ticks point
  into the plot, so the tick labels move up to the axis line.
- **Minor ticks** — that many unlabelled ticks between neighbouring major ticks
  on both axes (0 to 9, default 0, which hides them). Minor ticks are half the
  tick length and use the same width and direction. Categorical axes, and axes
  showing fewer than two major ticks, have no minor ticks.

## With an AI client

You can ask an AI client connected to Plotium to do this for you, for example:

> On the plot named Kinetics, make the y axis logarithmic and set the x range to
> 0–120 s. Show me a preview before you apply it.

> Put the Temperature curve on a second y axis on the right and title that axis
> Temperature (K).

Values you set by hand are kept unless you ask the AI to change them, and the
change is one Undo step. Check the preview before you agree. See
[Access, review and undo](/ai/control-and-review/).
