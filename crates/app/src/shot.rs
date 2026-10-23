//! Screenshot harness: set `PLOTIUM_SHOT` to an output directory and the app
//! drives a scripted synthetic session, writes PNG frame captures there, and
//! exits. Development aid only; inert without the env var.
//!
//! The session is a flat list of [`Scene`]s (see [`SCENES`]): each scene waits a
//! fixed number of frames for pending compute and layout to settle, then runs an
//! optional mutation and/or captures a named screenshot. To add or reorder a
//! capture, edit that list — no frame arithmetic is threaded across the file.
//!
//! Every scene is captured once per palette by replaying the whole list in the
//! light theme and again in the dark theme, so a UX audit gets matching pairs
//! without a second invocation. Set `PLOTIUM_SHOT_THEME` to `light` or `dark` to
//! restrict the run to a single palette. Captures land at
//! `<PLOTIUM_SHOT>/<theme>/<scene>.png`.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui::UserData;
use num_complex::Complex64;
use plotium_core::actions::Action;
use plotium_core::settings::Settings;
use plotium_core::state::{
    AnalysisSelection, AxisRange, DEFAULT_CANVAS_SIZE_MM, Dataset, FrameRef, LineShapeKind,
    Nmr2DDataset, NmrDataset, Peak2DOrigin, Peak2DPoint, Peak2DReview, PlotiumApp, Region,
    RegionId, Tool, WorkflowTab, XpsDataset, region_color,
};
use plotium_io::xps::{
    XpsEnergyKind, XpsExperiment, XpsMeasurement, XpsMeasurementId, XpsRegion, XpsRegionId,
};
use plotium_io::{AxisSource, Dim, Domain, NmrData, NmrData2D, PseudoAxis, PseudoKind, QuadMode};

mod capture;
mod scenes;
use scenes::SCENES;
mod craft_shot;
mod follow_shot;
mod thumbnail_shot;
mod workspace_shot;

const FIT_LO: f64 = 1.4;
const FIT_HI: f64 = 3.2;

/// Generous ceiling for the whole run (two palette passes). Real runs finish in
/// a few seconds; this only guards a wedged session so CI fails loudly.
const SHOT_TIMEOUT: Duration = Duration::from_secs(120);

/// Window size every pass starts from, so the light and dark capture of a scene
/// share dimensions instead of inheriting whatever the previous pass left.
const BASE_WINDOW: [f32; 2] = [1500.0, 1000.0];

/// A palette to replay the full scene list under.
#[derive(Clone, Copy)]
enum Theme {
    Light,
    Dark,
}

impl Theme {
    fn preference(self) -> egui::ThemePreference {
        match self {
            Theme::Light => egui::ThemePreference::Light,
            Theme::Dark => egui::ThemePreference::Dark,
        }
    }

    /// Output subdirectory name for this palette.
    fn label(self) -> &'static str {
        match self {
            Theme::Light => "light",
            Theme::Dark => "dark",
        }
    }
}

/// A mutation applied at the start of a scene, before its screenshot. Parameters
/// live in the variant so the scene list stays a plain data table.
#[derive(Clone, Copy)]
enum Op {
    Workspace(bool),
    ImportResults,
    /// Reset zoom and window size, then load the synthetic FID and arm a fit.
    Setup,
    /// Fit the FID band and frame the result.
    LineFit,
    Thumbnails,
    /// Load a homonuclear COSY plane and open its symmetry-review workflow.
    SymmetrySetup,
    /// Pin the ordinary readout cursor on the synthetic COSY.
    InspectCursor,
    /// Pin a two-point measurement on the synthetic COSY.
    DeltaCursor,
    /// Pin a detected cross-diagonal pair after the background audit settles.
    PinSymmetry,
    /// Build a live pseudo-2D region result and open its fit context.
    RegionResult,
    /// Open the result's synchronized read-only values.
    RegionData,
    History(bool),
    ScientificConflict,
    FollowResults(Option<bool>),
    XpsSetup,
    CraftSetup,
    PanelControls(bool),
    XpsTab(plotium_core::state::XpsWorkbenchTab),
    /// Show a Ribbon task tab. Sets the state directly (like [`Op::XpsTab`])
    /// so the capture shows the tab's command row without the side effects a
    /// real tab click has (opening task cards or sidebar tool groups).
    RibbonTab(plotium_core::state::WorkflowTab),
    Zoom(f32),
    Resize(f32, f32),
}

/// One scripted beat: advance `settle` frames so pending compute and layout
/// quiesce, then apply `op` (if any) and capture `shot` (if any) on that frame.
#[derive(Clone, Copy)]
struct Scene {
    settle: u32,
    op: Option<Op>,
    shot: Option<&'static str>,
}

const fn act(settle: u32, op: Op) -> Scene {
    Scene {
        settle,
        op: Some(op),
        shot: None,
    }
}

const fn shot(settle: u32, name: &'static str) -> Scene {
    Scene {
        settle,
        op: None,
        shot: Some(name),
    }
}

pub struct ShotDriver {
    dir: PathBuf,
    /// Palettes still to replay; one full pass over `SCENES` each.
    themes: Vec<Theme>,
    theme_idx: usize,
    scene_idx: usize,
    /// Frames left before the current scene fires.
    wait: u32,
    /// Whether the current pass's fresh document and theme have been applied.
    pass_primed: bool,
    saved: usize,
    /// Total captures across every palette pass.
    expected: usize,
    started_at: Instant,
    failed: bool,
    follow_capture: Option<follow_shot::Capture>,
}

impl ShotDriver {
    pub fn from_env() -> Option<Self> {
        let dir = std::env::var_os("PLOTIUM_SHOT")?;
        let themes = themes_from_env();
        let per_pass = SCENES.iter().filter(|s| s.shot.is_some()).count();
        Some(Self {
            dir: dir.into(),
            expected: per_pass * themes.len(),
            themes,
            theme_idx: 0,
            scene_idx: 0,
            wait: 0,
            pass_primed: false,
            saved: 0,
            started_at: Instant::now(),
            failed: false,
            follow_capture: None,
        })
    }

    pub fn drive(&mut self, app: &mut PlotiumApp, ctx: &egui::Context) {
        if self.failed {
            request_exit(app, ctx);
            return;
        }

        // Screenshots are delivered asynchronously a frame or two after the
        // request, so drain and save whatever has arrived every frame.
        self.collect(app, ctx);
        if self.failed {
            return;
        }
        if self.saved >= self.expected {
            request_exit(app, ctx);
            return;
        }
        if self.started_at.elapsed() >= SHOT_TIMEOUT {
            self.fail(
                app,
                ctx,
                format!(
                    "timed out after {}s with {}/{} screenshots saved",
                    SHOT_TIMEOUT.as_secs(),
                    self.saved,
                    self.expected
                ),
            );
            return;
        }

        self.step(app, ctx);
        if let Some(capture) = &self.follow_capture {
            capture.refresh(app);
        }
        if self.failed {
            return;
        }
        ctx.request_repaint();
    }

    /// Advance the scripted state machine, issuing screenshot requests. Exit is
    /// gated separately on `saved >= expected` so in-flight captures still land.
    fn step(&mut self, app: &mut PlotiumApp, ctx: &egui::Context) {
        if self.theme_idx >= self.themes.len() {
            // Every pass has been issued; idle while the last captures drain.
            return;
        }
        if !self.pass_primed {
            // Replay from an identical clean slate so a scene's light and dark
            // captures differ only in palette, never in accumulated state.
            *app = PlotiumApp::new_with_settings(Settings::default());
            ctx.set_theme(self.themes[self.theme_idx].preference());
            self.scene_idx = 0;
            self.wait = SCENES[0].settle;
            self.pass_primed = true;
            return; // let the fresh document and theme take effect next frame
        }
        if self.wait > 0 {
            self.wait -= 1;
            return;
        }

        let scene = SCENES[self.scene_idx];
        if let Some(Op::FollowResults(paused)) = scene.op {
            self.follow_capture = paused.map(|paused| follow_shot::Capture::new(app, paused));
        }
        if let Some(op) = scene.op
            && let Err(error) = run_op(op, app, ctx)
        {
            self.fail(app, ctx, error);
            return;
        }
        if let Some(name) = scene.shot {
            let theme = self.themes[self.theme_idx].label();
            shoot(ctx, format!("{theme}/{name}"));
        }

        self.scene_idx += 1;
        if self.scene_idx >= SCENES.len() {
            self.theme_idx += 1;
            self.pass_primed = false;
        } else {
            self.wait = SCENES[self.scene_idx].settle;
        }
    }

    fn collect(&mut self, app: &mut PlotiumApp, ctx: &egui::Context) {
        let shots: Vec<(String, Arc<egui::ColorImage>)> = ctx.input(|i| {
            i.events
                .iter()
                .filter_map(|e| match e {
                    egui::Event::Screenshot {
                        image, user_data, ..
                    } => {
                        let name = user_data.data.as_ref()?.downcast_ref::<String>()?.clone();
                        Some((name, image.clone()))
                    }
                    _ => None,
                })
                .collect()
        });
        for (rel, image) in shots {
            if let Err(error) = capture::save_png(&self.dir.join(format!("{rel}.png")), &image) {
                self.fail(app, ctx, format!("failed to save {rel}: {error}"));
                return;
            }
            self.saved += 1;
        }
    }

    fn fail(&mut self, app: &mut PlotiumApp, ctx: &egui::Context, error: String) {
        self.failed = true;
        crate::record_shot_failure(error);
        request_exit(app, ctx);
    }
}

/// Palettes to capture: both by default, or the single one named by
/// `PLOTIUM_SHOT_THEME`.
fn themes_from_env() -> Vec<Theme> {
    match std::env::var("PLOTIUM_SHOT_THEME").ok().as_deref() {
        Some("light") => vec![Theme::Light],
        Some("dark") => vec![Theme::Dark],
        _ => vec![Theme::Light, Theme::Dark],
    }
}

fn run_op(op: Op, app: &mut PlotiumApp, ctx: &egui::Context) -> Result<(), String> {
    match op {
        Op::Workspace(content) => workspace_shot::setup(app, content),
        Op::ImportResults => workspace_shot::imports(app),
        Op::Setup => {
            workspace_shot::setup(app, false);
            setup(app);
        }
        Op::LineFit => line_fit(app, ctx)?,
        Op::Thumbnails => thumbnail_shot::setup(app),
        Op::SymmetrySetup => symmetry_setup(app)?,
        Op::InspectCursor => inspect_cursor(app)?,
        Op::DeltaCursor => delta_cursor(app)?,
        Op::PinSymmetry => pin_symmetry(app)?,
        Op::RegionResult => region_result(app),
        Op::History(open) => workspace_shot::activity(app, ctx, open)?,
        Op::ScientificConflict => workspace_shot::scientific_conflict(app, ctx)?,
        Op::FollowResults(_) => {}
        Op::RegionData => {
            app.session.ui.sheet_open = Some(1);
            app.session.ui.curve_fit_task_collapsed = true;
        }
        Op::XpsSetup => xps_setup(app, ctx)?,
        Op::CraftSetup => craft_shot::setup(app, ctx)?,
        Op::PanelControls(expanded) => {
            app.session.ui.ribbon_expanded = expanded;
            app.session.ui.craft_task_collapsed = !expanded;
        }
        Op::XpsTab(tab) => app.session.ui.xps_workbench_tab = tab,
        Op::RibbonTab(tab) => app.session.ui.ribbon_tab = tab,
        Op::Zoom(factor) => ctx.set_zoom_factor(factor),
        Op::Resize(w, h) => {
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(w, h)));
        }
    }
    Ok(())
}

fn xps_setup(app: &mut PlotiumApp, ctx: &egui::Context) -> Result<(), String> {
    *app = PlotiumApp::new_with_settings(Settings::default());
    let energy = (0..241)
        .map(|index| 292.0 - index as f64 * 0.05)
        .collect::<Vec<_>>();
    let intensity = energy
        .iter()
        .map(|value| {
            150.0
                + (292.0 - value) * 2.0
                + plotium_analysis::xps::gl_peak(*value, 284.8, 1.15, 4_500.0, 0.3)
                + plotium_analysis::xps::gl_peak(*value, 286.3, 1.15, 1_900.0, 0.3)
        })
        .collect::<Vec<_>>();
    let measurement = XpsMeasurementId(1);
    let region = XpsRegionId(1);
    let experiment = XpsExperiment {
        source: "synthetic-xps.vms".into(),
        measurements: vec![XpsMeasurement {
            id: measurement,
            label: "Location 1".into(),
            position_mm: Some([0.0, 0.0, 0.0]),
            metadata: Default::default(),
        }],
        regions: vec![XpsRegion {
            id: region,
            measurement,
            name: "C 1s".into(),
            native_energy_kind: XpsEnergyKind::Binding,
            native_energy_ev: energy.clone(),
            binding_energy_ev: Some(energy.clone()),
            intensity_cps: intensity,
            counts: None,
            photon_energy_ev: Some(1486.69),
            dwell_time_s: Some(0.1),
            sweeps: Some(5),
            imported_fit: None,
            metadata: Default::default(),
        }],
        metadata: Default::default(),
        import_warnings: Vec::new(),
    };
    let mut xps = XpsDataset::load(experiment);
    let workspace = xps.fit_workspaces.get_mut(&region).unwrap();
    let first = plotium_analysis::xps::XpsComponentId::new(1);
    workspace.invocation.peaks = vec![
        plotium_analysis::xps::XpsPeakSpec::independent(first, "Aromatic C", 284.8, 4_000.0),
        plotium_analysis::xps::XpsPeakSpec {
            id: plotium_analysis::xps::XpsComponentId::new(2),
            label: "C=N / C-O".into(),
            center: plotium_analysis::xps::XpsCenterConstraint::Free {
                initial_ev: 286.3,
                bounds_ev: [285.8, 286.8],
            },
            fwhm: plotium_analysis::xps::XpsFwhmConstraint::Shared { reference: first },
            area: plotium_analysis::xps::XpsAreaConstraint::Free {
                initial: 1_800.0,
                bounds: [0.0, 36_000.0],
            },
        },
    ];
    workspace.next_component_id = 3;
    let dataset = xps.resource_id;
    let action = Action::insert_dataset_with_default_canvas(
        app,
        Dataset::Xps(Box::new(xps)),
        "Canvas 1 — synthetic XPS".into(),
        DEFAULT_CANVAS_SIZE_MM,
    );
    app.execute_action(action);
    app.run_xps_fit(dataset, region)?;
    app.session.ui.requested_tool_group = Some(plotium_core::state::ToolGroup::Xps);
    app.session.secondary_sidebar_visible = true;
    app.session.secondary_sidebar_width = 390.0;
    app.session.active_canvas = Some(0);
    crate::ui::canvas::request_board_fit(app, ctx, FrameRef::Page(0));
    Ok(())
}

fn request_exit(app: &mut PlotiumApp, ctx: &egui::Context) {
    // Synthetic sessions are deliberately dirty. They never represent user work,
    // so the harness must bypass the production Save / Discard / Cancel prompt.
    app.session.allow_close = true;
    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
}

fn shoot(ctx: &egui::Context, name: String) {
    ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(UserData::new(name)));
}

fn setup(app: &mut PlotiumApp) {
    let data = synthetic_fid();
    let action = Action::insert_dataset_with_default_canvas(
        app,
        Dataset::Nmr(Box::new(
            NmrDataset::load(data).expect("the synthetic FID is valid"),
        )),
        "Canvas 1 — synthetic".to_owned(),
        DEFAULT_CANVAS_SIZE_MM,
    );
    app.execute_action(action);
    app.set_tool(Tool::LineFit);
    if let Some(ci) = app.session.active_canvas
        && let Some(object) = app.doc.canvases[ci].active_plot_object_id()
    {
        app.session.ui.analysis_selection = Some(AnalysisSelection {
            dataset: app.doc.datasets[0].resource_id(),
            canvas: app.doc.canvases[ci].resource_id,
            object,
            x_range: AxisRange::new(FIT_LO, FIT_HI),
            y_range: None,
            field: None,
            unit: None,
            source_stream: None,
        });
    }
}

fn line_fit(app: &mut PlotiumApp, ctx: &egui::Context) -> Result<(), String> {
    app.run_line_fit(0, FIT_LO, FIT_HI, LineShapeKind::Lorentzian)
        .map_err(|error| format!("line fit failed: {error}"))?;
    app.session.ui.analysis_selection = None;
    app.session.active_canvas = Some(0);
    if let Some(id) = app.doc.canvases[0].active_plot_object_id() {
        app.select_object(0, id);
    }
    app.focus_single(0);
    crate::ui::canvas::request_board_fit(app, ctx, FrameRef::Page(0));
    Ok(())
}

fn symmetry_setup(app: &mut PlotiumApp) -> Result<(), String> {
    *app = PlotiumApp::new_with_settings(Settings::default());
    let mut dataset = Nmr2DDataset::load(synthetic_cosy()).expect("valid synthetic 2D acquisition");
    let ids = dataset
        .peaks
        .add_pair(
            Peak2DPoint {
                f2: 7.0,
                f1: 3.0,
                intensity: 1.0,
            },
            Peak2DPoint {
                f2: 3.0,
                f1: 7.0,
                intensity: 0.82,
            },
            [0.08, 0.08],
            Peak2DOrigin::Manual,
        )
        .map_err(|error| error.to_string())?;
    dataset.peaks.set_review(ids[0], Peak2DReview::Confirmed);
    let action = Action::insert_dataset_with_default_canvas(
        app,
        Dataset::Nmr2D(Box::new(dataset)),
        "Canvas 1 — synthetic COSY".to_owned(),
        DEFAULT_CANVAS_SIZE_MM,
    );
    app.execute_action(action);
    app.set_tool(Tool::Symmetry);
    app.session.secondary_sidebar_visible = true;
    app.session.ui.requested_tool_group = Some(plotium_core::state::ToolGroup::Nmr2dExperiment);
    app.start_symmetry_audit(0)
}

fn region_result(app: &mut PlotiumApp) {
    *app = PlotiumApp::new_with_settings(Settings::default());
    let mut dataset =
        Nmr2DDataset::load(synthetic_series()).expect("valid synthetic 2D acquisition");
    dataset.region_analysis.regions.push(Region {
        id: RegionId::new(0),
        lo: 4.65,
        hi: 5.35,
        name: "Main signal".to_owned(),
        label_position: None,
        color: region_color(0),
        metric: None,
    });
    dataset.region_analysis.next_region_id = RegionId::new(1);
    let action = Action::insert_dataset_with_default_canvas(
        app,
        Dataset::Nmr2D(Box::new(dataset)),
        "Canvas 1 — synthetic series".to_owned(),
        DEFAULT_CANVAS_SIZE_MM,
    );
    app.execute_action(action);
    app.create_region_table(0);
    crate::ui::tools::open_curve_fit_task(app, 1);
}

fn pin_symmetry(app: &mut PlotiumApp) -> Result<(), String> {
    app.poll_symmetry_audit();
    app.set_tool(Tool::Symmetry);
    app.session.ui.symmetry_pin = app.symmetry_reading(0, 7.0, 3.0, true);
    if app.session.ui.symmetry_pin.is_none() {
        return Err("synthetic COSY symmetry audit did not produce a reading".to_owned());
    }
    Ok(())
}

fn cursor_point(
    app: &PlotiumApp,
    f2: f64,
    f1: f64,
    intensity: f64,
) -> Result<plotium_core::state::CursorPoint, String> {
    let canvas = app
        .doc
        .canvases
        .first()
        .ok_or_else(|| "synthetic COSY canvas is missing".to_owned())?;
    let object = canvas
        .selected_plot_object_id()
        .or_else(|| canvas.active_plot_object_id())
        .ok_or_else(|| "synthetic COSY plot is missing".to_owned())?;
    let dataset = app
        .doc
        .datasets
        .first()
        .ok_or_else(|| "synthetic COSY dataset is missing".to_owned())?;
    Ok(plotium_core::state::CursorPoint {
        canvas: canvas.resource_id,
        object,
        dataset: dataset.resource_id(),
        x: f2,
        y: Some(f1),
        intensity,
    })
}

fn inspect_cursor(app: &mut PlotiumApp) -> Result<(), String> {
    app.set_tool(Tool::InspectCursor);
    app.session.ui.inspect_cursor_pin = Some(cursor_point(app, 7.0, 3.0, 1.0)?);
    Ok(())
}

fn delta_cursor(app: &mut PlotiumApp) -> Result<(), String> {
    app.set_tool(Tool::DeltaCursor);
    app.session.ui.delta_cursor_pin = Some(plotium_core::state::CursorDelta {
        first: cursor_point(app, 7.0, 3.0, 1.0)?,
        second: cursor_point(app, 3.0, 7.0, 0.82)?,
    });
    Ok(())
}

fn synthetic_fid() -> NmrData {
    let npoints = 4096;
    let (sw, obs, carrier) = (4000.0, 400.0, 5.0);
    let dt = 1.0 / sw;
    let peaks = [(1.8, 1.0, 0.15), (2.3, 0.6, 0.10), (2.6, 0.8, 0.20)];
    let points = (0..npoints)
        .map(|k| {
            let t = k as f64 * dt;
            peaks
                .iter()
                .map(|&(ppm, amp, t2): &(f64, f64, f64)| {
                    let freq_hz = (ppm - carrier) * obs;
                    Complex64::from_polar(
                        amp * (-t / t2).exp(),
                        std::f64::consts::TAU * freq_hz * t,
                    )
                })
                .sum()
        })
        .collect();
    NmrData {
        points,
        domain: Domain::Time,
        spectral_width_hz: sw,
        observe_freq_mhz: obs,
        carrier_ppm: carrier,
        nucleus: "1H".to_owned(),
        source: "synthetic".to_owned(),
        group_delay: 0.0,
    }
}

fn synthetic_series() -> NmrData2D {
    const ROWS: usize = 12;
    const COLS: usize = 128;
    let direct = Dim {
        spectral_width_hz: 1_000.0,
        observe_freq_mhz: 400.0,
        carrier_ppm: 5.0,
        nucleus: "1H".to_owned(),
        group_delay: 0.0,
    };
    let data = (0..ROWS)
        .flat_map(|row| {
            (0..COLS).map(move |col| {
                let x = (col as f64 - 64.0) / 8.0;
                let amplitude = (-(row as f64) / 5.0).exp();
                Complex64::new(amplitude * (-0.5 * x * x).exp(), 0.0)
            })
        })
        .collect();
    NmrData2D {
        data,
        rows: ROWS,
        cols: COLS,
        domain: Domain::Frequency,
        direct: direct.clone(),
        indirect: direct,
        quad: QuadMode::Complex,
        indirect_conjugate: false,
        experiment: Some("synthetic relaxation".to_owned()),
        pseudo_axis: Some(PseudoAxis {
            name: "Delay".to_owned(),
            kind: PseudoKind::Delay,
            values: (0..ROWS).map(|index| index as f64 * 0.1).collect(),
            unit: "s".to_owned(),
            source: AxisSource::EmbeddedList,
        }),
        diffusion: None,
        nus: None,
        source: "synthetic series".to_owned(),
    }
}

fn synthetic_cosy() -> plotium_io::NmrData2D {
    const SIDE: usize = 96;
    let mut data = vec![Complex64::new(0.0, 0.0); SIDE * SIDE];
    let peaks = [
        (29.0, 67.0, 1.0),
        (67.0, 29.0, 0.82),
        (48.0, 48.0, 1.7),
        (29.0, 29.0, 1.3),
        (67.0, 67.0, 1.1),
        (20.0, 76.0, 0.42),
    ];
    for row in 0..SIDE {
        for col in 0..SIDE {
            let signal = peaks
                .iter()
                .map(|&(cx, cy, amplitude): &(f64, f64, f64)| {
                    let dx = col as f64 - cx;
                    let dy = row as f64 - cy;
                    amplitude * (-(dx * dx + dy * dy) / 5.0).exp()
                })
                .sum::<f64>();
            let noise = (((row * 37 + col * 19) % 23) as f64 - 11.0) * 0.0008;
            data[row * SIDE + col] = Complex64::new(signal + noise, 0.0);
        }
    }
    let dimension = plotium_io::Dim {
        spectral_width_hz: 4_000.0,
        observe_freq_mhz: 400.0,
        carrier_ppm: 5.0,
        nucleus: "1H".to_owned(),
        group_delay: 0.0,
    };
    plotium_io::NmrData2D {
        data,
        rows: SIDE,
        cols: SIDE,
        domain: Domain::Frequency,
        direct: dimension.clone(),
        indirect: dimension,
        quad: plotium_io::QuadMode::Complex,
        indirect_conjugate: false,
        experiment: Some("cosy".to_owned()),
        pseudo_axis: None,
        diffusion: None,
        nus: None,
        source: "synthetic COSY".to_owned(),
    }
}

#[cfg(test)]
#[path = "shot/tests.rs"]
mod tests;
