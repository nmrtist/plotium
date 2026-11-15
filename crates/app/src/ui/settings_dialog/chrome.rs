//! Settings dialog chrome, navigation rail, theme, and footer.

use egui::{
    Align, Align2, CornerRadius, FontId, Layout, Response, RichText, Sense, Ui, pos2, vec2,
};
use egui_phosphor::regular as icon;
use plotium_core::settings::ThemeMode;
use plotium_core::state::{SettingsCategory, SettingsDialog};

use crate::ui::system_accent;

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
                control: 8,
                surface: 12,
            },
            DesktopPlatform::Windows => Self {
                control: 4,
                surface: 8,
            },
            DesktopPlatform::Linux => Self {
                control: 8,
                surface: 12,
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

/// Muted chrome text (Ribbon group titles, hints) must reach 4.5:1 on the card
/// fill while staying visibly lighter than labels. Opaque greys keep that
/// independent of egui's alpha-derived weak colour; dark labels are raised so
/// both constraints fit.
fn apply_text_contrast(visuals: &mut egui::Visuals) {
    let (text, weak) = if visuals.dark_mode {
        (180, 140)
    } else {
        (80, 112)
    };
    visuals.widgets.noninteractive.fg_stroke.color = egui::Color32::from_gray(text);
    visuals.weak_text_color = Some(egui::Color32::from_gray(weak));
}

pub(crate) fn apply_chrome_theme(ctx: &egui::Context, mode: ThemeMode) {
    let pref = match mode {
        ThemeMode::System => egui::ThemePreference::System,
        ThemeMode::Light => egui::ThemePreference::Light,
        ThemeMode::Dark => egui::ThemePreference::Dark,
    };
    ctx.set_theme(pref);
    let accent = system_accent::read();
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        ctx.style_mut_of(theme, |style| {
            apply_chrome_geometry(style, ChromeGeometry::current());
            system_accent::apply(&mut style.visuals, accent);
            // Disabled widgets keep the normal button fill and fade only via
            // `disabled_alpha`. Stock egui swaps in the near-panel
            // `noninteractive` fill, which makes light-theme buttons *brighten*
            // when a modal disables the chrome behind it.
            style.visuals.widgets.noninteractive.weak_bg_fill =
                style.visuals.widgets.inactive.weak_bg_fill;
            apply_text_contrast(&mut style.visuals);
        });
    }
    ctx.data_mut(|data| {
        data.insert_temp(egui::Id::new("plotium_applied_chrome_theme"), mode);
    });
    system_accent::record(ctx, accent);
}

pub(crate) fn sync_chrome_theme(ctx: &egui::Context, mode: ThemeMode) {
    let applied =
        ctx.data(|data| data.get_temp::<ThemeMode>(egui::Id::new("plotium_applied_chrome_theme")));
    let accent_changed =
        system_accent::regained_focus(ctx) && system_accent::read() != system_accent::applied(ctx);
    if applied != Some(mode) || accent_changed {
        apply_chrome_theme(ctx, mode);
    }
}

pub(super) fn rail_row(ui: &mut Ui, cat: SettingsCategory, selected: bool) -> Response {
    let width = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(width, 30.0), Sense::click());
    let hover = crate::ui::motion::hover(ui.ctx(), resp.id, resp.hovered() && !selected);
    let visuals = ui.visuals();
    let color = if selected {
        visuals.strong_text_color()
    } else {
        crate::ui::motion::lerp_color(visuals.text_color(), visuals.strong_text_color(), hover)
    };
    if selected {
        ui.painter().rect_filled(
            rect,
            visuals.widgets.active.corner_radius,
            visuals.selection.bg_fill,
        );
    } else if hover > 0.0 {
        ui.painter().rect_filled(
            rect,
            visuals.widgets.hovered.corner_radius,
            visuals.widgets.hovered.bg_fill.gamma_multiply(hover),
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
    let mut job = egui::text::LayoutJob::simple_singleline(
        cat.label().into(),
        crate::typography::headline_font(),
        color,
    );
    job.wrap = egui::text::TextWrapping::truncate_at_width((width - 46.0).max(0.0));
    let galley = ui.fonts_mut(|fonts| fonts.layout_job(job));
    let text_top = cy - galley.size().y / 2.0;
    painter.galley(pos2(rect.left() + 38.0, text_top), galley, color);
    resp.on_hover_text(cat.label())
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
                control: 8,
                surface: 12,
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
                control: 8,
                surface: 12,
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

    fn relative_luminance(color: egui::Color32) -> f32 {
        let channel = |value: u8| {
            let c = f32::from(value) / 255.0;
            if c <= 0.039_28 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(color.r()) + 0.7152 * channel(color.g()) + 0.0722 * channel(color.b())
    }

    fn contrast(a: egui::Color32, b: egui::Color32) -> f32 {
        let (la, lb) = (relative_luminance(a), relative_luminance(b));
        (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
    }

    #[test]
    fn chrome_text_meets_contrast_on_cards_and_panels() {
        for dark in [false, true] {
            let mut visuals = if dark {
                egui::Visuals::dark()
            } else {
                egui::Visuals::light()
            };
            apply_text_contrast(&mut visuals);
            let card = crate::ui::card_frame(dark, egui::Margin::ZERO).fill;
            let text = visuals.text_color();
            let weak = visuals.weak_text_color();
            for surface in [card, visuals.panel_fill] {
                assert!(contrast(weak, surface) >= 4.5, "dark={dark} {surface:?}");
                assert!(contrast(text, surface) >= 7.0, "dark={dark} {surface:?}");
            }
            assert!(contrast(text, card) >= 1.5 * contrast(weak, card));
        }
    }
}
