---
title: Combining and aligning spectra
description: Spectrum arithmetic and multi-spectrum alignment.
---

Two tools operate across spectra rather than on one: **Spectrum arithmetic**
builds a new dataset from existing traces, and **Align spectra** shifts a
whole series so a reference peak lines up. Both are opened from the Analysis
panel of a 1D spectrum, from **Process › Transform** in the Ribbon, or from the
command palette (<kbd>Ctrl</kbd>+<kbd>K</kbd>).

## Spectrum arithmetic

**Spectrum arithmetic** combines 1D NMR spectra, XRD patterns, XPS regions or
plotted table curves into a new, independent dataset — the source datasets are
never modified.

Five operations are available:

- **A + k·B** and **A − k·B** — add or subtract a second spectrum,
  scaled by a coefficient *k* (default 1). Tuning *k* is how you null a
  solvent line in a subtraction, balance a difference spectrum, or remove an
  empty-cell or substrate pattern.
- **A × k** — multiply a spectrum by a constant.
- **A + c** — add a constant offset.
- **Average** — merge repeat scans of XRD patterns, XPS regions or table curves.
  The result uses A's axis, and each point averages the traces that cover it.

Operands must be the same kind of dataset and their x ranges must overlap. XRD
and XPS use the processed trace on screen: for XPS the active region, for a table
the first plotted curve. XRD patterns must not declare different wavelengths, XPS
regions must share an energy scale, table axes and values must share units, and
table rows with blank cells are skipped. Negative XRD results are set to zero,
and the status bar reports how many points changed.

Two NMR spectra must share the same nucleus; mixed-nucleus pairs are
rejected with the reason shown in the dialog. If the axes differ
(different point counts or ranges), the second trace is linearly
interpolated onto the first one's axis, and points outside the overlap
are treated as zero — the dialog notes when this will happen.

The result is an independent dataset of the same kind placed on its own page,
named after the expression (for example `A − 0.9·B`). It behaves like
any other dataset of that kind — for an NMR spectrum, peaks, integrals, regions,
and export all work — and creating it is a single undoable step.

## Aligning spectra

**Align spectra** shifts a series of 1D spectra so a shared reference
peak lands on one chemical-shift position — useful for reaction
monitoring, titrations, and other series where a reference line drifts
between acquisitions.

The dialog works on your Data-list multi-selection (or every dataset
when fewer than two are selected):

- Pick a **ppm window**; it defaults to the current view of the active
  spectrum. Each spectrum's tallest significant peak inside the window
  becomes its reference peak.
- Pick the **target**: the reference spectrum's own peak position (the
  active dataset leads), or a ppm value you type.
- A preview table lists every dataset with the peak found and the shift
  that will be applied. Spectra with no significant peak in the window,
  a different nucleus, or a non-1D type are skipped, with the reason
  shown.

The shift is written through each spectrum's **Reference** step — the
same chemical-shift referencing used for a single spectrum — so it stays
visible and editable in the pipeline afterwards. Applying the alignment
is a single undoable step across all datasets.
