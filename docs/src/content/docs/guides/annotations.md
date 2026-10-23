---
title: Annotations
description: Text labels, panel labels, shapes, arrows, labels inside plots, and grouping on the board.
---

Figures rarely ship without a label or an arrow. Plotium's annotation objects
live on the board next to your plots, move and export with them, and are
fully undoable.

## Text and panel labels

- **Text** (`T`) — a free text label or caption. Set the font size, color,
  alignment, and bold weight in the Object inspector.
- **Panel label** — the same object with defaults tuned for figure panel
  letters: bold, 8 pt, ready for journal-sized layouts. Available from the
  Ribbon and the command palette.

While editing a label, `Enter` commits, `Shift` + `Enter` inserts a newline,
and `Esc` cancels.

## Shapes

- **Rectangle** (`R`) and **Ellipse** (`O`) — outline a feature; each has a
  stroke color, stroke width, and an optional fill.
- **Line** (`L`) and **Arrow** — a straight connector drawn between the two
  corners you drag; the arrow adds a head at the far end.

Drag on the canvas to create a shape, then move or resize it like any frame.

## Labels inside plots

Text annotations and reference-line labels that belong to a plot (for example
those an agent adds through [automation](/guides/automation/)) place
themselves clear of the data:

- A reference-line label starts at the top of a vertical line or the right end
  of a horizontal one. When that spot covers a curve, points, bars, error
  bars, an arrow, the legend or another label, it moves along its line to the
  middle or the other end, or to the other side of the line.
- A text annotation sits at its anchor point when that spot is clear.
  Otherwise it moves to the nearest clear spot within about three text heights
  of the anchor. An arrow that points at the text does not push it away.
- Value labels on bars and points stay on their marks.

Drag a label to place it yourself; `Esc` during the drag puts it back, and the
move is one Undo step. A dragged text keeps its distance from its anchor and a
dragged reference label keeps its place along the line, so both follow axis
range changes, zoom and pan. Agent edits to the text, anchor or size keep your
placement. Double-click the label to return it to automatic placement.

## Style defaults

After styling an object, you can set its look as the default for new objects
of that kind ("set as default for new …"), so a series of labels comes out
consistent without restyling each one. These defaults last for the session;
axis and title text sizes are a document-level setting instead — see
[Figure Typography](/guides/layout-and-export/#plot-styling-and-typography).

## Selecting, grouping, and locking

- `Ctrl` + `A` selects all objects on the page; `Esc` steps the selection
  back out one level.
- `Ctrl` + `G` groups the selected objects so they select and move together;
  `Ctrl` + `Shift` + `G` ungroups.
- Objects can be locked against accidental edits, or hidden, from their
  context menu.
- `Delete` or `Backspace` removes the selected annotation objects.

Alignment, distribution, and z-ordering for annotations work exactly as for
plot frames — see [the board](/guides/layout-and-export/).
