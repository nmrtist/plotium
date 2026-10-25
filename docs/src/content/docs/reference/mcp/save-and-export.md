---
title: MCP save and export
description: Save the desktop project and export figures with desktop approval.
---

## Save the current desktop project

Saving uses the same native snapshot, background worker, backups and completion
path as **Save project**. It saves the current desktop state, not a separate CLI
project. First read `plotium_get_state`; `save_state` reports the current path,
persisted archive revision, dirty flag, edit generation and up to 16 retained
`receipt_ids`. Query each ID with `plotium_save_status` for the complete receipt
and recovery action. An ID alone does not indicate submission success or completion.

Plan a first save or Save As with no `ids` or `targets`:

```json
{
  "expected_project_context": "PROJECT_CONTEXT",
  "expected_revision": 46,
  "tool_id": "project.save",
  "parameters": {"path": "/absolute/path/figure.plotium"}
}
```

Omit `path` to save the current file; an untitled project requires a path.
The parent directory must exist. Targets must be regular, non-symlink `.plotium`
files or absent paths. Preflight resolves the parent directory and freezes the
observed target. `include_view_snapshots` optionally overrides the project's
current native save profile. The plan returns `save_id`, the resolved path and
`overwrite_required`; it grants no file-write permission.

Submit the one-use plan with `plotium_execute {"plan_id":"PLAN_ID"}`. The result
contains `save.status: "awaiting_authorization"`. In Plotium, review the exact
path and press **Allow save**. If a file already exists, first check **Replace the
existing file at this exact path**. **Deny** leaves the target untouched.
This permission is independent of **Read and modify**, applies to one save only,
and cannot be supplied as a tool argument. A changed target requires a fresh
plan and approval. One external save may be pending at a time.

Serialization uses an exclusively created private sibling temporary, not a fixed
`<target>.tmp`. For an existing target, its frozen metadata is checked again after
backup preparation, immediately before replacement. A detected change refuses
installation and preserves the later target. Permission admission and this file
check are separate: the check plus replacement is not an inter-process
compare-and-swap. A non-cooperating write after the check, or one that preserves
the checked metadata, may go undetected; avoid concurrent saves to the same path.

Poll `plotium_save_status {"save_id":"SAVE_ID"}`; this read needs no document
revision or context, works while saving, and does not dirty the project.
`awaiting_authorization`, `queued` and `saving` are incomplete states with null
`phase`. Only `completed` gives a final phase:

| Phase | File state and next action |
| --- | --- |
| `not_installed` | This save did not install its snapshot at the target. Resolve the reported denial/failure, inspect current state, then obtain fresh authorization if another save is needed. |
| `installed_unconfirmed` | The snapshot replaced/created the target, but backup finalization or durable sync failed. Keep the desktop project open; inspect the target and storage before an explicitly authorized retry. Do not assume the old file remains. |
| `committed` | The captured snapshot completed the native durable commit. Later edits may still be unsaved. |

On Windows, a partial replacement failure can remove or move the old target even
when the new snapshot is `not_installed`. Plotium keeps the serialized candidate
and reports its exact recovery path. It restores a known original backup only
without overwriting an existing target; otherwise it retains and reports that
backup and any older staged backup. If Windows moved the original without
returning its location, the error says so. Keep the project open and inspect the
reported files before retrying; this failure does not clear dirty state.

The receipt records original `project_context`, `snapshot.document_revision`,
`snapshot.edit_generation`, `persisted_revision`, exact path, `state_applied`,
`dirty_after_completion`, error and recovery message. The status response also
reports current context/path/revision/dirty and `matches_current_project`.
The snapshot is captured after desktop approval and before background work.
New edits after capture remain dirty; a committed save does not mean every
current edit is on disk. An installed-but-unconfirmed
save keeps dirty and records the installed baseline for native recovery.

Switching projects or stopping the connection revokes pending write permission.
The worker only holds its original snapshot and path, checks authorization at
installation, and cannot adopt a completion into a different project, even at
an equal document revision. Once the worker has atomically admitted installation, stopping cannot guarantee
cancellation of that original file write; inspect its receipt. Revocation before
admission prevents installation. Saving creates no Undo step and Undo does not delete or restore files.

The desktop retains the latest 16 receipts across reconnects and project changes
until application exit. After a lost response, query the original `save_id`
before doing anything else; do not replay the submitted call. After application
restart or `unknown_save`, inspect the target and freshly opened project instead
of assuming success or failure. Stable stdio configuration continues to work;
reopening/restarting still requires fresh state, target discovery and plans.
Connection recovery is not evidence of a successful save.

## Export a captured canvas as SVG, PDF and PNG

Use `figure.export` with one exact whole-canvas TargetRef and no top-level
`ids`/`targets`. The native exporters preserve the canvas background, complete
page, curves, absolute Y-error amplitudes, native text and embedded images.
There is no trimming, theme application or project modification. Keep the
editable `.plotium` project as well as these delivery formats.

```json
{
  "expected_project_context": "PROJECT_CONTEXT",
  "expected_revision": 48,
  "tool_id": "figure.export",
  "parameters": {
    "canvas": {"resource":{"id":"CANVAS_ID","kind":"plotium.canvas"}},
    "size_mm": [180,90],
    "dpi": 300,
    "outputs": [
      {"format":"svg","path":"/absolute/path/figure.svg"},
      {"format":"pdf","path":"/absolute/path/figure.pdf"},
      {"format":"png","path":"/absolute/path/figure.png"}
    ]
  }
}
```

`size_mm` defaults to the native page size. An explicit size uniformly scales
the whole figure and must retain its aspect ratio (0.001 mm height tolerance);
it does not resize the editable page. `dpi` defaults to 300, accepts 1–2400 and
only controls PNG. Each PNG axis is rounded to the nearest integer from the
native physical points / 72 × DPI × uniform size scale, with a minimum of one
pixel. A 180 × 90 mm page at 300 DPI is **2126 × 1063 pixels**. SVG/PDF retain
physical page dimensions. Text uses the native generic sans-serif family;
a machine without usable system fonts returns a font error. SVG font appearance
on another machine depends on that machine's available sans-serif fonts.

There are 1–8 outputs per batch. Each absolute path needs its matching lowercase
extension and an existing parent directory; targets must be regular non-symlink
files or absent. Duplicate resolved output paths are refused. Preflight returns
`export_id`, physical/pixel size and an error or resolved target for each file.
A bad directory does not hide the other files' results. `overwrite` defaults to
false: an existing target reports `overwrite_not_requested` and remains untouched.
Set `overwrite:true` only when replacement is intended; this still grants no
write permission.

Submit the one-use plan through `plotium_execute`. Valid files await Plotium's
**Allow AI to export this figure?** prompt, listing the exact canvas, revision,
size and paths. Review it and press **Allow export**. Existing files additionally
require the unchecked **Replace the listed existing output files** checkbox.
**Deny** installs nothing. **Read and modify** is not export authorization;
Read only may request fresh, separate export consent. All-invalid batches finish
with per-file errors without asking for consent.

Poll `plotium_export_status {"export_id":"EXPORT_ID"}` without context/revision.
It is non-auditing, works while rendering, and returns `export`, current context,
current revision and `matches_current_project`. Batch status is
`awaiting_authorization`, `rendering` or `completed`. Each `files[]` entry has
format, exact path, overwrite requirement, `status`, `error_code`, `message`
and installed byte count. File status is `pending`, `succeeded`, `failed` or
`unknown` (worker failed; inspect that path before deciding what to do).
Completion does not imply every file succeeded. Missing/corrupt images, fonts,
changed targets, permission denial and rendering/budget errors are reported
without replacing that output. Successful files in the same batch remain.

Rendering runs off-thread on the canvas/assets captured after desktop consent,
bound to the original `project_context`, `canvas` and `render_revision`. Later
edits do not change that snapshot. Project transition, connection Stop or access
change revokes pending installation permission. Each file's atomic permission
admission precedes its installation: revocation before admission blocks it;
an already admitted file may finish and must be checked in the receipt. Old
completion never changes the new project's document, save path, dirty state or
status. Unique owned sibling staging and atomic no-clobber creation preserve
unapproved files. Existing-file metadata is checked just before replacement;
this is not a cross-process compare-and-swap, so avoid concurrent writers.
Export success means installed output, not the native save's durable-commit
phase; Undo cannot remove or restore exported files.

Limits: one running batch, 32 shared pending plans, 4096 bytes per path, PNG
16,384 pixels per edge and 16,777,216 pixels total; referenced embedded assets
16 MiB, decoded asset RGBA 64 MiB, SVG 8 MiB and encoded output 64 MiB per file.
No external asset lookup or placeholder substitution occurs. Limits and the
complete schema are discoverable in the `limits` and `operation_constraints`
capability sections.

The last 16 export receipts survive reconnects and project changes in this
desktop process, not application exit. After response loss query the original
`export_id`; do not resubmit the export. After restart or `unknown_export`,
inspect the exact output files first. Reopen the editable project, fetch fresh
state and targets, then make a new plan only for a newly intended operation.
