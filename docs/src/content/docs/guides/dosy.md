---
title: DOSY diffusion analysis
description: Confirm diffusion encoding, fit region decays, and build DOSY maps in Plotium.
---

DOSY analysis relates the attenuation of a spectrum across acquired increments
to diffusion encoding, expressed as b values. Plotium uses the same resolved
b values for region curves, per-column fitting, and inverse Laplace transform
(ILT) maps.

Choose the result you need:

| Goal | Workflow | Result |
| --- | --- | --- |
| Measure diffusion for selected peaks | Extract region curves, then fit **Diffusion (b values)** | A diffusion coefficient D for each fitted curve, with fit diagnostics |
| Survey diffusion across the spectrum | **Build DOSY map** | One fitted D per accepted chemical-shift column |
| Estimate a distribution of diffusion coefficients at each chemical shift | **Build ILT DOSY map** | A nonnegative distribution on a chosen D grid |

## Prepare the spectrum stack

1. Import the pseudo-2D dataset and finish spectral processing. Map building
   requires a stack of frequency-domain spectra calibrated in ppm.
2. Select the dataset and choose **Analyze → DOSY…**. The dedicated **DOSY**
   task card is available for spectrum stacks even when acquisition parameters
   are missing.
3. Review the input source and b-value range shown in the card. Resolve any
   missing or unconfirmed input before fitting or building a map.

For showing, comparing, and aligning increments on the canvas, see
[Pseudo-2D analysis](/guides/pseudo-2d/). Canvas trace shifts affect the plot
only; they do not correct the processed spectra used for DOSY or ILT maps.

## Confirm diffusion inputs

When Plotium can verify the acquisition evidence, it offers an acquisition
suggestion automatically. Otherwise, use **Sequence model** or **Direct b
values**, then choose **Confirm and apply analysis input**. Map building stays
disabled until the input and processed stack are valid.

### Sequence model

Use this mode for supported rectangular-gradient **PGSE / STE** or **JEOL
BPP-DSTE** experiments. Confirm the actual pulse sequence and enter its
parameters using these units:

| Quantity | Unit or convention |
| --- | --- |
| Gyromagnetic ratio γ | rad/(s·T) |
| Durations δ, Δ, τ / τ₁, τ₂ | Seconds |
| Imported gradient strengths | T/m; `0.02–0.28 T/m` means `20–280 mT/m` |
| JEOL BPP-DSTE δ | Total encoding width; each gradient lobe lasts δ/2 |

Blank intervals remain missing. Enter zero explicitly when an interval is zero.
Sequence mode needs an imported gradient axis in T/m. For other sequences or
gradient shapes, supply b values calculated for the actual experiment.

### Direct b values

Choose **Direct b values** to paste numbers or use **Import b values…** for a
text, CSV, or TSV file containing the numbers. Separate values with spaces,
commas, semicolons, or newlines; omit headers and other columns.

- Provide one finite, nonnegative b value for every acquired increment, with
  at least three distinct values.
- Select the supplied unit: `s/m²`, `s/cm²`, or `s/mm²`. Plotium converts to
  `s/m²` for analysis.
- Keep acquisition order. Sorting or hiding traces on the canvas does not
  change which increment a b value belongs to.

Direct b input does not require gradient metadata. **Use acquisition
suggestion** clears the saved manual override; if no verified suggestion is
available, you must provide valid input again.

## Fit selected peak regions

1. Display the **Stack**, then choose **Analyze → Draw Regions** and draw
   regions over the peaks of interest.
2. In the Regions task card, choose **View extracted curves**. Each region
   becomes a point series with b on the x axis; an auxiliary gradient column
   is retained when available.
3. In **Curve Fit**, check that **Diffusion (b values)** is selected and choose
   **Run Fit**. The model fits `I = I₀ exp(−Db)`, with D in `m²/s`.
4. Inspect the fitted curve, parameter uncertainty, and diagnostics. Use
   **View data** to review the synchronized table or **Back to regions** to
   adjust the source regions.

Region curves use signed, phased real-part intensities. Full-spectrum maps
use the magnitude at each spectral column, so the two workflows need not give
identical results. Solver, weighting, and diagnostic options are described in
[Curve fitting](/guides/curve-fitting/).

## Build a full-spectrum map

Choose **Build DOSY map** for a per-column single-exponential fit, or **Build
ILT DOSY map** for a diffusion distribution. Builds run in the background;
**Cancel DOSY computation** discards a pending build. You cannot start another
map build for the same dataset until the active one finishes or is cancelled.

Under **Show**, choose **Stack** or **DOSY map**. If the selected map is
unavailable, Plotium displays the stack instead. The map axes are chemical
shift and diffusion coefficient D (`m²/s`).

### Choose a method

The per-column method assigns one D to each accepted column. Use it to inspect
single-exponential behavior across the spectrum. Weak columns and columns
without an acceptable fit are omitted; a missing feature is not a measured
zero diffusion coefficient.

ILT estimates a nonnegative distribution over D at each column. Use it when
you want to examine more than one diffusion contribution at a chemical shift.
Interpret the distribution alongside the region decay fits and its sensitivity
to the settings below.

### ILT settings

Expand **ILT settings** in the **DOSY** card before building:

| Setting | Meaning |
| --- | --- |
| λ | Weight of the penalty on distribution amplitudes. Larger values penalize amplitudes more strongly; smaller values give the measured decay more influence. Allowed range: `0.000001` to `1000`. |
| D min / D max | Positive lower and upper bounds of the diffusion grid, in `m²/s`. Choose a range containing the diffusion coefficients you want to examine. |
| Grid points | Number of logarithmically spaced D values, from 16 to 512. More points sample the distribution more finely and increase computation. |

A finer grid alone does not establish that two components are resolved. Check
whether features persist when you change λ and the D range. Signals collected
at the grid boundary warrant reviewing the range before interpreting them.

The next build uses settings explicitly entered for this dataset, then its
previous ILT result's settings. Otherwise, λ comes from **Preferences →
Processing** (default `0.01`) and the grid uses built-in defaults.

## Change inputs and save results

Manual analysis inputs do not alter the imported acquisition facts. Changing
them is undoable, cancels active map builds, and invalidates old maps and
linked region fits. Frozen tables retain their captured input snapshot.
Changing spectral processing also cancels and invalidates maps. Finish those
changes, then rebuild the result.

Use **Map method → Per-column / ILT** to switch between stored results without
rebuilding. A method becomes selectable after its map has been built.

The `.plotium` project saves both kinds of map, their analysis inputs and ILT
settings, and the display selection. Reopening normally restores the result
without rebuilding. If a saved map no longer matches reconstructed data,
Plotium keeps it visible with a warning in the **DOSY** card and Operation
history. If the saved map cannot be read, the stack is shown instead.
Rebuilding clears the warning.

## Encoding verification and reproducibility

For recognized JEOL Delta BPP-DSTE programs with rectangular gradients,
Plotium checks the complete pulse section, parameter units, and timing
relations. It calculates `C = Δ − 2δ/3 − (τ₁ + τ₂)/2` and
`b = (γδg)² C`, where δ is the total encoding width. The vendor-program
`st_coef` is retained as evidence and checked with a rounding tolerance.
Consistent evidence is accepted automatically; modified or incomplete evidence
requires explicit model confirmation or direct b values.

Results record the analysis input, model and rule version, acquisition
evidence, final b values, and data fingerprints. Input resolution prefers an
explicit invocation input, then the selected result's recorded input, saved
settings, and finally a verified acquisition suggestion. This preserves the
input used for a result rather than silently substituting a later default.
