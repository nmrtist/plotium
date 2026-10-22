---
title: The board and page layout
description: Arranging figures on the infinite board and sizing pages for journals.
---

## The board

Plots live on an infinite board organized into pages. Dragging a frame snaps
it to the page grid, margins, and the edges of neighboring frames; snapping
can be toggled off on the **Arrange** tab. The **Arrange** tab also offers
alignment (with two or more frames selected), horizontal / vertical
distribution (three or more), z-ordering, and a *Tidy up frames* command.

Drag over empty board space to marquee-select page and sheet frames. Hold
`Shift` to add the enclosed frames or `Ctrl` (`Cmd` on macOS) to toggle them.
Dragging the header of any selected frame moves the whole selection; the move
undoes as one step. In the Canvas list, `Shift`-click selects a continuous range
and `Ctrl`-click adds or removes one canvas. `Ctrl` + `A` selects every frame or
canvas according to the area you last used.

## Scientific summary and notes

Plotium shows an automatic summary of the plotted data below each page on the
board. On a multi-panel page, shared information appears once and the remaining
details follow the panel labels.

Open **Canvas settings** to add a page note, or select a Panel and edit its
**Note**. Notes appear after the summary and are not included in exports. Use
**Show summary below page** to show or hide this whole board-only block.

## Add external images

To place each image on its own fitted page, choose **Add Images…** from the File
menu, Ribbon, or command palette. You can select multiple PNG, JPEG, TIFF, WebP,
or BMP files. You can also drop files onto the board; each image gets a new page
even if you drop it over an existing one.

To add an image inside an existing Panel, select or enter an unlocked Panel,
then drop the file inside the green outline. To paste an image or a copied list
of image files, use **Paste Image** or press `Ctrl`/`Cmd` + `V`. On Windows, you
can copy files from File Explorer or copy image pixels from an application such
as Paint. Pasting in a text field continues to paste text normally.

When Plotium creates a page for an image, it uses the image's physical size when
DPI information is available and assumes 96 DPI otherwise. The page is at most
89 mm wide, and its height follows the image's aspect ratio. The image fills the
page without distortion. Importing several images is one undoable action; a
failed or cancelled import leaves the project unchanged.

For formats that need special handling, use these commands:

- **Add Animated Image First Frame…** imports the first frame of an animated
  PNG or WebP.
- **Add Images Without Metadata…** removes metadata by converting the image to
  PNG before adding it.
- **Add All TIFF Pages…** adds every readable page of a multi-page TIFF. The
  standard **Add Images…** command adds only its first page.

Plotium asks for confirmation before importing a very large image. Select an
image and use **Image** in the Object inspector to crop or rotate it, adjust its
opacity, or choose how it is interpolated. These changes do not alter the
embedded source image.

The board uses a bounded proxy for responsive editing, but SVG, PDF, EMF,
PNG, JPEG, TIFF, and **Copy Figure** sample the embedded source. Crop,
quarter-turn rotation, opacity, fit, interpolation, Panel clipping, and layer
order therefore match the authored page without depending on the source file's
original path.

If an embedded image is missing or damaged, the project still opens and keeps
the image item in place as a labelled placeholder. Use **Replace Image…** in
the Object inspector to repair it. Normal save and publication export stop
until it is replaced; the Export dialog can explicitly allow labelled
placeholders when a review copy is still useful.

## Work with figure panels

Use the Panel commands in the Arrange menu or command palette to build a
multi-panel figure. You can create an empty Panel, turn selected objects into a
Panel, or duplicate, merge, split, dissolve, and delete Panels. Expand a Panel
in Layers to see the objects it contains.

Click anywhere inside a Panel to select it, then drag to move it or use a corner
handle to resize it together with its contents. `Shift`-click selects multiple
Panels. To work with an object inside a Panel, double-click the Panel or press
`Enter`. Use the breadcrumb above the page or press `Esc` to return to the page.
You can also `Ctrl`/`Cmd`-click an object to select it without entering first.

To exchange two Panels on the same canvas, drag one over the other and release
when both slots are highlighted. The Panels exchange positions and sizes; their
contents scale with the destination slot. This is one undoable step. Hidden or
locked Panels cannot be swap targets, and multi-Panel drags remain group moves.

Inside a Panel, objects snap to the Panel edge and to one another. To move an
object into another unlocked Panel, drag it over the Panel and release when the
target is highlighted. You can make the same move in Layers by dragging the
object's row onto the Panel's row. A Stack or Grid Panel rearranges its contents
after the move. Hidden objects remain selectable in Layers; locked objects can
be selected but not edited.

Use the Panel inspector to control visibility, locking, clipping, labels, and
layout. **Vertical Stack**, **Horizontal Stack**, and **Grid** arrange the
contents using the selected gap, padding, and alignment. **Shared X Stack**
stacks plots on one x axis; see
[Stack plots on a shared x axis](#stack-plots-on-a-shared-x-axis). Automatic panel letters
appear only when a page has more than one visible Panel and follow reading order:
rows from top to bottom, left to right within a row. They update as soon as you
move or add a Panel. Choose **Locked auto** to keep a Panel's current
letter, or clear **Participates in sequence** to leave it unlettered. A letter
placed over an image automatically switches between black and white for
contrast, and a plot's y-axis title moves down when needed so the letter never
sits on it. Manual and locked labels remain
visible on a page with one Panel. To hide every letter on a page at once, clear
**Show panel letters** in **Canvas settings**; the letters keep their sequence
and return unchanged when you turn it back on, and axis titles no longer make
room for them while they are hidden.

Drag a single Panel completely clear of every canvas and keep it outside for
0.5 seconds to prepare a new canvas. The preview shows a progress bar, then a
green outline and **Release to create canvas**. Release to move the whole Panel
onto a new canvas fitted to its current size at that board position. Its contents,
labels, note, and internal layout are retained; the source canvas stays in place.
The move and canvas creation undo together. Returning any part of the Panel to a
canvas resets the timer. Resizing or dragging multiple Panels does not create a
canvas, and pressing `Esc` cancels the gesture.

## Canvas size

The active page shows a size chip above its top-left corner — the current
dimensions plus the matched preset (for example *89 × 60 mm · Nature ·
Single column*). Click it, use **Canvas Size & Settings…** on the Figure
Ribbon tab, or run any "Canvas Size" entry in the command palette to change
the size.

The preset list is searchable and grouped: journal figure widths (Nature,
Science, Cell Press, ACS, Elsevier, PNAS, and IEEE, taken from the
publishers' artwork guidelines), paper sizes with a portrait/landscape
toggle, presentation slides, recently used entries, and your own saved
custom sizes. Journal presets fix the page *width* — the height stays
content-driven up to the journal's maximum figure depth, and a warning
appears when the page grows past that depth.

By default a size change never moves or resizes your content; if objects
end up outside the page, the chip row shows an overflow warning that scales
the content down to fit in one undoable click (font sizes keep their
physical pt values). Turn on **Scale content when applying sizes** to have
presets scale objects together with the page instead.

Two helpers automate the rest:

- **Auto height** keeps the width fixed and lets the page height follow the
  content's depth, clamped to the journal's maximum.
- When the layout grid asks for two or more panel columns on a
  single-column page, a dismissible hint offers the same journal's full
  width in one click — it never resizes the page on its own.

The export dialog pre-selects the matching journal preset, so a page
authored at a column width exports at that width by default.

## Spacing between panels

Panels of a multi-panel figure read best when their data areas are evenly
spaced — but each plot reserves a different amount of room for its tick
labels and axis titles, so equal frame gaps rarely look equal.
**Canvas Size & Settings…** sets both the spacing you want and how it is
measured.

**Minimum spacing** is the gap itself, in the canvas unit; the **Tight**
(2 mm), **Normal** (5 mm), and **Spacious** (10 mm) presets fill in common
values.

**Spacing basis** decides what that gap is measured between:

- **Visual** (the default) measures between the data areas of neighboring
  plots, counting the tick labels and axis titles that sit between them, so a
  panel with a long y title is given the room it needs. The value is a
  minimum: the gap you see can end up wider, never narrower, and frames never
  overlap.
- **Frame** measures between the plot frames and ignores axis text. Frames
  then sit exactly the requested distance apart, and the visible space between
  data areas varies from pair to pair.

The basis applies wherever Plotium places plots for you — **Apply grid**, and
dragging a plot onto a page that already holds one.

Dragging a plot onto another page moves it there and re-tiles the destination.
During the drag the plot travels with the pointer, keeping the point you grabbed
under the cursor, and the destination page draws where every plot will sit once
you release. On a page that already holds two or more plots, the whole page
re-tiles into an even grid and the arriving plot takes the cell you are pointing
at.

If the move leaves the source page empty, Plotium deletes that page as part of the
drop, so the move and the deletion undo together. Hold `Alt` as you release to
keep the empty page instead; the Operation history shows which way `Alt` will flip the
current drop. To keep empty source pages by default, turn on **Keep source canvas
when tiling its last object** in Preferences → General — `Alt` then removes them
for that one drop.

With the Select tool active, each non-zero page margin is drawn as a dashed
line across the page, showing the content area you are laying out into; a
margin of zero draws no line. Turning on the layout grid adds the cell
outlines, and snapping guides appear in a contrasting color while you drag.

## Simplify inner axes

In a grid of panels that share the same axes, repeating the tick numbers and
axis titles on every panel wastes space. **Simplify inner axes** keeps the
x-axis text only on the bottom plot of each column and the y-axis text only on
the leftmost plot of each row. Axis lines and tick marks stay on every panel.

There are two ways in:

- Tick **Simplify inner axes** beside **Apply grid** in Canvas settings to
  arrange and simplify as one undoable step. The frames are then measured
  against the simplified axes, so the panels grow into the space the hidden
  text used to take.
- Run **Simplify Inner Axes** — on the Arrange Ribbon tab, in the canvas
  right-click Arrange menu, or from the command palette — to simplify plots
  that are already in place. It needs at least two plots aligned in a grid;
  otherwise the Operation history says what to fix.

To bring text back on one panel, select it and use **Axes** in the Object
inspector: the **X text** and **Y text** rows toggle **Tick labels** and
**Title** individually, and **Automatic** returns that axis to showing both.

To write large tick values as `1,000` rather than `1000`, open **Advanced** in
the same **Axes** section and turn on **X thousands separator** or **Y
thousands separator**. When the axis reaches ±10,000, its labels are then
written in full, such as `15,000`, instead of using a shared ×10ⁿ multiplier.
An axis that reaches 10¹⁵ or more, or whose largest value is below 0.001, keeps
the multiplier. Agents can set the same option through the
`object.axes.x_thousands_separator` and `object.axes.y_thousands_separator`
properties.

## Stack plots on a shared x axis

A Shared X Stack puts two or more plots in one Panel, one above the other, on a
common x axis — for example an XRD pattern above its reference-card sticks, or
several spectra kept apart. Each plot keeps its own y axis, legend and chart
type.

There are two ways in:

- Select a Panel that holds two or more plots and choose **Shared X Stack** in
  the Panel inspector's **Layout** list, the **Edit ▸ Panel** menu or the
  command palette.
- Select several Panels, each holding a plot, and run **Stack with Shared X**.
  The Panels are merged into the first one and stacked top to bottom in their
  page order, as one undoable step.

In the stack:

- The plots fill the Panel's width and touch at their inner edges. The gap
  starts at 0 pt; raise **Gap** in the Panel inspector to separate them while
  keeping them aligned.
- Plot areas share one left and one right edge, so the y-axis titles line up
  whatever the width of the tick numbers.
- Only the bottom plot draws x tick numbers and the x-axis title, and only the
  top plot draws a figure title. Where two plots meet, their x ticks point
  inward; the bottom axis keeps the tick direction you chose. A ×10ⁿ multiplier
  of a lower plot's y axis moves just inside its top-left corner, and a y tick
  number that would sit on the shared edge is left out.
- The plots share one x window. Entering the stack sets an x range covering all
  of their data on every plot, so the plots must use the same x scale (linear
  or logarithmic) first. Every plot follows the bottom plot's x direction: an
  NMR spectrum, whose ppm axis runs right to left, stacked above an ascending
  plot is drawn left to right.
- Zooming or panning x on one plot moves all of them. Changing the x range,
  logarithmic scale, x display unit or thousands separator on one plot changes
  every plot in the stack in one undoable step; a change that one of the plots
  cannot take, such as a logarithmic axis on an NMR spectrum, is refused.
- The top-to-bottom order is the Panel's content order. Each plot's height sets
  its share of the stack: drag a plot's bottom edge on the canvas to make it
  taller or shorter than the others.
- The whole stack is one Panel with one panel letter.

The canvas, the Operation-history checks and every export use the same stacked
geometry. To leave the stack, set the layout back to **Free** or **Vertical
Stack**; each plot then draws its own x axis and x direction again. The shared
x range stays until you set **X range** back to **Auto**.

## Stacked and multi-dataset plots

A single plot frame can display several 1D datasets — superimposed, or
stacked with adjustable vertical spacing and 3D shear. 2D datasets combine
as a color overlay.

To align line series along x, select the plot and choose **Align Traces…** in
the **Align** group on the **Analyze** tab. The command is available when the
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

## Plot styling and typography

Plotium styles plots for print automatically: clean bottom-and-left axes with
outward ticks, tick precision that follows the displayed range, tick density
that thins as a panel narrows, and NMR isotope numbers set as superscripts.
New dataset pages start at the 89 × 60 mm single-column size, so a plot
spanning the page already shows text at its printed journal size; assemble
multi-panel figures on a wider page later, keeping each panel at its natural
size.

What you control directly:

- Select one plot and use **Axes** in the Object inspector to override its X
  and Y titles or numeric ranges, or to hide either axis's tick labels and
  title. For a true 2D NMR spectrum, **F1/F2 equal scale (1:1)** uses the same
  data units per screen unit on both axes. Plotium sets its initial value when
  the spectrum is imported; you can change it afterward without changing the
  import preference. Leave a title blank, or keep a range on **Auto**, to use
  the value derived from the data. A manual range becomes that axis's full
  range: zooming and panning stay inside it, and a double-click on the plot
  returns to it. Charts without visible axes offer no axis settings, and
  categorical axes have no range controls.
- **Frame** in the same **Axes** section chooses *Left and bottom* or *All four
  sides*. The four-sided frame adds top and right axis lines with the same
  stroke as the other axes; ticks and labels stay on the left and bottom axes.
  Most plots start with left and bottom axes; 2D maps such as contour plots and
  heatmaps start with all four sides.
- **X scale** and **Y scale**, under the more options of the **Axes** section,
  switch a line, scatter or stick plot between *Linear* and *Logarithmic*. A
  logarithmic axis labels powers of ten (10¹, 10², …), thins them when the
  axis is short, and uses ordinary ticks when fewer than two powers of ten are
  visible;
  minor ticks, when enabled, mark 2–9 × each decade. Its automatic range fits
  only the positive data. Zero and negative values have no position: lines
  break there, and error bars and sticks that reach below the axis end at its
  edge. A manual range on a logarithmic axis must start above zero. Zooming and
  panning move by decades. Logarithmic axes are not offered for categorical
  axes, 2D maps, bars and distributions, or NMR spectra, whose integral, phase
  and cursor tools measure in linear ppm.
- Use **Legend & scales** in the Object inspector for any plot. With
  **Visibility** on **Auto**, Plotium shows a categorical legend when the plot
  has two or more named entries and shows a continuous colour scale for a
  heatmap. **Show** also permits a one-entry legend; **Hide** removes the legend
  or colour scale from that plot. **Position** on **Auto** places a legend
  inside the plot in whichever corner or edge midpoint covers the fewest data
  points and lines, preferring the roomier spot and then the upper right. When
  every such spot covers data, Plotium extends **Auto** axis ranges on the
  legend's side, by up to half their span, just far enough to clear one;
  manual ranges, zoomed views and aspect-locked plots are never changed.
  **Inside** keeps it in the upper right, and **Outside right** or **Outside
  bottom** reserve space beside the plot. **Layout** switches a categorical
  legend between a vertical list and a horizontal row; **Title** is available
  under the section's advanced controls.
- Legends draw no border and no background, on screen and in every export
  format; only their entries and title are drawn.
- With **Select** active, click a legend or colour scale to open its
  **Legend & scales** settings. Drag a categorical legend to place it freely;
  the custom position survives resizing and is used by every export format.
  Double-click a legend or colour scale to restore its automatic placement.
- Axis titles, legend entries and text use true superscripts and subscripts:
  type `cm^{-2}` or `H_{2}O`. Characters such as `⁻²` and `₂` are drawn the
  same way. EMF output falls back to those Unicode characters.
- By default, figure text is set in Arial on the canvas and in every export
  format. Without Arial, Plotium uses Helvetica or a metric-compatible sans
  such as Liberation Sans. SVG files name the typeface, so a vector editor
  keeps the Arial that Nature-portfolio journals ask for.
- **Font** in the **Figure typography** section switches the text inside every
  plot — tick labels, axis titles, the figure title, legends, colour scales
  and plot annotations — between *Arial* (the default), *Helvetica* and
  *Times New Roman*. Helvetica is drawn as Helvetica on macOS; Windows and
  Linux draw it with Arial or Liberation Sans, which have the same character
  widths, and SVG files still name Helvetica first. Without Times New Roman,
  Plotium uses a metric-compatible serif such as Liberation Serif. **Font weight** (an advanced control) sets that text
  *Regular* or *Bold*. Text boxes and panel letters on the canvas keep their
  own Arial styling. Both settings travel with themes and saved styles; the
  built-in themes set Arial Regular.
- **Figure typography** in the Figure Ribbon tab's **Style** group opens the
  document-wide typography section; **Figure Typography…** in the command
  palette shows the same section in its own window with a **Reset to
  defaults** button. Either sets the text sizes (tick labels, axis titles, the
  figure title, and legends) for every plot at once, in absolute points — a
  document-level style, so resizing a panel never changes its type size.
  These sizes accept 1 to 72 pt. Legends default to 7 pt.
- The same section's advanced controls set every plot's axis strokes:
  **Axis line width** (axis lines and tick marks, 0.05 to 10 pt, default
  1 pt), **Tick length** (0 to 20 pt, default 3 pt) and **Tick direction**
  (*Outward*, the default, or *Inward*). Inward ticks point into the plot, so
  the tick labels move up to the axis line.
- **Minor ticks** in the same advanced controls draws that many unlabelled
  ticks between neighbouring major ticks on both axes (0 to 9, default 0,
  which hides them). Minor ticks are half the tick length and use the same
  width and direction. Categorical axes, and axes showing fewer than two
  major ticks, have no minor ticks.
- **Figure typography** in the Object inspector holds the same tick-label size
  as **Tick-label size**, plus **Legend size** and **Legend text color**.
  Changing a value in either surface changes the one document-wide value.
  Because it belongs to the document rather than to a plot, the section is
  shown whatever is selected — including when nothing is.
- **Line** in the Object inspector sets the **Stroke width** of the selected
  plot's line series, in points. New line series are 0.5 pt; drag or type any
  value from 0.05 to 10 pt, or take one from **Presets** — *Fine* 0.50 pt,
  *Medium* 0.75 pt, *Bold* 1.25 pt. The section appears only when something in
  the selection is drawn as a line, and its header counts the series it is
  about to change. Select several plots and it edits them together; select
  nothing and it follows the page's active plot.

  When the selected series do not all carry the same width the control shows an
  em dash and the word *mixed* rather than presenting one of them as the
  setting; setting the row is what makes them agree. A selected series that is
  drawn as something else — a contour, say — is reported as skipped in the
  Operation history, and the rest still take the value. A width that differs from the
  one Plotium would choose for this data is marked with a dot, and the reset
  button beside it re-derives that default.
- **Advanced** under **Line** holds each curve's marker colours, apart from its
  line colour. **Marker fill** sets the inside of closed markers (circle, square, diamond,
  triangles, star, pentagon, hexagon) and **Marker edge color** their outline; both follow the curve
  colour until set. **Marker edge width** draws that outline, from 0 (no
  outline, the default) to 5 pt. **Hollow marker** leaves closed markers
  unfilled and keeps their outline, 0.5 pt when the edge width is 0; the
  fill colour returns when it is turned off. Plus and cross markers stay in the curve
  colour. The legend swatch shows the same marker. Charts whose colours carry
  meaning keep their marker colours.
- **Canvas themes** carry matching sizes — the Presentation Dark theme, for
  example, enlarges the axis text for slides. The **Nature** theme follows the
  Nature Portfolio final-artwork guidelines: 7 pt text, 0.6 pt axes with
  outward 3 pt ticks, 0.75 pt line series, 3 pt markers without an outline and
  the colour-blind-safe Okabe–Ito palette. Its sizes assume a Nature column
  width, so applying it to a canvas that matches none of the Nature size presets
  (89, 136 or 183 mm wide) adds a note to the status bar naming those widths;
  the canvas keeps its size. The
  line widths and marker sizes stay when you switch to a theme that does not
  set them.
- **Save Canvas Style…** (View ▸ Canvas Theme, or **Save Style** in the
  Figure Ribbon tab's Style group) saves the active canvas's look as a named
  style: background, text colour and size, figure typography (including the
  font and font weight), axis and tick
  strokes, curve colours with their marker fills, outlines and hollow setting,
  line width and marker size. **Canvas Styles…** lists saved styles to apply to the active canvas or
  delete. Applying a style is one undoable step. To change a style, apply it,
  adjust the canvas and save it again under the same name. Styles are stored
  as `.plotiumstyle` files in the `styles` folder of Plotium's configuration
  directory, so they can be copied to another computer.

When the figure is ready, see [Exporting](/guides/exporting/).
