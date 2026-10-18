---
title: Automation
description: Apply one action to many datasets or figures, or run a saved workflow.
---

When the same operation has to happen to a whole series of experiments,
automation replaces the click-per-dataset routine with one reviewed batch.

Open **File → Automation…** (also on the command palette). The window has two
tabs:

## Current Project

Works on what is already open. Search for datasets or figures and check the
ones you want — or press **Current selection** to pull in your current
selection — then pick a tool and press **Preflight** to see which targets it
will affect and which will be skipped. **Confirm and execute** applies
it, and the whole batch collapses into a single **Undo automation** step.

### Settings

Three tools reach the settings the **Object inspector** and the processing
steps edit, so one level, colour, width, type size or window function can be
applied across a whole project as a single reviewed batch:

| Tool | What it does |
| --- | --- |
| **Inspect a property** | Reads the current value, the default, and the range the setting accepts |
| **Set a property** | Writes a value |
| **Reset a property** | Re-derives the value from the current data, as the panel's reset button does |

Name the setting in **Parameters (JSON)** by its id:
`{"key": "series.contour.count", "value": 12}` for **Set a property**, and
`{"key": "series.contour.count"}` for the other two.

The three tools reach every setting the panels edit: object and series styling,
processing-step parameters, document and canvas settings, and application
preferences. Which resource to name follows from the setting — a contour or
line setting lives on a plot object, an apodization setting on a dataset,
figure typography on the document (listed as **Plotium document**), a page size
on a canvas, and a preference on the application (listed as **Plotium
application**).

| Setting in the Object inspector | id | Accepts |
| --- | --- | --- |
| **Lowest level** | `series.contour.base.magnitude` | a number above 0; what it measures depends on the anchor |
| **Anchor** | `series.contour.base.policy` | `absolute`, `noise_floor`, `background_scale`, `fraction_of_range` |
| **Levels** | `series.contour.count` | 1 to 256 |
| **Level ratio** | `series.contour.ratio` | above 1, at most 10 |
| **Negative contours** | `series.contour.negative.enabled` | `true` or `false` |
| **Positive colour** | `series.contour.positive_color` | `"#rrggbb"` |
| **Negative colour** | `series.contour.negative_color` | `"#rrggbb"` |
| **Line width** | `series.contour.line_width` | 0.05 to 10 pt; default 0.5 |
| **Stroke width**, in the **Line** section | `series.line.stroke_width` | 0.05 to 10 pt; default 0.5 |
| **Tick-label size**, in the **Figure typography** section | `document.figure.typography.tick_pt` | 1 to 72 |

Application preferences take the same three tools. For
`settings.appearance.accent.color`, **Set a property** accepts `"#rrggbb"` and
pins the canvas accent to that colour; **Reset a property** clears it, and the
accent follows the theme again.

| Setting on an apodization step | id | Accepts |
| --- | --- | --- |
| **Window** | `dataset.processing.apodization.kind` | `none`, `cosine_bell`, `exponential`, `gaussian` |
| **LB** | `dataset.processing.apodization.lb_hz` | −10000 to 10000 |
| **GB** | `dataset.processing.apodization.gb_hz` | greater than 0, at most 10000 |

[Contour levels](/guides/contour-levels/) explains what each contour setting
does and which data each anchor needs; [Processing](/guides/processing/) does
the same for the apodization rows.

#### What a checked target expands to

The resource you check is rarely the thing that is written — the setting decides
what inside it is. A plot object expands to its series, a dataset to its
processing steps, and the document to itself. Preflight and the result list show
one row per component, so a plot holding three series produces three rows,
naming the object and the series inside it.

A component the setting does not reach is listed as **Skipped** with the reason,
and the rest still apply: a heatmap drawn under a contour, a zero-fill step when
the setting belongs to apodization, or a **GB** asked of a step whose window is
*Exponential*. A resource with nothing to address at all — a text box, a dataset
with no pipeline — is skipped the same way.

Result rows and the run record name the component beside the resource:

```json
{
  "resource": { "id": "…", "kind": "plotium.dataset" },
  "component": { "kind": "processing_step", "id": 3 }
}
```

Series components read `{"kind": "series", "id": 2}`. Both ids are local to the
resource they travel with, so a step id means nothing next to a different
dataset.

#### Why a target was skipped

Every skipped row carries a sentence written to be read. Rows the write itself
passed over also carry `skip_reason`, a stable token a workflow can branch on
without matching on wording:

| `skip_reason` | Means |
| --- | --- |
| `already_at_value` | The target already held that value, so nothing was written |
| `not_applicable` | The setting does not apply to this target |
| `target_missing` | The address no longer names anything in the document |

Rows already ruled out at preflight carry the sentence alone. They are never
same-value no-ops, so the absence of the token is itself the distinction.

A **Set a property** whose every target already holds the value writes nothing
at all: each target is reported as skipped, the document revision does not move,
and nothing is added to the undo stack.

A value outside the range is refused at **Preflight**, before you confirm, and
names both the value you gave and the limit that rejected it. Past that point
either every listed component takes the value or none does, and what is written
collapses into one **Undo automation** step.

**Inspect a property** returns its readings to whoever ran it: run it as a
workflow step and the values land in the run record, which is also what
[the command line](/reference/cli/) writes to its manifest file. In the window
the numbers appear under **Result value (JSON)**, below the per-component rows:
one reading per component, each with its current value, its default and the
range it accepts.

#### What a reading contains

Each reading names its `target` and carries the current `value`, a
`default_value` where the setting has one, `modified` — whether the value
differs from that default — an `availability`, and the `schema` that bounds it.

A setting the current state does not allow you to write is still read back:
`availability` is `"disabled"` and `disabled_reason` names what has to change
first, such as switching the phase mode to *Manual* before φ0 can be set.

Schemas are tagged by `type`: `bool`, `text`, `int`, `stepped_int`, `float`,
`enum`, or `color`.

- `int` and `stepped_int` carry `min` and `max`, plus a `unit` where the setting
  has one. `stepped_int` also carries the `step` its values must land on — a
  Savitzky-Golay window, for instance, runs from 3 to 201 in steps of 2.
- `float` carries `min`, `max` and `exclusive_min`, and where a setting refuses
  particular values, `excluded` (one value) or `excluded_magnitude` (everything
  at or below that magnitude). Its `display` — `linear`, `degrees` or `log10` —
  is how the panel shows the number; the `unit` and `log` fields beside it
  restate the same thing.
- `enum` lists its variants, each with a stable id and a label.

Bounds and values are always in the setting's own units, whatever `display`
says: a phase whose `display` is `degrees` is radians on the wire.

## External Inputs

Runs a saved workflow that starts from files on disk — for example: import
every experiment in a folder, apply a processing recipe, and export each
figure. Press **Open workflow…** to load it, **Validate** to check it, then
**Confirm and run workflow**. Progress is reported step by step and a long run
can be cancelled.

## What is recorded

A workflow run leaves a record of the workflow and its hash, the Plotium version,
and every step's targets, parameters and results. Run it here and that record is
kept in the project; run it from [the command line](/reference/cli/) and the
same record is written to a file. A batch from **Current Project** leaves no
such record — it is an ordinary edit, in the document and on the undo stack.

Anything that writes files outside the project — exporting figures, for
instance — is also listed under **Help → Operation and Diagnostic History**,
with each file's path, size and SHA-256 checksum. An export that fails partway
still names the files it had already written, so nothing reaches disk
unaccounted for.

Workflow files are plain JSON and can also be run without the desktop app —
see [the command line](/reference/cli/), which executes the same workflows
headlessly. [File formats](/reference/file-formats/) describes the workflow
and run-record files themselves.

## Local agent connections (MCP)

Connect an AI client to ask questions about your open project, inspect a figure,
or make supported edits in Plotium. MCP is the connection that lets the client
use Plotium's tools; you continue chatting in Codex, Claude Code or Claude Desktop.

### Connect your AI client

Keep the complete Plotium release folder, including `plotium-mcp`
(`plotium-mcp.exe` on Windows). Install the AI client you want to use before
adding it to Plotium.

1. Open or create a project in Plotium.
2. Open **Preferences → AI connections (MCP)**. You can also click the robot
   beside Operation history in the Ribbon, or use **File → AI connections (MCP)…**.
3. Turn on **Enable MCP**. Under **Access**, choose **Read only** to explore
   without allowing edits, or **Read and modify** to allow supported changes.
   The default is **Read and modify**.
4. Add your client using the matching option below.

| Client | What to do in Plotium | What to do in the client |
| --- | --- | --- |
| Codex | Press **Add** in the Codex card | Reload MCP configuration or restart the client if needed |
| Claude Code | Press **Add** in the Claude Code card | Reload MCP configuration or restart the client if needed |
| Claude Desktop (macOS and Windows) | Press **Add to Claude Desktop…** | Confirm installation of the extension, then reload or restart if needed |

Start your AI client yourself; Plotium does not launch it. For Codex and Claude
Code, **Configured** means the connection settings have been added. To check that
the client can actually reach Plotium, send this first request:

> Use Plotium to list the datasets and figures in my current project. Do not
> change anything.

The client should describe your project, and Plotium should show recent client
activity. The robot turns green while the connection is active. Opening the
Claude Desktop extension package alone does not confirm a working connection.

### Try a task

Use plain language in your AI client. For example:

- **Inspect a figure:** “Use Plotium to preview my canvas and describe the curves
  and labels. Do not change anything.” Your client must support receiving images.
- **Adjust a line:** “Find the curve named Current, check its line width, then
  change only that curve to 1.5 pt.” This requires **Read and modify** access.
- **Create a plot:** “Import the CSV at `/absolute/path/experiment.csv`. Use time
  as X and current as Y, with units s and mA. Create an editable line plot and
  show me a preview.” Replace the path with your file's absolute path and state
  the columns and units you want.

The connection can import CSV/TSV tables, read table values, inspect project
structure, preview canvases, create line or scatter plots, change a curve's
source columns, derive tables with supported native plans, and rename datasets,
canvases or objects. Display edits cover line width, contour width and count, and
figure tick-label and axis-label sizes. Processing parameters, fitting and other
computations beyond supported table transforms, other import formats, saving and
exporting are not available through this connection. Use Plotium's interface for
those tasks. There is no built-in chat window.

### Control access and review changes

**Access** applies to all clients using this Plotium connection and to every
project you open. Plotium remembers both the permission and **Enable MCP** across
restarts. Choose **Read only** to block further edits, including edits that have
been prepared but not yet applied. You can still inspect data and preview figures.

Supported changes appear in the desktop immediately and use Plotium's normal
**Undo** and **Redo**. Reading project data also adds a run record, so the project
may show unsaved changes even after a read-only request.

Turn off **Enable MCP** to stop access immediately. Your client setup is retained.
Closing Preferences or switching projects keeps the connection running; exiting
Plotium stops access. After restarting Plotium or turning MCP back on, you can
continue in the same AI session without adding the client again. After a project
switch, ask the AI to inspect the current project before continuing edits.

### Understand the robot icon

Hover over the robot for the project, permission and last activity. Click it to
open connection settings; it remains available when the Ribbon is collapsed.

| Appearance | Status | What it means for you |
| --- | --- | --- |
| Gray, short slash | Off | Turn on **Enable MCP** when you want to connect |
| Blue, hollow dot | Starting / Ready | Wait for startup, or send a request from your client |
| Green, filled dot | Active | There has been client activity in the last 30 seconds; this does not confirm that a task has finished |
| Orange, exclamation | Needs attention | Open connection settings, read the error and follow the suggested action |

### If the connection does not work

| What you see | What to try |
| --- | --- |
| Plotium cannot find Codex or Claude Code | Use **Installation help** to install the client, or **Choose executable…** to select it, then press **Add** |
| An error mentions Node.js | Check the client's Node.js installation, or select a standalone client executable, then try again |
| A different server named `plotium` already exists | Review the existing entry. Choose **Back up and replace this entry** only if you want to replace it; Plotium backs it up and leaves other servers unchanged |
| **Configured**, but no client activity | Reload the client's MCP configuration or restart it, then send the first request above. Check whether the client has disabled or blocked the server |
| The client asks you to open a project | Open or create a project in Plotium, then ask the client to inspect it again |
| Another Plotium instance owns the connection | Use the instance providing the connection, or exit it so this instance can connect. Switching window focus does not switch the AI's target |
| Windows reports that the connector is in use during an update | Exit AI clients using Plotium, then press **Retry** in connection settings |
| A connector or Claude Desktop extension is missing | Extract the complete Plotium release again; keep its files together |

Startup failures retry automatically. Connection settings show recent operations
and errors; use **Retry** when offered after resolving the cause. If a connection
breaks during an edit, check the project before asking the AI to try again: the
edit may already have completed, and repeating it could create duplicate content.

### Other clients and technical details

For another local MCP client, expand **Manual configuration and diagnostics** and
press **Copy stable stdio configuration**. Use the copied configuration in your
client. This connection supports clients on the same machine and user account;
remote connections and access across Windows and WSL are not supported.

See the [MCP reference](/reference/mcp/) for manual registration commands, tool
parameters, examples and limits.
