---
title: Keyboard shortcuts
description: Keyboard and mouse shortcuts.
---

`Ctrl` means `Cmd` on macOS. Shortcuts are ignored while a text field has
keyboard focus, except for the native macOS Close Window and Quit commands.
After you click into a data sheet, it takes the board's keys: `Ctrl` + `C`
copies the selected cell, `Delete` or `Backspace` clears it, and `Ctrl` + `V`
pastes a single value into it. Undo, redo, `Ctrl` + `Shift` + `V` and the
command palette keep working. Click the board or press `Esc` to return
the keys to the board. See [Tables](/guides/tables/#editing).

## Tools

Single keys, no modifiers:

Press the same key again to leave a tool. Analysis tools return to **Zoom**;
drawing tools such as **Text** and **Rectangle** return to **Select**. Pressing
`V` or `Z` again leaves that neutral tool active. The cursor family is the
exception: repeated `C` presses advance to the next applicable cursor, and
`Esc` leaves the cursor.

| Key | Tool |
| --- | --- |
| `V` | Select |
| `Z` | Zoom (rubber-band box zoom) |
| `I` | Integrate (1D band or true-2D rectangle, according to the dataset) |
| `P` | Peaks |
| `C` | Cycle cursors: **Inspect**, **Delta**, then **Symmetry review** when the selected spectrum supports it |
| `S` | Slice |
| `D` | Peak Fit |
| `T` | Text |
| `R` | Rectangle |
| `O` | Ellipse |
| `L` | Line |

**Inspect** reports the coordinates and sampled intensity under the pointer;
click to pin it. **Delta** measures chemical-shift, frequency, and intensity
differences between two clicked positions. On an eligible homonuclear true-2D
spectrum, the third `C` press selects **Symmetry review**. On other spectra
that support cursors, `C` alternates between **Inspect** and **Delta**.

## Navigation

Pan and zoom are ambient — available in every tool. The scroll wheel acts on
the plot under the cursor, or on the board over empty space. Trackpad swipes
and pinches move the board; over the selected plot's body they move its data.

| Input | Action |
| --- | --- |
| Scroll wheel over a plot body | Adjust peak height on numeric 1D traces, keeping each baseline fixed; zoom both axes of a 2D plot. Other 1D charts zoom X |
| Two-finger swipe on a macOS trackpad | Pan the board; over the selected plot's body, pan its data |
| Pinch | Zoom the board; over the selected plot's body, zoom both of its axes |
| `Alt` + scroll wheel over a plot body | Adjust peak height on numeric 1D traces, the lowest contour level, or a heatmap's colour range |
| `Alt` + drag over a plot body | Rubber-band a box to zoom into, in any tool |
| Scroll wheel over an axis strip | Zoom that axis only |
| `Ctrl` + scroll wheel | Zoom the board instead of the plot |
| `Cmd` + two-finger swipe or pinch | Zoom the board, even over the selected plot |
| Middle-drag or `Space` + drag | Pan the plot (the board when over empty space or holding `Ctrl`) |
| Drag on an axis strip | Select a range on that axis to zoom into |
| Double-click a plot | Reset both axes to full range |
| Double-click an axis strip | Reset that axis only |
| `H` | Fit the y axis to the data visible in the current x window (the x window stays put) |
| `F` over a plot body | Fit both axes of that plot to the full data range |
| `F` elsewhere | Show Selection: fit the selected board items in the current selection scope (disabled without a visible selection) |
| `Enter` | Zoom the board to the selected page or sheet |

**Show All** fits every page and visible table sheet on the board, including
workspaces containing only tables. **Show Selection** fits the visible items
selected where you are working: frames on the board, pages in the canvas list,
or objects and panels in the canvas or layer list. In the data list, tables
fit their board sheets; tables without a sheet fit the first page displaying
them. Other datasets have no board target here. Without a visible target,
Show Selection is disabled. These commands move the board view; **Fit Plot to
Data** and **Fit Plot Vertically** adjust the data range inside a plot.

Both board fits follow their targets as they move or resize. Changing the
selection leaves the current fit in place; invoke Show Selection again to fit
the new selection. Hidden or deleted targets are skipped. When none remain,
selection fit ends at the current view, while Show All waits for new content.
Panning, zooming, or starting a direct drag ends either fit.

**Peak height** uses one display multiplier for the whole plot. Stacked spectra
keep their individual zero-signal baselines and spacing, including negative
peaks; relative attenuation and scientific results stay unchanged. The hover
hint shows the multiplier. Use **View → Reset Peak Height** (also in the command
palette) to restore ×1 without changing axis ranges. Double-clicking Y or the
plot, and the `H` / plot `F` fits, also reset the multiplier. Horizontal zoom
remains available on the X-axis strip and with box zoom. Numeric curves with
logarithmic Y, two Y axes, transposed coordinates or filled marks keep their
coordinate-navigation behavior.

`H` acts on the plot under the pointer, or on the active plot when the pointer
is elsewhere — the vertical-fit convention NMR software users expect. Both fits
are single undoable steps, and both are available from the command palette as
**Fit Plot Vertically** and **Fit Plot to Data**.

Hovering a plot body or an axis strip outlines the area the wheel will act on
and names the action in its top-left corner, including which setting `Alt` +
scroll wheel would change and on how many series. Where one plot draws two
things with display settings of their own — contours over a heatmap — `Alt` +
scroll wheel does nothing rather than guessing which you meant; change the
layer you want from the Object inspector. After a trackpad gesture the hint
describes swiping and pinching instead, and asks you to select the plot first.

When axis range zoom is available, hovering an axis strip shows a crosshair
and a “Drag to zoom X axis” or “Drag to zoom Y axis” hint. The crosshair and
selection band stay with the zoom gesture when the pointer leaves the strip.
Holding `Space` or dragging with the middle button uses the pan cursor instead.

## Selection and editing

| Input | Action |
| --- | --- |
| `Ctrl` + `N` | Start a new project |
| `Ctrl` + `W` | Close the current window on macOS; close the current project on Windows / Linux |
| `Cmd` + `Q` (macOS) | Quit Plotium, with a save prompt for unsaved changes |
| `Ctrl` + `S` | Open project save options |
| `Ctrl` + `Z` | Undo |
| `Ctrl` + `Shift` + `Z` or `Ctrl` + `Y` | Redo |
| `Ctrl` + `A` | Select every item in the current context: page objects, board frames, canvases, or datasets |
| `Ctrl` + `Shift` + `A` | Clear the selection in the current context |
| `Shift` + click | Select a continuous range in the Canvas, Layers, or Data list |
| `Ctrl` + click | Add or remove one item without clearing the rest |
| `Ctrl` + `Shift` + click | Add a continuous range to the existing list selection |
| `↑` / `↓`, `Home` / `End` | Move selection through the active Canvas, Layers, or Data list; hold `Shift` to extend |
| `Space` | Add or remove the lead item in the active list |
| `←` `→` `↑` `↓` | On the canvas, move the selected legend, label, arrow or reference line, otherwise the selected objects or panels, by 0.5 pt; hold `Shift` for 5 pt. Holding a key is one undo step |
| `Ctrl` + `G` | Group the selected objects |
| `Ctrl` + `Shift` + `G` | Ungroup |
| `Delete` or `Backspace` | In the Data list, delete the selected datasets that nothing uses; otherwise delete the selected plot annotation, the selected annotation objects, the selected panel and its contents, or the selected board pages; in Peaks or Integrate, delete the selected peak or region; in Symmetry review, delete the selected cross-peak mark |
| `+` (or `=`) / `-` | Raise / lower the lowest contour level of the selected plot |
| `F2` | Rename the selected dataset or canvas |
| `Esc` | Cancel the active drag; further presses clear the Analysis Range and selections one at a time (a selected legend or label first returns to its whole plot), then leave the active tool |
| `Ctrl` + `C` | Copy the single selected page frame (or the active canvas) to the clipboard as bitmap + vector |
| `Ctrl` + `Shift` + `V` | Paste a delimited table (comma, tab, or semicolon) from the clipboard as a new data table |
| `Ctrl` + `,` | Open Preferences |
| `Ctrl` + `B` | Show or hide the left Side Bar |
| `Ctrl` + `Shift` + `B` | Show or hide the right Side Bar |
| `Ctrl` + `K` or `Ctrl` + `Shift` + `P` | Open the [command palette](/reference/command-palette/) |

`+` and `-` act on the plot you have selected, or on the active plot when
nothing is selected, and only when it draws contours. Each press moves the
lowest level by one rung of that plot's own level ratio, so one press adds or
removes roughly one contour ring whatever the intensity scale. The current
lowest level is shown in the top-right corner of the plot while the keys apply.
See [Contour levels](/guides/contour-levels/). Holding `Ctrl` changes the UI
scale instead.

While editing a board note: `Enter` commits, `Shift` + `Enter` inserts a
newline, `Esc` cancels.

## UI scale

Plotium picks a legible UI scale for each display automatically (**UI scale**
under Preferences → Appearance); these shortcuts adjust it in 10% steps, and
the adjustment is remembered per display.

| Input | Action |
| --- | --- |
| `Ctrl` + `+` / `Ctrl` + `-` | Increase / decrease the UI scale on the current display |
| `Ctrl` + `0` | Reset the UI scale to automatic |

## Present mode

| Input | Action |
| --- | --- |
| `→` / `↓` / `Space` / `PageDown` | Next page |
| `←` / `↑` / `PageUp` | Previous page |
| `Home` / `End` | First / last page |
| `Esc` | Exit present mode |
