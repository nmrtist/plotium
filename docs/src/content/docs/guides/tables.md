---
title: Data tables
description: Working with spreadsheet-style data tables.
---

Tables are first-class datasets in Plotium. They hold imported tabular data,
values extracted by the region tools, and fit results.

## Editing

On the board, a table appears as a sheet frame that previews its first 24 rows
and 8 columns; the frame's title shows the full table size.

Double-click a table on the board to open it as an editable sheet. The sheet
is a spreadsheet-style grid: numbered rows, resizable columns, and a header
that shows each column's name with its unit and type underneath. Empty cells
render as a dimmed dash; `NaN` and infinities render dimmed. Decimal numbers
show up to six significant digits, in scientific notation for very large or small
magnitudes, the same way as on the board; editing and copying always use the
full value. The grid paints only its visible row window, so very large tables
stay responsive.

The sheet is docked below the board, between the side bars, and the board
shrinks to make room, so it never covers a page. Drag the sheet's top edge to
change its height; Plotium remembers the height between sessions. The header
shows the table's name, whether it is editable, read-only and synchronized with
its source, or in need of a refresh, and its size. Use the arrow at its left to
collapse the sheet to the header, the buttons at its right to maximize the sheet
and restore it, and × to close it. When you switch between tables, each keeps
its own scroll position and selected cell. Wide tables scroll sideways inside
the sheet, and the arrow keys bring the selected cell into view.

**New Empty Data Table** opens an empty table in the sheet with its first cell
selected, so you can start typing at once; one **Undo** removes the table.
Results that get a canvas of their own, such as patch-clamp IV and window
statistics tables, do not open the sheet; double-click the table in the Data
list to open it. Imports open their result as described in the
[interface glossary](/reference/ui-overview/).

Click a cell to select it and move with the arrow keys. Double-click, press
**Enter** or **F2**, or simply start typing to edit; **Enter** commits and
moves down, **Tab** commits and moves right, and **Esc** cancels. Invalid
input keeps the editor open with a red outline and an explanation on hover.
Click a row number to select the whole row; **Ctrl+C** copies the selected
cell or row. The line below the grid shows the current selection.

The sheet takes the keyboard only after you click a cell, row number, or column
header; the selected cell's outline turns from grey to the accent color. While
it does, **Delete** or **Backspace** clears the selected cell (a column that
does not allow empty values explains why instead), and **Ctrl+V** starts
editing the selected cell with a single value from the clipboard. Pasting
several cells at once is not supported yet. **Ctrl+Shift+V** still pastes a
new table. Click the board or press **Esc** to give the keys back to the board
and tools.

While a cell edit is unfinished, AI assistants can read the committed data but
must wait before making changes. Use **Apply** to commit the input or **Discard**
to cancel it. If the table changes or becomes unavailable during editing, the
sheet preserves your text and explains the conflict. **Copy draft** lets you
keep the input before discarding it and reopening the cell.
Finish or discard the edit before leaving the project.

Every edit is undoable; undo and redo step back and forth through the table's
edit history rather than replacing the whole table each time. Text, Boolean,
date, and empty cells edit exactly the way numbers do.

## Transforming and refreshing tables

Right-click a column header to reshape the table around that column. The menu
offers **Rename**, **Sort ascending** and **Sort descending**, **Keep only this
column**, **Filter out empty rows**, **Duplicate as computed column**, and
**Count rows per value**. Floating-point columns add **Mark non-finite as
missing** and, when the column has a convertible unit, a **Convert unit**
submenu. When the table holds several columns of one type you can **Unpivot
matching columns** into name/value rows, and a text column can **Pivot using
these names** to turn its values into column headings. The **Combine** menu in
the sheet toolbar unions or joins the table with another table in the project.
Every transform produces a new table and leaves the original untouched. The
sheet switches to the new table without adding a canvas; **Derived from** in
its header returns to the source table.

Transforms run in the background: the sheet shows the elapsed time and a
**Cancel** button, and a cancelled or failed transform never replaces the
table. A table built from another table gains a **Refresh** action — it re-runs
the steps that produced the table and reapplies your later cell edits on top.
If the source has changed so that edited rows can no longer be matched, or a
column's type or unit no longer fits, Refresh stops and reports the conflict
instead of guessing which row is which.

These transforms can also run unattended as part of a batch job — see
[Automation](/guides/automation/) and [the command line](/reference/cli/).

## Exporting a table

**Export Data…** exports the complete table as CSV, TSV, XLSX, or clipboard
TSV. Empty cells stay empty, while `NaN`, `+Inf`, and `-Inf` are written as
those literal tokens because they are real values, not blanks. CSV, TSV and
XLSX carry the table's column types, units, and error-bar relationships
alongside the data (clipboard TSV does too on Windows), so the table can be
imported again without losing them; see
[Plotium files](/reference/file-formats/#table-schema-companions). For export
options, see [Export numerical data](/guides/exporting/#export-numerical-data).

## Analyzing a table

- To plot a table's columns and pick a chart type, see
  [Plots and chart types](/guides/plots-and-charts/), including
  [error bars](/guides/plots-and-charts/#error-bars).
- To fit x-y values with a mathematical model, choose **Fit Curves** in the
  **Curve Fit** group of the **Analyze** tab; see
  [Curve fitting](/guides/curve-fitting/). The fitted curves and their results
  stay with the table. A completed fit records which rows it used, and which it
  excluded and why, so later edits to the table cannot quietly change the points
  the result was based on. To export parameters, diagnostics, covariance, and
  point predictions/residuals, use **Export Data…** and choose **Curve-fit
  parameters**; the export lists the included and excluded rows.
- To compare groups, test correlations, check normality, or run ANOVA, choose
  **Statistics** in the **Analyze** tab; see [Statistics](/guides/statistics/).
- To separate overlapping spectral peaks in a plotted trace, use **Peak Fit**
  instead; see [Choosing an analysis tool](/guides/choosing-a-tool/).

## With an AI client

You can ask an AI client connected to Plotium to change table data for you, for
example:

> Change only this source cell, show which derived tables are out of date, then
> refresh this table. Keep my colors, axis ranges and layout.

> Make a new table from this one with a column equal to current × 2, keep only
> rows where time is not 0, and sort by time descending.

Editing a cell and refreshing a derived table are separate Undo steps, and your
plot styling stays. A derived table is a new table; its source is not changed.
Check the refreshed values before you rely on them. See [Access, review and undo](/ai/control-and-review/).
