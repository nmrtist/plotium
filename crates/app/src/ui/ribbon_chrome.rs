use egui::{TextStyle, TextWrapMode, Ui, Vec2, WidgetText};
use egui_phosphor::regular as icon;
use plotium_core::state::{PlotiumApp, WorkflowTab};

const INLINE_TITLE_MAX_WIDTH: f32 = 240.0;
const INLINE_TITLE_MIN_WIDTH: f32 = 72.0;

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct RibbonChrome {
    pub(super) macos_traffic_lights: Option<Vec2>,
}

impl RibbonChrome {
    #[cfg(any(target_os = "macos", test))]
    pub(crate) fn macos(mut traffic_lights_size: Vec2, fullscreen: bool) -> Self {
        // AppKit hides the buttons in full screen, but still reports their
        // windowed frames. Reclaim their width without changing the row height.
        if fullscreen {
            traffic_lights_size.x = 0.0;
        }
        Self {
            macos_traffic_lights: Some(traffic_lights_size),
        }
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn current(ctx: &egui::Context, frame: &eframe::Frame) -> RibbonChrome {
    use raw_window_handle::HasWindowHandle;

    let size = frame
        .window_handle()
        .ok()
        .and_then(|handle| eframe::WindowChromeMetrics::from_window_handle(&handle.as_raw()))
        .map(|metrics| metrics.traffic_lights_size / ctx.zoom_factor())
        .unwrap_or(egui::vec2(76.0, 28.0));
    let fullscreen = ctx.input(|input| input.viewport().fullscreen.unwrap_or(false));
    RibbonChrome::macos(size, fullscreen)
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn current(_ctx: &egui::Context, _frame: &eframe::Frame) -> RibbonChrome {
    RibbonChrome::default()
}

pub(super) fn inline_project_title(app: &PlotiumApp) -> String {
    let project = app
        .doc
        .project_path
        .as_deref()
        .and_then(std::path::Path::file_name)
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Untitled".to_owned());
    if app.doc.dirty {
        format!("* {project}")
    } else {
        project
    }
}

pub(super) fn controls_need_compacting(
    app: &PlotiumApp,
    ui: &Ui,
    leading: f32,
    row_width: f32,
) -> bool {
    task_row_width(app, ui, leading, false) > row_width
}

fn task_row_width(app: &PlotiumApp, ui: &Ui, leading: f32, compact: bool) -> f32 {
    leading + task_tabs_width(ui) + controls_width(app, ui, compact) + 16.0
}

pub(super) fn available_title_width(
    app: &PlotiumApp,
    ui: &Ui,
    leading: f32,
    row_width: f32,
    compact_controls: bool,
) -> Option<f32> {
    let remaining = row_width
        - leading
        - task_tabs_width(ui)
        - controls_width(app, ui, compact_controls)
        - 24.0;
    (remaining >= INLINE_TITLE_MIN_WIDTH).then(|| remaining.min(INLINE_TITLE_MAX_WIDTH))
}

fn task_tabs_width(ui: &Ui) -> f32 {
    WorkflowTab::ALL
        .iter()
        .map(|tab| {
            text_width(
                ui,
                crate::typography::headline(tab.label()),
                TextStyle::Button,
            )
        })
        .sum::<f32>()
        + TASK_TAB_SPACING * (WorkflowTab::ALL.len().saturating_sub(1) as f32)
}

fn controls_width(app: &PlotiumApp, ui: &Ui, compact: bool) -> f32 {
    use plotium_core::update::UpdateStatus;

    let search = if compact {
        icon::MAGNIFYING_GLASS.to_owned()
    } else {
        format!("{} Search commands", icon::MAGNIFYING_GLASS)
    };
    let update = match app.session.updates.status() {
        UpdateStatus::Downloading { percent, .. } if compact => {
            percent.map_or_else(|| icon::ARROW_CLOCKWISE.to_owned(), |p| format!("{p}%"))
        }
        UpdateStatus::Downloading { percent, .. } => {
            percent.map_or_else(|| "Updating…".to_owned(), |p| format!("Updating… {p}%"))
        }
        UpdateStatus::Installed { .. } if compact => icon::ARROW_CLOCKWISE.to_owned(),
        UpdateStatus::Installed { .. } => {
            format!("{} Restart to update", icon::ARROW_CLOCKWISE)
        }
        _ => String::new(),
    };
    let spacing = CONTROL_SPACING;
    [search, update]
        .into_iter()
        .filter(|text| !text.is_empty())
        .map(|text| text_width(ui, text, TextStyle::Button))
        .sum::<f32>()
        // The two sidebar toggles, Ribbon toggle, separator, and
        // their share of the item spacing.
        + 3.0 * SIDEBAR_TOGGLE_WIDTH
        + 6.0
        + 5.0 * spacing
        // The follow, history, and AI connection buttons and their gaps.
        + 3.0 * (SIDEBAR_TOGGLE_WIDTH + CONTROL_SPACING)
}

/// Fixed width of one sidebar layout toggle; shared with the width estimate
/// in `controls_width` so compaction accounts for the pair.
pub(super) const SIDEBAR_TOGGLE_WIDTH: f32 = super::panel_chrome::BUTTON_WIDTH;

/// Fixed gap between task tabs. Command density applies only below this row,
/// so switching tabs cannot move the task buttons or trailing chrome.
pub(super) const TASK_TAB_SPACING: f32 = 8.0;

/// Gap between the trailing chrome controls.
pub(super) const CONTROL_SPACING: f32 = 8.0;

/// Always-visible sidebar toggle for the task row. The glyph mirrors the
/// window layout: the band marks which side the command controls and is
/// filled while that sidebar is visible, so the pair doubles as a live
/// layout indicator.
pub(super) fn sidebar_toggle_button(
    app: &mut PlotiumApp,
    clipboard: &mut super::clipboard_table::ClipboardTablePaste,
    ui: &mut Ui,
    id: super::commands::CommandId,
) {
    use super::commands;

    let command = commands::describe(app, id);
    let tip = match &command.shortcut {
        Some(shortcut) => format!("{} ({shortcut})", command.label),
        None => command.label.clone(),
    };
    let edge = if id == commands::CommandId::TogglePrimarySidebar {
        super::panel_chrome::Edge::Left
    } else {
        super::panel_chrome::Edge::Right
    };
    if super::panel_chrome::toggle(ui, edge, command.checked == Some(true), &tip).clicked() {
        commands::execute(id, app, clipboard, ui.ctx());
    }
}

fn text_width(ui: &Ui, text: impl Into<WidgetText>, fallback: TextStyle) -> f32 {
    text.into()
        .into_galley(ui, Some(TextWrapMode::Extend), f32::INFINITY, fallback)
        .size()
        .x
        + 2.0 * ui.spacing().button_padding.x
}

pub(super) fn frame(dark: bool) -> egui::Frame {
    // On macOS the task row occupies the native title-bar area; elsewhere it
    // continues the surface of the drawn title bar above it.
    let top = if cfg!(target_os = "macos") { 0 } else { 4 };
    super::window_shell::edge_frame(
        dark,
        egui::Margin {
            left: 8,
            right: 8,
            top,
            bottom: 8,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> PlotiumApp {
        PlotiumApp::new_with_settings(plotium_core::settings::Settings::default())
    }

    #[test]
    fn macos_fullscreen_reclaims_button_width_and_preserves_row_height() {
        let size = egui::vec2(76.0, 28.0);
        for fullscreen in [false, true, false] {
            let chrome = RibbonChrome::macos(size, fullscreen);
            assert_eq!(
                chrome.macos_traffic_lights,
                Some(egui::vec2(if fullscreen { 0.0 } else { size.x }, size.y))
            );
        }
    }

    #[test]
    fn inline_title_describes_clean_empty_and_modified_projects() {
        let mut app = app();
        assert_eq!(inline_project_title(&app), "Untitled");

        app.close_project();
        assert_eq!(inline_project_title(&app), "Untitled");

        app.doc.dirty = true;
        assert_eq!(inline_project_title(&app), "* Untitled");

        app.doc.project_path = Some(std::path::PathBuf::from("/tmp/report.plotium"));
        assert_eq!(inline_project_title(&app), "* report.plotium");
    }

    #[test]
    fn task_tabs_width_uses_the_fixed_tab_spacing() {
        let ctx = crate::typography::test_context();
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            let labels_width = WorkflowTab::ALL
                .iter()
                .map(|tab| {
                    text_width(
                        ui,
                        crate::typography::headline(tab.label()),
                        TextStyle::Button,
                    )
                })
                .sum::<f32>();
            let gaps = WorkflowTab::ALL.len().saturating_sub(1) as f32;

            assert_eq!(task_tabs_width(ui), labels_width + gaps * 8.0);
            assert_eq!(TASK_TAB_SPACING, 8.0);
        });
    }

    #[test]
    fn task_row_fits_the_minimum_window_width() {
        let app = app();
        let frame = frame(false);
        let row_width =
            crate::MIN_WINDOW_PT[0] - frame.outer_margin.sum().x - frame.inner_margin.sum().x;
        let ctx = crate::typography::test_context();
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            // No leading inset on Windows and Linux; the macOS traffic lights.
            for leading in [0.0, 80.0] {
                let compact = controls_need_compacting(&app, ui, leading, row_width);
                assert!(task_row_width(&app, ui, leading, compact) <= row_width);
            }
        });
    }
}
