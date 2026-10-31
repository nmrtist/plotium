---
title: Images in figures
description: Add micrographs, photos, and other images to figure pages and panels.
---

## Add images

To place each image on its own fitted page, choose **Add Images…** from the File
menu, Ribbon, or command palette. You can select multiple PNG, JPEG, TIFF, WebP,
BMP, or SVG files. Plotium recognizes the format from the file's contents, not
its extension. GIF images are not supported. You can also drop files onto
the board; each image gets a new page even if you drop it over an existing one.

Plotium asks for confirmation before importing an image above 100 megapixels
or an estimated 512 MiB when decoded, and rejects images above 500 megapixels or
2 GiB.

To add an image inside an existing Panel, select or enter an unlocked Panel,
then drop the file inside the green outline. To paste an image or a copied list
of image files, use **Paste Image** or press `Ctrl`/`Cmd` + `V`. On Windows, you
can copy files from File Explorer or copy image pixels from an application such
as Paint. Pasting in a text field continues to paste text normally.

When Plotium creates a page for an image, it uses the image's physical size when
DPI information is available and assumes 96 DPI otherwise. The page is at most
89 mm wide, and its height follows the image's aspect ratio. The image fills the
page without distortion. Importing several images is one undoable action; a
failed or cancelled import leaves the project unchanged.

For formats that need special handling, use these commands:

- **Add Animated Image First Frame…** imports the first frame of an animated
  PNG or WebP. **Add Images…** tells you when an image is animated.
- **Add Images Without Metadata…** removes metadata by converting the image to
  PNG before adding it.
- **Add All TIFF Pages…** adds every readable page of a multi-page TIFF. The
  standard **Add Images…** command adds only its first page.

## Adjust an image

Select an image and use **Image** in the Object inspector to crop or rotate it, adjust its
opacity, or choose how it is interpolated. These changes do not alter the
embedded source image. Images are stored in the `.plotium` project; duplicating
an image does not duplicate its stored source, and each copy keeps its own crop,
rotation, opacity, fit, and interpolation.

The board uses a bounded proxy for responsive editing, but SVG, PDF, EMF,
PNG, JPEG, TIFF, and **Copy Figure** sample the embedded source. Crop,
quarter-turn rotation, opacity, fit, interpolation, Panel clipping, and layer
order therefore match the authored page without depending on the source file's
original path.

## Vector images

SVG files, such as chemical structures saved from ChemDraw, Ketcher, or RDKit,
stay vector. Plotium converts their text to outlines when you add them, so
labels look the same on every computer but can no longer be edited, and it
ignores links to other files. A text label needs its font installed when the
file is added. Export to SVG, PDF, PNG, JPEG, and TIFF keeps the drawing sharp
at any size; EMF output and **Copy Figure** to Office currently use pixels for
it. Line widths scale with the image and do not follow the figure's theme.

## Missing or damaged images

Plotium checks every embedded image when a project opens. If one is missing,
damaged, or does not match its record, the project still opens and keeps
the image item in place as a labelled placeholder. Use **Replace Image…** in
the Object inspector to repair it. Normal save and publication export stop
until it is replaced; the Export dialog can explicitly allow labelled
placeholders when a review copy is still useful.

For the image checks the Export dialog runs, see
[Exporting](/guides/exporting/#export-a-figure).

## With an AI client

You can ask an AI client connected to Plotium to do this for you, for example:

> Add the micrograph at /Users/me/images/sem.png to panel b, fitted to the
> panel. Show me a preview.

The AI can add local SVG, PNG, and JPEG files of up to 1 MiB each (PNG and JPEG
at most 4 MiB when decoded); give it the file's full path. Add larger images yourself with
**Add Images…**. The
change appears in Plotium right away and is one Undo step. Check the preview
before you agree. See
[Access, review and undo](/ai/control-and-review/).
