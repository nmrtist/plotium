---
title: Your first figure
description: From a spectrum or a table to an exported, publication-ready figure in one sitting.
---

This walkthrough takes you from your data to an exported figure. Start from a
spectrum if your data comes from an instrument such as an NMR spectrometer, or
from a table if it is a CSV or Excel file. Both routes end with the same layout,
export, and save steps.

You can also ask an AI client connected to Plotium to make a first draft for you
and then refine it yourself on the board; see
[Connect an AI client](/ai/connect/).

## Start from a spectrum

### 1. Import

Drag your data file — for example a JEOL `.jdf` — onto the Plotium window, or
choose **Open File…** from the **File** menu. For a Bruker TopSpin experiment,
use **Open Folder…** and pick the experiment directory instead.

The dataset appears in the Primary Side Bar and is placed on the board
automatically, already plotted on its own page.

### 2. Process

Open the **Process** tab of the Ribbon. A new 1D dataset already carries the
standard pipeline — apodization, zero filling, FFT, phase correction, and
baseline correction — with automatic phasing enabled, so in most cases the
spectrum on screen is already usable.

The tab opens the Processing card at the upper right of the canvas, listing
those steps in order. Edit them there: every change is previewed live and can
be undone. Two edits cover most sessions:

- If the baseline rolls, enable the **Baseline correction** step.
- If the automatic phase is off, open the **Phase correction** step and adjust
  φ0 / φ1 with live preview.

See [Processing pipeline](/guides/processing/) for the full list of steps.

### 3. Analyze

Press `P` to select the **Peaks** tool, then drag the threshold line on the
plot: peaks above it are detected when you release. Add, move, or delete peak
marks by hand where the automatic detection needs help.

Extracted values — peak lists, integrals, fit parameters — land in data tables
linked to the spectrum. This walkthrough stops at peaks; the
[analysis tool overview](/guides/choosing-a-tool/) shows what else is
available and when to reach for it.

Continue with [Lay out the page](#lay-out-the-page).

## Start from a table

### 1. Import the table

Choose **Import Table…** on the **Data** tab and pick a CSV, TSV, or XLSX file,
or paste a copied table with `Ctrl` + `Shift` + `V`. The **Review table import**
dialog shows each column's detected type and unit; check them, then choose
**Import table**. See [Import a table](/guides/importing-data/#import-a-table)
for how column types are decided.

The table appears on the board as a sheet, beside a new page with an empty plot.

### 2. Choose the columns

Click the plot to select it. In the Object inspector, open **Data → Curve
columns**, choose the **X** and **Y** columns, and optionally a **Y error**
column, then choose **Apply curve binding**. The curve appears on the plot.

To plot another column on the same axes, open **Add curve**, choose the table
and its **X** and **Y** columns, pick **Line** or **Scatter**, and choose
**Append curve**.

### 3. Pick a chart type

The plot starts as a line chart. To show the data as bars, a histogram, box
plots, or another chart, choose **Chart** on the **Figure** tab and pick from
the **Chart type** gallery. See [Plots and chart types](/guides/plots-and-charts/)
for which chart answers which question.

## Lay out the page

Click the size chip above the page's top-left corner (or **Canvas Size &
Settings…** on the **Figure** Ribbon tab) and pick your target journal's
single-column preset. The page width snaps to the journal's artwork
specification; axis text is already at printed size.

Add a panel label or annotation if you need one — press `T` for the Text tool
and click the canvas. See [Annotations](/guides/annotations/).

## Export

Use the **Figure** tab's **Output** group (or **File → Export**) and choose a vector format — SVG for
further editing, PDF for manuscripts — or a raster preset such as
*Single column · 89 mm · 600 dpi · TIFF*. The export precheck warns if font
sizes or line widths violate the chosen preset before anything is written.

For a quick paste into a document, `Ctrl` + `C` copies the selected frame to
the clipboard as bitmap and vector at once.

## Save the project

`Ctrl` + `S` saves everything — data, processing, analysis, and layout — as a
single `.plotium` file you can reopen later or send to a colleague. See
[Plotium files](/reference/file-formats/).
