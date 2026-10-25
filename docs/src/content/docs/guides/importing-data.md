---
title: Importing data
description: Open instrument files, folders, and tables, follow import progress, and review how table columns are typed.
---

Plotium reads instrument files and tables directly, with no conversion step.
For the full list of formats and what each importer reads, see
[Supported formats](/guides/supported-formats/).

## Opening files

Drag a file onto the Plotium window, or use the **Data** tab's **Import** group:
*Open File…*, *Open Folder…* (for acquisition directories such as Bruker
TopSpin, Varian/Agilent VnmrJ, and Waters MassLynx RAW), *Open Project…*, or
*Import Table…*.
Each imported dataset appears in the Primary Side Bar and is placed on the
board automatically.

The file picker accepts several files at once. Opening a folder recursively
imports every `.abf`, `.spm`, `.pfc`, `.vms`, `.wiff`, structured CasaXPS
`.txt`, and recognized `.raw` bundle below it. Companion files, such as a
`.wiff.scan` file or the internal files of a `.raw` directory, are not imported
as separate datasets.

Where extensions are shared, Plotium identifies the format from the file's
content: a `.txt` file opens as CasaXPS or Rigaku XRD only when its header says
so, and otherwise goes to [table import](#import-a-table).

## Follow import progress

Scientific data files and acquisition folders load in the background, so you can
continue working. When you import multiple datasets, they appear on the board in
their original discovery order without changing your current page or selection.
The status bar shows the current file and success/failure counts; review failures
in the diagnostic history.
Additional imports wait for the current batch. Opening, closing, or creating a
project cancels unfinished imports for the previous project.

NMR import includes default processing. NUS reconstruction can take substantially
longer than reading the file. Project files, table previews, ZIP files, and imports
with a manually supplied sampling table use separate import workflows.

## Import a table

Choose **Import Table…** for a CSV, TSV, delimited `.txt`, or XLSX file. Tables
can also be pasted straight from the clipboard with `Ctrl` + `Shift` + `V` —
comma-, tab-, or semicolon-delimited text becomes a new data table.

Importing a table, from a file or the clipboard, first opens a **Review table
import** dialog. It shows each column's inferred type and unit, whether the
column allows empty cells, a preview of the first rows, and any import
diagnostics. Choose **Import table** to add it, or **Cancel** to leave your
project and recent-file list untouched. An XLSX workbook with several tables
adds a **Table** selector so you can preview each one; a single **Import
table** brings them all in as separate tables. While a preview is pending,
selecting a second table file is rejected; finish or cancel the current preview
first.

The imported table appears on the board as a sheet beside a new page for its
plot. To choose which columns the plot shows, see
[Plots and chart types](/guides/plots-and-charts/).

### How column types are decided

Plotium keeps Boolean, whole-number, decimal, text, and empty cells distinct. A
column that mixes kinds of value, or whose values are ambiguous, is kept as text
rather than dropped. Unless the file carries Plotium's own type information (see
below), only unmistakable values are typed automatically: `true`/`false`,
base-10 integers, `YYYY-MM-DD` dates, and `YYYY-MM-DDTHH:MM:SSZ` UTC timestamps.
Locale-specific dates and columns that mix numbers with text stay as text, so
Plotium never guesses a regional format.

Rows shorter than the header are filled with empty cells, as spreadsheets write
when some columns end early; the diagnostics say how many. When the columns
alternate as x/y pairs of different lengths, such as Source Data where each
curve has its own x column, the diagnostics list the pairs.

### Error-bar columns

To add error bars to your own data, put a column named `<y>_sigma` immediately
after a y column in a CSV or TSV, holding each point's uncertainty (zero or
positive); on import Plotium links it to that y column as a symmetric standard
deviation.

### Origin CSV exports

CSV files exported from Origin start with label rows (`Short Name`,
`Long Name`, `Units`, `Comments`) and a row-number column. Plotium names each
column from its Long Name (or its Comments, then its Short Name), appends the
unit in parentheses, converts Origin's superscript, subscript and Greek markup
(`cm\+(-1)` becomes cm⁻¹), keeps Comments with the column, and drops the
row-number column. When Origin exported a single column without its x values,
the row numbers are kept as the x column and the diagnostics warn about it.

Origin project files (`.opj`) open in the same preview; see
[Origin projects](/guides/supported-formats/#origin-projects-experimental).

### Tables exported by Plotium

When Plotium exports a CSV or TSV, it writes a companion `.plotium-schema.json` file
next to it, and Copy TSV puts the same information on the clipboard beside the
plain text (on Windows). Reopening either restores the original column types,
units, and error-bar relationships. Without that companion, Plotium infers the
types on import and flags anything ambiguous in the review dialog.

### Excel workbooks

In an `.xlsx` workbook, each visible worksheet imports as its own table, and
Plotium keeps its type information on a hidden worksheet. Plotium reads the value
Excel cached for each formula but does not recalculate formulas itself; a
formula cell with no cached value imports as empty and is listed in the
diagnostics. Exported XLSX files hold plain values, so they never depend on
Excel recalculating them.

### Source Data worksheets

Worksheets laid out like journal Source Data are split into clean tables:

- **Side-by-side panels.** Fully empty columns separate tables, and so do
  panel letters (`a`, `b`, …) on the first row. Each table is named after the
  worksheet and its panel letter, such as *Figure 3 b*; without a letter, the
  name adds the panel's title row or its range, such as *Figure 2 H1:W2003*.
- **Several header rows.** All rows above the first value form the header. A
  row with a single cell is the table's title; the other rows are joined into
  each column's name. Sample names written over only one column of each x/y
  pair are shared by the whole pair. Columns without a header are named by
  their letter, such as *Column AE*.
- **Placeholders.** In numeric columns, placeholders such as `--`, `NaN`,
  `N/A` and `n.d.`, and any other text among the values, are read as empty
  cells; notes below the values are ignored.
- **Stacked segments.** When a repeated header row interrupts the numeric
  columns of a group, as cyclers export charge and discharge, each segment
  becomes its own column, numbered *(1)*, *(2)*, … and aligned with the
  matching x column. A group is the columns that share one sample name, or
  the whole table when there are no sample names.

The diagnostics give each table's range and every change, so you can check
them in the review dialog. A worksheet with one row of distinct text headers
over one block of plain data is read as it is.

## Supplying a missing NMR sampling table

Use this option for supported 2D Bruker NUS or JEOL acquisitions that sampled only
part of the indirect grid. You need the original sampling table and acquisition
files with enough grid and calibration information to check it.

1. Choose **File → Import NMR with Sampling Table…** (also available in the
   command palette), then select the Bruker `ser` or JEOL `.jdf` file.
2. Enter the table's source or an explanation, and the full original indirect
   grid size, including unsampled points.
3. Enter **Lanes per observation**: the number of component records acquired at
   each listed indirect point. Use the acquisition's value, not the number of
   points in the table.
4. Select **Zero-based** or **One-based** to match the original table. Enter one
   indirect index per line, in acquisition order. Each line represents all lanes
   for that observation; keep repeated observations.
5. Click **Validate and import**. If validation fails, check the reported mismatch
   against the acquisition records. Conflicts with an existing sampling list,
   incorrect observation or lane counts, and missing grid or calibration information
   prevent import.

Vendor files are not modified. Save the project to retain the table and its source;
reopening the project does not require the original files. Repeated coordinates
can be imported but currently prevent [NUS reconstruction](/guides/processing/#reconstruct-a-non-uniformly-sampled-spectrum).
For scripts, see the [CLI declaration format](/reference/cli/#sampling-declarations).

## With an AI client

You can ask an AI client connected to Plotium to import data for you, for example:

> Import the CSV at `/absolute/path/experiment.csv`. Time is in s and current
> is in mA. Show me its columns before you plot anything.

The AI client can import instrument files and CSV, TSV or XLSX tables that you
name by their full path. Units are taken only from what you state; Plotium does not
guess them. The whole import is one Undo step, and a file that fails adds
nothing to the project. Check the columns and units it reports before you plot.
See [Access, review and undo](/ai/control-and-review/).
