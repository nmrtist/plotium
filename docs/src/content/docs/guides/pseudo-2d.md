---
title: Pseudo-2D analysis
description: Work with spectrum stacks, extract region curves, and fit T1/T2 relaxation.
---

A pseudo-2D dataset is a stack of 1D spectra acquired while one parameter
varies. Plotium reads that varying parameter from the acquisition parameters on
import: a gradient-strength series marks the dataset as DOSY, a delay series
as relaxation (T1 or T2 — which one is your choice of fit model).

For diffusion inputs, region fitting, and diffusion maps, see
[DOSY diffusion analysis](/guides/dosy/).

## Display and compare increments

The initial stack contains one plot series per increment. Select the plot and
use **Data** in the Object inspector to show or hide individual increments.
Use **Choose trace…** to replace a series with another increment from the same
dataset. **Add series…** adds every increment from a compatible dataset. Use
**Show all**, **Hide all**, the row checkboxes, or remove buttons to keep the
exact values you want to compare. To identify them on the plot, set
**Visibility** to **Show** under **Legend & scales**. The legend labels each
increment by its gradient strength, relaxation delay, or imported pseudo-axis
value and display unit.

To compare increments from multiple compatible datasets, select the datasets
in the Data browser and choose **Stack selected data**. The trace composer
starts with every increment included. Search by dataset, increment label, or
parameter value and adjust the row checkboxes, or use the global and filtered
selection controls. **Create stack** creates one plot from the chosen
increments; **Cancel** leaves the project unchanged. This remains available
while a dataset displays a DOSY map because the composer selects stable stack
increments rather than copying data from the displayed map.

To align peaks in a stack on the canvas, select the plot and choose **Align
Traces…** in the **Align** group on the **Analyze** tab. You can also choose
**Align traces…** under **Data** in the Object inspector. Choose a reference
increment and a peak window, then review the proposed shifts before selecting
**Apply**. Hidden increments and increments without a usable peak are listed as
**Skipped** and remain unchanged. **Trace start** is also available when you
need to align the first finite plotted sample instead of a peak.

For a manual correction, edit **X shift (ppm)** on the increment's line row.
These undoable shifts belong to the plot, so the processed spectra and DOSY or
ILT maps remain unchanged.

Scroll over the stack to change the height of all traces by the same display
factor. Every zero-signal baseline and the spacing between baselines stays
fixed; negative peaks scale about their own baseline too. This preserves DOSY
attenuation ratios and leaves processing, integrations and fitting unchanged.
Use the X-axis strip or box zoom to inspect a chemical-shift range, and adjust
**Vertical spacing** separately when arranging the stack. **View → Reset Peak
Height** restores ×1 without resetting the axis ranges.

## Workflow

1. Import the pseudo-2D dataset.
2. On the **Analyze** tab, choose **Draw Regions** in the **Regions** group,
   then draw over the peaks of interest.
3. Choose **View extracted curves** in the Regions task card. Plotium selects and
   zooms to the new scatter plot, with one point series per region.
4. The **Curve Fit** task card opens beside the result. Check the suggested
   model, choose whether to fit every curve or one selected curve, then choose
   **Run Fit**. Use **View data** for the synchronized read-only values or
   **Back to regions** to return to the source.

## Fit models

- Mono-exponential decay
- Inversion recovery (T1), `a + b·exp(−x/T)`
- Saturation recovery
- Diffusion decay — see [DOSY diffusion analysis](/guides/dosy/)
- Bi-exponential and stretched-exponential
- Linear

## Notes on intensities

Extracted intensities are signed (phased real-part projections), not
magnitudes, so inversion-recovery data fits correctly without folding.
