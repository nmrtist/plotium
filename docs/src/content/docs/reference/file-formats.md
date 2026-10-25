---
title: Plotium files
description: The files Plotium writes itself — projects, processing recipes, styles, fit models, workflows, run records, and table schema companions.
---

This page covers the files Plotium creates. For the instrument and table
formats it reads, see [Supported formats](/guides/supported-formats/).

## `.plotium` projects

A `.plotium` file stores the whole session in one file: imported data,
processing pipelines, analysis results, board layout, embedded images, any saved
workflows, and a record of every automation run. That makes it self-contained —
copy it to another machine or send it to a colleague and it opens exactly as you
saved it, with no side files to remember. Keeping your original instrument data
is still good practice, as with any analysis software.

**Preferences → General → Project backup copies** keeps previous saves as
hidden files beside the project, so an accidental overwrite can be recovered.
Automatic crash-recovery checkpoints are separate internal files. They are
updated only after new edits and are not portable project copies. See
[Saving projects](/guides/organizing-data/#saving-projects).

For homonuclear 2D review, the project stores cross-peak marks, reciprocal
pair links, and each mark's review state. The pinned cursor position and
symmetry audit results are not stored.

For pseudo-2D DOSY, a project also holds the per-column and ILT maps
themselves, the ILT settings each was built with, your **Show** and **DOSY
method** choices, and enough about how each map was produced for Plotium to warn
you on reopening if the map no longer matches the data the project
reconstructs. The contour drawing is not stored — Plotium redraws it from the
saved map when the project opens.

Images added to a page are stored inside the project. Duplicating an image does
not duplicate its stored source. Plotium checks every embedded image when a
project opens; a missing or damaged one becomes a replaceable placeholder. See
[Images in figures](/guides/images/).

## `.plotiumproc` processing recipes

A `.plotiumproc` file stores one processing pipeline, without any data — save a
recipe once and apply it to a whole series of similar experiments, on any
machine. See [Recipes and templates](/guides/templates/).

Named processing templates are kept in the `templates` folder of Plotium's
[configuration directory](/getting-started/installation/#where-plotium-keeps-its-settings).

## `.plotiumstyle` canvas styles

**Save Canvas Style…** writes the active canvas's look as a `.plotiumstyle`
file in the `styles` folder of the configuration directory. Copy the file to the
same folder on another computer to use the style there. See
[Typography, themes and styles](/guides/themes-and-styles/).

## Custom fit models

Custom fit models are written in the `.plotiumfit` model language in the Curve
Fit editor and saved together in your model library, `fit-models.json` in the
configuration directory. A model is plain data and never runs code. See
[Custom fit models](/guides/custom-models/).

## Workflow and run-record files

An [automation](/guides/automation/) workflow file is a JSON description of a
batch run — which files to import, what to apply, what to export. Running one
produces a run-record (manifest) file stating exactly what happened to which
dataset, so a batch result is always traceable. Both are plain JSON, and both
are covered in [the command line](/reference/cli/).

A workflow is not a recipe: a recipe holds one processing pipeline, while a
workflow describes a whole run and may reference a recipe as one of its steps.

## Table schema companions

When Plotium exports a table as CSV or TSV, it writes a `.plotium-schema.json`
file next to it with the table's column types, units, and error-bar
relationships. XLSX exports keep the same information on a hidden worksheet,
and Copy TSV puts it on the clipboard (on Windows). Importing the table again restores those
details; see [Tables exported by Plotium](/guides/importing-data/#tables-exported-by-plotium).
