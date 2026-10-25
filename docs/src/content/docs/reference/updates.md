---
title: Updates
description: How Plotium checks for, downloads, and installs new versions.
---

## Automatic updates

From Plotium 0.2.0 onward, macOS application bundles and Windows/Linux ZIP
installations can update automatically. With **Automatic updates** enabled
(the default), Plotium periodically checks for newer versions, downloads the
matching package, and verifies its SHA-256 against GitHub release metadata.
Downloads happen in the background; installation happens after Plotium exits.

### macOS application bundles

Install Plotium.app in Applications or another writable local folder and eject
the DMG before running it. The updater downloads the DMG for your processor and
checks the new app's identifier, version, architecture, and code signature.
After exit it replaces the complete application, including its MCP connector,
extension, resources, and bundle metadata. A failed replacement attempts to
restore the previous app. Projects and preferences are kept.

macOS may ask permission to update applications. If installation is not writable,
move Plotium to a writable local folder or replace it manually from a new DMG.
Copies running directly from a mounted DMG or an App Translocation folder cannot
update themselves. Quit other running copies before installing an update.

The apps are **ad-hoc signed and unnotarized**. This checks bundle integrity,
not publisher identity or Apple's malware approval. Updates do not disable
Gatekeeper or remove quarantine attributes. If macOS blocks launching a new
copy, follow [First launch](/getting-started/installation/#first-launch).
The first downloaded launch and future macOS policy can still require user action.

### Windows and Linux ZIP installations

The updater replaces the executable and its shipped MCP companions and license
notices together. Keep the extracted package files in the same writable folder.

## Checking and restarting

While an update downloads, its progress appears at the right end of the
Ribbon's task row. Once it is ready, select **Restart to update** to use the new version
immediately. Plotium asks about unsaved changes before restarting; canceling
leaves the app open and does not arm a later restart. If you close normally,
the update is installed after exit and takes effect next time you open Plotium.

**Preferences → General** shows the installed version and the updater's
state, offers **Check now** for an immediate manual check (also available as
**Check for updates…** in the command palette), and has its own
**Restart now** button.

Failed background checks (for example, when you are offline) are silent;
only a check you started yourself reports its error. An installation failure
after exit is shown under **Preferences → General** on the next launch; use
**Check now** to retry. Keep a manual download available if relaunch is blocked.
If the update was installed but cleanup or relaunch failed, this page shows an
"Update installed" warning instead; open Plotium manually if needed.

## Release channels

Plotium offers three release channels:

- **Stable** receives regular releases and is recommended for most users.
- **Beta** provides earlier access to features that are still being tested.
- **Alpha** provides the earliest builds and may be less reliable.

Choose a channel under **Update channel**. **Follow build** uses the channel
that came with your copy of Plotium. Changing to a more stable channel does not
downgrade the installed version; Plotium waits for a newer release on the
selected channel.

## Turning it off

Disable **Automatic updates** in **Preferences → General** to stop all
background checks. Manual checks with **Check now** still work.
