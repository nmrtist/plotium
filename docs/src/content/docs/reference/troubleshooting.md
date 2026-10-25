---
title: Troubleshooting
description: What to check when an import, analysis, export, or update doesn't behave.
---

## Importing

**An image will not import.** Plotium checks the file's actual content, not
just its extension; see [Images in figures](/guides/images/) for the accepted formats and
the commands for animated and multi-page images. Open **Operation and
Diagnostic History** to see the file name, detected format, point of failure,
and reason. A failed import does not change the project.

**My ABF file won't open.** ABF1 files and compressed ABF2 data are not
supported — re-export the recording as uncompressed ABF2 from your acquisition
software. See [Electrophysiology (ABF2)](/guides/supported-formats/#electrophysiology-abf2).

**A numeric column imported as text.** Plotium types values automatically only
when they are unmistakable, so locale-specific formats (comma decimals, regional
dates) and columns that mix numbers with text stay as text. Check the **Review
table import** dialog's diagnostics, fix the source file, or convert the column
after import. See [How column types are decided](/guides/importing-data/#how-column-types-are-decided).

**Cells from my Excel sheet came in empty.** Plotium reads the value Excel
cached for each formula but never recalculates formulas. Open the workbook in
Excel once and save it to refresh the cached values.

## Processing and analysis

**Peak Fit refuses to run with too many components.** A single fit handles at
most 24 components. Narrow the fit range or remove peak marks so fewer fall
inside.

**My DOSY map build was cancelled.** Changing a dataset's processing or
diffusion input while a map is building cancels the build, because the result
would no longer match its inputs. Finish the changes, then rebuild the map.
See [DOSY diffusion analysis](/guides/dosy/).

**A statistics or window-statistics result reports an error instead of a
number.** Plotium refuses to fabricate values from empty windows, non-finite
samples, or excluded cells; the message states what was wrong. Adjust the
window or confirm the exclusion it proposes.

## Exporting

**The export precheck flags my figure.** Font sizes or line widths violate
the selected journal preset. Fix them on the board — **Figure Typography…**
sets all axis text sizes at once — or pick a preset that matches your
intended size.

## Projects and updates

**I overwrote a project by mistake.** If **Preferences → General → Project
backup copies** is on, previous saves are kept as hidden files beside the
project file.

**No update arrives.** Failed background checks (for example, offline) are
silent by design. Use **Check now** in **Preferences → General** to get an
explicit result, and check your update channel — switching to a more stable
channel never downgrades, it waits for the next release.

**A project opens with a missing-image placeholder.** The embedded resource is
missing, damaged, or fails its integrity check. Check **Operation
and Diagnostic History**, select the placeholder, and use **Replace Image…**.
Plotium keeps the page editable but blocks normal save and export so the damaged
state is not silently published. The Export dialog can explicitly create a
labelled-placeholder review copy. See
[Missing or damaged images](/guides/images/#missing-or-damaged-images).

## Reporting a problem

If none of this helps, report the issue on the
[GitHub issue tracker](https://github.com/nmrtist/plotium/issues) with your
Plotium version (shown in **Preferences → General**), your platform, and, when
possible, a file that reproduces the problem. After a crash, attach the newest
crash report from the `crashes` folder; for other problems, attach the newest
session log from the `logs` folder beside it. On macOS both are in
`~/Library/Application Support/plotium`; see
[Reporting problems](/reference/reporting-problems/) for other platforms.
