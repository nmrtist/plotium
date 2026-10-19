---
title: Updates
description: How Plotium checks for, downloads, and installs new versions.
---

## macOS application bundles: replace the app from a DMG

Quit Plotium with **Command-Q**, download the new DMG for your processor, and
replace Plotium.app in Applications. Your projects and preferences are kept.
**Preferences → General** links to downloads and shows the installed version;
automatic updates and channel controls are disabled for bundle installations.
The saved preferences are retained for portable installations.

The portable updater replaces individual executables. It is not used inside
`.app` bundles because that would invalidate their signature and leave bundle
metadata out of sync. Current DMGs are ad-hoc signed and unnotarized; see
[Installation](/getting-started/installation/#first-launch) for first-launch conditions.

## Portable ZIP installations

Plotium keeps itself up to date. With automatic updates enabled (the default),
it periodically checks for new versions. Updates are downloaded and securely
verified in the background, so you can keep working. They are applied when
Plotium closes and take effect on the next launch.

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
only a check you started yourself reports its error.

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
