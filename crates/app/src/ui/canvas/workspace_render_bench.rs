use plotium_core::state::{PlotiumApp, build_render_document};
use plotium_render::screen::{RenderStats, ScreenRenderDetail};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const SCREEN_WIDTH: f32 = 1_600.0;
const SCREEN_HEIGHT: f32 = 1_000.0;
const WARMUP_FRAMES: usize = 3;
const MEASURED_FRAMES: usize = 30;

/// Exercises the desktop UI without a window/GPU. The external data stays out
/// of the repository; use the same build profile for before/after comparisons.
#[test]
#[ignore = "requires PLOTIUM_DOSY_REFERENCE_FILE"]
fn bench_dosy_desktop_frames() {
    let path = std::env::var_os("PLOTIUM_DOSY_REFERENCE_FILE").expect("reference JDF path");
    let mut app = PlotiumApp::new_with_settings(plotium_core::settings::Settings::default());
    let start = Instant::now();
    app.load_from(Path::new(&path));
    assert_eq!(app.doc.datasets.len(), 1, "{}", app.session.status);
    wait_for_compute(&mut app);
    println!("import/processing: {:?}", start.elapsed());
    desktop_frames(&mut app, "stack");
    let start = Instant::now();
    app.request_dosy_map(0);
    wait_for_compute(&mut app);
    app.set_pseudo_display(0, plotium_core::PseudoDisplay::DosyMap);
    wait_for_compute(&mut app);
    let data = app.doc.datasets[0].as_nmr2d().unwrap();
    let map = data.dosy_map.as_ref().expect("DOSY map");
    assert!(
        app.doc
            .canvases
            .iter()
            .flat_map(|canvas| &canvas.objects)
            .filter_map(|object| object.plot())
            .any(|plot| !plot.figure().contours.is_empty()),
        "the benchmark must display the map"
    );
    println!(
        "DOSY build: {:?}; acquired={}x{}; columns={} fitted={}",
        start.elapsed(),
        data.data.rows,
        data.data.cols,
        map.ppm.len(),
        map.d.iter().filter(|d| d.is_finite() && **d > 0.0).count(),
    );
    use std::hash::{Hash, Hasher};
    let mut digest = std::collections::hash_map::DefaultHasher::new();
    for value in map.ppm.iter().chain(&map.d).chain(&map.amp) {
        value.to_bits().hash(&mut digest);
    }
    format!("{:?}", map.diagnostics).hash(&mut digest);
    println!("DOSY numerical/diagnostic digest: {:016x}", digest.finish());
    let start = Instant::now();
    std::hint::black_box(app.doc.datasets[0].field_descriptors());
    println!("one descriptor discovery: {:?}", start.elapsed());
    desktop_frames(&mut app, "DOSY");
    run_detail(&app, ScreenRenderDetail::Full);
    run_detail(&app, ScreenRenderDetail::Interactive);
    if let Some(directory) = std::env::var_os("PLOTIUM_DOSY_BENCH_OUTPUT_DIR") {
        let directory = Path::new(&directory);
        std::fs::create_dir_all(directory).unwrap();
        let canvas = &app.doc.canvases[0];
        std::fs::write(
            directory.join("dosy.svg"),
            plotium_render::svg::export_document(&build_render_document(canvas)),
        )
        .unwrap();
        let raster = plotium_core::export::rasterize_canvas(
            canvas,
            plotium_core::export::RasterOptions::new(144),
        )
        .unwrap();
        std::fs::write(directory.join("dosy.png"), raster.to_png().unwrap()).unwrap();
    }
}

fn desktop_frames(app: &mut PlotiumApp, label: &str) {
    let ctx = crate::typography::test_context();
    let mut clipboard = crate::ui::clipboard_table::ClipboardTablePaste::default();
    let mut automation = crate::ui::batch_workflow::AutomationUi::default();
    let mut connections = crate::agent_connections::AgentConnections::default();
    let mut title = None;
    let mut samples = Vec::new();
    let mut tessellation = Vec::new();
    for frame in 0..13 {
        let start = Instant::now();
        let output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(SCREEN_WIDTH, SCREEN_HEIGHT),
                )),
                events: vec![egui::Event::PointerMoved(egui::pos2(
                    800.0 + frame as f32,
                    500.0,
                ))],
                ..Default::default()
            },
            |ui| {
                crate::ui::render(
                    app,
                    &mut clipboard,
                    &mut automation,
                    &mut connections,
                    &mut title,
                    ui,
                    false,
                    crate::ui::RibbonChrome::default(),
                );
            },
        );
        let ui_time = start.elapsed();
        let start = Instant::now();
        std::hint::black_box(ctx.tessellate(output.shapes, output.pixels_per_point));
        if frame >= 3 {
            samples.push(ui_time);
            tessellation.push(start.elapsed());
        }
    }
    samples.sort_unstable();
    tessellation.sort_unstable();
    println!(
        "{label} desktop UI: median={:?} p95={:?}; tessellation median={:?} p95={:?}",
        samples[5], samples[9], tessellation[5], tessellation[9]
    );
}

#[test]
#[ignore = "requires PLOTIUM_BENCH_NMR_DIR and a release build"]
fn bench_multi_page_nmr_workspace_navigation() {
    let Ok(directory) = std::env::var("PLOTIUM_BENCH_NMR_DIR") else {
        println!("PLOTIUM_BENCH_NMR_DIR not set; skipping benchmark");
        return;
    };
    let paths = sorted_jdf_paths(Path::new(&directory));
    assert!(
        !paths.is_empty(),
        "benchmark directory contains no JDF files"
    );

    let mut app = PlotiumApp::new_with_settings(plotium_core::settings::Settings::default());
    for path in &paths {
        let before = app.doc.datasets.len();
        app.load_from(path);
        assert_eq!(
            app.doc.datasets.len(),
            before + 1,
            "failed to load {}: {}",
            path.display(),
            app.session.status
        );
    }
    wait_for_compute(&mut app);

    println!(
        "loaded {} JDF file(s) into {} workspace page(s)",
        paths.len(),
        app.doc.canvases.len()
    );
    run_detail(&app, ScreenRenderDetail::Full);
    run_detail(&app, ScreenRenderDetail::Interactive);
}

fn sorted_jdf_paths(directory: &Path) -> Vec<PathBuf> {
    let mut paths = std::fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("read {}: {error}", directory.display()))
        .map(|entry| entry.expect("read benchmark directory entry").path())
        .filter(|path| {
            path.extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("jdf"))
        })
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

fn wait_for_compute(app: &mut PlotiumApp) {
    let start = Instant::now();
    while app.compute_busy() {
        assert!(
            start.elapsed() < Duration::from_secs(600),
            "background processing did not finish within ten minutes"
        );
        app.poll_compute();
        std::thread::sleep(Duration::from_millis(5));
    }
    // The completion that rebuilds a figure may enqueue its contour geometry.
    while app.poll_compute() {
        assert!(
            start.elapsed() < Duration::from_secs(600),
            "contour generation did not finish within ten minutes"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn run_detail(app: &PlotiumApp, detail: ScreenRenderDetail) {
    let ctx = egui::Context::default();
    let mut samples = Vec::with_capacity(MEASURED_FRAMES);
    let mut total_stats = RenderStats::default();
    for frame in 0..(WARMUP_FRAMES + MEASURED_FRAMES) {
        let pan = [frame as f32 * 3.0, frame as f32 * -2.0];
        let mut frame_stats = RenderStats::default();
        let start = Instant::now();
        let output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(SCREEN_WIDTH, SCREEN_HEIGHT),
                )),
                ..Default::default()
            },
            |ui| {
                let screen = plotium_render::Rect::new(0.0, 0.0, SCREEN_WIDTH, SCREEN_HEIGHT);
                for canvas in &app.doc.canvases {
                    let document = build_render_document(canvas);
                    plotium_render::screen::paint_document_for_editor_with_detail_and_stats(
                        ui.painter(),
                        screen,
                        &document,
                        plotium_render::DocumentViewport {
                            zoom: 1.0,
                            pan: [canvas.board_pos[0] + pan[0], canvas.board_pos[1] + pan[1]],
                        },
                        detail,
                        Some(&mut frame_stats),
                    );
                }
            },
        );
        let primitives = ctx.tessellate(output.shapes, output.pixels_per_point);
        std::hint::black_box(primitives.len());
        let elapsed = start.elapsed();
        if frame >= WARMUP_FRAMES {
            samples.push(elapsed);
            add_stats(&mut total_stats, &frame_stats);
        }
    }
    samples.sort_unstable();
    let median = samples[samples.len() / 2];
    let p95 = samples[(samples.len() * 95).div_ceil(100).saturating_sub(1)];
    println!(
        "{detail:?}: median={median:?} p95={p95:?} documents={} line visited/submitted={}/{} contour scanned/visited/submitted={}/{}/{}",
        total_stats.documents_painted,
        total_stats.line_source_points_visited,
        total_stats.line_points_submitted,
        total_stats.contour_source_segments_scanned,
        total_stats.contour_segments_visited,
        total_stats.contour_segments_submitted,
    );
}

fn add_stats(total: &mut RenderStats, frame: &RenderStats) {
    total.documents_painted += frame.documents_painted;
    total.full_documents_painted += frame.full_documents_painted;
    total.interactive_documents_painted += frame.interactive_documents_painted;
    total.line_series_visited += frame.line_series_visited;
    total.line_source_points_scanned += frame.line_source_points_scanned;
    total.line_points_emitted += frame.line_points_emitted;
    total.line_source_points_visited += frame.line_source_points_visited;
    total.line_points_submitted += frame.line_points_submitted;
    total.contour_source_segments_scanned += frame.contour_source_segments_scanned;
    total.contour_segments_visited += frame.contour_segments_visited;
    total.contour_segments_submitted += frame.contour_segments_submitted;
}
