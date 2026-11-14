---
title: Organizing data
description: Browse datasets and their derived results, and save your work as a project.
---

## The Data browser

Switch the Primary Side Bar to **Data** to see a searchable tree of everything
in the project. Imported datasets are roots. Slices, projections,
spectrum-arithmetic results, region tables, peak-fit tables, and multiplet
tables appear under the dataset they came from, in **Derived data**.
**Analysis** expands to individual peaks, integrals, regions, peak fits,
multiplets, and fitted table columns.

A link icon marks a result with more than one source — for example a table
built from two spectra. It is the same dataset shown under each source, not a
copy: selection, highlighting, renaming, and opening the data sheet stay
synchronized. Hover the icon to see every source, or use **Reveal sources**
from its context menu.

Search is case-insensitive and keeps the complete ancestor path visible;
clearing it restores your previous expanded and collapsed branches.

Search terms can match across the dataset name, experiment, tags, data type,
dimensionality, source, and scientific summary (for example, `600.0 MHz`).
The search `hnco 3d batch-12` can match an
experiment label, the acquisition dimensionality, and a source folder. Hover a
row for source details. Click a tag to filter by it; remove the filter with
its × button.

In **Canvas**, click the pencil beside the card's dataset summary. If the
canvas contains several datasets, choose which one to edit. In **Data**,
right-click a dataset and choose **Edit experiment and tags…**, or select
several datasets and use the button above the list to edit them together.
Enter a display name or choose an NMR suggestion. For a single dataset, the
editor shows the imported name separately. **Use imported names** clears your
overrides. Experiment names and tags are labels; they do not change
processing or assignment. Add a new tag or choose one from **Project tags**.
On a mixed selection, a count beside a tag shows how many datasets have it.
Removing a tag leaves the other tags in place. These edits support undo and
redo and are saved with the project.

Use **Locate plot** to go straight to a dataset's series, including a series
in an overlay. If the dataset appears more than once, choose a destination in
the **Plot destinations** window, even if the Data List filters hide the row.
Hidden destinations are marked; use **Reveal** to show one. Locating a plot
keeps your filters and briefly highlights the visible series or outlines the
plot, for example when locating a heatmap.

## Selecting and opening

Click a dataset or reference to select it. `Shift`-click selects the continuous
range from the previous lead item; `Ctrl`-click (`Cmd` on macOS) adds or removes
one item, and combining both modifiers adds a range without clearing the current
selection. The same extended-selection model applies to the Canvas and Layers
lists. A multi-selection is how you stack several spectra in one plot or apply a
processing template to many datasets at once.

When one of these lists is active, use `↑` / `↓` or `Home` / `End` to move the
selection, hold `Shift` to extend it, and press `Space` to add or remove the lead
item. `Ctrl` + `A` selects all datasets; `Ctrl` + `Shift` + `A` clears them.
Double-click a dataset to open its data sheet. Click an analysis result to
focus its dataset; double-click it to jump to the plot and the corresponding
analysis tool.

To delete a dataset, choose **Delete Dataset** from its context menu or the
**Edit** menu, or select it in the Data list and press `Delete` or `Backspace`. A dataset that a plot, a derived dataset or
an analysis report still uses is kept; the status bar names what uses it, so you
can remove those first. Deletion can be undone.

Projects saved with much older versions of Plotium may show some derived results
as top-level datasets; they remain fully usable.

In **Canvas**, the line under each title summarizes the datasets directly
plotted there, including hidden series. A dataset used more than once is
counted once. Hover the title to see the full composition.

## Find canvases and images

The **Canvas** and **Data** tabs share the search text, tag filter and import
batch filter. Canvas search also matches canvas names and image layer names,
including canvases without plotted datasets. A tag or data import filter only
includes canvases containing a matching dataset. Clear these filters to include
pure image pages again.

Turn on **View → Spotlight Filter Matches** to reduce canvases that don't match
to their names and outlines. **Show Matches** fits the board view to matching
canvases. While spotlight is on, **Show All** also includes visible table sheets.
Turn off spotlight or clear the filters to restore full content. Spotlight
leaves canvas positions unchanged and isn't saved in the project.

Choose **Compact rows** for a text-only canvas list. Layers have their own
scrolling area below the list. Selecting a result locates its existing canvas;
it does not create a duplicate. Shift-selection and keyboard navigation follow
the filtered rows.

## Board views

In **Canvas** mode, the lower part of the side bar holds named **Board
views** — saved framings of the board you can return to with one click. They
are hidden while browsing data.

## Saving projects

`Ctrl` + `S` opens the project save options. A `.plotium` project file stores the
whole session — imported data, processing, analysis results, and layout — so
it reopens exactly where you left off and can be shared as a single file. See
[Plotium files](/reference/file-formats/) for what the file contains.

Saving runs in the background. If you continue editing while it runs, the
completed file contains the revision captured when saving began and Plotium
keeps the project marked as unsaved for the newer edits.

Plotium also writes an internal crash-recovery checkpoint after edits settle,
with a maximum one-minute recovery interval during continuous work. A
checkpoint is written only when the document has changed since the previous
one; it is cleared after a successful up-to-date save or a clean exit.

**Preferences → General → Project backup copies** keeps a chosen number of
complete previous saves as hidden files beside the project, so an accidental
overwrite is recoverable.

**File → New Project**, **Close Project**, opening another project, and
quitting all use the same **Save / Discard / Cancel** check when the current
project has unsaved changes.

Use the welcome page to open data, import a table, or open a saved project.
**Close Project** returns to this page.
