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
the existing controls. Basic processing does not establish that the spectrum has
an experimentally verified absorptive phase. Use the per-axis phase controls to
adjust zero-order phase, first-order phase and pivot; apply explicitly and cancel
unwanted processing. Complex components remain available for phase adjustment.

Save a normal `.plotium` project to retain the acquisition snapshot, processing
parameters and current plane. The project can reopen without the original
experiment directory. The first implementation targets dense 3D acquisitions;
it does not add 3D NUS reconstruction, 4D browsing or protein assignments.

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
