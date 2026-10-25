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
those an AI client adds) place
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
range changes, zoom and pan. Edits an AI client makes to the text or size keep
your placement, and it moves a label you placed only when you ask it to.
Right-click the label and choose **Automatic position** to return it to
automatic placement.

### Editing labels, arrows, and reference lines

- **Add**: right-click an empty spot inside a plot and choose **Add text
  here**, **Add arrow here**, **Add vertical line here**, or **Add horizontal
  line here**. The new annotation sits at the clicked data point; an arrow
  points at it.
- **Edit text**: double-click a text annotation or reference-line label, or
  choose **Edit text…** from its right-click menu. In a text annotation
  `Enter` starts a new line and `Ctrl`/`Cmd` + `Enter` saves; a reference-line
  label is one line, saved with `Enter`. `Esc` cancels.
- **Move**: drag an arrow to move it, or drag one of its ends to move only that
  end; drag a reference line across itself to change its value. Arrow keys
  nudge the selected annotation. An AI client keeps an arrow or line you moved
  where you put it unless you ask it to move it.
- **Style**: click an annotation to select it; the Object inspector shows its
  size and color (text), color, width, and style (reference lines), or color,
  width, and head (arrows).
- **Delete**: press `Delete` or `Backspace`, or choose **Delete annotation**
  from its right-click menu.

## Style defaults

After styling an object, you can set its look as the default for new objects
of that kind ("set as default for new …"), so a series of labels comes out
consistent without restyling each one. These defaults last for the session;
axis and title text sizes are a document-level setting instead — see
[Figure typography](/guides/themes-and-styles/#figure-typography).

## Selecting, grouping, and locking

- `Ctrl` + `A` selects all objects on the page; `Esc` steps the selection
  back out one level.
- `Ctrl` + `G` groups the selected objects so they select and move together;
  `Ctrl` + `Shift` + `G` ungroups.
- Objects can be locked against accidental edits, or hidden, from their
  context menu.
- `Delete` or `Backspace` removes the selected annotation objects.

Alignment, distribution, and z-ordering for annotations work exactly as for
plot frames — see [the board](/guides/multi-panel-figures/#the-board).

## With an AI client

You can ask an AI client connected to Plotium to do this for you, for example:

> On the plot named Kinetics, add a dashed vertical line at 30 s labelled
> Injection, and an arrow pointing at the peak near 45 s. Show me a preview.

The new labels place themselves clear of the data, a label you placed by hand
stays where you put it, and the change is one Undo step. Check the preview
before you agree. See
[Access, review and undo](/ai/control-and-review/).
