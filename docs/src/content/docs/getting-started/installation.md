---
title: Installation
description: How to install and first launch Plotium on Windows, macOS, and Linux.
---

Plotium is a native desktop application for Windows, macOS, and Linux.

## Download and install

Prebuilt packages are published on the
[GitHub releases page](https://github.com/nmrtist/plotium/releases).
On macOS, download the DMG for your processor and open it. Drag the large
**Plotium.app** icon on the left onto **Applications** on the right. Eject the disk image, then open Plotium from Applications
in Finder. Installation does not require running a terminal command.

On Windows and Linux, unpack the ZIP anywhere you like and run `plotium`.
These portable packages do not need an installer.

| Platform | Download suffix | Requirement |
| --- | --- | --- |
| Windows x86-64 | `windows-x86_64.zip` | 64-bit Windows |
| Linux x86-64 | `linux-x86_64.zip` | glibc 2.35+ (for example Ubuntu 22.04+), graphical desktop |
| macOS Intel | `macos-x86_64.dmg` | macOS 13+ |
| macOS Apple Silicon | `macos-aarch64.dmg` | macOS 13+ |

The release's `SHA256SUMS` file covers all ZIPs and DMGs. The macOS portable
ZIPs remain available under their existing names and contain the flat executables
and MCP companions, not an application bundle.

The current DMGs contain **ad-hoc-signed, unnotarized** apps. This verifies local
bundle integrity, not an Apple-identified developer or approval for Internet
distribution. Gatekeeper can block a downloaded copy; see First launch below.

### Optional: connect an AI client

The macOS application bundle already includes the MCP connector and Claude
Desktop extension. Keep the app intact; do not move files out of its Contents.
For portable ZIP installations, keep `plotium-mcp` (`plotium-mcp.exe` on Windows)
beside the main executable. macOS and Windows ZIPs also include `plotium.mcpb`.

When you are ready, follow [Connect your AI client](/guides/automation/#connect-your-ai-client).
Each client only needs to be added once.

## First launch

Because the packages are downloaded from the internet, your operating system
may warn before the first run:

- **Windows** — if SmartScreen reports an unrecognized app, choose
  **More info → Run anyway**.
- **macOS** — follow the actual Gatekeeper message. Current DMGs are not
  Developer ID signed or notarized, so a local build launching successfully does
  not establish that a downloaded copy will be accepted. See Apple's
  [safety guidance](https://support.apple.com/en-us/102445) before deciding whether
  to allow an app. Do not ignore a malware or damaged-app warning.

On first launch Plotium shows a welcome screen. Drag a data file onto the window
or use **File → Open File…** to get started — the
[quick tour](/getting-started/quick-tour/) walks through the interface.

Double-click a `.plotium` project in Finder to open it in Plotium. If several
copies of Plotium are installed, use **Open With** to choose the intended copy.
Processing recipes (`.plotiumproc`) are applied to datasets inside Plotium;
they are not registered as standalone Finder documents.

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

For a macOS `.app` installation, quit Plotium with **Command-Q**, download the
new DMG, and replace Plotium.app in Applications. This keeps your projects and
preferences. Bundle installations do not use the portable binary updater.
Portable ZIP installations retain background updates; see [Updates](/reference/updates/).
To uninstall, delete Plotium.app (or the portable application folder) and,
if you want a clean slate, the configuration folder above.

## Building from source

Developers can build Plotium from a checkout; see the
[repository README](https://github.com/nmrtist/plotium#build-from-source) for
instructions. As a user of the released packages you never need to do this.
