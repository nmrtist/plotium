---
title: Working with an AI client
description: What an AI client can do in Plotium, how to ask, and how to preview, apply and check its work.
---

Once [connected](/ai/connect/), an AI client works on the project open in
Plotium. Everything it changes appears in the desktop app at once, as ordinary
Plotium content you can keep editing by hand.

## What it can do

| Area | Examples |
| --- | --- |
| Look | List datasets and figures, read table values, look at a figure as an image, check a figure for clipped or overlapping text, a legend covering data, unreadable axes or misaligned panels |
| Data | Import data files Plotium opens and CSV, TSV or XLSX tables, add calculated columns, filter and sort into a new table, correct cells and refresh tables derived from them |
| Plots | Create line, scatter, bar, box and violin plots from table columns, add or swap curves |
| Styling | Colours, line widths and styles, markers, axes, legends, font sizes, built-in themes and your saved canvas styles |
| Layout | Page size, panel positions and grids, panel labels, stacked plots with a shared x axis, text, images, annotations, arrows and reference lines, deleting chosen items |
| Analysis | NMR processing steps and recipes, diffusion inputs, region fitting and DOSY maps |
| Files | Save the project and export figures, after you confirm in Plotium |

General curve fitting, statistics and Preferences are not
available to an AI client; use Plotium's interface for those. Each task page in
this manual ends with a **With an AI client** section where the AI can help,
for example [Legends and lines](/guides/legends-and-lines/) or
[Tables](/guides/tables/).

## How to ask

Plain language works. A few habits make the result predictable:

- **Name what to change.** “The left panel”, “the curve named Current” or “the
  figure on page 2” is clearer than “this”. If you have selected something in
  Plotium, the AI can see the selection and use it as “this”.
- **Say what to keep.** “Keep my axis ranges and colours” protects choices the
  AI cannot guess. Values you set by hand in Plotium, including a legend you
  dragged, are kept anyway unless you ask for them to change; see
  [Place a legend by hand](/guides/legends-and-lines/#place-a-legend-by-hand).
- **Give files as absolute paths and state units.** “Import
  `/Users/me/data/run3.csv`, time in s, current in mA.” Plotium never guesses
  units.
- **Ask to look first.** “Do not change anything” keeps a request read-only;
  switching access to **Read only** guarantees it.

Before creating a plot the AI asks what the plot should show and the one point
it should support, and for error bars, what they represent. See
[Plots and charts](/guides/plots-and-charts/).

## Preview, apply, check

For any change you care about, ask the AI to work in three steps:

1. **Preview.** The AI prepares the change and shows you an image of the
   result before anything changes in your project. For a style choice, ask for
   two or three options; it previews each and applies only the one you pick.
   Your AI client must support receiving images to show previews.
2. **Apply.** The change appears in Plotium as one Undo step.
3. **Check.** Ask it to look at the real figure again, and to run a figure
   check at the size you will publish, for example “At 180 × 90 mm, check for
   clipped text and legends covering data.” It reports problems; it does not
   fix them on its own, and it should not shrink text or hide data just to
   clear a warning.

A request that combines the steps might read:

> Look at this figure and suggest the smallest fix for the overlapping labels.
> Show me a preview of each change before you apply it, then check the real
> figure again. Keep my data, units and axis ranges.

If you edit the figure while the AI is preparing a change, it has to prepare
the change again. Previewing a prepared change needs **Read and modify** access but does not
change the project; looking at the current figure works in **Read only**.

## Save and export

The AI can save the project and export figures, but Plotium asks you first:

> Save this project to `/Users/me/figures/fig2.plotium`. Then export this
> canvas at 180 × 90 mm to `/Users/me/figures/fig2.svg`, a matching PDF and a
> 300 DPI PNG. Do not overwrite existing files.

Plotium shows **Allow AI to save this project?** or **Allow AI to export this
figure?** with the exact paths. Replacing an existing file needs its own
checkbox. Ask the AI to confirm that every file finished and to open the
results; a request that is still running is not finished. Saving and exporting
cannot be undone. See [Access, review and undo](/ai/control-and-review/#save-and-export-confirmation).

A good habit before you send a figure off is to save, reopen the `.plotium`
file in Plotium and check that curves, text and images are still editable.

## Build your own client

To call Plotium's tools from your own program, see the
[MCP reference](/reference/mcp/).
