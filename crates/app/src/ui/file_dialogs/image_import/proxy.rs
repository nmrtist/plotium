use super::*;

const PROXY_BUDGET: usize = 128 * 1024 * 1024;
const VECTOR_PREVIEW_MIN_EDGE: u32 = 1024;
const VECTOR_PREVIEW_MAX_EDGE: u32 = 4096;

struct ProxyRebuild {
    hash: [u8; 32],
    page_index: u32,
    format: String,
    edge: u32,
    receiver: mpsc::Receiver<Result<plotium_io::image::ProxyImage, String>>,
}

#[derive(Default)]
struct ProxyRebuilds {
    active: Option<ProxyRebuild>,
    #[cfg(test)]
    started: usize,
    context: Option<plotium_core::state::ProjectContextId>,
    completed: std::collections::BTreeMap<([u8; 32], u32), u32>,
    failed: std::collections::BTreeSet<([u8; 32], u32)>,
}

thread_local! {
    static REBUILDS: RefCell<ProxyRebuilds> = RefCell::new(ProxyRebuilds::default());
}

type ThumbnailDemand = std::collections::BTreeSet<CanvasId>;

pub(crate) fn request_thumbnail(ctx: &egui::Context, canvas: CanvasId) {
    ctx.data_mut(|data| {
        data.get_temp_mut_or_default::<ThumbnailDemand>(egui::Id::new("image_thumbnail_demand"))
            .insert(canvas);
    });
}

pub(crate) fn poll_rebuilds(app: &mut PlotiumApp, ctx: &egui::Context) {
    REBUILDS.with(|rebuilds| {
        let mut rebuilds = rebuilds.borrow_mut();
        if rebuilds.context != Some(app.session.project_context) {
            *rebuilds = ProxyRebuilds {
                context: Some(app.session.project_context),
                ..Default::default()
            };
        }
        let mut ready = None;
        if let Some(active) = &rebuilds.active {
            match active.receiver.try_recv() {
                Ok(Ok(proxy)) => {
                    ready = Some(((active.hash, active.page_index), active.edge, proxy));
                    rebuilds.active = None;
                }
                Ok(Err(reason)) => {
                    ctx.request_repaint();
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
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    ctx.request_repaint();
                    let format = active.format.clone();
                    let operation = app.session.begin_operation();
                    record_failures(
                        app,
                        operation,
                        0,
                        vec![failure(
                            "Embedded image",
                            &format,
                            "proxy_rebuild",
                            "The preview worker stopped unexpectedly.".to_owned(),
                            "Reopen the project to retry the preview.",
                        )],
                    );
                    let hash = active.hash;
                    let page_index = active.page_index;
                    rebuilds.failed.insert((hash, page_index));
                    rebuilds.active = None;
                }
            }
        }
        let viewport = crate::ui::canvas::navigation_rect(ctx);
        let view = viewport.map(|rect| BoardView {
            center: app.session.board.world_center,
            half_extent: (rect.size() * 0.5 / app.session.board.zoom).into(),
            pixels_per_point: app.session.board.zoom * ctx.pixels_per_point(),
        });
        // Allocate a fair square-pixel allowance across unique visible pages.
        // Missing pages and upgrades use the same allowance, so neither can
        // evict another visible page and restart the rebuild cycle.
        let thumbnails = ctx.data_mut(|data| {
            std::mem::take(
                data.get_temp_mut_or_default::<ThumbnailDemand>(egui::Id::new(
                    "image_thumbnail_demand",
                )),
            )
        });
        let mut demand = std::collections::BTreeMap::new();
        for canvas in &app.doc.canvases {
            for item in &canvas.objects {
                if !item.visible {
                    continue;
                }
                let CanvasObjectKind::RasterImage(image) = &item.kind else {
                    continue;
                };
                let Some(asset) = app.doc.assets.get(&image.asset) else {
                    continue;
                };
                let Some(frame) = canvas.content_page_frame(item.id) else {
                    continue;
                };
                let mut visible_frame = egui::Rect::from_min_size(
                    egui::pos2(frame.x, frame.y),
                    egui::vec2(frame.width, frame.height),
                );
                if let Some(panel) = canvas.parent_panel(item.id).and_then(|id| canvas.panel(id)) {
                    if !panel.visible {
                        continue;
                    }
                    if panel.clip_children {
                        let clip = panel.frame;
                        visible_frame = visible_frame.intersect(egui::Rect::from_min_size(
                            egui::pos2(clip.x, clip.y),
                            egui::vec2(clip.width, clip.height),
                        ));
                    }
                }
                if !visible_frame.is_positive() {
                    continue;
                }
                let origin = [
                    canvas.board_pos[0] + visible_frame.min.x,
                    canvas.board_pos[1] + visible_frame.min.y,
                ];
                let edge = if let Some(view) = view
                    .as_ref()
                    .filter(|view| view.shows(origin, visible_frame.size().into()))
                {
                    view.preview_edge([frame.width, frame.height], image.crop)
                } else if thumbnails.contains(&canvas.resource_id) {
                    256
                } else {
                    continue;
                };
                let entry = demand
                    .entry((asset.sha256, image.page_index))
                    .or_insert((asset, 0));
                entry.1 = entry.1.max(edge);
            }
        }
        let allowance = fair_edge(demand.len());
        for (_, edge) in demand.values_mut() {
            *edge = (*edge).min(allowance);
        }
        let proxies = &mut app.session.ui.raster_proxies;
        proxies.retain(|proxy| demand.contains_key(&(proxy.hash, proxy.page_index)));
        for proxy in proxies.iter_mut() {
            let edge = demand[&(proxy.hash, proxy.page_index)].1;
            if let Some((size, rgba8)) = shrink_preview(proxy.pixel_size, &proxy.rgba8, edge) {
                proxy.pixel_size = size;
                proxy.rgba8 = Arc::from(rgba8);
                rebuilds
                    .completed
                    .insert((proxy.hash, proxy.page_index), edge);
            }
        }
        // Recheck demand and fit the result into its current allowance before
        // admitting it. An obsolete result cannot displace a visible preview.
        if let Some(((hash, page_index), decoded_edge, mut proxy)) = ready
            && let Some((_, edge)) = demand.get(&(hash, page_index))
        {
            if let Some((size, rgba8)) = shrink_preview(proxy.pixel_size, &proxy.rgba8, *edge) {
                proxy.pixel_size = size;
                proxy.rgba8 = rgba8;
            }
            proxies.retain(|entry| entry.hash != hash || entry.page_index != page_index);
            proxies.push(plotium_core::state::RasterProxy {
                hash,
                page_index,
                pixel_size: proxy.pixel_size,
                rgba8: Arc::from(proxy.rgba8),
            });
            rebuilds
                .completed
                .insert((hash, page_index), decoded_edge.min(*edge));
            // The board and sidebar were already painted this frame.
            ctx.request_repaint();
        }
        if rebuilds.active.is_some() {
            return;
        }
        let candidate = demand.iter().find(|((hash, page), (_, edge))| {
            !rebuilds.failed.contains(&(*hash, *page))
                && !proxies.iter().any(|proxy| {
                    proxy.hash == *hash
                        && proxy.page_index == *page
                        && (proxy.pixel_size[0].max(proxy.pixel_size[1]) >= *edge
                            || rebuilds
                                .completed
                                .get(&(*hash, *page))
                                .is_some_and(|completed| completed >= edge))
                })
        });
        let Some(((_, page_index), (asset, edge))) = candidate else {
            return;
        };
        let page_index = *page_index;
        let edge = *edge;
        let hash = asset.sha256;
        let format = asset.format.clone();
        let bytes = asset.bytes.clone();
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let result = rebuild_preview_at_edge(&bytes, page_index, edge)
                .map_err(|error| error.to_string());
            // The receiver disappears when the application closes.
            if sender.send(result).is_err() {
                log::debug!("Image preview receiver closed");
            }
        });
        #[cfg(test)]
        {
            rebuilds.started += 1;
        }
        rebuilds.active = Some(ProxyRebuild {
            hash,
            page_index,
            format,
            edge,
            receiver,
        });
        ctx.request_repaint_after(std::time::Duration::from_millis(30));
    });
}

fn shrink_preview(size: [u32; 2], rgba8: &[u8], edge: u32) -> Option<([u32; 2], Vec<u8>)> {
    if size[0].max(size[1]) <= edge {
        return None;
    }
    let source = image::RgbaImage::from_raw(size[0], size[1], rgba8.to_vec())?;
    let resized = image::DynamicImage::ImageRgba8(source)
        .thumbnail(edge, edge)
        .into_rgba8();
    Some(([resized.width(), resized.height()], resized.into_raw()))
}

pub(super) fn insert_proxy(
    app: &mut PlotiumApp,
    hash: [u8; 32],
    page_index: u32,
    proxy: plotium_io::image::ProxyImage,
) {
    if proxy.rgba8.is_empty() {
        return;
    }
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

fn fair_edge(count: usize) -> u32 {
    let mut edge = VECTOR_PREVIEW_MAX_EDGE;
    while (edge as usize).pow(2) * 4 > PROXY_BUDGET / count.max(1) && edge > 1 {
        edge /= 2;
    }
    edge
}

pub(super) fn rebuild_preview(
    bytes: &[u8],
    page_index: u32,
) -> Result<plotium_io::image::ProxyImage, plotium_io::image::ImageError> {
    let edge = if plotium_io::image::sniff(bytes) == plotium_io::image::RasterFormat::Svg {
        VECTOR_PREVIEW_MIN_EDGE
    } else {
        256
    };
    rebuild_preview_at_edge(bytes, page_index, edge)
}

fn rebuild_preview_at_edge(
    bytes: &[u8],
    page_index: u32,
    edge: u32,
) -> Result<plotium_io::image::ProxyImage, plotium_io::image::ImageError> {
    if plotium_io::image::sniff(bytes) == plotium_io::image::RasterFormat::Svg {
        return plotium_io::image::rasterize_svg_rgba8(bytes, edge);
    }
    if page_index == 0 {
        return plotium_io::image::decode_proxy_rgba8(bytes, edge);
    }
    let decoded = plotium_io::image::decode_rgba8_page(bytes, page_index, false)?;
    let source =
        image::RgbaImage::from_raw(decoded.probe.width, decoded.probe.height, decoded.rgba8)
            .ok_or(plotium_io::image::ImageError::InvalidDecodedSize)?;
    let preview = if source.width() <= edge && source.height() <= edge {
        source
    } else {
        image::DynamicImage::ImageRgba8(source)
            .thumbnail(edge, edge)
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

    fn set_viewport(ctx: &egui::Context, size: f32) {
        crate::ui::canvas::store_navigation_rect(
            ctx,
            egui::Rect::from_min_size(egui::pos2(200.0, 100.0), egui::vec2(size, size)),
        );
    }

    #[test]
    fn viewport_and_panel_clipping_limit_demand_and_stale_result_is_dropped() {
        let ctx = egui::Context::default();
        set_viewport(&ctx, 100.0);
        let mut app = PlotiumApp::default();
        app.session.board.zoom = 1.0;
        let mut canvas = CanvasDocument::new("Images".into(), [89.0, 60.0]);
        for hash in 0..4_u8 {
            let asset = AssetId::new();
            app.doc.assets.insert(
                asset,
                AssetRecord {
                    id: asset,
                    sha256: [hash; 32],
                    format: "png".into(),
                    pixel_size: [1024, 1024],
                    bytes: Vec::new(),
                },
            );
            let object = canvas.allocate_object_id();
            canvas.objects.push(CanvasObject {
                id: object,
                name: "Image".into(),
                frame: ObjectFrame::new(if hash == 1 { 60.0 } else { 0.0 }, 0.0, 10.0, 10.0),
                locked: false,
                visible: true,
                kind: CanvasObjectKind::RasterImage(RasterImageContent::new(asset)),
            });
            let panel = canvas.create_panel_for_content(object).unwrap();
            if hash == 2 {
                canvas.panel_mut(panel).unwrap().visible = false;
            } else if hash == 3 {
                let panel = canvas.panel_mut(panel).unwrap();
                panel.clip_children = true;
                panel.frame.x = -20.0;
                canvas.object_mut(object).unwrap().frame.x = 20.0;
            }
        }
        app.doc.canvases.push(canvas);
        insert_proxy(
            &mut app,
            [0; 32],
            0,
            plotium_io::image::ProxyImage {
                pixel_size: [1, 1],
                rgba8: vec![0; 4],
            },
        );
        let (sender, receiver) = mpsc::channel();
        sender
            .send(Ok(plotium_io::image::ProxyImage {
                pixel_size: [4096, 4096],
                rgba8: vec![0; PROXY_BUDGET],
            }))
            .unwrap();
        REBUILDS.with(|state| {
            *state.borrow_mut() = ProxyRebuilds {
                context: Some(app.session.project_context),
                active: Some(ProxyRebuild {
                    hash: [1; 32],
                    page_index: 0,
                    format: "png".into(),
                    edge: 4096,
                    receiver,
                }),
                completed: [(([0; 32], 0), 1024)].into(),
                ..Default::default()
            };
        });
        poll_rebuilds(&mut app, &ctx);
        assert_eq!(REBUILDS.with(|state| state.borrow().started), 0);
        assert_eq!(app.session.ui.raster_proxies.len(), 1);
        assert_eq!(app.session.ui.raster_proxies[0].hash, [0; 32]);
    }

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
    fn twelve_large_previews_share_the_budget_without_eviction() {
        let edge = fair_edge(12);
        assert_eq!(edge, 1024);
        assert!((edge as usize).pow(2) * 4 * 12 <= PROXY_BUDGET);
        assert_eq!(fair_edge(1), 4096);
        assert!(fair_edge(500) < edge);
    }
    #[test]
    fn twelve_large_images_keep_all_decoded_previews_in_budget() {
        let mut app = PlotiumApp::default();
        let edge = fair_edge(12);
        let source = br#"<svg xmlns="http://www.w3.org/2000/svg" width="4000" height="3000"><rect width="4000" height="3000" fill="red"/></svg>"#;
        for id in 0..12 {
            let proxy = rebuild_preview_at_edge(source, 0, edge).unwrap();
            assert_eq!(proxy.pixel_size, [1024, 768]);
            insert_proxy(&mut app, [id; 32], 0, proxy);
        }
        assert_eq!(app.session.ui.raster_proxies.len(), 12);
        assert!(
            app.session
                .ui
                .raster_proxies
                .iter()
                .map(|p| p.rgba8.len())
                .sum::<usize>()
                <= PROXY_BUDGET
        );
    }
    #[test]
    fn asynchronous_large_raster_batch_converges_and_thumbnail_survives() {
        let source = image::RgbaImage::from_pixel(4000, 3000, image::Rgba([40, 80, 120, 255]));
        let mut encoded = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(source)
            .write_to(&mut encoded, image::ImageFormat::Png)
            .unwrap();
        let mut app = PlotiumApp::default();
        for id in 0..12 {
            let mut bytes = encoded.get_ref().clone();
            bytes.push(id);
            let asset = AssetId::new();
            app.doc.assets.insert(
                asset,
                AssetRecord {
                    id: asset,
                    sha256: plotium_io::image::sha256(&bytes),
                    format: "png".into(),
                    pixel_size: [4000, 3000],
                    bytes,
                },
            );
            let mut canvas = CanvasDocument::new(format!("Image {id}"), [89.0, 60.0]);
            let object = canvas.allocate_object_id();
            canvas.objects.push(CanvasObject {
                id: object,
                name: "Image".into(),
                frame: ObjectFrame::new(0.0, 0.0, 100.0, 75.0),
                locked: false,
                visible: true,
                kind: CanvasObjectKind::RasterImage(RasterImageContent::new(asset)),
            });
            canvas.create_panel_for_content(object).unwrap();
            app.doc.canvases.push(canvas);
        }
        let ctx = egui::Context::default();
        set_viewport(&ctx, 1000.0);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        loop {
            poll_rebuilds(&mut app, &ctx);
            if app.session.ui.raster_proxies.len() == 12
                && REBUILDS.with(|r| r.borrow().active.is_none())
            {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "previews did not converge"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let started = REBUILDS.with(|r| r.borrow().started);
        for _ in 0..20 {
            poll_rebuilds(&mut app, &ctx);
        }
        assert_eq!(REBUILDS.with(|r| r.borrow().started), started);
        assert_eq!(started, 12);
        assert!(
            app.session
                .ui
                .raster_proxies
                .iter()
                .map(|p| p.rgba8.len())
                .sum::<usize>()
                <= PROXY_BUDGET
        );
        // A visible sidebar row still demands its preview with the board elsewhere.
        app.session.board.world_center = [1e9, 1e9];
        let canvas = app.doc.canvases[0].resource_id;
        request_thumbnail(&ctx, canvas);
        poll_rebuilds(&mut app, &ctx);
        assert_eq!(app.session.ui.raster_proxies.len(), 1);
        assert!(app.session.ui.raster_proxies[0].pixel_size[0] <= 256);
        request_thumbnail(&ctx, canvas);
        poll_rebuilds(&mut app, &ctx);
        assert_eq!(REBUILDS.with(|r| r.borrow().started), started);
        app.session.board.world_center = [0.0, 0.0];
        app.session.board.zoom = 20.0;
        for canvas in app.doc.canvases.iter_mut().skip(1) {
            canvas.objects[0].visible = false;
        }
        loop {
            poll_rebuilds(&mut app, &ctx);
            if REBUILDS.with(|r| r.borrow().active.is_none()) {
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(app.session.ui.raster_proxies[0].pixel_size[0] > 256);
        let upgraded = REBUILDS.with(|r| r.borrow().started);
        for _ in 0..20 {
            poll_rebuilds(&mut app, &ctx);
        }
        assert_eq!(REBUILDS.with(|r| r.borrow().started), upgraded);
        let mut other = PlotiumApp::default();
        poll_rebuilds(&mut other, &ctx);
        assert_eq!(REBUILDS.with(|r| r.borrow().started), 0);
    }
    #[test]
    fn small_second_tiff_page_does_not_upgrade_forever() {
        let mut encoded = std::io::Cursor::new(Vec::new());
        {
            let mut encoder = tiff::encoder::TiffEncoder::new(&mut encoded).unwrap();
            encoder
                .write_image::<tiff::encoder::colortype::Gray8>(20, 20, &[0; 400])
                .unwrap();
            encoder
                .write_image::<tiff::encoder::colortype::Gray8>(1, 2, &[40, 80])
                .unwrap();
        }
        let bytes = encoded.into_inner();
        let mut app = PlotiumApp::default();
        let asset = AssetId::new();
        app.doc.assets.insert(
            asset,
            AssetRecord {
                id: asset,
                sha256: plotium_io::image::sha256(&bytes),
                format: "tiff".into(),
                pixel_size: [20, 20],
                bytes,
            },
        );
        let mut canvas = CanvasDocument::new("TIFF".into(), [89.0, 60.0]);
        let object = canvas.allocate_object_id();
        let mut raster = RasterImageContent::new(asset);
        raster.page_index = 1;
        canvas.objects.push(CanvasObject {
            id: object,
            name: "Page 2".into(),
            frame: ObjectFrame::new(0.0, 0.0, 100.0, 75.0),
            locked: false,
            visible: true,
            kind: CanvasObjectKind::RasterImage(raster),
        });
        canvas.create_panel_for_content(object).unwrap();
        app.doc.canvases.push(canvas);
        app.session.board.world_center = [1e9, 1e9];
        let canvas_id = app.doc.canvases[0].resource_id;
        let ctx = egui::Context::default();
        poll_rebuilds(&mut app, &ctx);
        assert_eq!(REBUILDS.with(|r| r.borrow().started), 0);
        // A newly rendered sidebar row starts work in the same render pass.
        request_thumbnail(&ctx, canvas_id);
        poll_rebuilds(&mut app, &ctx);
        assert_eq!(REBUILDS.with(|r| r.borrow().started), 1);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            request_thumbnail(&ctx, canvas_id);
            poll_rebuilds(&mut app, &ctx);
            if !app.session.ui.raster_proxies.is_empty()
                && REBUILDS.with(|r| r.borrow().active.is_none())
            {
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        for _ in 0..20 {
            request_thumbnail(&ctx, canvas_id);
            poll_rebuilds(&mut app, &ctx);
        }
        assert_eq!(REBUILDS.with(|r| r.borrow().started), 1);
        assert_eq!(app.session.ui.raster_proxies[0].pixel_size, [1, 2]);
    }
}
