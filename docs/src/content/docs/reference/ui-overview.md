---
title: Interface glossary
description: The names this manual uses for the parts of the Plotium window.
---

This manual refers to the parts of the Plotium window by consistent names. If a
page says "open the Object inspector" and you are not sure where to look,
this glossary is the map. The [quick tour](/getting-started/quick-tour/)
introduces the same regions in walkthrough form.

## Window regions

- **Workspace overview** — when no canvas is selected, the central area lists
  the project's dataset and canvas counts. Click a canvas to open it, **View
  table** to open a table sheet, or **Select dataset** to select data in the
  Primary Side Bar. While data imports run, an importing indicator appears
  alongside existing content. An empty project offers data and project opening actions.
- **Messages** — the message banner shows queued imports, file
  discovery, progress, and success/failure counts. You can dismiss it; a completed
  import with no failures disappears automatically. **File → Import results** opens the
  retained batches, with links to available results and per-file errors. For
  background scientific-data batches, **Retry failed items** retries only the
  failed files. If discovery itself fails, open the source folder again.
  A single manual import into an empty project opens its result. A batch of spectra
  shows its successful plots together on the board, without opening a results window.
  Failed imports remain in the dismissible message banner; details remain available
  through **File → Import results**. Appending to an
  existing project preserves your view. Clicking, typing, or scrolling while
  waiting cancels automatic opening; hovering does not. Confirmed table imports
  follow the same single-result and existing-project behavior.
- **Primary Side Bar** — the left panel. Its **Canvas** mode lists plots,
  pages, and saved board views; its **Data** mode shows every dataset and the
  results derived from it. Each canvas row includes a live page thumbnail;
  click to select it, double-click its name to rename it, or use its context menu.
- **Canvas** / **the board** — the central area: an infinite board holding
  your plots and ordinary data sheets. "Page" is one framed area of the board
  that exports as one figure. Drag a page or table-sheet header to place it;
  near another frame, it snaps to that frame's edges and the standard gap.
  Hold <kbd>Alt</kbd> while dragging to bypass snapping.
- **Data sheet** — the panel docked below the board where a table is edited.
  The board shrinks to make room. Drag its top edge to resize it; its header
  collapses, maximizes, or closes it. See [Tables](/guides/tables/#editing).
- **Secondary Side Bar** — the right panel, holding the Object inspector and,
  under the **Dataset tools** heading, the contextual analysis workbenches for
  the selection (XPS, mass spectrometry, patch clamp, and the per-technique
  tool panels). It scrolls as one column, so a long inspector never pushes the
  tools below it out of reach. Processing is not here; it lives on the canvas.
- **Task dock** — the card at the upper right of the canvas that holds the
  multi-step tasks: Processing, Regions, Curve Fit, Statistics, and CRAFT.
  When two or more are open it grows tabs — **Process**, **Regions**, **Fit**,
  **Stats**, **CRAFT** —
  and shows one page at a time. Its Processing page lists one pipeline for a 1D
  or pseudo-2D dataset and two, **F2 (direct)** then **F1 (indirect)**, for a
  true 2D spectrum. See [Processing](/guides/processing/).
- **Ribbon** — the command strip organized into task tabs (**Data**,
  **Process**, **Analyze**, **Figure**, **Arrange**, **View**). On macOS its
  task row also holds the native window controls and project name. It is a
  shortcut surface: everything on it is also in the menus or command palette.
  See [the Ribbon](/reference/ribbon/) for every tab's groups.
- **Operation history** — the clock-arrow button in the Ribbon task row opens
  the current status and dataset summary. **Results** lists objects created or
  updated by background operations, with links to view them, and shows running
  scientific tasks. Repeated updates stay in the same result row; deleted results
  are marked unavailable. **Messages** keeps the latest 200
  observed status changes for this session; **Diagnostics** shows structured
  operation reports. **Clear history** clears results, messages, and diagnostics; running
  tasks continue. Hover the button for the current status and result count.
  Errors and warnings also appear in the feedback banner.

Scientific tasks in **Results** distinguish completion, failure, cancellation,
and results that were **not applied**. Editing the project while a computation
is running can prevent its result from being applied; the task explains the
reason. Where available, **Recompute** starts a new calculation using current
data. If it is unavailable, hover the button for the required next step.
Recomputing is always an explicit action. A completed calculation can still
carry scientific warnings, such as an unconverged fit or an empty result;
review those warnings before using the result.

AI results are followed by default: the workspace focuses on the canvas affected by
the latest completed operation. Results from one operation that affects multiple
canvases are shown together. The follow button beside the AI connection button shows whether
following is active, paused, or off. In **Results**, use **Pause**, **Resume**,
**Stop following**, or **View latest**. Updates to a canvas already in view preserve its zoom.
Opening or selecting other content, panning, zooming, or editing pauses following;
hovering and inspecting operation details do not. Resume following when you want
new results to appear again, or stop following to keep browsing independently.
While paused, a badge on the follow button counts new or updated results you have
not viewed. Repeated changes to the same object count once. Open **Results** to
review them without resuming following. Updates to the object currently in view
also show a brief, dismissible notification.
Viewing a result marks that result as seen; **View latest** leaves older unseen
results in the count. Resuming following clears the count.
While following is paused, a notification also offers **Resume following**,
**Dismiss**, and **Don't remind again**. Dismissing hides the current reminder
without marking results as seen. Repeated changes to the same object do not
reopen it; a new result object can trigger another reminder. The reminder closes
automatically after about eight seconds. **Don't remind again** disables these
reminders across restarts; restore them in **Preferences → General → Remind me
about results while following is paused**. Result counts and error notifications
remain available.
Pausing immediately freezes the current view, including an unfinished automatic
zoom. Resuming or viewing the latest result focuses the latest operation's results,
without collecting earlier canvases. Viewing the latest result once does not turn following back on. Read-only requests
and empty canvases do not move the workspace, and following does not cancel tasks.
Changing projects or losing the AI connection stops following; resume it explicitly
after the new project or connection is ready.

A spinner on the AI connection button indicates a running scientific task.
Hover for its operation, progress when available, and any risk of editing while
it runs. This indicates known application work, not completion of the AI's
overall task.

Both Side Bars can be shown or hidden at any time: click the pair of layout
buttons at the right end of the Ribbon's task row, press
<kbd>Ctrl</kbd>+<kbd>B</kbd> (left) or <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>B</kbd>
(right; <kbd>Cmd</kbd> on macOS), or drag a Side Bar's inner edge toward the
nearest window edge. When **Release to hide sidebar** appears, release to
hide it. You can make the Side Bar as narrow as possible without hiding it;
just stop before reaching the window edge. To cancel hiding, drag back
before releasing.

The Ribbon's show/hide button uses the same window-outline style, with a band
at the top. Task cards use a bottom band for their body; a filled band means
the region is visible. Closing a task card remains a separate **×** button.

## Recurring elements

- **Figure panel (Panel)** — a labelled section of a figure that can contain
  plots, images, text, and shapes. Panels cannot be placed inside other Panels.
  A **group** moves and arranges objects together but does not add a panel label.

- **Task page** — one task's page in the task dock. Drag its lower edge to
  change its height. Switching tabs keeps a page's settings; closing it with
  its ✕ is what discards them, and selecting a tab makes that page's dataset
  active.
- **Object inspector** — the properties panel for the selected board object:
  chart type, styling, geometry, and the display settings of what it draws, such
  as [contour levels](/guides/contour-levels/). Everyday settings are shown
  directly; the rest are folded into **Advanced**. A row of small buttons at the
  top — **Layout**, **Data**, **Type**, and whichever style sections apply —
  scrolls straight to that section. Its heading names what you have selected: an
  object and its dataset, or, for several objects at once, how many objects and
  how many datasets they draw from. Its **Contour** and **Line** sections appear
  only when the selection draws one; **Figure typography** belongs to the
  document and is always shown.
- **Setting row** — one setting as it appears in the Object inspector, Canvas
  settings, a processing step, or Preferences. Every row behaves the same
  way wherever it is shown. A dot marks a value that differs from its default —
  hover it to see that default — and the reset button beside it goes back.
  <kbd>Ctrl</kbd>+<kbd>K</kbd> searches these settings by name: activating a
  result opens the panel that holds the row, expands what it is folded into,
  scrolls to it, and highlights it. A row that cannot be edited in the current
  state stays visible but greyed; hover it for the switch to change first.
  When your selection covers several objects, one edit applies to all of them.
  If they do not already agree, the row reads **mixed** and shows a dash in
  place of a value rather than presenting one object's as the answer; hover it
  to see how many disagree and what setting a value now would do.
- **Settings group** — a named set of related settings with one home. The Ribbon
  carries a button per group that opens that home rather than repeating the
  controls: **Contour settings**, **Line settings** and **Figure typography
  settings** in **Figure → Style**, **Apodization settings** in
  **Process → Processing**. The canvas right-click menu lists the same groups
  that currently apply, as *Contour settings…* and so on, and
  <kbd>Ctrl</kbd>+<kbd>K</kbd> finds the individual settings inside them.
  Application preferences belong to no object, so the canvas menu does not list
  them: reach them with **Preferences…** in **View → Display**, from the
  command palette, or with <kbd>Ctrl</kbd>+<kbd>,</kbd>.
- **Data sheet** — the spreadsheet view of a data table, opened by
  double-clicking the table. A synchronized region table remains in the Data
  browser and opens read-only; its extracted-curves plot is the only board
  frame created for it. Use **Save editable snapshot** when you need values
  that can diverge from the source regions.
- **Command palette** — the searchable list of commands, settings, and data on
  <kbd>Ctrl</kbd>+<kbd>K</kbd>; see
  [Command palette](/reference/command-palette/).
- **Size chip** — the label above a page's top-left corner showing its
  dimensions and matched journal preset.
- **Step row** — one step of the active dataset's processing pipeline. Click it
  to edit its parameters; move it with **Move earlier** / **Move later** in its
  ⋯ menu or with <kbd>Alt</kbd>+<kbd>↑</kbd>/<kbd>↓</kbd>. Every step in the
  list is a step that runs, FFT included; an imported spectrum has no FFT row
  because its recipe has no FFT. See [Processing](/guides/processing/).

## Data terms

- **Dataset** — anything importable or derived that holds data: a spectrum, a
  recording, or a table.
- **Derived data** — results that came from another dataset (slices,
  projections, region tables, fit tables); the data browser lists them under
  their source.
- **Pseudo-2D** — a stack of 1D spectra acquired while one parameter varies
  (gradient strength for DOSY, delay for T1/T2), as opposed to a **true 2D**
  spectrum such as COSY or HSQC.
- **Pipeline** — the ordered list of processing steps applied to a dataset's
  raw data.
- **Recipe / template** — a saved pipeline, as a shareable file
  (`.plotiumproc`) or under a name in your settings; see
  [Recipes and templates](/guides/templates/).
