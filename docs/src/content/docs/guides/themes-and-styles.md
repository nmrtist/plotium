---
title: Typography, themes and styles
description: Set fonts and text sizes for every plot, apply canvas themes such as Nature, and save reusable styles.
---

Plotium styles plots for print automatically. New dataset pages start at the
89 × 60 mm single-column size, so a plot spanning the page already shows text at
its printed journal size; assemble multi-panel figures on a wider page later,
keeping each panel at its natural size.

## Fonts

By default, figure text is set in Arial on the canvas and in every export
format. Without Arial, Plotium uses Helvetica or a metric-compatible sans such
as Liberation Sans. SVG files name the typeface, so a vector editor keeps the
Arial that Nature-portfolio journals ask for.

**Font** in the **Figure typography** section switches the text inside every
plot — tick labels, axis titles, the figure title, legends, colour scales and
plot annotations — between *Arial* (the default), *Helvetica* and *Times New
Roman*. Helvetica is drawn as Helvetica on macOS; Windows and Linux draw it with
Arial or Liberation Sans, which have the same character widths, and SVG files
still name Helvetica first. Without Times New Roman, Plotium uses a
metric-compatible serif such as Liberation Serif. **Font weight** (an advanced
control) sets that text *Regular* or *Bold*. Text boxes and panel letters on the
canvas keep their own Arial styling. Both settings travel with themes and saved
styles; the built-in themes set Arial Regular.

## Figure typography

**Figure typography** in the Figure Ribbon tab's **Style** group opens the
document-wide typography section; **Figure Typography…** in the command palette
shows the same section in its own window with a **Reset to defaults** button.
Either sets the text sizes (tick labels, axis titles, the figure title, and
legends) for every plot at once, in absolute points — a document-level style, so
resizing a panel never changes its type size. These sizes accept 1 to 72 pt.
Legends default to 7 pt.

The same section's advanced controls set every plot's axis strokes and ticks;
see [Tick marks and axis lines](/guides/axes/#tick-marks-and-axis-lines).

**Figure typography** in the Object inspector holds the same tick-label size as
**Tick-label size**, plus **Legend size** and **Legend text color**. Changing a
value in either surface changes the one document-wide value. Because it belongs
to the document rather than to a plot, the section is shown whatever is
selected — including when nothing is.

## Canvas themes

To apply a theme to the active canvas, choose **Paper**, **Nature**, **Dark** or
**Vibrant** in the **Style** group of the **Figure** tab, or pick it under
**View ▸ Canvas Theme**. Applying a theme is one undoable step.

Canvas themes carry matching sizes — the Presentation Dark theme, for example,
enlarges the axis text for slides.

The **Nature** theme follows the Nature Portfolio final-artwork guidelines: 7 pt
text with strokes scaled to it — 0.3 pt axes, ticks, bar outlines and new
reference lines and arrows, 0.5 pt line series — outward 3 pt ticks, 3 pt
markers without an outline and the colour-blind-safe Okabe–Ito palette. Marker
outlines you drew by hand stay. Its sizes assume a Nature column width, so
applying it to a canvas that matches none of the Nature size presets (89, 136 or
183 mm wide) adds a note to the status bar naming those widths; the canvas keeps
its size. A canvas already at one of those widths without a size preset takes on
that Nature preset, so the export precheck applies Nature's minimums (5 pt text,
0.25 pt lines).

The line widths and marker sizes stay when you switch to a theme that does not
set them.

## Values you set by hand

Values you set by hand — in the Object inspector, by scrolling over a control or
by dragging a legend — are kept for you. Any theme leaves them alone: curve and
marker colours, line widths, marker sizes and outlines, text and shape colours
and the figure typography. The status bar says how many it kept. An AI client
connected to Plotium also skips them unless you ask it to change them; values
the AI sets are not counted as yours. Resetting a value (or double-clicking a
dragged legend) hands it back to the theme.

## Saved styles

**Save Canvas Style…** (View ▸ Canvas Theme, or **Save Style** in the Figure
Ribbon tab's Style group) saves the active canvas's look as a named style:
background, text colour and size, figure typography (including the font and font
weight), axis and tick strokes, curve colours with their marker fills, outlines
and hollow setting, line width and marker size. **Canvas Styles…** lists saved
styles to apply to the active canvas or delete. Applying a style is one undoable
step. To change a style, apply it, adjust the canvas and save it again under the
same name.

Styles are stored as `.plotiumstyle` files in the `styles` folder of Plotium's
configuration directory, so they can be copied to another computer: on macOS
`~/Library/Application Support/plotium/styles`; for other platforms see
[Where Plotium keeps its settings](/getting-started/installation/#where-plotium-keeps-its-settings).

## With an AI client

You can ask an AI client connected to Plotium to do this for you, for example:

> Apply the Nature theme to this canvas and show me a preview. Keep the colours
> I set by hand.

> Apply my saved style Group meeting to every canvas in this project.

Values you set by hand are kept unless you ask the AI to change them, and each
change is one Undo step. Check the preview before you agree. See
[Access, review and undo](/ai/control-and-review/).
