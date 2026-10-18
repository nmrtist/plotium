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

Open **Preferences → Agent connections**. **File → Agent connections…** and
**Agent connections** in the command palette take you to the same settings page.
Press **Start connection**.

### Connect Codex CLI

1. Install and sign in to Codex CLI (`--no-daemon` support is required; verified
   with version 0.163.0).
2. Press **Copy Codex launch command** in Plotium.
3. Paste the command into a terminal and run it as-is: use zsh or bash on macOS
   and Linux, or PowerShell on Windows. It starts a new Codex session with the
   current connection address and token. No manual substitutions or separate
   `export` / `codex mcp add` steps are needed.
4. In Codex, enter `/mcp` and check that `plotium` is connected with seven
   tools. Try: “Use plotium to inspect the current Plotium project without
   changing it.”

The command supplies temporary startup configuration and a token environment
variable for that invocation. It uses an independent Codex process so an existing
background service cannot retain an old environment. It does not write Codex
configuration files. Run it from the folder you want Codex to work in. After
stopping and restarting the Plotium connection, copy a new command and start a
new Codex session.

### Other MCP clients

**Copy connection information** provides a generic JSON object with the actual
`url` and authentication `headers`. Configure a client that supports HTTP MCP and
custom authentication headers using those values. The shape is:

```json
{
  "url": "http://127.0.0.1:PORT/mcp",
  "headers": { "Authorization": "Bearer TOKEN" }
}
```

The copied object already contains the actual port and token; your client's
surrounding configuration may differ. Plotium does not edit other applications'
settings. Each start chooses a new local port and secret token. Connection
settings and permissions are never saved. Closing Preferences keeps the connection
running. Stop the connection to revoke the token; project transitions and quitting
also stop it. Only local clients can connect. Browser origins must match the
server's own origin; cross-origin access is unavailable.

### Permissions and supported operations

Connections start with **Read and modify** access, so agents can apply supported
edits immediately after connecting. Changes use Plotium's normal **Undo** and
**Redo**. Choose **Read only** under **Access** to prevent further edits, including
pending plans; queries and preflight remain available. The selected access applies
to every client holding that connection token and lasts until the connection is
stopped. A new connection starts with **Read and modify** again.

Real Agent project reads and executions save run records and mark the project as
unsaved; handshakes, tool listing and the static property catalog do not.

The first version supports project and resource queries, renaming datasets,
canvases and canvas objects, and these display properties only:

| Property ID | Target |
| --- | --- |
| `series.line.stroke_width` | Plot object (its line series) |
| `series.contour.line_width` | Plot object (its contour series) |
| `series.contour.count` | Plot object (its contour series) |
| `document.figure.typography.tick_pt` | `document` |
| `document.figure.typography.label_pt` | `document` |

Processing parameters, preferences, imports, computation, exports, image
previews, built-in chat and stdio connections are outside this interface.

An example agent task is: “Find my line plot, inspect its stroke width, preflight
changing it to 1.5 pt, then apply the saved plan.” The tool sequence is:

1. `plotium_get_state` with `{}` returns `revision`, project overview, selection
   and connection permissions.
2. `plotium_resources_search` with `expected_revision` and a `query` such as
   `{"kinds":["plotium.canvas.object"],"name_contains":"Plot"}` locates resources.
   Resolve results to explicit stable IDs; `plotium_resources_inspect` reads
   descriptors using `ids` and `expected_revision`.
3. `plotium_properties_list` with `{}` lists supported IDs and scopes. Discover
   curve addresses with `plotium_properties_inspect`:

   ```json
   {"ids":["CANVAS_UUID/OBJECT_ID"],"key":"series.line.stroke_width","expected_revision":7}
   ```

   The response's `value.readings` contains one row per applicable curve. Copy
   the complete `value.readings[0].target` for the curve you want, for example:

   ```json
   {
     "resource": {
       "id":"CANVAS_UUID/OBJECT_ID",
       "kind":"plotium.canvas.object",
       "parent_id":"CANVAS_UUID",
       "local_id":"OBJECT_ID"
     },
     "component":{"kind":"series","id":0}
   }
   ```

4. Pass that target unchanged to `plotium_plan` to edit exactly that curve:

   ```json
   {
     "tool_id":"properties.set",
     "parameters":{"key":"series.line.stroke_width","value":1.5},
     "targets":[{
       "resource":{
         "id":"CANVAS_UUID/OBJECT_ID",
         "kind":"plotium.canvas.object",
         "parent_id":"CANVAS_UUID",
         "local_id":"OBJECT_ID"
       },
       "component":{"kind":"series","id":0}
     }],
     "expected_revision":7
   }
   ```

5. Review `preflight`, then call `plotium_execute` with only
   `{"plan_id":"RETURNED_PLAN_ID"}`. Plotium immediately displays the change;
   **Undo** restores it and **Redo** reapplies it. Repeating the current value is
   reported as skipped. A batch applies as one undo step.

Both property inspection and planning require exactly one nonempty `ids` or
`targets` array. `ids` expands each resource to all components addressed by the
property; `targets` addresses only the supplied Series components. Series IDs
are local to their plot, so always copy the full target, including its resource.
Duplicate targets are processed once. A missing component or forged resource
identity rejects the entire request; an existing curve to which the property
does not apply is skipped. Inspection returns values, defaults, bounds and
applicability. To inspect just one curve, use `targets` in the inspection request.
To reset it, plan `properties.reset` with `{"key":"series.line.stroke_width"}`
and the same target. Component addressing supports only the five listed display
properties; processing parameters are unavailable. `resource.rename` requires
`ids`.

Tool responses include structured content and its JSON text representation.
Project reads and preflight require the current `expected_revision`; fetch the
state again after edits or Undo. Execution rechecks the saved revision and current
permission. Plans are single-use, including failed submissions, and at most 32
can be pending. Restarting the connection clears them. Requests cancelled before
desktop execution are skipped without changing the project. Cancellation cannot
roll back an edit that has already started its atomic commit; use **Undo**.

Errors include stable codes: `permission_denied`, `expired_plan`,
`stale_revision`, `busy`, `invalid_parameters`, `unsupported_operation`,
`plan_limit`, `request_cancelled`, `connection_stopped` and `execution_failed`.
A `busy` response is retryable after the active edit, recovery confirmation or background project
operation finishes. For stale or expired plans, read state and preflight again.
The Agent connections settings page shows recent operations and errors without
opening itself in response to background activity.
