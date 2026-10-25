---
title: MCP setup and permissions
description: Register Plotium with a local MCP client and the rules every tool call follows.
---

These pages are for people writing their own MCP client or checking what a
single tool call does. To connect Codex, Claude Code or Claude Desktop and work
with Plotium in plain language, see [Connect an AI client](/ai/connect/).

| Page | Covers |
| --- | --- |
| This page | Manual registration, permissions, rules shared by every plan, tool annotations |
| [State and discovery](/reference/mcp/discovery/) | `plotium_get_state`, capability discovery, document structure, property catalog |
| [Data import and tables](/reference/mcp/tables/) | Native and CSV/TSV/XLSX import, table reads, transforms, cell patches, refresh |
| [Plots and curves](/reference/mcp/plots/) | `plot.create`, appending and rebinding curves |
| [Styling](/reference/mcp/styling/) | Display properties, batches, hand-set values, legends, themes |
| [Page layout and deletion](/reference/mcp/layout/) | `figure.edit`: page, panels, text, images, annotations, deletion |
| [Preview, diagnostics and recovery](/reference/mcp/review/) | Canvas and candidate previews, figure diagnostics, recovery, error codes |
| [Save and export](/reference/mcp/save-and-export/) | `project.save`, `figure.export` |
| [Scientific operations](/reference/mcp/scientific/) | Processing, diffusion, DOSY, tasks, plotting results |

## Manual configuration and diagnostics

On Windows, Plotium finds standalone client executables on `PATH`. For Codex
installed with npm, it resolves the native executable inside the npm package,
using prefixes on `PATH` and the default `%APPDATA%\npm` prefix. Both x64 and
ARM64 installations are supported. For a custom npm prefix, add it to `PATH` and
restart Plotium, or use **Choose executable…** to select the package's
`codex.exe`. Adding a client requires the Codex CLI to be installed.

In **Preferences → AI connections (MCP)**, expand **Manual configuration and diagnostics**
and press **Copy stable stdio configuration**. The copied JSON contains the actual
absolute connector path; the path below is a placeholder:

```json
{"mcpServers":{"plotium":{"command":"/ABSOLUTE/USER/DATA/bin/plotium-mcp","args":[]}}}
```

Use the copied path for your platform (`plotium-mcp.exe` on Windows). Plotium
copies the connector to its user data directory when enabled and updates it on
application upgrades. Keep `plotium-mcp` beside the desktop executable in a release
installation. The official CLI registration forms are:

```sh
codex mcp add plotium -- /ABSOLUTE/USER/DATA/bin/plotium-mcp
claude mcp add --transport stdio --scope user plotium -- /ABSOLUTE/USER/DATA/bin/plotium-mcp
```

See [Codex MCP](https://learn.chatgpt.com/docs/extend/mcp?surface=cli),
[Claude Code MCP](https://code.claude.com/docs/en/mcp) and the
[MCP Bundle manifest](https://github.com/modelcontextprotocol/mcpb/blob/main/MANIFEST.md).
Temporary HTTP diagnostics include a secret token and expire on service restart;
they are an advanced diagnostic entry, not the saved client configuration.
Discovery is private to the current user and local machine. Remote and cross-environment
WSL access are outside this interface.

After upgrading Plotium, restart its MCP connection in your AI client to load
the updated tools. If connector installation fails, open
**AI connections (MCP)** to read the error and press **Retry**.

## Permissions and supported operations

Edits use Plotium's normal **Undo** and **Redo**. **Read only** prevents further
edit execution, including pending plans. Table reads and property preflight remain
available; import, native binding and creation and transform/patch/refresh preflight require Read and modify.
The selected permission persists until you change it.

Project queries and plan creation require both `expected_revision` and
`expected_project_context`, using the `project_context` from `plotium_get_state`.
The examples in these pages use `PROJECT_CONTEXT` as a placeholder for that value.
Every project open, new, close or recovery creates a new context, even when
revision numbers match. Switching clears plans and rejects queued work;
`busy` during a transition means wait until it finishes or is cancelled.
On `project_changed`, reread state and replan with newly discovered targets.
Never reuse old targets or plans. Executions bind the context of their saved plan.

Project edits, scientific computations and data transformations retain durable run
provenance and mark the project as unsaved. Ordinary queries, including state,
resource/property/table/document browsing, scientific result inspection, previews
and diagnostics, do not append project audit records or change dirty state, edit
generation or automation revision. Query-only workflows also leave the project
unchanged; workflows containing modifying operations retain the complete run,
including query inputs needed to reproduce those operations. Handshakes, tool listing, authenticated status, five-second heartbeats,
the static property catalog and save coordination (planning, submission and
`plotium_save_status`), and export planning/submission/status do not add audit records.

The connection covers the operations described on the pages listed above.
[`plotium_properties_list`](/reference/mcp/discovery/#discover-structure-without-relying-on-names)
lists the properties it can write, with complete schemas; [Setting IDs](/reference/settings-ids/#figures) maps figure settings in
Plotium's panels to their IDs. The `object.axes.y2_*` properties
apply only while some curve of the plot has `series.y_axis` `right`.
Preferences, new format parsers, automatic scientific-result plotting and
built-in chat are outside this interface.

## Rules every plan follows

Tool responses include structured content and its JSON text representation.
Project reads and preflight require the current `expected_revision`; fetch the
state when dynamic values are needed or after desktop edits or Undo. Known exact
targets may reuse committed synchronous results, as described in
[State and discovery](/reference/mcp/discovery/#read-state-and-discover-capabilities-on-demand). Execution rechecks the saved revision and current
permission. Plans are single-use, including failed submissions, and at most 32
can be pending. Restarting the connection clears them. Requests cancelled before
desktop execution are skipped without changing the project. Cancellation cannot
roll back an edit that has already started its atomic commit; use **Undo**.

## Tool annotations and client approval

Every tool advertises MCP `annotations` in `tools/list`, independently of the
current **Access** setting.
`readOnlyHint` describes environment changes; `destructiveHint` distinguishes
potentially destructive writes from additive ones; `idempotentHint` describes
whether repeating identical arguments has additional effects; `openWorldHint`
describes interaction with external entities, including local input/output files.
Destructive and idempotent hints are meaningful only for non-read-only tools.
See the [MCP annotation definitions](https://modelcontextprotocol.io/specification/2025-11-25/schema#toolannotations).

Queries, previews and receipt inspection are read-only and operate on native
project/application state. Diagnosis is also read-only, but continuation cursors
are single-use. `plotium_plan` is not read-only: each successful call retains a new
executable plan and can read external data, schema, processing-scheme or image
files and prepare computation. It does not commit the candidate or write
save/export outputs. `plotium_execute` covers edits, deletion, scientific tasks
and save/export requests, so the entire tool advertises potentially destructive,
non-idempotent, open-world behavior. Task cancellation is a potentially destructive
write to task state; repeating cancellation for the same retained task has no
additional effect.

A client's policy may use trusted-server hints when deciding whether to ask for
approval; clients can still ask, and untrusted annotations must not authorize
calls. Client approval, Plotium **Access**, and desktop file consent are separate
decisions. Annotations grant no permission: revision/context checks, revocation
and one-use plans still apply. Every save/export request retains its own desktop
confirmation and exact file/overwrite scope, including in **Read only**.
Never replay a call after disconnection or an unknown outcome, even when marked
idempotent; inspect state or retained receipts instead. After upgrading Plotium
and its connector, reconnect the client to refresh the tool catalog.
