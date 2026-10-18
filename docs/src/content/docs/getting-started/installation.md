---
title: Installation
description: How to install and first launch Plotium on Windows, macOS, and Linux.
---

Plotium is a native desktop application for Windows, macOS, and Linux.

## Download and install

Prebuilt packages are published on the
[GitHub releases page](https://github.com/nmrtist/plotium/releases).
Download the archive for your platform, unpack it anywhere you like, and run
the `plotium` executable. No installer or administrator rights are required.

| Platform | Archive suffix | Requirement |
| --- | --- | --- |
| Windows x86-64 | `windows-x86_64.zip` | 64-bit Windows |
| Linux x86-64 | `linux-x86_64.zip` | glibc 2.35+ (for example Ubuntu 22.04+), graphical desktop |
| macOS Intel | `macos-x86_64.zip` | macOS 13+ |
| macOS Apple Silicon | `macos-aarch64.zip` | macOS 13+ |

The release's `SHA256SUMS` file lists checksums for all archives. Packages are
unsigned portable executables; macOS packages do not contain an `.app` bundle.

### Optional: connect an AI client

To use Codex, Claude Code or Claude Desktop with Plotium, keep `plotium-mcp`
(`plotium-mcp.exe` on Windows) beside the main executable. macOS and Windows
packages also include `plotium.mcpb`, the Claude Desktop extension.

When you are ready, follow [Connect your AI client](/guides/automation/#connect-your-ai-client).
Each client only needs to be added once.

## First launch

Because the packages are downloaded from the internet, your operating system
may warn before the first run:

- **Windows** — if SmartScreen reports an unrecognized app, choose
  **More info → Run anyway**.
- **macOS** — if Gatekeeper blocks the app, right-click it, choose **Open**,
  and confirm; macOS remembers the choice.

On first launch Plotium shows a welcome screen. Drag a data file onto the window
or use **File → Open File…** to get started — the
[quick tour](/getting-started/quick-tour/) walks through the interface.

## Where Plotium keeps its settings

Preferences, saved processing templates, and custom fit models live in a small
per-user configuration folder:

| Platform | Location |
| --- | --- |
| Windows | `%APPDATA%\plotium\config` |
| macOS | `~/Library/Application Support/plotium` |
| Linux | `~/.config/plotium` |

Your data and projects are never stored there — they stay wherever you save
them. Deleting the folder resets Plotium to its defaults.

## Updating and uninstalling

Plotium checks for new versions and updates itself in the background — see
[Updates](/reference/updates/). To uninstall, delete the application folder
and, if you want a clean slate, the configuration folder above.

## Building from source

Developers can build Plotium from a checkout; see the
[repository README](https://github.com/nmrtist/plotium#build-from-source) for
instructions. As a user of the released packages you never need to do this.
