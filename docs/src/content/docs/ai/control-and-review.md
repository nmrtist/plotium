---
title: Access, review and undo
description: Control what an AI client can do, follow and undo its changes, and fix connection problems.
---

You stay in charge of what an AI client does in Plotium: you choose whether it
may change anything, every change can be undone, and nothing is written to disk
without your confirmation in Plotium.

## Choose what the AI may do

Set this in **Preferences → AI connections (MCP)**, or click the robot icon in
the Ribbon.

| Setting | Effect |
| --- | --- |
| **Access → Read only** | The AI can look at data, tables and figures, including images of the current figures, but cannot prepare, preview or apply changes. Switching to Read only also blocks changes it has prepared but not yet applied |
| **Access → Read and modify** | The AI can also make supported changes. This is the default |
| **Enable MCP** off | Stops all access immediately and discards changes the AI prepared but did not apply. Your client setup is kept |

The access level applies to every connected client and every project you open,
and Plotium remembers it across restarts. Closing Preferences or switching
projects keeps the connection running; exiting Plotium stops it. After a
restart you can continue in the same AI conversation without adding the client
again. After you switch projects, ask the AI to look at the current project
before it continues editing.

Your AI client may also ask you to approve its tool calls. That approval is
the client's own and separate from Plotium's access level: approving a call in
the client never bypasses **Read only** or the save and export confirmations.

## Review and undo changes

- Every change appears in Plotium immediately and is one step in **Undo** and
  **Redo**, however many items it touched. A save or export is not a change to
  the project and cannot be undone.
- Values you set by hand, such as a colour you chose in the Object inspector or
  a legend you dragged, are kept when the AI restyles a figure or applies a
  theme, unless you ask it to change them.
- Looking, previewing and checking do not change the project and do not mark
  it as unsaved.
- **Operation history** (the clock-arrow button in the Ribbon) lists the results
  of recent AI operations under **Results**, with links to view them.

## Save and export confirmation

When the AI asks to save or export, Plotium shows **Allow AI to save this
project?** or **Allow AI to export this figure?** with the exact file paths.

- Press **Allow save** or **Allow export** to go ahead, or **Deny** to write
  nothing.
- Replacing an existing file needs its own checkbox in the same prompt.
- Each confirmation covers that one request. A changed path needs a new one.
- This confirmation is needed even with **Read and modify** access, and the AI
  can ask for it even in **Read only**.

A save captures the project at the moment you allow it; later edits are still
unsaved. Export does not change the project or mark it as saved.

## Follow AI results

By default, the workspace follows the AI: it shows the canvas affected by the
latest completed operation. Results that touch several canvases are shown
together, and a canvas already in view keeps its zoom. Requests that only look
at the project, and empty canvases, do not move the workspace.

The follow button beside the robot icon shows whether following is active,
paused or off. Working yourself pauses following: opening or selecting other
content, panning, zooming or editing. Hovering and reading operation details do
not. In Operation history's **Results**, use **Pause**, **Resume**, **Stop
following** or **View latest**.

While following is paused:

- A badge on the follow button counts new or updated results you have not
  viewed. Repeated changes to the same object count once. Viewing a result marks
  it as seen; **View latest** shows the latest result but leaves older ones in
  the count, and does not turn following back on. Resuming clears the count.
  Open **Results** to review them without resuming.
- Updates to the object in view also show a brief notification you can close.
- A reminder offers **Resume following**, **Dismiss** and **Don't remind
  again**. It closes after about eight seconds. **Dismiss** hides it without
  marking results as seen; further changes to the same object do not bring it
  back, but a new result object can. **Don't remind again** turns reminders off across restarts;
  turn them back on in **Preferences → General → Remind me about results while
  following is paused**. Result counts and error notifications stay available.

Pausing freezes the view at once, including an unfinished automatic zoom.
Resuming, or viewing the latest result, shows the latest operation's results
only. Changing projects or losing the connection
stops following; resume it yourself when you are ready. Following never cancels
a running task.

A spinner on the robot icon means a scientific task is running. Hover over it
for the operation, progress when known, and whether editing now could stop the
result from being applied. It shows Plotium's own running work, not whether the
AI has finished your request.

## The robot icon

Hover over the robot for the project, access level and last activity. Click it
to open the connection settings; it stays available when the Ribbon is
collapsed.

| Appearance | Status | What it means |
| --- | --- | --- |
| Gray, short slash | Off | Turn on **Enable MCP** when you want to connect |
| Blue, hollow dot | Starting / Ready | Wait for startup, or send a request from your client |
| Green, filled dot | Active | A client was active in the last 30 seconds; this does not mean its task is finished |
| Orange, exclamation | Needs attention | Open the connection settings, read the error and follow the suggested action |

## If the connection does not work

| What you see | What to try |
| --- | --- |
| Plotium cannot find Codex or Claude Code | Use **Installation help** to install the client, or **Choose executable…** to select it, then press **Add** |
| An error mentions Node.js | Check the client's Node.js installation, or select a standalone client executable, then try again |
| A different server named `plotium` already exists | Review the existing entry. Choose **Back up and replace this entry** only if you want to replace it; Plotium backs it up and leaves other servers unchanged |
| **Configured**, but no client activity | Reload the client's MCP configuration or restart it, then send the [first request](/ai/connect/#check-the-connection). Check whether the client has disabled or blocked the server |
| The AI says the project has changed or its information is out of date | Ask it to look at the current project again before continuing |
| Another Plotium instance owns the connection | Use the instance providing the connection, or exit it so this one can connect. Switching window focus does not switch the AI's target |
| Windows reports that the connector is in use during an update | Exit AI clients using Plotium, then press **Retry** in the connection settings |
| The connector or Claude Desktop extension is missing | Extract the complete Plotium release again and keep its files together |

Startup failures retry automatically. The connection settings show recent
operations and errors; press **Retry** when offered after fixing the cause.

If the connection breaks while the AI is changing something, look at the
project before asking it to try again: the change may already have been made,
and repeating it could create duplicate content. The same applies to saves and
exports: ask the AI to check what happened to the request rather than sending
it again.
