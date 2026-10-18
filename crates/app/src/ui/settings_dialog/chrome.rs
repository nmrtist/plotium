//! Settings dialog chrome, navigation rail, theme, and footer.

use egui::{
    Align, Align2, CornerRadius, FontId, Layout, Response, RichText, Sense, Ui, pos2, vec2,
};
use egui_phosphor::regular as icon;
use plotium_core::settings::ThemeMode;
use plotium_core::state::{SettingsCategory, SettingsDialog};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ChromeGeometry {
    control: u8,
    surface: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DesktopPlatform {
    MacOs,
    Windows,
    Linux,
}

impl ChromeGeometry {
    const fn for_platform(platform: DesktopPlatform) -> Self {
        match platform {
            DesktopPlatform::MacOs => Self {
                control: 6,
                surface: 10,
            },
            DesktopPlatform::Windows => Self {
                control: 4,
                surface: 8,
            },
            DesktopPlatform::Linux => Self {
                control: 6,
                surface: 10,
            },
        }
    }

    const fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::for_platform(DesktopPlatform::MacOs)
        } else if cfg!(windows) {
            Self::for_platform(DesktopPlatform::Windows)
        } else {
            Self::for_platform(DesktopPlatform::Linux)
        }
    }
}

pub(crate) fn surface_corner_radius() -> CornerRadius {
    CornerRadius::same(ChromeGeometry::current().surface)
}

fn apply_chrome_geometry(style: &mut egui::Style, geometry: ChromeGeometry) {
    let control = CornerRadius::same(geometry.control);
    let widgets = &mut style.visuals.widgets;
    widgets.noninteractive.corner_radius = control;
    widgets.inactive.corner_radius = control;
    widgets.hovered.corner_radius = control;
    widgets.active.corner_radius = control;
    widgets.open.corner_radius = control;

    let surface = CornerRadius::same(geometry.surface);
    style.visuals.window_corner_radius = surface;
    style.visuals.menu_corner_radius = surface;
}

pub(crate) fn apply_chrome_theme(ctx: &egui::Context, mode: ThemeMode) {
    let pref = match mode {
        ThemeMode::System => egui::ThemePreference::System,
        ThemeMode::Light => egui::ThemePreference::Light,
        ThemeMode::Dark => egui::ThemePreference::Dark,
    };
    ctx.set_theme(pref);
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        ctx.style_mut_of(theme, |style| {
            apply_chrome_geometry(style, ChromeGeometry::current());
            // Disabled widgets keep the normal button fill and fade only via
            // `disabled_alpha`. Stock egui swaps in the near-panel
            // `noninteractive` fill, which makes light-theme buttons *brighten*
            // when a modal disables the chrome behind it.
            style.visuals.widgets.noninteractive.weak_bg_fill =
                style.visuals.widgets.inactive.weak_bg_fill;
        });
    }
    ctx.data_mut(|data| {
        data.insert_temp(egui::Id::new("plotium_applied_chrome_theme"), mode);
    });
}

pub(crate) fn sync_chrome_theme(ctx: &egui::Context, mode: ThemeMode) {
    let applied =
        ctx.data(|data| data.get_temp::<ThemeMode>(egui::Id::new("plotium_applied_chrome_theme")));
    if applied != Some(mode) {
        apply_chrome_theme(ctx, mode);
    }
}

pub(super) fn rail_row(ui: &mut Ui, cat: SettingsCategory, selected: bool) -> Response {
    let width = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(width, 30.0), Sense::click());
    let visuals = ui.visuals();
    let color = if selected || resp.hovered() {
        visuals.strong_text_color()
    } else {
        visuals.text_color()
    };
    if selected {
        ui.painter().rect_filled(
            rect,
            visuals.widgets.active.corner_radius,
            visuals.selection.bg_fill,
        );
    } else if resp.hovered() {
        ui.painter().rect_filled(
            rect,
            visuals.widgets.hovered.corner_radius,
            visuals.widgets.hovered.bg_fill,
        );
    }
    let cy = rect.center().y;
    let painter = ui.painter();
    painter.text(
        pos2(rect.left() + 14.0, cy),
        Align2::LEFT_CENTER,
        rail_icon(cat),
        FontId::proportional(15.0),
        color,
    );
    painter.text(
        pos2(rect.left() + 38.0, cy),
        Align2::LEFT_CENTER,
        cat.label(),
        crate::typography::headline_font(),
        color,
    );
    resp
}

fn rail_icon(cat: SettingsCategory) -> &'static str {
    match cat {
        SettingsCategory::General => icon::GEAR_SIX,
        SettingsCategory::Appearance => icon::PALETTE,
        SettingsCategory::Processing => icon::WAVEFORM,
        SettingsCategory::Export => icon::EXPORT,
        SettingsCategory::Recent => icon::CLOCK_COUNTER_CLOCKWISE,
        SettingsCategory::AgentConnections => icon::PLUG,
    }
}

pub(super) fn footer(ui: &mut Ui, dialog: &SettingsDialog) -> (bool, bool) {
    let mut done = false;
    let mut reset = false;
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        if ui.button("Reset to Defaults").clicked() {
            reset = true;
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.button("Done").clicked() {
                done = true;
            }
            if let Some(err) = &dialog.last_error {
                ui.add_space(10.0);
                let color = ui.visuals().error_fg_color;
                ui.label(RichText::new(err).small().color(color));
            }
        });
    });
    (done, reset)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_geometry_uses_the_desktop_radius_hierarchy() {
        assert_eq!(
            ChromeGeometry::for_platform(DesktopPlatform::MacOs),
            ChromeGeometry {
                control: 6,
                surface: 10,
            }
        );
        assert_eq!(
            ChromeGeometry::for_platform(DesktopPlatform::Windows),
            ChromeGeometry {
                control: 4,
                surface: 8,
            }
        );
        assert_eq!(
            ChromeGeometry::for_platform(DesktopPlatform::Linux),
            ChromeGeometry {
                control: 6,
                surface: 10,
            }
        );
    }

    #[test]
    fn every_widget_state_and_surface_receive_the_same_level_radius() {
        for platform in [
            DesktopPlatform::MacOs,
            DesktopPlatform::Windows,
            DesktopPlatform::Linux,
        ] {
            let geometry = ChromeGeometry::for_platform(platform);
            for visuals in [egui::Visuals::light(), egui::Visuals::dark()] {
                let mut style = egui::Style {
                    visuals,
                    ..Default::default()
                };
                apply_chrome_geometry(&mut style, geometry);
                let control = CornerRadius::same(geometry.control);
                let widgets = &style.visuals.widgets;
                assert_eq!(widgets.noninteractive.corner_radius, control);
                assert_eq!(widgets.inactive.corner_radius, control);
                assert_eq!(widgets.hovered.corner_radius, control);
                assert_eq!(widgets.active.corner_radius, control);
                assert_eq!(widgets.open.corner_radius, control);

                let surface = CornerRadius::same(geometry.surface);
                assert_eq!(style.visuals.window_corner_radius, surface);
                assert_eq!(style.visuals.menu_corner_radius, surface);
            }
        }
    }
}
