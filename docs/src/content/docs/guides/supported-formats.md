---
title: Supported formats
description: File formats Plotium can import, by instrument and data type, with what each importer reads and its limits.
---

Plotium reads vendor NMR, LC–MS, XRD, XPS, XAS, AFM, and electrophysiology formats
directly, with no conversion step. For how to open files and follow an import,
see [Importing data](/guides/importing-data/).

## Overview

| Format | Extension | Notes |
| --- | --- | --- |
| JEOL Delta | `.jdf` | Raw and processed 1D/2D and parameter series; experimental support |
| Bruker TopSpin | `fid` / `ser` / `pdata` | Raw and processed 1D/2D; NUS support is experimental |
| Varian/Agilent VnmrJ | `.fid` directory | Raw 1D, 2D with `phase=[1,2]`, or a series varying one parameter |
| JCAMP-DX | `.dx` / `.jdx` / `.jcamp` | 1D frequency-domain NMR spectra |
| Waters MassLynx RAW | `.raw` directory | Validated low-resolution runs, including SQD2 data |
| SCIEX legacy WIFF | `.wiff` + `.wiff.scan` | Single- and multi-sample legacy runs; both files must remain together |
| mzML | `.mzML` | Centroided/profile spectra and TIC, BPC, SIM, or SRM chromatograms with 32-bit or 64-bit arrays |
| Rigaku powder XRD | `.rasx`, FI `.raw`, RAS_RAW `.txt` | Diffraction pattern, acquisition metadata, and attenuation when available |
| XAS scans | XDI, NSLS XDAC, labeled beamline columns (any extension) | Energy, detector channels, sample and edge metadata |
| VAMAS XPS | `.vms` | ISO 14976 `NORM` / `REGULAR` XPS blocks, including multiple measurement positions and regions |
| CasaXPS text | `.txt` | Structured eight-line export with raw spectrum, background, envelope, components, and fitted parameters |
| Bruker NanoScope AFM | `.spm` / `.pfc` | Images, force curves, force-volume and PeakForce Capture cubes |
| Axon Binary Format 2 | `.abf` | int16/float32, multiple channels and sweeps, embedded DAC/epoch stimuli |
| Tabular data | `.csv`, `.tsv`, `.txt`, `.xlsx` | Column types and empty cells preserved; one table per XLSX worksheet or Source Data panel |
| Origin project (experimental) | `.opj`, `.opju` | Worksheets from two verified OPJ profiles; graphs are not imported, and `.opju` is detection-only |
| Zip archive | `.zip` | An archived dataset folder |
| Plotium project | `.plotium` | Full project: data, processing, and layout; see [Plotium files](/reference/file-formats/) |

## NMR

Plotium supports 1D and 2D NMR, including series with a parameter axis.
Choose the input that matches the data you want to work with:

- **Bruker:** select an experiment directory for raw data, or a spectrum inside
  `pdata` for processed data. If a directory contains ambiguous candidates,
  select a specific file or processing directory. States 2D (`FnMODE=4`) is supported.
  Processed spectra retain the real and imaginary components present in the files.
- **JEOL:** select a `.jdf` file containing a supported raw or processed acquisition.
- **JCAMP-DX:** ordinary XYDATA spectra with one intensity per coordinate,
  version 5.00 or 5.01, are supported with Hz or ppm axes. LINK, NTUPLES, and
  peak tables are not supported.

Missing nucleus, frequency-reference, or digital-filter-delay information remains
unknown. Hz spectra can be displayed directly, but some analyses need additional
calibration. Raw data with unknown filter delay initially appears as an FID;
see [Group-delay correction](/guides/processing/#group-delay-correction) to view
an uncorrected spectrum, and [calibration requirements](/guides/processing/#check-calibration-before-analysis)
before choosing an analysis. Unknown coordinates or unsupported data arrangements
produce an import error or diagnostic.

JEOL and Bruker NUS support is experimental and does not cover every instrument
or acquisition setting. Check import diagnostics and compare results with a trusted
reference before relying on an unfamiliar acquisition type. If a 2D NUS
acquisition lacks its sampling table, see
[Supplying a missing NMR sampling table](/guides/importing-data/#supplying-a-missing-nmr-sampling-table).

DOSY, T1, and T2 experiments are detected automatically from the acquisition
parameters and get their own analysis tools; see
[Pseudo-2D analysis](/guides/pseudo-2d/) and
[DOSY diffusion analysis](/guides/dosy/). Missing diffusion parameters can be
supplied in the **DOSY** task card after import.

Saving a `.plotium` project retains the imported NMR components, sampling order,
source information, warnings, and processing settings. Reopening recomputes spectra
from the saved settings without requiring the original vendor files.

### Varian/Agilent VnmrJ

Choose **Open Folder…** and select the `.fid` directory, or choose **Open
File…** and select the `fid` file inside it. Keep the `fid` and `procpar` files
together in the same directory.

Supported acquisitions are raw 1D, a series varying one ungrouped parameter,
and 2D with `phase=[1,2]`. Samples may be big-endian 16-bit integers, 32-bit
integers, or 32-bit floating-point values. Grouped or multiple parameter
arrays, other phase orders, Varian NUS, more than two dimensions, and processed
spectra are unsupported. Import stops if the recorded dimensions or component
arrangement do not match the data.

## Mass spectrometry

All three LC–MS formats import into the same dataset and chart workflow. For
extracting spectra and ion chromatograms after import, see
[Mass spectrometry](/guides/mass-spectrometry/).

### Waters MassLynx RAW

Open or drop the `.raw` directory itself; its internal files are not treated as
separate datasets. Plotium imports its supported MS functions and optical
detector channels. Temperature, pressure, and other readable auxiliary channels
remain in the dataset but are not plotted by default.

Plotium supports the low-resolution MassLynx encoding validated with SQD2 runs.
If a required MS function uses another encoding, the import stops and
identifies the function and instrument. Unsupported optional or reference
functions produce an import warning when the rest of the run is readable.

### SCIEX legacy WIFF

Open or drop the `.wiff` file. Keep the paired file with `.scan` appended to
the full filename beside it, for example `sample.wiff` and
`sample.wiff.scan`; the `.wiff.scan` file is never imported as a separate
dataset. SCIEX Analyst, SCIEX OS, or ProteoWizard does not need to be installed.

Plotium imports native scan IDs, retention times, m/z and intensity arrays,
precursor details when available, polarity, instrument and acquisition-start
metadata, and a separate TIC for each of the 11 verified experiment slots.
Spectra are grouped into independent sample/experiment acquisition streams and
retain cycle order, including zero-TIC and empty DDA slots; experiments are not
merged or interpolated in time. Duplicate sample names remain separate and are
displayed with stable suffixes such as `yjs_10ppm #1` and `yjs_10ppm #2`.

Only the validated legacy layout is supported. Plotium rejects a missing
companion, an empty container, or an unrecognized WIFF layout rather than
creating a partial dataset. Encrypted or newer `.wiff2` and `.timeseries.data`
acquisitions are not supported; convert them to mzML before opening them in
Plotium.

### mzML

Open or drop a `.mzML` file. Spectra are grouped by MS level and polarity,
while each scan retains acquisition-function metadata such as its mzML
instrument configuration, preset scan configuration, and filter string when
present. Scan times recorded in seconds or minutes are displayed in minutes.
Plotium also imports file-supplied TIC, BPC, SIM, and SRM chromatograms,
including chromatogram-only acquisitions. Transition precursor/product m/z,
polarity, collision energy, and activation method are retained when present.

For MS2 and higher spectra, Plotium separately retains the precursor spectrum
reference, selected-ion m/z, selected-ion intensity and charge, isolation-window
target and offsets, collision energy, and activation method. This distinction
preserves DIA isolation targets even when a selected ion is not present. Plotium
stores one precursor and one selected ion per spectrum; an import warning
identifies spectra with additional values and states that only the first was
retained.

The importer accepts little-endian 32-bit and 64-bit floating-point m/z, time,
and intensity arrays with no compression or zlib compression. Numpress,
big-endian arrays, and spectra or chromatograms without their required arrays
stop the import with an error.

## XAS scans

Beamline files often use run numbers as extensions, such as `.000` or `.0001`, so
Plotium recognizes them by content: an XDI or XDAC header, or labeled columns
that name energy and either μ or I0 with It or a fluorescence signal. Other text
stays a table, and so does any `.csv` or `.tsv` file. In the open dialog, choose
**All files** for a run-number extension other than `.000`. Importing a folder
finds these files by content. Energy is converted to eV, rows are sorted by
energy, and the first row of a repeated energy is kept.

Transmission is used when the sample shows an edge step of at least 0.1;
otherwise fluorescence is used, then an absorption column from the file. The
import report lists rows that were dropped and any stage that could not run, and
the Data list shows the latest of these. The dataset offers normalized μ(E), μ(E), flattened μ(E), k-weighted
χ(k), |χ(R)|, and the reference channel, computed with Athena's default parameters using
the [Larch](https://xraypy.github.io/xraylarch/) algorithms. A scan that ends below k = 4 Å⁻¹ stops
at normalization.

Select the scan and choose **Processing** on the **Process** tab. The **XAS
Processing** card shows the signal channels, energy shift, E0, normalization
ranges, background, and Fourier transform, and lists anything the scan could not
complete. A new scan uses **Athena defaults**, a Hanning window from k = 3 Å⁻¹ to
2 Å⁻¹ before the end of the data; **Larch defaults** transform over k 0–20 Å⁻¹ with
a Kaiser-Bessel window. A scan with less than 300 eV of post-edge range gets its
normalization range from Larch automatically. The energy
button shifts the reference foil's edge to its tabulated value. It is unavailable
without a reference channel, when that channel shows no edge, or when it steps
more than 1 eV at its edge. A fixed E0 moves with the energy shift.

**Summary Figure** on the **Process** tab adds a double-column page with the XANES
around E0, k-weighted χ(k), and |χ(R)|. Each panel stays linked to the scan, but
the XANES range is set from E0 when the page is built. The data export writes the
μ(E), reference, normalized, flattened, background, χ(k), and |χ(R)| curves as
columns. Processing schemes and templates carry the XAS parameters, but not the
energy shift, a fixed E0, or the signal, which belong to each scan. A scan too short
for a scheme's post-edge or k range keeps its own.

Over MCP, `processing.inspect` also reports the columns, calibration, and three
checks: `e0_far_from_table`, `short_post_edge`, and `noisy_high_k`.

## Rigaku powder XRD

Open the `.rasx` file when it is available. Plotium reads the measured 2theta,
intensity, and attenuation columns together with the instrument name, X-ray
target, Kalpha1 wavelength, tube voltage/current, scan step, and scan speed.
The initial page is an XRD pattern. To process it, select the pattern and choose
**Processing** on the **Process** tab. The **XRD Processing** card that opens
provides optional SNIP background subtraction, Savitzky-Golay smoothing, and
normalization by maximum intensity or integrated area; these settings and the
original observations are saved in the Plotium project. The NMR pipeline steps in
[Processing pipeline](/guides/processing/) do not apply to XRD.

Rigaku `FI`-layout `.raw` files and profile `.txt` exports whose header identifies
`RAS_RAW` also open as XRD. The binary reader retains the scan ruler, intensity,
X-ray target, Kalpha1 wavelength, tube voltage/current, and scan speed. Other
`.raw` signatures are rejected explicitly because the extension is shared by
incompatible vendor formats. Other `.txt` files keep the normal table-import
preview, so a generic numeric table is not silently reclassified.

## XPS

VAMAS `.vms` support is limited to content-signed ISO 14976 `NORM` /
`REGULAR` XPS blocks with regular energy rulers. One file remains one
experiment with stable measurement and region identities. Plotium retains native
energy, counts, CPS, photon energy, dwell time, sweeps, position, acquisition
conditions, and free metadata. Non-XPS blocks can be skipped with warnings only
when their declared block boundaries are trusted; malformed XPS payloads reject
the file. Ordinate extrema are metadata rather than data points, and ordinate
labels plus signal mode determine whether pulse counts are converted to CPS.

CasaXPS `.txt` files are recognized from their eight-line structured header,
not from the extension alone. Their source arrays and fitted parameters are
stored as an imported result. Other `.txt` files continue through table import.
See [XPS](/guides/xps/) for energy conversion, processing, and fitting.

## Bruker NanoScope AFM

Plotium imports NanoScope `.spm` images, force curves, and force-volume grids,
plus PeakForce Capture `.pfc` data cubes. Image channels plot as maps at the
recorded scan size, in the file's physical units, with the aspect ratio locked.
Force curves plot as separate approach and retract branches; when the file
records a deflection sensitivity, the vertical axis is deflection in
nanometres, otherwise the curve stays in the unit stored in the file. Plotium
shows the acquired data as is — it does not infer a contact point, indentation,
or modulus, and does not fit a contact-mechanics model.

A PeakForce Capture file usually has an AllImages `.spm` export saved beside
it. Plotium finds that companion, checks that its image grid matches the force
grid, and imports the pair as one dataset; opening a folder also imports the
pair once, not as two datasets. The default canvas places the channel map
beside a force curve from the centre pixel of the grid. If no companion is
found, or its grid does not match, the `.pfc` file still imports with its
force curves alone.

PeakForce Capture curves are the per-pixel signals as acquired. Derived QNM
maps such as modulus arrive as their own image channels; Plotium does not
recompute them from the curves.

## Electrophysiology (ABF2)

Plotium reads ABF 2.x files with int16 or float32 samples, multiple channels
and sweeps, and embedded DAC/epoch stimuli. ABF1 files and compressed ABF2 data
are not supported; re-export the recording as uncompressed ABF2 from your
acquisition software. The file picker accepts several ABF files at once, and
each file's immediate parent folder becomes its initial, editable cell ID.
For sweeps, filtering, time-window statistics, stimulus handling, and IV
analysis, see [Electrophysiology](/guides/electrophysiology/).

## Tables

CSV, TSV, delimited `.txt`, and XLSX files, as well as tables pasted from the
clipboard, go through the table import preview. Column typing, Origin CSV
headers, and XLSX worksheets are described in
[Import a table](/guides/importing-data/#import-a-table).

## Origin projects (experimental)

Origin `.opj` and `.opju` files can be chosen from both **Open File…** and
**Import Table…**; both routes identify the format from its content, not only
its extension. Worksheets from a supported `.opj` open in the
[table import preview](/guides/importing-data/#import-a-table). Origin does not
need to be installed, launched, or called during import.

Successful import is limited to two exact, content-detected OPJ producer profiles:

- Origin 7.0552 (`CPYA 4.2673 build 552`) imports verified `f64`, `f32`, signed
  `i32`, signed `i16`, fixed-width ASCII text, mixed numeric/text cells, nulls,
  and nonzero row offsets. Project parameters and notes are retained as source
  metadata.
- Origin 9.51 build 195 W64 (`CPYA 4.3268 build 195 W64`) imports worksheet
  names, column names, numeric `f64` values, nulls, and validated empty columns.
  This profile does not yet import text cells, project parameters, or notes.

Compatibility is limited to these profiles as tested; it does not extend to
other files merely because the extension or major Origin version is the same.

Plotium preserves validated Origin window or group names and column names. Each
supported window is represented as one table under the generated worksheet
name `Sheet1`; original worksheet labels are not decoded. Mixed Origin 7
columns are retained as text, and unequal column lengths are padded with nulls.
Long names, units, comments, column designations, dates, categorical values,
and unverified code pages are not guaranteed to import.

An `.opju` file is recognized from its content signature, but `.opju` is not
importable in this release and Plotium creates no partial result.

Unsupported content includes graphs, formulas, scripts, analysis
recomputation, saved analysis results as executable analyses, matrices,
embedded objects, modern OPJ text cells, non-ASCII text without a verified code
page, encrypted or protected projects, unverified OPJ versions or profiles, and
unverified OPJU containers. For the supported Origin 9.51 profile, Plotium stops
after the validated window list and warns that the remaining project objects
were not imported.

Plotium never guesses an import. Corrupt or truncated files, files above the
current 128 MiB input cap, extension/signature mismatches, and malformed or
otherwise unsupported files produce a clear error before any table is
committed. Inside an otherwise supported OPJ, an unsupported worksheet column
may be omitted, or an unsupported non-table object skipped, only when each is
independently framed and its outer boundaries are trusted. Plotium shows a
warning for every such omission, so an imported worksheet may contain only the
supported columns. If framing is ambiguous, Plotium rejects the file rather
than shifting data.

## Zip archives

A `.zip` file is opened as an archived dataset folder.
