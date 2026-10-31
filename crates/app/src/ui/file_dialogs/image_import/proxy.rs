use super::*;

const PROXY_BUDGET: usize = 128 * 1024 * 1024;
const VECTOR_PREVIEW_MIN_EDGE: u32 = 1024;
const VECTOR_PREVIEW_MAX_EDGE: u32 = 4096;

struct ProxyRebuild {
    hash: [u8; 32],
    page_index: u32,
    format: String,
    receiver: mpsc::Receiver<Result<plotium_io::image::ProxyImage, String>>,
}

#[derive(Default)]
struct ProxyRebuilds {
    active: Option<ProxyRebuild>,
    failed: std::collections::BTreeSet<([u8; 32], u32)>,
}

thread_local! {
    static REBUILDS: RefCell<ProxyRebuilds> = RefCell::new(ProxyRebuilds::default());
}

pub(super) fn poll_rebuilds(app: &mut PlotiumApp, ctx: &egui::Context) {
    REBUILDS.with(|rebuilds| {
        let mut rebuilds = rebuilds.borrow_mut();
        if let Some(active) = &rebuilds.active {
            match active.receiver.try_recv() {
                Ok(Ok(proxy)) => {
                    let hash = active.hash;
                    let page_index = active.page_index;
                    insert_proxy(app, hash, page_index, proxy);
                    rebuilds.active = None;
                }
                Ok(Err(reason)) => {
                    let hash = active.hash;
                    let page_index = active.page_index;
                    let format = active.format.clone();
                    rebuilds.failed.insert((hash, page_index));
                    rebuilds.active = None;
                    let operation = app.session.begin_operation();
                    record_failures(
                        app,
                        operation,
                        0,
                        vec![failure(
                            "Embedded image",
                            &format,
                            "proxy_rebuild",
                            reason,
                            "Replace the image with a source below the 500 megapixel hard limit.",
                        )],
                    );
                }
                Err(mpsc::TryRecvError::Empty) => {
                    ctx.request_repaint_after(std::time::Duration::from_millis(30));
                    return;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    let hash = active.hash;
                    let page_index = active.page_index;
                    rebuilds.failed.insert((hash, page_index));
                    rebuilds.active = None;
                }
            }
        }
        let view = BoardView {
            center: app.session.board.world_center,
            half_extent: (ctx.content_rect().size() * 0.5 / app.session.board.zoom).into(),
            pixels_per_point: app.session.board.zoom * ctx.pixels_per_point(),
        };
        let existing: std::collections::BTreeMap<_, _> = app
            .session
            .ui
            .raster_proxies
            .iter()
            .map(|proxy| ((proxy.hash, proxy.page_index), proxy.pixel_size))
            .collect();
        let items = || {
            app.doc
                .canvases
                .iter()
                .flat_map(|canvas| canvas.objects.iter().map(move |item| (canvas, item)))
                .filter_map(|(canvas, item)| {
                    let CanvasObjectKind::RasterImage(image) = &item.kind else {
                        return None;
                    };
                    let asset = app.doc.assets.get(&image.asset)?;
                    Some((canvas, item, image, asset))
                })
                .filter(|(_, _, image, asset)| {
                    !rebuilds.failed.contains(&(asset.sha256, image.page_index))
                })
        };
        let missing = items()
            .find(|(_, _, image, asset)| !existing.contains_key(&(asset.sha256, image.page_index)))
            .map(|(_, _, image, asset)| (asset, image.page_index, None));
        let on_screen: Vec<_> = items()
            .filter(|(_, _, _, asset)| asset.is_vector())
            .filter_map(|(canvas, item, image, asset)| {
                let frame = canvas.content_page_frame(item.id)?;
                let origin = [canvas.board_pos[0] + frame.x, canvas.board_pos[1] + frame.y];
                view.shows(origin, [frame.width, frame.height]).then(|| {
                    (
                        asset,
                        image.page_index,
                        view.preview_edge([frame.width, frame.height], image.crop),
                    )
                })
            })
            .collect();
        let upgrade = on_screen
            .iter()
            .find(|(asset, page_index, edge)| {
                existing
                    .get(&(asset.sha256, *page_index))
                    .is_some_and(|size| size[0].max(size[1]) < *edge)
            })
            .copied();
        let vectors: std::collections::BTreeSet<_> = app
            .doc
            .assets
            .values()
            .filter(|asset| asset.is_vector())
            .map(|asset| asset.sha256)
            .collect();
        let shown: std::collections::BTreeSet<_> = on_screen
            .iter()
            .map(|(asset, page_index, _)| (asset.sha256, *page_index))
            .collect();
        let candidate = match (missing, upgrade) {
            (Some(missing), _) => Some(missing),
            (None, Some((asset, page_index, edge))) => {
                let proxies = &mut app.session.ui.raster_proxies;
                let held = |proxies: &[plotium_core::state::RasterProxy]| {
                    proxies.iter().map(|proxy| proxy.rgba8.len()).sum::<usize>()
                };
                if !fits_budget(edge, held(proxies)) {
                    // Off-screen sharpness is what gives way; those previews
                    // come back at the minimum edge.
                    release_off_screen_vectors(proxies, &vectors, &shown);
                }
                fits_budget(edge, held(proxies)).then_some((asset, page_index, Some(edge)))
            }
            (None, None) => None,
        };
        let Some((asset, page_index, edge)) = candidate else {
            return;
        };
        let hash = asset.sha256;
        let format = asset.format.clone();
        let bytes = asset.bytes.clone();
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let result = match edge {
                Some(edge) => plotium_io::image::rasterize_svg_rgba8(&bytes, edge),
                None => rebuild_preview(&bytes, page_index),
            }
            .map_err(|error| error.to_string());
            let _ = sender.send(result);
        });
        rebuilds.active = Some(ProxyRebuild {
            hash,
            page_index,
            format,
            receiver,
        });
        ctx.request_repaint_after(std::time::Duration::from_millis(30));
    });
}

pub(super) fn insert_proxy(
    app: &mut PlotiumApp,
    hash: [u8; 32],
    page_index: u32,
    proxy: plotium_io::image::ProxyImage,
) {
    app.session
        .ui
        .raster_proxies
        .retain(|entry| entry.hash != hash || entry.page_index != page_index);
    app.session
        .ui
        .raster_proxies
        .push(plotium_core::state::RasterProxy {
            hash,
            page_index,
            pixel_size: proxy.pixel_size,
            rgba8: Arc::from(proxy.rgba8),
        });
    while app
        .session
        .ui
        .raster_proxies
        .iter()
        .map(|entry| entry.rgba8.len())
        .sum::<usize>()
        > PROXY_BUDGET
        && app.session.ui.raster_proxies.len() > 1
    {
        app.session.ui.raster_proxies.remove(0);
    }
}

struct BoardView {
    center: [f32; 2],
    half_extent: [f32; 2],
    pixels_per_point: f32,
}

impl BoardView {
    fn shows(&self, origin: [f32; 2], size: [f32; 2]) -> bool {
        (0..2).all(|axis| {
            origin[axis] < self.center[axis] + self.half_extent[axis]
                && origin[axis] + size[axis] > self.center[axis] - self.half_extent[axis]
        })
    }

    /// Power-of-two buckets of the whole image's on-screen size, so a zoom
    /// gesture re-renders only at each doubling.
    fn preview_edge(&self, frame: [f32; 2], crop: [f32; 4]) -> u32 {
        let visible = (crop[2] - crop[0]).min(crop[3] - crop[1]).max(f32::EPSILON);
        let pixels = frame[0].max(frame[1]) / visible * self.pixels_per_point;
        (pixels.ceil().min(VECTOR_PREVIEW_MAX_EDGE as f32) as u32)
            .next_power_of_two()
            .clamp(VECTOR_PREVIEW_MIN_EDGE, VECTOR_PREVIEW_MAX_EDGE)
    }
}

/// Upgrades stop at the budget: past it, eviction would rebuild the same
/// previews forever instead of sharpening them.
fn fits_budget(edge: u32, held: usize) -> bool {
    held + (edge as usize).pow(2) * 4 <= PROXY_BUDGET
}

/// Raster previews never give way: they would come back at their full size.
fn release_off_screen_vectors(
    proxies: &mut Vec<plotium_core::state::RasterProxy>,
    vectors: &std::collections::BTreeSet<[u8; 32]>,
    shown: &std::collections::BTreeSet<([u8; 32], u32)>,
) {
    proxies.retain(|proxy| {
        !vectors.contains(&proxy.hash)
            || shown.contains(&(proxy.hash, proxy.page_index))
            || proxy.pixel_size[0].max(proxy.pixel_size[1]) <= VECTOR_PREVIEW_MIN_EDGE
    });
}

pub(super) fn rebuild_preview(
    bytes: &[u8],
    page_index: u32,
) -> Result<plotium_io::image::ProxyImage, plotium_io::image::ImageError> {
    if plotium_io::image::sniff(bytes) == plotium_io::image::RasterFormat::Svg {
        return plotium_io::image::rasterize_svg_rgba8(bytes, VECTOR_PREVIEW_MIN_EDGE);
    }
    if page_index == 0 {
        let edge = if plotium_io::image::probe(bytes)?.class
            == plotium_io::image::ResourceClass::ProxyRequired
        {
            2048
        } else {
            4096
        };
        return plotium_io::image::decode_proxy_rgba8(bytes, edge);
    }
    let decoded = plotium_io::image::decode_rgba8_page(bytes, page_index, false)?;
    let source =
        image::RgbaImage::from_raw(decoded.probe.width, decoded.probe.height, decoded.rgba8)
            .ok_or(plotium_io::image::ImageError::InvalidDecodedSize)?;
    let preview = if source.width() <= 4096 && source.height() <= 4096 {
        source
    } else {
        image::DynamicImage::ImageRgba8(source)
            .thumbnail(4096, 4096)
            .into_rgba8()
    };
    Ok(plotium_io::image::ProxyImage {
        pixel_size: [preview.width(), preview.height()],
        rgba8: preview.into_raw(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const VIEW: BoardView = BoardView {
        center: [100.0, 100.0],
        half_extent: [50.0, 50.0],
        pixels_per_point: 2.0,
    };
    const FULL: [f32; 4] = [0.0, 0.0, 1.0, 1.0];

    #[test]
    fn vector_preview_edge_follows_screen_size_in_bounded_buckets() {
        assert_eq!(VIEW.preview_edge([10.0, 5.0], FULL), 1024);
        assert_eq!(VIEW.preview_edge([513.0, 5.0], FULL), 2048);
        assert_eq!(VIEW.preview_edge([9000.0, 5.0], FULL), 4096);
    }

    #[test]
    fn cropped_previews_need_more_of_the_whole_image() {
        assert_eq!(VIEW.preview_edge([513.0, 5.0], [0.0, 0.0, 0.5, 1.0]), 4096);
    }

    #[test]
    fn only_frames_inside_the_view_are_shown() {
        assert!(VIEW.shows([90.0, 90.0], [20.0, 20.0]));
        assert!(VIEW.shows([40.0, 40.0], [11.0, 11.0]));
        assert!(!VIEW.shows([200.0, 90.0], [20.0, 20.0]));
    }

    #[test]
    fn only_sharp_off_screen_vector_previews_are_released() {
        let proxy = |id: u8, edge: u32| plotium_core::state::RasterProxy {
            hash: [id; 32],
            page_index: 0,
            pixel_size: [edge, edge],
            rgba8: Arc::from(Vec::new()),
        };
        let mut proxies = vec![
            proxy(1, 3000),
            proxy(2, 4096),
            proxy(3, 2048),
            proxy(4, 1024),
        ];
        let vectors = [[2; 32], [3; 32], [4; 32]].into_iter().collect();
        let shown = [([2; 32], 0)].into_iter().collect();
        release_off_screen_vectors(&mut proxies, &vectors, &shown);
        let kept: Vec<_> = proxies.iter().map(|proxy| proxy.hash[0]).collect();
        assert_eq!(kept, [1, 2, 4]);
    }

    #[test]
    fn sharper_previews_stop_at_the_budget() {
        assert!(fits_budget(2048, 0));
        assert!(!fits_budget(4096, PROXY_BUDGET));
    }
}
