---
title: Connect an AI client
description: Connect Codex, Claude Code or Claude Desktop to Plotium so it can work with your open project.
---

An AI client connected to Plotium can look at your open project, build and
adjust figures, and run supported analyses while you keep working in the
desktop app. You talk to it in the client (Codex, Claude Code or Claude
Desktop); Plotium has no built-in chat window. The connection runs on your own
computer and uses the Model Context Protocol (MCP).

## Before you start

- Install the AI client you want to use.
- Keep the Plotium installation complete. The macOS app already contains the
  connector; for a ZIP installation keep `plotium-mcp` (`plotium-mcp.exe` on
  Windows) beside the main program.

## Connect

1. Open Plotium.
2. Open **Preferences → AI connections (MCP)**. You can also click the robot
   icon beside Operation history in the Ribbon, or choose **File → AI
   connections (MCP)…**.
3. Turn on **Enable MCP**.
4. Under **Access**, choose **Read only** to let the AI look without changing
   anything, or **Read and modify** to allow supported edits. The default is
   **Read and modify**; [Access, review and undo](/ai/control-and-review/)
   explains both.
5. Add your client:

| Client | In Plotium | In the client |
| --- | --- | --- |
| Codex | Press **Add** in the Codex card | Reload MCP configuration or restart the client if needed |
| Claude Code | Press **Add** in the Claude Code card | Reload MCP configuration or restart the client if needed |
| Claude Desktop (macOS and Windows) | Press **Add to Claude Desktop…** | Confirm installation of the extension, then reload or restart if needed |

Each client only needs to be added once. Plotium remembers **Enable MCP** and
the access level across restarts.

## Check the connection

Start your AI client yourself; Plotium does not launch it. **Configured** on a
card only means the settings were added. To check that the client can reach
Plotium, send this first request:

> Use Plotium to list the datasets and figures in my current project. Do not
> change anything.

The client should describe your project, and the robot icon in Plotium turns
green while the client is active. Opening the Claude Desktop extension package
alone does not confirm a working connection. If nothing happens, see
[If the connection does not work](/ai/control-and-review/#if-the-connection-does-not-work).

Next, see [Working with an AI client](/ai/working-with-ai/) for what to ask.

## Other MCP clients

For another MCP client on the same computer, expand **Manual configuration and
diagnostics** and press **Copy stable stdio configuration**, then paste the
copied configuration into your client. The connection works only for clients
on the same computer and user account; remote connections and access between
Windows and WSL are not supported.
[MCP setup and permissions](/reference/mcp/) has the manual registration
commands and the technical reference.

## After updating Plotium

Restart the Plotium connection in your AI client (or restart the client) so it
loads the updated tools. You do not need to add the client again.
