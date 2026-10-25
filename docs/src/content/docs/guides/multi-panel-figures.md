---
title: Multi-panel figures
description: Arrange plots on the board, build figure panels, and size pages for journals.
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
A resize stops before any plot inside the Panel would lose the room for its
data to its axes, ticks and colour scale. While you move or resize a Panel or a
plot, its plot area also snaps to the plot-area edges of the other plots, so
axes line up even when their tick labels differ in width.

With **Clip contents to panel** off, content can extend beyond its Panel and
the page; drag the visible content to bring it back. With clipping on, dragging
or resizing content keeps it reachable inside the Panel. Where objects overlap,
the topmost visible object receives the drag.

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
sits on it; on a plot too short for that, the plot moves right of the letter
instead. Manual and locked labels remain
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
  overlap. The axis text of neighboring plots always keeps at least 8 pt
  apart.
- **Frame** measures between the plot frames and ignores axis text. Frames
  then sit exactly the requested distance apart, and the visible space between
  data areas varies from pair to pair.

A plot frame ends at its outermost text, tick, or axis line, with no blank
border of its own. A panel placed against the page edge therefore reaches the
edge of the exported figure, which suits journal sizes where the page is the
finished figure.

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
  whatever the width of the tick numbers. Each plot keeps or omits its own
  right y axis, and the right-axis titles of the plots that have one line up
  the same way.
- Only the bottom plot draws x tick numbers and the x-axis title, and only the
  top plot draws a figure title. Where two plots meet, their x ticks point
  inward; the bottom axis keeps the tick direction you chose. A ×10ⁿ multiplier
  of a lower plot's y axis moves just inside its top-left corner, and a y tick
  number that would sit on the shared edge is left out.
- The plots share one x window. Entering the stack sets an x range covering all
  of their data and the top plot's x major tick step on every plot, so the
  plots must use the same x scale (linear or logarithmic) first. Every plot follows the bottom plot's x direction: an
  NMR spectrum, whose ppm axis runs right to left, stacked above an ascending
  plot is drawn left to right.
- Zooming or panning x on one plot moves all of them. Changing the x range,
  logarithmic scale, x display unit, thousands separator or major tick step on
  one plot changes
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

## With an AI client

You can ask an AI client connected to Plotium to do this for you, for example:

> Make this page 180 × 90 mm and arrange these two panels side by side with
> 4 mm margins and a 5 mm gap. Label them a and b, and show me a preview. Leave
> the other panels alone.

Plotium changes only the panels you name, and the whole layout is one Undo
step; you can keep editing every plot and text item afterwards. Check the
preview before you agree. See [Access, review and undo](/ai/control-and-review/).

When the figure is ready, see [Exporting](/guides/exporting/).
