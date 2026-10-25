---
title: Automation
description: Apply one action to many datasets or figures, run a read-only scientific script, or run a saved workflow.
---

When the same operation has to happen to a whole series of experiments,
automation replaces the click-per-dataset routine with one reviewed batch.

Open **File → Automation…** (also on the command palette). The window has two
tabs: **Current Project** works on what is already open, and **External
Inputs** runs a saved workflow that starts from files on disk.

To have an AI client do this kind of work from a plain-language request
instead, see [Working with an AI client](/ai/working-with-ai/).

## Current Project

Search for datasets or figures and check the ones you want, or press **Current
selection** to check what is selected in the workspace. Then choose one of the
two modes below the list: **Scientific Script** or **Registered Tool**.

### Apply a tool to the checked items

In **Registered Tool**, pick a tool from the list. The window shows what the
tool does and the parameters it accepts; fill them in under **Parameters
(JSON)**. Press **Preflight** to see which targets the tool will affect and
which it will skip, then **Confirm and execute**. The whole batch collapses
into a single **Undo automation** step.

Three of the tools reach the settings that the **Object inspector**, the
processing steps and Preferences edit, so one contour level, colour, line
width, type size or window function can be applied across a whole project in
one batch:

| Tool | What it does |
| --- | --- |
| **Inspect a property** | Reads the current value, the default, and the range the setting accepts |
| **Set a property** | Writes a value |
| **Reset a property** | Re-derives the value from the current data, as the panel's reset button does |

Name the setting by its ID. [Setting IDs](/reference/settings-ids/#reading-and-writing-settings)
lists the IDs, the parameter format, which item each one lives on, and how
results report skipped targets.

A value outside the range is refused at **Preflight**, before you confirm.
Past that point either every listed target takes the value or none does. A
target that already holds the value is reported as skipped; if every target
does, nothing is written and nothing is added to the undo stack.

### Run a scientific script

A scientific script reads LC–MS data and returns its own summary, for example
the peaks it found in each chromatogram. It cannot change the project, open
other files or reach the network.

1. Check the LC–MS datasets to analyse. Checking a canvas includes the datasets
   it shows.
2. Choose **Scientific Script**, press **Open script…** and pick a
   `.plotiumscript` file. The window shows how many checked LC–MS datasets the
   script will process.
3. Press **Run Scientific Script**. The datasets are processed one after
   another, with a progress bar.
4. Each dataset gets a result card. If the script returns a `summary`, its
   fields appear as a table; **Technical details** shows the complete result.
   A dataset the script could not process shows the error instead, and the
   others still run.
5. Press **Save results…** to write all results to one JSON file.

**Run Scientific Script** in the command palette opens this mode and runs the
chosen script on the checked datasets. Only LC–MS datasets, such as mzML and
Waters MassLynx imports, are passed to a script, and the command is unavailable
until one is loaded.

Scripts are written in [Rhai](https://rhai.rs/). `load_input()` returns the
dataset as a map: its `source` and `instrument`, the `chromatograms` stored in
the file, the total-ion and base-peak `stream_chromatograms`, a summary of every
scan under `scans`, and for Waters data the LC method under `lc_method`.
Retention times are in minutes. The script's last expression is its result.
These helpers are available:

| Function | Returns |
| --- | --- |
| `moving_average(values, width)` | The values smoothed with a centred window of `width` points |
| `rolling_percentile(values, width, q)` | The `q` quantile (0–1) in a centred window, for example as a baseline |
| `estimate_noise(values)` | A noise estimate for the trace |
| `detect_peaks(x, y, options)` | Peaks as maps with `index`, `x`, `y` and `prominence`; `options` may set `min_height`, `min_prominence`, `min_spacing` and `max_count` |
| `format_number(value, decimals)` | The number as text with up to 12 decimals |

For example, this script reports the five most prominent peaks of the first
total-ion or base-peak chromatogram:

```rust
let input = load_input();
let trace = input.stream_chromatograms[0];
let peaks = detect_peaks(trace.time_min, trace.values, #{ min_prominence: 1000.0, max_count: 5 });
#{
    summary: #{ "Peaks found": peaks.len() },
    peaks: peaks
}
```

A script stops with an error when it runs too long, builds arrays or text that
are too large, or returns more than 10 MB of results. Results stay in the window
until you run a script again, so save them if you need them later.

## External Inputs

Runs a saved workflow that starts from files on disk, for example: import every
experiment in a folder, apply a processing recipe, and export each figure.
Press **Open workflow…** to load it, **Validate** to check it, then **Confirm
and run workflow**. Progress is reported step by step and a long run can be
cancelled.

The window runs workflow files; it does not create them. A workflow is a plain
JSON file, described under [Workflow file anatomy](/reference/cli/#workflow-file-anatomy).

## What is recorded

A workflow run leaves a record of the workflow and its hash, the Plotium version,
and every step's targets, parameters and results. Run it here and that record is
kept in the project; run it from [the command line](/reference/cli/) and the
same record is written to a file. A batch from **Current Project** leaves no
such record: it is an ordinary edit, in the document and on the undo stack. A
scientific script changes nothing and leaves no record.

Native workflow operations that write outside the project, such as exporting
figures, are also listed under **Help → Operation and Diagnostic History**, with
each file's path, size and SHA-256 checksum. An export that fails partway still
names the files it had already written, so nothing reaches disk unaccounted for.

Workflow files can also be run without the desktop app; see
[the command line](/reference/cli/). [File formats](/reference/file-formats/)
describes the workflow and run-record files themselves.
