use super::*;
use plotium_core::state::DEFAULT_CANVAS_SIZE_MM;

fn png_payload() -> Vec<u8> {
    let source = image::RgbaImage::from_pixel(2, 2, image::Rgba([1, 2, 3, 4]));
    let mut cursor = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(source)
        .write_to(&mut cursor, image::ImageFormat::Png)
        .unwrap();
    cursor.into_inner()
}

fn multipage_tiff_payload() -> Vec<u8> {
    use tiff::encoder::{TiffEncoder, colortype};
    let mut cursor = std::io::Cursor::new(Vec::new());
    {
        let mut encoder = TiffEncoder::new(&mut cursor).unwrap();
        encoder
            .write_image::<colortype::Gray8>(2, 1, &[10, 20])
            .unwrap();
        encoder
            .write_image::<colortype::RGB8>(1, 2, &[1, 2, 3, 4, 5, 6])
            .unwrap();
    }
    cursor.into_inner()
}

fn candidate(name: &str, pixel_size: [u32; 2]) -> Candidate {
    let bytes = png_payload();
    Candidate {
        basename: name.to_owned(),
        sha256: plotium_io::image::sha256(&bytes),
        preview: rebuild_preview(&bytes, 0).unwrap(),
        bytes: Arc::new(bytes),
        format: "png".to_owned(),
        pixel_size,
        auto_page_size_mm: physical_image_size_mm(pixel_size, None),
        page_index: 0,
        warning: None,
    }
}

fn ready_job(app: &mut PlotiumApp, target: ImportImageTarget) -> ImportJob {
    let (_events, receiver) = mpsc::channel();
    let (consent, _worker_consent) = mpsc::channel();
    ImportJob {
        project_context: app.session.project_context,
        request: ImportImageRequest {
            paths: Vec::new(),
            payloads: Vec::new(),
            target,
            allow_first_frame: false,
            strip_metadata: false,
            import_all_tiff_pages: false,
        },
        operation: app.session.begin_operation(),
        state: ImportImageState::ReadyToCommit,
        receiver,
        cancelled: Arc::new(AtomicBool::new(false)),
        consent,
        staged: Vec::new(),
    }
}

#[test]
fn worker_reports_ordered_states_and_a_ready_candidate() {
    let (sender, receiver) = mpsc::sync_channel(32);
    let (_consent_sender, consent) = mpsc::channel();
    worker(
        Vec::new(),
        vec![("sample.dat".to_owned(), png_payload())],
        Arc::new(AtomicBool::new(false)),
        sender,
        consent,
        ImportOptions {
            allow_first_frame: false,
            strip_metadata: false,
            import_all_tiff_pages: false,
        },
    );
    assert!(matches!(
        receiver.recv().unwrap(),
        WorkerEvent::State(ImportImageState::Probing)
    ));
    let WorkerEvent::Candidate(Ok(candidate)) = receiver.recv().unwrap() else {
        panic!("expected candidate");
    };
    assert!(matches!(
        receiver.recv().unwrap(),
        WorkerEvent::State(ImportImageState::ReadyToCommit)
    ));
    assert!(matches!(receiver.recv().unwrap(), WorkerEvent::Finished));
    assert_eq!(candidate.pixel_size, [2, 2]);
    assert_eq!(candidate.format, "png");
}

#[test]
fn cancelled_worker_releases_payload_without_a_commit_event() {
    let (sender, receiver) = mpsc::sync_channel(32);
    let (_consent_sender, consent) = mpsc::channel();
    worker(
        Vec::new(),
        vec![("sample.png".to_owned(), png_payload())],
        Arc::new(AtomicBool::new(true)),
        sender,
        consent,
        ImportOptions {
            allow_first_frame: false,
            strip_metadata: false,
            import_all_tiff_pages: false,
        },
    );
    assert!(matches!(
        receiver.recv().unwrap(),
        WorkerEvent::State(ImportImageState::Probing)
    ));
    assert!(matches!(
        receiver.recv().unwrap(),
        WorkerEvent::State(ImportImageState::Cancelled)
    ));
    assert!(receiver.try_recv().is_err());
}

#[test]
fn disconnected_worker_is_removed_and_reported() {
    let mut app = PlotiumApp::default();
    let job = ready_job(&mut app, ImportImageTarget::NewPages);
    MANAGER.with(|manager| manager.borrow_mut().jobs.push(job));

    poll(&mut app, &egui::Context::default());

    assert!(!has_active_jobs());
    let report = app
        .session
        .operation_history
        .operations()
        .next_back()
        .expect("the disconnected worker must produce an operation report");
    assert_eq!(report.kind, OperationKind::ImageImport);
    assert!(
        report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == DiagnosticCode::ImageImportFailed)
    );
}

#[test]
fn all_tiff_pages_share_original_bytes_and_keep_distinct_page_indices() {
    let (_sender_guard, sender) = mpsc::channel::<bool>();
    let (events, receiver) = mpsc::sync_channel(32);
    prepare_bytes(
        "stack.tiff".to_owned(),
        multipage_tiff_payload(),
        &events,
        &sender,
        &AtomicBool::new(false),
        ImportOptions {
            allow_first_frame: false,
            strip_metadata: false,
            import_all_tiff_pages: true,
        },
    )
    .unwrap();
    let candidates: Vec<_> = receiver
        .try_iter()
        .filter_map(|e| match e {
            WorkerEvent::Candidate(Ok(c)) => Some(c),
            _ => None,
        })
        .collect();
    assert_eq!(candidates.len(), 2);
    assert_eq!(candidates[0].page_index, 0);
    assert_eq!(candidates[1].page_index, 1);
    assert_eq!(candidates[1].pixel_size, [1, 2]);
    assert_eq!(candidates[1].preview.pixel_size, [1, 2]);
    assert_eq!(candidates[1].preview.rgba8, [1, 2, 3, 255, 4, 5, 6, 255]);
    assert!(Arc::ptr_eq(&candidates[0].bytes, &candidates[1].bytes));
}

#[test]
fn successful_import_into_empty_project_creates_one_undoable_page() {
    let mut app = PlotiumApp::default();
    assert!(app.doc.canvases.is_empty());
    let bytes = png_payload();
    let preview = rebuild_preview(&bytes, 0).unwrap();
    let candidate = Candidate {
        basename: "dropped.png".to_owned(),
        sha256: plotium_io::image::sha256(&bytes),
        bytes: Arc::new(bytes),
        format: "png".to_owned(),
        pixel_size: [2, 2],
        auto_page_size_mm: physical_image_size_mm([2, 2], None),
        page_index: 0,
        warning: None,
        preview,
    };
    let (_events, receiver) = mpsc::channel();
    let (consent, _worker_consent) = mpsc::channel();
    let mut job = ImportJob {
        project_context: app.session.project_context,
        request: ImportImageRequest {
            paths: Vec::new(),
            payloads: Vec::new(),
            target: ImportImageTarget::NewPages,
            allow_first_frame: false,
            strip_metadata: false,
            import_all_tiff_pages: false,
        },
        operation: app.session.begin_operation(),
        state: ImportImageState::ReadyToCommit,
        receiver,
        cancelled: Arc::new(AtomicBool::new(false)),
        consent,
        staged: Vec::new(),
    };

    commit(&mut app, &mut job, vec![Ok(candidate)]);
    assert_eq!(app.doc.canvases.len(), 1);
    assert_eq!(app.doc.canvases[0].objects.len(), 1);
    assert_eq!(app.doc.assets.len(), 1);
    assert_eq!(app.session.active_canvas, Some(0));
    assert_eq!(
        app.doc.canvases[0].objects[0].frame,
        ObjectFrame::new(
            0.0,
            0.0,
            app.doc.canvases[0].size_pt()[0],
            app.doc.canvases[0].size_pt()[1],
        )
    );

    let path = std::env::temp_dir().join(format!(
        "plotium-empty-project-image-import-{}.plotium",
        uuid::Uuid::new_v4()
    ));
    plotium_core::project::save_project(&app, &path, false).unwrap();
    let loaded = plotium_core::project::load_project(&path).unwrap();
    std::fs::remove_file(path).unwrap();
    assert_eq!(loaded.doc.canvases.len(), 1);
    assert_eq!(loaded.doc.canvases[0].objects.len(), 1);
    assert_eq!(loaded.doc.assets.len(), 1);

    app.undo();
    assert!(app.doc.canvases.is_empty());
    assert!(app.doc.assets.is_empty());
    assert_eq!(app.session.active_canvas, None);

    app.redo();
    assert_eq!(app.doc.canvases.len(), 1);
    assert_eq!(app.doc.canvases[0].objects.len(), 1);
    assert_eq!(app.doc.assets.len(), 1);
}

#[test]
fn multiple_images_into_empty_project_create_distinct_pages_in_source_order() {
    let mut app = PlotiumApp::default();
    let make_candidate = |name: &str, pixel_size: [u32; 2]| {
        let bytes = png_payload();
        Candidate {
            basename: name.to_owned(),
            sha256: plotium_io::image::sha256(&bytes),
            preview: rebuild_preview(&bytes, 0).unwrap(),
            bytes: Arc::new(bytes),
            format: "png".to_owned(),
            pixel_size,
            auto_page_size_mm: physical_image_size_mm(pixel_size, None),
            page_index: 0,
            warning: None,
        }
    };
    let (_events, receiver) = mpsc::channel();
    let (consent, _worker_consent) = mpsc::channel();
    let mut job = ImportJob {
        project_context: app.session.project_context,
        request: ImportImageRequest {
            paths: Vec::new(),
            payloads: Vec::new(),
            target: ImportImageTarget::NewPages,
            allow_first_frame: false,
            strip_metadata: false,
            import_all_tiff_pages: false,
        },
        operation: app.session.begin_operation(),
        state: ImportImageState::ReadyToCommit,
        receiver,
        cancelled: Arc::new(AtomicBool::new(false)),
        consent,
        staged: Vec::new(),
    };
    commit(
        &mut app,
        &mut job,
        vec![
            Ok(make_candidate("first.png", [400, 200])),
            Ok(make_candidate("second.png", [200, 400])),
        ],
    );
    assert_eq!(app.doc.canvases.len(), 2);
    assert_eq!(app.doc.canvases[0].objects[0].name, "first");
    assert_eq!(app.doc.canvases[1].objects[0].name, "second");
    assert_ne!(app.doc.canvases[0].size_mm, app.doc.canvases[1].size_mm);
    assert_eq!(app.session.undo_stack.len(), 1);
    assert_eq!(
        app.doc.canvases[0]
            .panel_letter(app.doc.canvases[0].objects[0].id)
            .as_deref(),
        Some("a")
    );
    app.undo();
    assert!(app.doc.canvases.is_empty());
    app.redo();
    assert_eq!(app.doc.canvases.len(), 2);
}

#[test]
fn sequential_new_page_batches_never_retarget_the_active_imported_page() {
    let mut app = PlotiumApp::default();
    app.doc.canvases.push(CanvasDocument::new(
        "Existing".to_owned(),
        DEFAULT_CANVAS_SIZE_MM,
    ));
    app.session.active_canvas = Some(0);

    let mut first = ready_job(&mut app, ImportImageTarget::NewPages);
    commit(
        &mut app,
        &mut first,
        vec![
            Ok(candidate("first.png", [400, 200])),
            Ok(candidate("second.png", [200, 400])),
        ],
    );
    assert_eq!(app.doc.canvases.len(), 3);
    assert_eq!(app.session.active_canvas, Some(2));
    assert!(app.doc.canvases[0].objects.is_empty());
    assert_eq!(app.session.undo_stack.len(), 1);

    let mut second = ready_job(&mut app, ImportImageTarget::NewPages);
    commit(
        &mut app,
        &mut second,
        vec![Ok(candidate("third.png", [300, 300]))],
    );
    assert_eq!(app.doc.canvases.len(), 4);
    assert_eq!(app.session.active_canvas, Some(3));
    assert_eq!(app.doc.canvases[2].objects.len(), 1);
    assert_eq!(app.doc.canvases[3].objects[0].name, "third");
    assert_eq!(app.session.undo_stack.len(), 2);

    app.undo();
    assert_eq!(app.doc.canvases.len(), 3);
    assert_eq!(app.session.active_canvas, Some(2));
    app.undo();
    assert_eq!(app.doc.canvases.len(), 1);
    assert_eq!(app.session.active_canvas, Some(0));
    app.redo();
    app.redo();
    assert_eq!(app.doc.canvases.len(), 4);
    assert_eq!(app.session.active_canvas, Some(3));
}

#[test]
fn panel_target_that_locks_before_commit_fails_without_loose_content() {
    let mut app = PlotiumApp::default();
    let mut page = CanvasDocument::new("Existing".to_owned(), DEFAULT_CANVAS_SIZE_MM);
    let panel = page.create_panel(
        "Panel".to_owned(),
        ObjectFrame::new(10.0, 10.0, 100.0, 80.0),
    );
    let canvas = page.resource_id;
    app.doc.canvases.push(page);
    app.session.active_canvas = Some(0);
    let mut job = ready_job(
        &mut app,
        ImportImageTarget::Panel {
            canvas,
            panel,
            position: Some([20.0, 20.0]),
        },
    );
    app.doc.canvases[0].panel_mut(panel).unwrap().locked = true;

    commit(
        &mut app,
        &mut job,
        vec![Ok(candidate("late.png", [200, 100]))],
    );

    assert_eq!(job.state, ImportImageState::Cancelled);
    assert!(app.doc.canvases[0].objects.is_empty());
    assert!(app.doc.assets.is_empty());
    assert!(app.session.undo_stack.is_empty());
    assert!(app.session.status.contains("failed"));
}

#[test]
fn failed_import_into_empty_project_does_not_create_a_page_or_undo_step() {
    let mut app = PlotiumApp::default();
    let (_events, receiver) = mpsc::channel();
    let (consent, _worker_consent) = mpsc::channel();
    let mut job = ImportJob {
        project_context: app.session.project_context,
        request: ImportImageRequest {
            paths: Vec::new(),
            payloads: Vec::new(),
            target: ImportImageTarget::NewPages,
            allow_first_frame: false,
            strip_metadata: false,
            import_all_tiff_pages: false,
        },
        operation: app.session.begin_operation(),
        state: ImportImageState::ReadyToCommit,
        receiver,
        cancelled: Arc::new(AtomicBool::new(false)),
        consent,
        staged: Vec::new(),
    };
    commit(
        &mut app,
        &mut job,
        vec![Err(failure(
            "broken.png",
            "PNG",
            "decode",
            "truncated payload".to_owned(),
            "Choose another image.",
        ))],
    );
    assert!(app.doc.canvases.is_empty());
    assert!(app.doc.assets.is_empty());
    assert_eq!(job.state, ImportImageState::Failed);
    app.undo();
    assert!(app.doc.canvases.is_empty());
}

#[test]
fn initial_image_size_uses_page_points_and_preserves_aspect() {
    let size = initial_image_size([510.0, 340.0], 2.0);
    assert_eq!(size, [331.5, 165.75]);
    let portrait = initial_image_size([510.0, 340.0], 0.5);
    assert!((portrait[0] - 110.5).abs() < 1e-4);
    assert!((portrait[1] - 221.0).abs() < 1e-4);
}

#[test]
fn vector_images_keep_their_natural_size_until_they_exceed_the_bounds() {
    assert_eq!(natural_vector_size([100, 40], [252.0, 170.0]), [75.0, 30.0]);
    let fitted = natural_vector_size([615, 461], [252.0, 170.0]);
    assert!((fitted[1] - 170.0).abs() < 1e-3);
    assert!((fitted[0] - 226.8).abs() < 0.1);
}

fn svg_candidate(pixel_size: [u32; 2]) -> Candidate {
    Candidate {
        format: "svg".to_owned(),
        ..candidate("structure.svg", pixel_size)
    }
}

#[test]
fn vector_image_on_a_page_stays_natural_and_on_the_page() {
    let mut app = PlotiumApp::default();
    let page = CanvasDocument::new("Page".to_owned(), DEFAULT_CANVAS_SIZE_MM);
    let [page_width, page_height] = page.size_pt();
    let canvas = page.resource_id;
    app.doc.canvases.push(page);
    let target = ImportImageTarget::Canvas {
        canvas,
        position: None,
    };

    let mut job = ready_job(&mut app, target);
    commit(
        &mut app,
        &mut job,
        vec![
            Ok(svg_candidate([100, 40])),
            Ok(svg_candidate([4000, 1000])),
        ],
    );
    let natural = app.doc.canvases[0].objects[0].frame;
    assert_eq!(
        [natural.x, natural.y, natural.width, natural.height],
        [12.0, 12.0, 75.0, 30.0]
    );
    let fitted = app.doc.canvases[0].objects[1].frame;
    assert_eq!(fitted.x, 0.0);
    assert!((fitted.width - page_width).abs() < 1e-3);
    assert!(fitted.y + fitted.height <= page_height);
}

#[test]
fn tiny_vector_images_keep_the_minimum_content_frame() {
    let mut app = PlotiumApp::default();
    let page = CanvasDocument::new("Page".to_owned(), DEFAULT_CANVAS_SIZE_MM);
    let canvas = page.resource_id;
    app.doc.canvases.push(page);
    let target = ImportImageTarget::Canvas {
        canvas,
        position: None,
    };

    let mut job = ready_job(&mut app, target);
    commit(&mut app, &mut job, vec![Ok(svg_candidate([1000, 1]))]);
    let frame = app.doc.canvases[0].objects[0].frame;
    assert!(frame.height >= 1.0);
    assert!(frame.width >= 1.0);
}

#[test]
fn vector_image_in_a_panel_fits_the_panel() {
    let mut app = PlotiumApp::default();
    let mut page = CanvasDocument::new("Page".to_owned(), DEFAULT_CANVAS_SIZE_MM);
    let panel = page.create_panel(
        "Panel".to_owned(),
        ObjectFrame::new(20.0, 30.0, 120.0, 90.0),
    );
    let canvas = page.resource_id;
    app.doc.canvases.push(page);
    let target = ImportImageTarget::Panel {
        canvas,
        panel,
        position: Some([40.0, 50.0]),
    };

    let mut job = ready_job(&mut app, target);
    commit(&mut app, &mut job, vec![Ok(svg_candidate([800, 400]))]);
    let frame = app.doc.canvases[0].objects[0].frame;
    assert_eq!(
        [frame.x, frame.y, frame.width, frame.height],
        [0.0, 20.0, 120.0, 60.0]
    );
}

#[test]
fn automatic_page_uses_physical_size_and_caps_width_at_nature_single_column() {
    let sized = physical_image_size_mm([600, 300], Some([300.0, 300.0]));
    assert!((sized[0] - 50.8).abs() < 1e-4);
    assert!((sized[1] - 25.4).abs() < 1e-4);

    let capped = physical_image_size_mm([2400, 1200], None);
    assert!((capped[0] - 89.0).abs() < 1e-4);
    assert!((capped[1] - 44.5).abs() < 1e-4);

    let portrait = physical_image_size_mm([1200, 2400], None);
    assert!((portrait[0] - 89.0).abs() < 1e-4);
    assert!((portrait[1] - 178.0).abs() < 1e-4);
}

#[test]
fn cancelled_batch_commits_prepared_images_as_one_undo_step() {
    let mut app = PlotiumApp::default();
    let mut job = ready_job(&mut app, ImportImageTarget::NewPages);
    let (sender, receiver) = mpsc::sync_channel(4);
    job.receiver = receiver;
    sender
        .send(WorkerEvent::Candidate(Ok(candidate(
            "prepared.png",
            [2, 2],
        ))))
        .unwrap_or_else(|_| panic!("test receiver must remain open"));
    sender
        .send(WorkerEvent::State(ImportImageState::Cancelled))
        .unwrap_or_else(|_| panic!("test receiver must remain open"));
    MANAGER.with(|manager| manager.borrow_mut().jobs.push(job));
    poll(&mut app, &egui::Context::default());
    assert_eq!(app.doc.canvases.len(), 1);
    assert_eq!(app.doc.canvases[0].objects[0].name, "prepared");
    assert!(!has_active_jobs());
    app.undo();
    assert!(app.doc.canvases.is_empty());
    assert!(app.doc.assets.is_empty());
}

#[test]
fn staged_candidates_release_decoded_allocations_before_commit() {
    let mut app = PlotiumApp::default();
    let mut job = ready_job(&mut app, ImportImageTarget::NewPages);
    let (sender, receiver) = mpsc::sync_channel(2);
    job.receiver = receiver;
    sender
        .send(WorkerEvent::Candidate(Ok(candidate(
            "prepared.png",
            [2, 2],
        ))))
        .unwrap_or_else(|_| panic!("test receiver must remain open"));
    MANAGER.with(|manager| manager.borrow_mut().jobs.push(job));
    poll(&mut app, &egui::Context::default());
    MANAGER.with(|manager| {
        let mut manager = manager.borrow_mut();
        let candidate = manager.jobs[0].staged[0].as_ref().ok().unwrap();
        assert_eq!(candidate.preview.rgba8.capacity(), 0);
        assert!(!candidate.bytes.is_empty());
        manager.jobs.clear();
    });
    assert!(app.doc.canvases.is_empty());
}

#[test]
fn project_switch_discards_prepared_image_batch() {
    let mut original = PlotiumApp::default();
    let mut job = ready_job(&mut original, ImportImageTarget::NewPages);
    job.staged.push(Ok(candidate("old-project.png", [2, 2])));
    let cancelled = Arc::clone(&job.cancelled);
    MANAGER.with(|manager| manager.borrow_mut().jobs.push(job));
    let mut other = PlotiumApp::default();
    poll(&mut other, &egui::Context::default());
    assert!(cancelled.load(Ordering::Relaxed));
    assert!(!has_active_jobs());
    assert!(other.doc.canvases.is_empty());
    assert!(other.session.undo_stack.is_empty());
}

#[test]
fn disconnected_worker_reports_and_keeps_prepared_images_once() {
    let mut app = PlotiumApp::default();
    let mut job = ready_job(&mut app, ImportImageTarget::NewPages);
    let (sender, receiver) = mpsc::sync_channel(2);
    job.receiver = receiver;
    sender
        .send(WorkerEvent::Candidate(Ok(candidate("ready.png", [2, 2]))))
        .unwrap_or_else(|_| panic!("test receiver remains open"));
    drop(sender);
    MANAGER.with(|manager| manager.borrow_mut().jobs.push(job));
    poll(&mut app, &egui::Context::default());
    assert_eq!(app.doc.canvases.len(), 1);
    assert_eq!(app.session.status, "Imported 1 image(s); 1 failed.");
    assert_eq!(app.session.undo_stack.len(), 1);
    assert!(!has_active_jobs());
    app.undo();
    assert!(app.doc.canvases.is_empty());
}
