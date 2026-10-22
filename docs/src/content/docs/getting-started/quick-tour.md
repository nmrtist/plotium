---
title: Quick tour
description: A five-minute walkthrough of the Plotium interface.
---

This page walks through the main parts of the Plotium window and the typical
flow from raw data to a finished figure.

## The window at a glance

- **Primary Side Bar** (left) — datasets and project structure.
- **Canvas** (center) — an infinite board holding your plots and data sheets.
  Plot pages and table sheets are placed without overlap. Drag their headers to
  move them freely; near another frame, they snap to its edges and the standard
  gap between frames.
- **Secondary Side Bar** (right) — the selected object's inspector and
  contextual analysis tools such as peaks and regions.
- **Task dock** (upper right of the canvas) — a card holding the multi-step
  tasks: Processing, Regions, Curve Fit, and Statistics. Open two or more and
  they become tabs on the same card, one page shown at a time.

Hide either side bar to give the canvas more room: click its layout button at
the right end of the Ribbon's task row, press <kbd>Ctrl</kbd>+<kbd>B</kbd>
(left) or <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>B</kbd> (right;
<kbd>Cmd</kbd> on macOS), or drag the side bar's inner edge toward the nearest
window edge. When **Release to hide sidebar** appears, release to hide it.
You can make the side bar as narrow as possible without hiding it; just stop
before reaching the window edge. To cancel hiding, drag back before releasing.
The same commands live in the **View** menu and the **View** Ribbon.

## Menus and task Ribbon

On Windows and Linux, the title bar holds the app logo, the **File**, **Edit**,
**View**, **Insert**, and **Help** menus, and the window controls in one row.
Drag its empty area to move the window, or double-click to maximize. On macOS
these commands use the system menu bar, including the standard Plotium
application and Window menus. The native traffic-light controls share the top
row with the Ribbon task tabs and project name, leaving more height for the
workspace.

On macOS, drag the project name or an empty part of the top row to move the
window. Double-click either area to maximize the window; double-click again to
restore it. The project name is window chrome, so it does not select text.

In macOS full screen, the task tabs move left into the space vacated by the
hidden traffic lights. Move the pointer to the top of the screen to reveal the
native window controls; leaving full screen restores the tabs' original spacing.

On macOS, the red close button and `Cmd+W` close only the window. Plotium
keeps running with your workspace in memory; click its Dock icon to reopen it.
Use **Plotium → Quit Plotium**, `Cmd+Q`, or **Quit** from the Dock icon's menu
to exit. Quitting asks you to save any unsaved project changes.
**File → Close Project** closes the project instead of the window.

**File** keeps an **Open Recent** submenu with the files, folders, and projects
you opened or saved most recently; the same entries are listed on the welcome
screen while no data is loaded. **Help** contains **User Manual**, which opens
this documentation in your browser.

The Ribbon is a focused shortcut surface, not a second complete menu.
Choose **Data**, **Process**, **Analyze**, **Figure**, **Arrange**, or **View** to see
grouped frequent commands for that stage. Use **Collapse ribbon** to collapse it to the task tabs.
When a tab's tiles no longer fit, groups shrink one at a time, lowest priority
first: to two rows of icon-and-label buttons, then to icon-only rows, then to a
single button that opens the group. Every group keeps its place; the command
area never hides on its own — only **Collapse ribbon** puts it away.

**Search commands** opens the command palette. Menu items, Ribbon buttons,
shortcuts, and palette rows share the same enabled and selected states.

The **Process** tab opens Processing for the active spectrum in the task dock.
The **Analyze** tab reveals the applicable tool group in the Secondary Side
Bar, but never reopens a side bar you have hidden.

## Data browser

The Primary Side Bar has two modes: **Canvas** lists your plots and pages, and
**Data** shows every dataset with the results derived from it. Click a dataset
to focus it; double-click to open its data sheet. See
[Organizing data](/guides/organizing-data/) for the full tour of the data tree,
multi-selection, and saved board views.

## A typical session

1. **Import** a dataset by dragging a file onto the window, using **File**, or
   choosing an import command on the **Data** Ribbon.
2. **Process** it from the **Process** Ribbon tab — the pipeline applies steps
   in order and previews the result live.
3. **Analyze** with the peak and region tools; region measurements appear in a
   plot with a synchronized data table.
4. **Arrange** plots on the board and **export** the figure.

[Your first figure](/getting-started/first-figure/) walks through these four
steps with a real dataset.

## Navigation

Pan and zoom are always available, in any tool:

- **Scroll wheel** — zoom the x axis of a 1D plot, or both axes of a 2D plot.
- **Two-finger swipe on a macOS trackpad** — pan the board. Click a plot to
  select it, then swipe over its data area to pan its data. Hold **Cmd** to
  zoom the board instead.
- **Pinch** — zoom the board; over the selected plot's data area, zoom both of
  its axes.
- **Alt + scroll wheel** — change what the plot shows rather than where you are
  looking: the y intensity of a 1D plot, the lowest contour level, or a
  heatmap's colour range. Hovering the plot names the setting it will change.
- **Alt + drag** — rubber-band a box to zoom into, in any tool.
- **Middle-drag** or **Space + drag** — pan.
- **Double-click** — auto-range the axes.
- **Alt + drag a page or table-sheet header** — move that frame without
  snapping for the current drag.
