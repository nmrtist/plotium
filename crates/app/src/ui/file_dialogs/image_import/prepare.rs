use super::*;

pub(super) fn worker(
    paths: Vec<PathBuf>,
    payloads: Vec<(String, Vec<u8>)>,
    cancelled: Arc<AtomicBool>,
    sender: mpsc::SyncSender<WorkerEvent>,
    consent: mpsc::Receiver<bool>,
    options: ImportOptions,
) {
    if sender
        .send(WorkerEvent::State(ImportImageState::Probing))
        .is_err()
    {
        return;
    }

    for path in paths {
        if cancelled.load(Ordering::Relaxed) {
            if sender
                .send(WorkerEvent::State(ImportImageState::Cancelled))
                .is_err()
            {
                log::debug!("Image import receiver closed after cancellation");
            }
            return;
        }
        match prepare_path(&path, &sender, &consent, &cancelled, options) {
            Ok(()) => {}
            Err(error) => {
                if sender.send(WorkerEvent::Candidate(Err(error))).is_err() {
                    return;
                }
            }
        }
        if cancelled.load(Ordering::Relaxed) {
            if sender
                .send(WorkerEvent::State(ImportImageState::Cancelled))
                .is_err()
            {
                log::debug!("Image import receiver closed after cancellation");
            }
            return;
        }
    }
    for (name, bytes) in payloads {
        if cancelled.load(Ordering::Relaxed) {
            if sender
                .send(WorkerEvent::State(ImportImageState::Cancelled))
                .is_err()
            {
                log::debug!("Image import receiver closed after cancellation");
            }
            return;
        }
        match prepare_bytes(name, bytes, &sender, &consent, &cancelled, options) {
            Ok(()) => {}
            Err(error) => {
                if sender.send(WorkerEvent::Candidate(Err(error))).is_err() {
                    return;
                }
            }
        }
        if cancelled.load(Ordering::Relaxed) {
            if sender
                .send(WorkerEvent::State(ImportImageState::Cancelled))
                .is_err()
            {
                log::debug!("Image import receiver closed after cancellation");
            }
            return;
        }
    }
    if sender
        .send(WorkerEvent::State(ImportImageState::ReadyToCommit))
        .is_err()
    {
        return;
    }
    if sender.send(WorkerEvent::Finished).is_err() {
        log::debug!("Image import receiver closed before completion");
    }
}

fn send_candidate(
    sender: &mpsc::SyncSender<WorkerEvent>,
    candidate: Candidate,
    cancelled: &AtomicBool,
) {
    if sender.send(WorkerEvent::Candidate(Ok(candidate))).is_err() {
        cancelled.store(true, Ordering::Relaxed);
    }
}

fn prepare_path(
    path: &Path,
    sender: &mpsc::SyncSender<WorkerEvent>,
    consent: &mpsc::Receiver<bool>,
    cancelled: &AtomicBool,
    options: ImportOptions,
) -> Result<(), Failure> {
    let basename = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("<unnamed>")
        .to_owned();
    let bytes = std::fs::read(path).map_err(|error| {
        failure(
            &basename,
            "unknown",
            "read",
            error.to_string(),
            "Check file permissions and retry.",
        )
    })?;
    prepare_bytes(basename, bytes, sender, consent, cancelled, options)
}

pub(super) fn prepare_bytes(
    basename: String,
    bytes: Vec<u8>,
    sender: &mpsc::SyncSender<WorkerEvent>,
    consent: &mpsc::Receiver<bool>,
    cancelled: &AtomicBool,
    options: ImportOptions,
) -> Result<(), Failure> {
    let detected = plotium_io::image::sniff(&bytes).name().to_owned();
    if plotium_io::image::sniff(&bytes) == plotium_io::image::RasterFormat::Svg {
        let candidate = prepare_svg(basename, &bytes, &detected, options)?;
        send_candidate(sender, candidate, cancelled);
        return Ok(());
    }
    let source_dpi = plotium_io::image::metadata_dpi(&bytes);
    let probe = plotium_io::image::probe(&bytes).map_err(|error| {
        failure(
            &basename,
            &detected,
            "probe",
            error.to_string(),
            "Choose a supported PNG, JPEG, TIFF, WebP, BMP, or SVG file.",
        )
    })?;
    if options.import_all_tiff_pages && probe.format != plotium_io::image::RasterFormat::Tiff {
        return Err(failure(
            &basename,
            &detected,
            "probe_pages",
            "the all-pages command accepts TIFF sources only".to_owned(),
            "Choose Add Images… for PNG, JPEG, WebP, BMP, or SVG sources.",
        ));
    }
    if probe.animated && !options.allow_first_frame {
        return Err(failure(
            &basename,
            &detected,
            "probe",
            "animated images are not supported by this import path".to_owned(),
            "Export the intended frame as a static PNG, JPEG, TIFF, WebP, or BMP and retry.",
        ));
    }
    if probe.class == plotium_io::image::ResourceClass::Rejected {
        return Err(failure(
            &basename,
            &detected,
            "limits",
            format!(
                "{} × {} pixels and {} decoded bytes exceed the hard limit",
                probe.width, probe.height, probe.decoded_bytes
            ),
            "Reduce the image dimensions before importing.",
        ));
    }
    let bytes = if options.strip_metadata {
        plotium_io::image::strip_metadata_to_png(&bytes, options.allow_first_frame).map_err(
            |error| {
                failure(
                    &basename,
                    &detected,
                    "strip_metadata",
                    error.to_string(),
                    "Use the normal import path for a large image, or reduce its dimensions first.",
                )
            },
        )?
    } else {
        bytes
    };
    let mut notices = Vec::new();
    if options.strip_metadata {
        notices.push(
            "Metadata was removed explicitly and the embedded pixels were re-encoded as PNG.",
        );
    }
    if probe.animated {
        notices.push(
            "Only the first animation frame is displayed; original animated bytes remain embedded.",
        );
    }
    if probe.high_precision {
        notices.push(
            "High-precision source is displayed as RGBA8 sRGB; original bytes remain embedded.",
        );
    }
    if probe.has_icc || probe.has_exif {
        notices.push("ICC/EXIF metadata is preserved in the embedded original bytes.");
    }
    if probe.pages > 1 && !options.import_all_tiff_pages {
        notices.push("Multi-page TIFF detected; only page one is imported by this command. Use Add All TIFF Pages… to create one image per page.");
    }
    let preview = if probe.class == plotium_io::image::ResourceClass::ProxyRequired {
        if sender
            .send(WorkerEvent::State(
                ImportImageState::AwaitingLargeImageConsent,
            ))
            .is_err()
        {
            cancelled.store(true, Ordering::Relaxed);
            return Ok(());
        }
        match consent.recv() {
            Ok(true) => {}
            Ok(false) | Err(_) => {
                cancelled.store(true, Ordering::Relaxed);
                return Err(failure(
                    &basename,
                    &detected,
                    "consent",
                    "large-image import was cancelled".to_owned(),
                    "Choose the image again if you want to import it.",
                ));
            }
        }
        if sender
            .send(WorkerEvent::State(ImportImageState::DecodingProxy))
            .is_err()
        {
            cancelled.store(true, Ordering::Relaxed);
            return Ok(());
        }
        let proxy = plotium_io::image::decode_proxy_rgba8(&bytes, 256).map_err(|error| {
            failure(
                &basename,
                &detected,
                "proxy_decode",
                error.to_string(),
                "Reduce the image dimensions below the 500 megapixel hard limit and retry.",
            )
        })?;
        notices.push("Large image embedded; a bounded proxy is used for interactive preview.");
        proxy
    } else {
        rebuild_preview(&bytes, 0).map_err(|error| {
            failure(
                &basename,
                &detected,
                "decode",
                error.to_string(),
                "Retry or re-encode the image in a supported format.",
            )
        })?
    };
    let warning = (!notices.is_empty()).then(|| notices.join(" "));
    let sha256 = plotium_io::image::sha256(&bytes);
    let bytes = Arc::new(bytes);
    let format = if options.strip_metadata {
        "png".to_owned()
    } else {
        probe.format.name().to_ascii_lowercase()
    };
    let first = Candidate {
        basename: basename.clone(),
        bytes: Arc::clone(&bytes),
        sha256,
        format,
        pixel_size: [probe.width, probe.height],
        auto_page_size_mm: physical_image_size_mm([probe.width, probe.height], source_dpi),
        page_index: 0,
        warning,
        preview,
    };
    send_candidate(sender, first, cancelled);
    if options.import_all_tiff_pages && probe.format == plotium_io::image::RasterFormat::Tiff {
        for page_index in 1..probe.pages {
            if cancelled.load(Ordering::Relaxed) {
                break;
            }
            let decoded = plotium_io::image::decode_rgba8_page(bytes.as_slice(), page_index, false)
                .map_err(|error| {
                    failure(
                        &basename,
                        "TIFF",
                        "decode_page",
                        error.to_string(),
                        "Re-encode the failing TIFF page as a standalone PNG and retry.",
                    )
                })?;
            let pixels = image::RgbaImage::from_raw(
                decoded.probe.width,
                decoded.probe.height,
                decoded.rgba8,
            )
            .ok_or_else(|| {
                failure(
                    &basename,
                    "TIFF",
                    "preview_page",
                    "The decoded TIFF page has inconsistent dimensions.".to_owned(),
                    "Re-encode the failing TIFF page as a standalone PNG and retry.",
                )
            })?;
            // The page was already decoded to validate it and obtain its dimensions.
            // Reuse those pixels rather than opening and decoding the page again.
            let preview = if pixels.width() <= 256 && pixels.height() <= 256 {
                pixels
            } else {
                image::DynamicImage::ImageRgba8(pixels)
                    .thumbnail(256, 256)
                    .into_rgba8()
            };
            let preview = plotium_io::image::ProxyImage {
                pixel_size: [preview.width(), preview.height()],
                rgba8: preview.into_raw(),
            };
            let candidate = Candidate {
                basename: format!(
                    "{} — page {}",
                    Path::new(&basename)
                        .file_stem()
                        .and_then(|name| name.to_str())
                        .unwrap_or("Image"),
                    page_index + 1
                ),
                bytes: Arc::clone(&bytes),
                sha256,
                format: "tiff".to_owned(),
                pixel_size: [decoded.probe.width, decoded.probe.height],
                auto_page_size_mm: physical_image_size_mm(
                    [decoded.probe.width, decoded.probe.height],
                    source_dpi,
                ),
                page_index,
                warning: decoded.probe.high_precision.then(|| {
                    "High-precision TIFF page is displayed as RGBA8 sRGB; original bytes remain embedded."
                        .to_owned()
                }),
                preview,
            };
            send_candidate(sender, candidate, cancelled);
        }
    }
    Ok(())
}

/// Embeds the normalized form, which already carries no source metadata.
fn prepare_svg(
    basename: String,
    source: &[u8],
    detected: &str,
    options: ImportOptions,
) -> Result<Candidate, Failure> {
    if options.import_all_tiff_pages {
        return Err(failure(
            &basename,
            detected,
            "probe_pages",
            "the all-pages command accepts TIFF sources only".to_owned(),
            "Choose Add Images… for PNG, JPEG, WebP, BMP, or SVG sources.",
        ));
    }
    let remedy = "Re-export the SVG from its source application and retry.";
    let bytes = plotium_core::export::normalize_svg_asset(source)
        .map_err(|error| failure(&basename, detected, "normalize", error.to_string(), remedy))?;
    let probe = plotium_io::image::probe(&bytes)
        .map_err(|error| failure(&basename, detected, "probe", error.to_string(), remedy))?;
    let preview = rebuild_preview(&bytes, 0)
        .map_err(|error| failure(&basename, detected, "decode", error.to_string(), remedy))?;
    Ok(Candidate {
        basename,
        sha256: plotium_io::image::sha256(&bytes),
        bytes: Arc::new(bytes),
        format: probe.format.name().to_ascii_lowercase(),
        pixel_size: [probe.width, probe.height],
        auto_page_size_mm: physical_image_size_mm([probe.width, probe.height], None),
        page_index: 0,
        warning: None,
        preview,
    })
}
