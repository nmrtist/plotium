---
title: Mass spectrometry
description: Browse LC–MS runs, extract mass spectra and ion chromatograms, and choose chromatogram channels.
---

Waters MassLynx RAW, SCIEX legacy WIFF, and mzML runs open as one kind of LC–MS
dataset. To open a Waters run, choose **Open Folder…** and select the `.raw`
directory, or drop the directory onto the window. This page covers what you do
after import; for what each importer
reads, see [Supported formats](/guides/supported-formats/#mass-spectrometry).

There is no LC–MS processing pipeline. The imported run, active acquisition
stream, detector channels, extracted spectra, extracted ion chromatograms, and
page layout are saved in the `.plotium` project.

## The initial page

The initial page shows the active stream's total ion chromatogram (TIC). When
a Waters run contains optical detector data, its UV channels are placed above
the TIC on a shared retention-time axis. Multiple UV channels are overlaid; their
legend uses stored wavelengths such as `214 nm`. Select the UV plot and use
**Legend & scales** in the Object inspector to hide, move, or lay out that
legend.

## Preview and extract a mass spectrum

Select the LC–MS dataset, then choose **Extract Mass Spectrum** on the
**Analyze** tab. Plotium opens **Dataset tools → Mass spectrometry** in the right
sidebar and activates retention-time range selection.

Click a TIC or UV chromatogram to show the nearest MS scan under **Scan
preview**. The preview identifies its retention time and native scan number; it
is neither added to the page nor saved as a result, and it is cleared when the
project is reopened. Choose **Extract current scan** to add that scan as a stick
spectrum.

To extract from a time window, choose a **Method**, select **Select range**, and
drag across a TIC or UV chromatogram. **Extract spectrum** adds the peak-apex
scan, nearest scan, mean spectrum, or summed spectrum to the page. Each
extracted spectrum records its function, time range, and method. It does not
change when the preview cursor moves and appears under **Analysis** in the Data
browser.

## Extract an ion chromatogram (XIC)

To create an extracted ion chromatogram (XIC; also called an EIC), open
**Dataset tools → Mass spectrometry** and choose **Select m/z range**. Drag
across the current mass-spectrum plot, confirm the displayed m/z interval and
acquisition stream, then choose **Extract ion chromatogram**.

The resulting line plot contains one point per scan at its retention time in
minutes. Each point is the sum of intensities within the selected m/z interval,
including both endpoints. The result is saved and does not change when you move
the scan preview or select another acquisition stream. XIC extraction does not
integrate chromatographic peaks or calculate peak area.

## Switch acquisition streams

If the run contains several supported MS functions or experiments, use
**Acquisition stream** under **Dataset tools → Mass spectrometry**. For a
Waters run, the initial stream is the first non-reference MS function. Stream
changes, spectrum extractions, and XIC extractions can be undone and redone
with the standard Edit commands.

## Browse chromatogram channels

For runs with many chromatogram channels, open **Dataset tools → Mass
spectrometry** from **Extract Mass Spectrum** or the command palette. The
**Chromatogram channels** browser lists a stable count and ordering, with TIC
and BPC ahead of SIM/SRM transitions. Search matches channel names and native
IDs. Structured transitions can also be filtered by precursor m/z, product
m/z, polarity, collision energy, and activation method. Numeric fields accept
an exact value, comparisons such as `>=400`, or a range such as `400..500`.

Selecting a row replaces the series on the current LC–MS chromatogram plot with
that single channel. Mass-spectrum plots are never retargeted. The choice is
undoable and is saved with the page in the project. Chromatogram-only runs use
the same browser. The panel reports when a run has no plottable channels, no
structured transition metadata, or no channels matching the current filters.
The list stays responsive for large scheduled-MRM runs.

## Where TIC and BPC values come from

A file-supplied TIC or BPC chromatogram is marked as a source chromatogram and
is used for its acquisition stream first. When that channel is absent, Plotium
uses each spectrum's file-supplied TIC or base-peak summary when available;
otherwise it derives TIC from the intensity array, or BPC from the largest
non-negative peak. A mixed run can therefore report both source summaries and
array-derived points. In a multi-stream run, a source chromatogram that is not
tied to a stream is kept as its own channel rather than guessed as a
replacement for a stream's TIC or BPC. The field metadata and the Scientific
Summary show which source was used.
