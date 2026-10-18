//! The eframe/egui application shell. Non-UI glue lives in the `plotium-core` crate.

// Release Windows builds are GUI apps: suppress the console window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod agent_connections;
mod agent_install;
mod fonts;
mod graphics;
mod observability;
mod scale;
mod shell;
mod shot;
mod typography;
mod ui;

#[cfg(windows)]
use graphics::log_gl_adapter;
use graphics::{
    HIGH_PERFORMANCE_ARG, high_performance_requested, startup_renderer, wgpu_power_preference,
};
use plotium_core::settings::GraphicsPowerPreference;
use plotium_core::state::PlotiumApp;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const RECOVERY_INTERVAL: Duration = Duration::from_secs(60);
const RECOVERY_DEBOUNCE: Duration = Duration::from_secs(10);
const RECOVERY_RETRY_INTERVAL: Duration = Duration::from_secs(15);
/// Intended logical (point) size of a fresh main window; also the physical
/// pixel size it is created at before the UI scale is known.
pub(crate) const DEFAULT_WINDOW_PT: [f32; 2] = [1100.0, 700.0];

fn desired_maximum_frame_latency() -> Option<u32> {
    // eframe synchronizes Metal presentation with CoreAnimation transactions
    // during a live resize. Keep wgpu's default triple buffering on macOS so
    // that transaction cannot exhaust a forced two-drawable surface.
    #[cfg(target_os = "macos")]
    {
        None
    }
    #[cfg(not(target_os = "macos"))]
    {
        // Keep only one rendered frame queued so direct manipulation reaches
        // the display with the least presentation latency.
        Some(1)
    }
}

#[cfg(target_os = "macos")]
const APPLICATION_ICON_PNG: &[u8] = include_bytes!("../../../assets/icon-256-macos.png");
#[cfg(not(target_os = "macos"))]
const APPLICATION_ICON_PNG: &[u8] = include_bytes!("../../../assets/icon-256.png");

/// A verified update prepared by the service. It is handed to the helper only
/// after the GUI loop exits.
static PENDING_INSTALL: Mutex<Option<plotium_core::update::InstallPlan>> = Mutex::new(None);
static RELAUNCH_REQUESTED: AtomicBool = AtomicBool::new(false);
static SHOT_FAILURE: Mutex<Option<String>> = Mutex::new(None);

struct RecoveryJob {
    generation: u64,
    handle: std::thread::JoinHandle<Result<(), plotium_core::project::ProjectError>>,
}

struct ManualProjectSaveJob {
    operation_id: plotium_core::operation::OperationId,
    path: std::path::PathBuf,
    include_view_snapshots: bool,
    captured_generation: u64,
    continue_transition: bool,
    handle: std::thread::JoinHandle<
        Result<plotium_core::project::SaveOutcome, plotium_core::project::ProjectError>,
    >,
}

fn recovery_needed(dirty: bool, generation: u64, recovered: Option<u64>) -> bool {
    dirty && recovered != Some(generation)
}

fn transition_ready_after_save(
    saved: bool,
    captured_generation: u64,
    current_generation: u64,
) -> bool {
    saved && captured_generation == current_generation
}

fn application_icon() -> egui::IconData {
    eframe::icon_data::from_png_bytes(APPLICATION_ICON_PNG)
        .expect("embedded application icon PNG is valid")
}

pub(crate) fn record_shot_failure(error: String) {
    let mut failure = SHOT_FAILURE.lock().unwrap();
    if failure.is_none() {
        *failure = Some(error);
    }
}

/// Called by the "Restart to update" / "Restart now" buttons.
pub(crate) fn request_relaunch() {
    RELAUNCH_REQUESTED.store(true, Ordering::Relaxed);
}

pub(crate) fn cancel_relaunch() {
    RELAUNCH_REQUESTED.store(false, Ordering::Relaxed);
}

struct Shell {
    app: PlotiumApp,
    recovery: Option<plotium_core::project::RecoveryManager>,
    pending_recovery: Option<plotium_core::project::RecoverySnapshot>,
    pending_crash_report: Option<std::path::PathBuf>,
    recovery_job: Option<RecoveryJob>,
    manual_save_job: Option<ManualProjectSaveJob>,
    recovery_written: bool,
    last_recovered_generation: Option<u64>,
    observed_edit_generation: u64,
    recovery_deadline: Option<Instant>,
    next_recovery_at: Instant,
    clipboard_table_paste: ui::clipboard_table::ClipboardTablePaste,
    batch_workflow: ui::batch_workflow::AutomationUi,
    agent_connections: agent_connections::AgentConnections,
    shot: Option<shot::ShotDriver>,
    scale: scale::ScaleDriver,
    last_window_title: Option<String>,
    #[cfg(target_os = "macos")]
    native_menu: ui::native_menu::NativeMenu,
}

impl Shell {
    fn shutdown(&mut self) {
        self.agent_connections.stop();
        if let Some(job) = self.manual_save_job.take() {
            match job.handle.join() {
                Ok(Ok(_)) => {}
                Ok(Err(error)) => log::error!("project save failed on exit: {error}"),
                Err(_) => log::error!("project save worker panicked on exit"),
            }
        }
        if let Some(job) = self.recovery_job.take() {
            match job.handle.join() {
                Ok(Ok(())) => {}
                Ok(Err(error)) => log::error!("automatic recovery save failed on exit: {error}"),
                Err(_) => log::error!("automatic recovery worker panicked on exit"),
            }
        }
        if let Some(recovery) = self.recovery.take()
            && let Err(error) = recovery.shutdown()
        {
            log::error!("failed to clear crash-recovery snapshot on clean exit: {error}");
        }
        log::logger().flush();
    }
}

impl Shell {
    fn start_pending_project_save(&mut self, ctx: &egui::Context) {
        if self.manual_save_job.is_some() || self.recovery_job.is_some() {
            if self.app.session.ui.pending_project_save.is_some() {
                ctx.request_repaint_after(Duration::from_millis(100));
            }
            return;
        }
        let Some(pending) = self.app.session.ui.pending_project_save.take() else {
            return;
        };
        let operation_id = self.app.session.begin_operation();
        let captured_generation = self.app.doc.edit_generation;
        let request = plotium_core::project::prepare_project_save(
            &self.app,
            &pending.path,
            pending.include_view_snapshots,
        );
        self.app.session.status = format!("Saving project {}…", pending.path.display());
        self.manual_save_job = Some(ManualProjectSaveJob {
            operation_id,
            path: pending.path,
            include_view_snapshots: pending.include_view_snapshots,
            captured_generation,
            continue_transition: pending.continue_transition,
            handle: std::thread::spawn(move || {
                plotium_core::project::save_project_snapshot(request)
            }),
        });
        ctx.request_repaint_after(Duration::from_millis(100));
    }

    fn drive_project_transition(&mut self, ctx: &egui::Context) {
        use plotium_core::state::{ProjectTransition, ProjectTransitionPhase};

        let Some(mut transition) = self.app.session.ui.project_transition.clone() else {
            return;
        };
        let phase = shell::phase_after_polls(
            transition.phase,
            self.app.doc.dirty,
            self.manual_save_job.is_some() || self.recovery_job.is_some(),
        );
        if phase != transition.phase {
            transition.phase = phase;
            if let Some(pending) = self.app.session.ui.project_transition.as_mut() {
                pending.phase = phase;
            }
            ctx.request_repaint();
        }
        if phase == ProjectTransitionPhase::CheckUnsaved {
            ctx.request_repaint_after(Duration::from_millis(100));
            return;
        }
        if transition.phase == ProjectTransitionPhase::NeedsConfirmation
            && let Some(save) = self.manual_save_job.as_mut()
        {
            save.continue_transition = true;
            transition.phase = ProjectTransitionPhase::Saving;
            if let Some(pending) = self.app.session.ui.project_transition.as_mut() {
                pending.phase = ProjectTransitionPhase::Saving;
            }
            ctx.request_repaint_after(Duration::from_millis(100));
            return;
        }
        if transition.phase == ProjectTransitionPhase::NeedsConfirmation
            && let Some(save) = self.app.session.ui.pending_project_save.as_mut()
        {
            save.continue_transition = true;
            if let Some(pending) = self.app.session.ui.project_transition.as_mut() {
                pending.phase = ProjectTransitionPhase::Saving;
            }
            ctx.request_repaint_after(Duration::from_millis(100));
            return;
        }
        if transition.phase != ProjectTransitionPhase::Ready {
            return;
        }
        if self.manual_save_job.is_some() || self.recovery_job.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
            return;
        }

        let completed = match transition.target {
            ProjectTransition::New => {
                self.clear_recovery_for_project_swap();
                self.app.start_new_project();
                true
            }
            ProjectTransition::Close => {
                self.clear_recovery_for_project_swap();
                self.app.close_project();
                true
            }
            ProjectTransition::Open(path) => {
                let opened = self.app.load_project_from(&path);
                if opened {
                    self.clear_recovery_for_project_swap();
                }
                opened
            }
            ProjectTransition::Quit => {
                self.app.session.ui.project_transition = None;
                self.app.session.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                return;
            }
        };
        if !completed {
            self.app.session.ui.project_transition = None;
        }
        self.observed_edit_generation = self.app.doc.edit_generation;
        self.last_recovered_generation = None;
        self.recovery_deadline = None;
        self.next_recovery_at = Instant::now() + RECOVERY_INTERVAL;
    }

    fn clear_recovery_for_project_swap(&mut self) {
        if let Some(Err(error)) = self
            .recovery
            .as_ref()
            .map(plotium_core::project::RecoveryManager::clear_current)
        {
            self.app.session.status =
                format!("Project changed, but old recovery data could not be cleared: {error}");
        }
        self.recovery_written = false;
        self.last_recovered_generation = None;
    }

    fn tick_recovery(&mut self, ctx: &egui::Context) {
        let now = Instant::now();
        if self
            .recovery_job
            .as_ref()
            .is_some_and(|job| job.handle.is_finished())
        {
            let job = self
                .recovery_job
                .take()
                .expect("finished recovery worker is present");
            match job.handle.join() {
                Ok(Ok(())) => {
                    self.recovery_written = true;
                    self.last_recovered_generation = Some(job.generation);
                    self.recovery_deadline = None;
                    self.next_recovery_at = now + RECOVERY_INTERVAL;
                }
                Ok(Err(error)) => {
                    self.app.session.status = format!("Automatic recovery save failed: {error}");
                    self.next_recovery_at = now + RECOVERY_RETRY_INTERVAL;
                }
                Err(_) => {
                    self.app.session.status =
                        "Automatic recovery worker failed unexpectedly.".into();
                    self.next_recovery_at = now + RECOVERY_RETRY_INTERVAL;
                }
            }
        }
        if self.pending_recovery.is_some() {
            self.next_recovery_at = Instant::now() + RECOVERY_INTERVAL;
            return;
        }
        if !self.app.doc.dirty {
            if self.recovery_job.is_none() && self.recovery_written {
                match self
                    .recovery
                    .as_ref()
                    .map(plotium_core::project::RecoveryManager::clear_current)
                {
                    Some(Err(error)) => {
                        self.app.session.status =
                            format!("Saved project, but could not clear recovery data: {error}");
                        self.recovery_written = false;
                        self.last_recovered_generation = None;
                    }
                    _ => {
                        self.recovery_written = false;
                        self.last_recovered_generation = None;
                    }
                }
            }
            self.recovery_deadline = None;
            self.next_recovery_at = now + RECOVERY_INTERVAL;
            return;
        }
        let generation = self.app.doc.edit_generation;
        if !recovery_needed(
            self.app.doc.dirty,
            generation,
            self.last_recovered_generation,
        ) {
            self.recovery_deadline = None;
            self.next_recovery_at = now + RECOVERY_INTERVAL;
            return;
        }
        if self.observed_edit_generation != generation || self.recovery_deadline.is_none() {
            self.observed_edit_generation = generation;
            let deadline = self
                .recovery_deadline
                .get_or_insert(now + RECOVERY_INTERVAL);
            self.next_recovery_at = (now + RECOVERY_DEBOUNCE).min(*deadline);
        }
        if self.recovery_job.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
            return;
        }
        if self.manual_save_job.is_some() || self.app.session.ui.pending_project_save.is_some() {
            self.next_recovery_at = now + RECOVERY_RETRY_INTERVAL;
            ctx.request_repaint_after(Duration::from_millis(100));
            return;
        }
        if now >= self.next_recovery_at {
            if self.app.compute_busy() {
                self.next_recovery_at = now + RECOVERY_RETRY_INTERVAL;
                ctx.request_repaint_after(RECOVERY_RETRY_INTERVAL);
                return;
            }
            if let Some(recovery) = &self.recovery {
                let request = match plotium_core::project::prepare_recovery_snapshot(&self.app) {
                    Ok(request) => request,
                    Err(error) => {
                        self.app.session.status =
                            format!("Automatic recovery save failed: {error}");
                        self.next_recovery_at = now + RECOVERY_RETRY_INTERVAL;
                        ctx.request_repaint_after(RECOVERY_RETRY_INTERVAL);
                        return;
                    }
                };
                let target = recovery.target();
                self.recovery_job = Some(RecoveryJob {
                    generation,
                    handle: std::thread::spawn(move || {
                        plotium_core::project::save_recovery_snapshot(request, target)
                    }),
                });
                ctx.request_repaint_after(Duration::from_millis(100));
            } else {
                self.next_recovery_at = now + RECOVERY_INTERVAL;
            }
        }
        ctx.request_repaint_after(self.next_recovery_at.saturating_duration_since(now));
    }

    fn show_recovery_prompt(&mut self, ctx: &egui::Context) {
        let Some(snapshot) = self.pending_recovery.clone() else {
            return;
        };
        let mut recover = false;
        let mut discard = false;
        ui::modal(ctx, "recover_unsaved_project_modal", ui::ModalKind::Dialog).show(ctx, |ui| {
            ui.set_width(440.0);
            ui.heading("Recover unsaved project");
            ui.separator();
            ui.label(
                "Plotium found an automatic recovery snapshot left by an interrupted session.",
            );
            if let Some(path) = &snapshot.original_path {
                ui.small(format!("Original project: {}", path.display()));
            } else {
                ui.small("The recovered document had not been saved yet.");
            }
            if let Some(path) = &self.pending_crash_report {
                ui.small(format!("Crash report: {}", path.display()));
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Recover").clicked() {
                    recover = true;
                }
                if ui.button("Discard recovery").clicked() {
                    discard = true;
                }
            });
        });
        if (recover || discard) && self.pending_crash_report.take().is_some() {
            observability::acknowledge_crash_report();
        }

        if recover {
            match plotium_core::project::restore_recovery(&snapshot) {
                Ok(mut app) => {
                    let adopted = self
                        .recovery
                        .as_mut()
                        .ok_or_else(|| "recovery manager is unavailable".to_owned())
                        .and_then(|recovery| {
                            recovery.adopt(&snapshot).map_err(|error| error.to_string())
                        });
                    let cleanup_warning = match adopted {
                        Ok(warning) => warning,
                        Err(error) => {
                            self.app.session.status =
                                format!("Could not claim recovery data: {error}");
                            return;
                        }
                    };
                    app.session.status = cleanup_warning.map_or_else(
                        || "Recovered unsaved work. Save the project to make it permanent.".into(),
                        |warning| {
                            format!(
                                "Recovered unsaved work. Save the project to make it permanent. {warning}"
                            )
                        },
                    );
                    self.app.install_loaded_project(app);
                    self.pending_recovery = None;
                    self.recovery_written = true;
                    self.last_recovered_generation = Some(self.app.doc.edit_generation);
                    self.observed_edit_generation = self.app.doc.edit_generation;
                    self.recovery_deadline = None;
                    self.next_recovery_at = Instant::now() + RECOVERY_INTERVAL;
                }
                Err(error) => {
                    self.app.session.status = format!("Recovery failed: {error}");
                }
            }
        } else if discard {
            let result = self
                .recovery
                .as_mut()
                .ok_or_else(|| "recovery manager is unavailable".to_owned())
                .and_then(|recovery| {
                    recovery
                        .discard(&snapshot)
                        .map_err(|error| error.to_string())
                });
            match result {
                Ok(()) => self.pending_recovery = None,
                Err(error) => {
                    self.app.session.status = format!("Could not discard recovery data: {error}");
                }
            }
        }
    }
}

/// Register native drag hit testing, then restore the DWM frame effects lost
/// by undecorated windows. Cosmetic DWM failures are deliberately ignored.
#[cfg(windows)]
fn apply_windows_frame_polish(cc: &eframe::CreationContext<'_>) {
    use windows_sys::Win32::Graphics::Dwm::{
        DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DwmExtendFrameIntoClientArea,
        DwmSetWindowAttribute,
    };
    use windows_sys::Win32::UI::Controls::MARGINS;

    let Some(hwnd) = ui::file_drop::register_native_window(cc) else {
        return;
    };
    let corner = DWMWCP_ROUND;
    // SAFETY: hwnd comes from the live winit window; both DWM calls only read
    // the passed attribute structs.
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE as u32,
            (&raw const corner).cast(),
            size_of_val(&corner) as u32,
        );
        // A one-pixel frame sheet re-enables the DWM drop shadow; egui paints
        // opaque content over it, so nothing shows through.
        let margins = MARGINS {
            cxLeftWidth: 0,
            cxRightWidth: 0,
            cyTopHeight: 0,
            cyBottomHeight: 1,
        };
        let _ = DwmExtendFrameIntoClientArea(hwnd, &raw const margins);
    }
}

fn main() -> eframe::Result<()> {
    if let Some(code) = plotium_core::update::run_helper_from_args() {
        std::process::exit(code);
    }
    observability::initialize();
    plotium_core::update::cleanup_after_restart();
    let shot_active = std::env::var_os("PLOTIUM_SHOT").is_some();
    let settings = plotium_core::settings::load();
    let high_performance_override = high_performance_requested(std::env::args_os().skip(1));
    let graphics_power = if high_performance_override {
        log::warn!(
            "using the one-shot {HIGH_PERFORMANCE_ARG} override; the saved graphics preference is unchanged"
        );
        GraphicsPowerPreference::HighPerformance
    } else {
        settings.appearance.graphics_power
    };
    let inner = if shot_active {
        [1500.0, 1000.0]
    } else {
        DEFAULT_WINDOW_PT
    };
    let viewport = egui::ViewportBuilder::default()
        .with_inner_size(inner)
        .with_min_inner_size([720.0, 460.0])
        .with_title("Plotium")
        .with_icon(application_icon());
    let viewport = ui::configure_ribbon_viewport(viewport);
    let mut wgpu_options = eframe::egui_wgpu::WgpuConfiguration {
        desired_maximum_frame_latency: desired_maximum_frame_latency(),
        ..Default::default()
    };
    if let eframe::egui_wgpu::WgpuSetup::CreateNew(setup) = &mut wgpu_options.wgpu_setup {
        setup.power_preference = wgpu_power_preference(graphics_power);
    }
    let renderer = startup_renderer(graphics_power);
    log::info!("graphics preference: {graphics_power:?}; renderer: {renderer}");
    let native_options = eframe::NativeOptions {
        viewport,
        wgpu_options,
        renderer,
        ..Default::default()
    };
    let graphics_started = Arc::new(AtomicBool::new(false));
    let graphics_started_in_app = Arc::clone(&graphics_started);
    let run_result = eframe::run_native(
        "Plotium",
        native_options,
        Box::new(move |cc| {
            graphics_started_in_app.store(true, Ordering::Relaxed);
            #[cfg(windows)]
            if graphics_power == GraphicsPowerPreference::LowPower {
                log_gl_adapter(cc);
            }
            #[cfg(windows)]
            apply_windows_frame_polish(cc);
            cc.egui_ctx.set_fonts(fonts::definitions());
            typography::apply(&cc.egui_ctx);
            // The UI-scale commands own Ctrl+= / Ctrl+- / Ctrl+0 so the change
            // persists per monitor; egui's built-in handler would apply an
            // unrecorded zoom on top.
            cc.egui_ctx
                .options_mut(|options| options.zoom_with_keyboard = false);
            ui::apply_chrome_theme(&cc.egui_ctx, settings.appearance.theme);
            let updated = plotium_core::update::launched_after_update(&settings.app_version);
            let mut app = PlotiumApp::new_with_settings(settings);
            let mut recovery = match plotium_core::project::RecoveryManager::new() {
                Ok(recovery) => Some(recovery),
                Err(error) => {
                    app.session.status = format!("Could not initialize recovery storage: {error}");
                    None
                }
            };
            let pending_recovery = match recovery.as_mut() {
                Some(recovery) => match recovery.pending_recovery() {
                    Ok(snapshot) => snapshot,
                    Err(error) => {
                        app.session.status = format!("Could not inspect recovery data: {error}");
                        None
                    }
                },
                None => None,
            };
            let mut pending_crash_report = observability::pending_crash_report();
            let crash_notice = if pending_recovery.is_none() {
                pending_crash_report.take().map(|path| {
                    observability::acknowledge_crash_report();
                    format!(
                        "Plotium did not shut down cleanly last time. A crash report was saved to {}.",
                        path.display()
                    )
                })
            } else {
                None
            };
            if updated {
                app.session.status = format!("Updated to Plotium {}.", env!("CARGO_PKG_VERSION"));
                // Stamp the new version so the notice shows only once.
                app.settings.app_version = env!("CARGO_PKG_VERSION").to_owned();
                app.persist_settings();
            }
            if let Some(notice) = crash_notice {
                if updated {
                    app.session.status.push(' ');
                    app.session.status.push_str(&notice);
                } else {
                    app.session.status = notice;
                }
            }
            #[cfg(target_os = "macos")]
            let native_menu =
                ui::native_menu::NativeMenu::new(&app, &cc.egui_ctx).map_err(|error| {
                    std::io::Error::other(format!("failed to install macOS menu: {error}"))
                })?;
            let observed_edit_generation = app.doc.edit_generation;
            Ok(Box::new(Shell {
                app,
                recovery,
                pending_recovery,
                pending_crash_report,
                recovery_job: None,
                manual_save_job: None,
                recovery_written: false,
                last_recovered_generation: None,
                observed_edit_generation,
                recovery_deadline: None,
                next_recovery_at: Instant::now() + RECOVERY_INTERVAL,
                clipboard_table_paste: Default::default(),
                batch_workflow: Default::default(),
                agent_connections: Default::default(),
                shot: shot::ShotDriver::from_env(),
                // The screenshot harness scripts its own zoom; adaptive scale
                // must not fight it.
                scale: scale::ScaleDriver::new(!shot_active),
                last_window_title: None,
                #[cfg(target_os = "macos")]
                native_menu,
            }))
        }),
    );
    if let Err(error) = run_result {
        if graphics::recover_startup_error(
            &error,
            graphics_power,
            graphics_started.load(Ordering::Relaxed),
        ) {
            return Ok(());
        }
        return Err(error);
    }
    if let Some(error) = SHOT_FAILURE.lock().unwrap().take() {
        log::error!("screenshot harness failed: {error}");
        log::logger().flush();
        std::process::exit(1);
    }
    if let Some(plan) = PENDING_INSTALL.lock().unwrap().take()
        && let Err(error) = plan.launch(RELAUNCH_REQUESTED.load(Ordering::Relaxed))
    {
        log::error!("failed to launch update helper: {error}");
    }
    log::logger().flush();
    Ok(())
}

#[cfg(test)]
#[path = "main_tests.rs"]
mod tests;
