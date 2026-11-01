---
title: Contour levels
description: Set the lowest contour level, the ladder above it, and the colours of a 2D contour plot.
---

A contour plot draws a ladder of levels: a lowest level, then each next level a
fixed ratio above the one before. Plotium edits that ladder for any 2D scalar
data — a phase-sensitive NMR plane, a magnitude plane, an AFM height map — from
one place.

Select the plot on the page. The **Contour** section appears in the Object
inspector at the top of the Secondary Side Bar, and only when something in the
selection draws a contour. Its header counts the series you are about to edit —
*3 series* — so a change you meant for one plot never lands on three unnoticed.
Select several plots and the section edits them together; select nothing and it
follows the page's active plot.

## Lowest level

**Lowest level** and **Line width** are the two rows shown before you open
**Advanced**. Lowest level decides what you see: everything below it is not
drawn at all. Raise it to suppress noise, lower it to reveal weak cross-peaks.

What the number means depends on the anchor (below). The default for a
phase-sensitive NMR plane is a background-adjusted noise floor, with a multiplier
of `5`. The row shows the unit and the resolved intensity beside the multiplier:
`5` `× noise floor` `= 1.2e4`. The resolved intensity includes baseline protection
and can exceed five times the estimated noise standard deviation, σ.

The noise scale is measured away from the interface. Until it arrives, the
resolved level reads `measuring…`. Spatial background checks do not prevent
Plotium from drawing the spectrum.

### The noise floor

Plotium measures the amplitudes of all finite samples in the final
phase-sensitive real spectrum. The noise scale is the median absolute deviation
from the spectrum's median, divided by `0.67449`. Measuring amplitudes avoids
assuming that adjacent points are independent: zero filling and windowing can
correlate noise and make point-to-point differences misleadingly small.

The threshold is `|median| + k × max(σ, f × P)`, where `k` is the multiplier,
`P` the largest absolute intensity, and `f` an explicit peak-fraction floor
(zero by default). The absolute median protects against an offset baseline;
it does not subtract a baseline or modify the spectrum. Both signs use the
same threshold magnitude.

An isolated strong peak does not automatically raise the default threshold
elsewhere. An explicitly stored nonzero peak-fraction floor is still honoured.
There is no additional background-tail threshold.

This is an initial display level, not a separation of signal from noise or a
false-peak probability. Dense or broad signals can raise the measured scale;
local solvent ridges, t₁ stripes and varying baselines can still be visible.
Adjust **Lowest level**, or choose **Absolute level** to set an intensity
directly, when the initial display does not suit the spectrum.

The amplitude noise anchor applies to phase-sensitive real spectra, not
magnitude spectra. A constant plane has no contour crossings; finite small
planes can still be measured.

If you set an **Absolute level** the data never reaches, that half draws nothing
and the Operation history reports both numbers — for example, *The positive contour
threshold 20000 is above this field's positive peak 1800, so no positive
contours are drawn. Lower the threshold below 1800.* A slipped decimal point is
visible at a glance that way.

### From the plot

Press `+` and `-` to raise and lower the lowest level of the selected plot — or
of the page's active plot when nothing is selected — without leaving the canvas.
On keyboards where `+` needs Shift, `=` raises as well. Each press multiplies or
divides the anchor setting by that plot's **Level ratio**. For an absolute level
this moves the lowest level by one ladder rung. For a noise or background
multiple it changes the multiplier; the baseline allowance stays fixed, so the
resolved intensity need not move by exactly one rung. The adjustment follows
the data's intensity scale without requiring a different step size per spectrum.

Hovering the plot body and holding `Alt` while you roll the mouse wheel does the
same thing without the keyboard. One wheel notch makes one adjustment. The contours
already on screen stay there while the new ones are computed, so sweeping
through several rungs never leaves you reading a half-drawn ladder.

A hint in the top-left corner of the hovered plot names the setting `Alt` +
scroll wheel will change and how many series it will change — *Alt+scroll:
Lowest level (3 series)*. If the plot also draws something else with a display
setting of its own, such as a heatmap under the contours, `Alt` + scroll wheel
does nothing rather than guessing which layer you meant; change the layer you
want from the Object inspector.

Whenever the keys apply, the current lowest level is shown in the top-right
corner of the plot, resolved the same way as in the panel. A
plot whose contour series do not all sit at the same level says so instead of
showing one of them. Stepping is an
ordinary edit: it can be undone, and a step past the highest value the current
anchor allows is refused, with the reason in the Operation history.

## Line width

**Line width** sets how heavy contour strokes are printed, in points. Contours
start at 0.5 pt; drag or type any value from 0.05 to 10 pt, or take one from
**Presets** — *Fine* 0.50 pt, *Medium* 0.75 pt, *Bold* 1.25 pt. A thin stroke is
what keeps a dense ladder legible at journal size. It does not change which
features are drawn — that is the lowest level and **Levels** — only how heavy
each one looks.

## Anchor and ladder

Open **Advanced** for the rest of the ladder.

- **Anchor** — what the lowest level is measured against. The choices offered
  depend on what the data can supply:
  - *Multiple of the noise floor* — needs data whose noise level can be
    estimated, such as a phase-sensitive NMR plane. This is the default there.
    See [The noise floor](#the-noise-floor) for what the floor is.
  - *Background + multiple of spread* — needs data with a measurable background
    level and spread, such as an AFM height map, where the background is not
    centred on zero.
  - *Fraction of value range* — needs single-signed data, such as a magnitude
    plane. It is not offered for data that has both positive and negative
    values, where a fraction of the range is not a meaningful threshold.
  - *Absolute level* — a raw intensity, always available.

  The anchor also sets what **Lowest level** accepts: a multiple of the noise
  floor or of the spread runs up to 10,000, a fraction of the range is between 0
  and 1, and an absolute level is any intensity above 0.
- **Levels** — how many levels the ladder has, from 1 to 256.
- **Level ratio** — the factor between one level and the next. It must be
  greater than 1, and at most 10.

The default ladder has **14 levels** and a **1.35 ratio**. Geometric spacing
keeps weak-peak detail near the lowest level while covering stronger peaks
without equally spaced rings crowding the bottom. Fourteen levels span about
50 times the lowest intensity; increase **Levels** when you also need to show
higher parts of a spectrum with a larger dynamic range. Changing the ratio
does not repair an unreliable noise estimate.

**Negative contours**, **Positive colour** and **Negative colour** are in the
same section.

The ladder stops early when the next level would be above the data's peak, so a
plot can show fewer levels than **Levels** asks for. That is not an error: the
remaining levels would draw nothing.

Raise the lowest level past one sign's peak and that half stops drawing
altogether. It is never quietly redrawn at some lower ladder instead, so
raising the level can only ever remove contours, never add them.

A ladder can also stop early at the bottom. A level that falls inside the noise
crosses most of the grid, and there is a limit to how much line one plot can
draw. Past that limit Plotium drops the remaining levels whole — never cutting a
contour off part-way along its own path — and drops the same levels from both
signs, so what you see is a complete ladder with a higher floor. The Operation history
says how many went and what to set instead, for example: *The lowest 14 contour
levels were not drawn: at 5.052e4 and below, this field crosses more of the grid
than one plot can render. Raise the lowest level to 6.820e4 or above to see
every level the panel lists.*

Changing the anchor, the lowest level, the count or the ratio applies to both
signs — the negative half mirrors the positive ladder and applies its own sign. Both halves remain centred on zero, with the same
conservative threshold magnitude. A baseline allowance can therefore suppress
more of the cleaner half; Plotium does not silently recenter the data.

## While it is being drawn

Contour geometry, and the noise or background estimate a ladder is anchored to,
are computed away from the interface so a large plane never freezes the
application. Until they land the plot is empty, and the Operation history says which
step is running — *Measuring this field's noise scale…*, then *Building contour
geometry…* — so a plot that is merely slow is never mistaken for one that has
failed. Large planes take a moment; the plot fills in by itself. The work is
shared: two plots drawn from the same data at the same levels wait on one
computation, not two.

## No single value

The anchor, the lowest level, the count and the ratio are shared: by the
positive and negative halves of a ladder, and by every series you have selected.
When they do not all agree, the control shows an em dash and the word *mixed*
instead of a number or a choice, so no one value is presented as the setting.
The resolved level beside the control disappears with it: there is no single
lowest level to resolve. Hover *mixed* to see which case it is — one series
whose two halves differ, or several series that differ from each other.

Setting the row applies your value to every selected series and to both halves
of each ladder, which is what makes them agree again.

## Negative contours

**Negative contours** is offered only for data that has both signs, such as a
phase-sensitive NMR plane. Single-signed data — a magnitude plane, a height map
— has no negative half to draw.

Positive and negative contours have separate colours, so the sign information of
a phase-sensitive spectrum survives in the figure.

## Changed values and reset

A setting that differs from what Plotium would choose for this data is marked with
a dot. Hover the dot to see the default; use the reset button next to it to go
back. Reset re-derives the default from the current data rather than restoring
an older value, so it stays correct after reprocessing. A row with no single
value carries the same marker.

Resetting the lowest level re-derives it under the anchor currently in force, not
under the one Plotium would have chosen: reset it while anchored to a fraction of
the range and you get a fraction, not a noise multiple.

**Reset contour** rebuilds the whole contour — anchor, ladder and colours — from
the defaults for this data. It touches only the series drawn as contours;
anything else in the same plot, such as a heatmap underneath, is left as it is
and reported as skipped in the Operation history.

## Finding a setting

Four routes reach a contour setting, and they all end at the same control:

- <kbd>Ctrl</kbd>+<kbd>K</kbd> (<kbd>Cmd</kbd> on macOS) searches settings as
  well as commands and data. Typing `contour threshold`, `sigma` or `levels`
  finds the matching row, opens the panel it lives in, expands its section and
  highlights it. See [Command palette](/reference/command-palette/).
- **Contour** in the **Style** group of the Ribbon's **Figure** tab jumps to the
  same place.
- Right-click the plot and choose **Contour settings…**.
- `+` and `-`, or `Alt` + scroll wheel over the plot body, change the lowest level
  directly on the plot.
