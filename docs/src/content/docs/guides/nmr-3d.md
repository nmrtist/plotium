---
title: Three-dimensional NMR spectra
description: Open, process and navigate a three-dimensional NMR acquisition.
---

Open a supported dense Bruker 3D experiment directory with the normal file-open
or drop action. Plotium processes the raw acquisition in the background and
shows a two-dimensional frequency plane in the ordinary workspace. Opening an
explicit processed file respects that selection; an extracted 2D spectrum remains
2D and does not stand in for a complete 3D acquisition.

Use the plane controls beside the spectrum to choose the fixed acquisition axis,
step through planes, or enter a chemical shift in ppm. Nuclear labels include the
acquisition dimension to distinguish experiments with two proton axes. A ppm
entry selects the nearest sampled plane and displays its actual coordinate.
Changing the plane reuses the processed volume. Existing zoom and contour settings
remain stable while browsing. Changing orientation retains the position on each
acquisition axis.

Positive and negative contours retain their sign. Adjust contour sensitivity with
the existing controls. Complex components remain available for
[phase correction](#phase-correction).

Save a normal `.plotium` project to retain the acquisition snapshot, processing
parameters and current plane. The project can reopen without the original
experiment directory. The first implementation targets dense 3D acquisitions;
it does not add 3D NUS reconstruction or 4D browsing.

## Phase correction

Select the 3D spectrum and choose **Process → Correct → Phase** to open its
phase panel. Check the target spectrum name before editing. Each acquisition
axis (F1/F2/F3) shows its nucleus (or `unknown`) and its current display role:
horizontal, vertical, or fixed. Changing planes updates these roles; phase
settings remain attached to their acquisition axes.

Adjust **PH0** and **PH1** for the required axis, then click **Apply** and inspect
the result on the spectrum. Apply recalculates the whole volume from the source,
replacing the previous phase settings. Browse other planes to check the result.
The displayed 2D slice does not support dragging to adjust phase, and parameter
changes have no live preview.

**Pivot** sets the reference position for first-order phase in ppm. Moving it
automatically compensates PH0 so the phase correction remains unchanged.
The allowed range extends one sample interval beyond the last point; the control
is unavailable when the axis has no usable uniform ppm grid.

- **Discard changes** restores the applied settings without recalculating.
- **Stop calculation** cancels the named calculation and retains your edits.
- Closing the panel retains your edits and lets any calculation continue.

You can browse planes or switch spectra during a calculation; the result applies
to the spectrum named when it started. If the source data or applied phase
settings change, unapplied edits are discarded; reopen the panel to edit the
current settings.

## Automation

The CLI tool registry and MCP use the same core operations as the desktop.
Import through `data.import`. Inspect acquisition dimensions and phase settings
through `processing.inspect`. Select a plane through `nmr.volume.edit`, with
zero-based `fixed_axis` and `index`. Its optional `phases` array contains three
objects with `p0_degrees`, `p1_degrees` and `pivot_fraction` (0–1). Inspect the
operation schema before invocation. Use stable dataset IDs and current document
revisions. Subsequent data preview and rendering address the current plane.

Generic numerical table export is unavailable for 3D volumes because the current
2D table format cannot preserve the fixed coordinate and acquisition-axis
identities. Save the project to retain the data, or export the figure for display.

## 3D peak editing

Choose **Analyze → Peaks** or **Pick peaks** in the peak panel, then click the
spectrum to add a manual observation. Click a mark to select it, drag it to move
its two displayed coordinates, and press Delete to remove it. The peak panel also
provides a Delete button and editors for all three coordinates and a note.
Each drag or completed text edit creates one undo step.

Single-click a peak-table row to select it, locate its slice and centre the view
while retaining the zoom scale. Previous and Next step through the same records.
Column labels include acquisition dimension, nucleus and ppm, so two proton axes
remain distinct. Each peak retains its ID and three acquisition-axis coordinates
across orientation changes. Only marks on the current slice are shown.

These positions are manual observations, not fitted peak centres. Intensities are
signed nearest-grid sample heights, not integrated volumes. Peak edits support
undo and redo and are saved in `.plotium` projects. Phase changes retain peak
identities and positions and refresh their sampled signed intensities.

**Export peak table…** writes JSON with the dataset ID and name, acquisition
identity, source and processed-volume canonical SHA-256 digests, applied phase
settings, axis identities, coordinates, notes and signed heights. These fields
identify the source and processed state even outside the project.

Automation uses `nmr.peaks3d.list` for read-only inspection and JSON export, and
`nmr.peaks3d` for add/edit/delete/locate operations. Inspect the tool schemas for
parameters, stable peak IDs and acquisition-axis ordering.

MCP exposes the read-only table through `plotium_peaks3d_inspect`. Use
`plotium_plan` / `plotium_execute` with `nmr.peaks3d` for edits.
