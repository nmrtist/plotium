//! The operating system's accent colour, applied to selection chrome.
//!
//! macOS and Windows expose a user-chosen accent; Linux has no portable source,
//! so it keeps egui's stock blue. The accent is re-read whenever the window
//! regains focus, which is how a change made in System Settings arrives.

use egui::{Color32, Context, Id, Visuals};

use super::canvas::chrome::{blend, contrast_adjusted, contrast_ratio};

const APPLIED: &str = "plotium_applied_system_accent";
const FOCUSED: &str = "plotium_system_accent_window_focused";

/// Selected text and selected-row labels are body text.
const SELECTED_TEXT_CONTRAST: f32 = 4.5;
/// Keeps a selected row distinguishable from the panel; egui's stock fills
/// sit around 1.5.
const FILL_CONTRAST: f32 = 1.3;
/// Matches the contrast of egui's stock link colour, which also tints Ribbon
/// glyphs, so a system accent never reads weaker than the default.
const LINK_CONTRAST: f32 = 3.0;

/// Reads the platform accent. `None` keeps egui's defaults.
pub(crate) fn read() -> Option<Color32> {
    platform::read()
}

/// The accent the chrome was last styled with.
pub(crate) fn applied(ctx: &Context) -> Option<Color32> {
    ctx.data(|data| data.get_temp::<Option<Color32>>(Id::new(APPLIED)))
        .flatten()
}

/// What canvas chrome follows when no `canvas_accent` is set: the system
/// accent, else the theme's selection fill.
pub(crate) fn canvas_default(ctx: &Context, visuals: &Visuals) -> Color32 {
    applied(ctx).unwrap_or(visuals.selection.bg_fill)
}

pub(super) fn record(ctx: &Context, accent: Option<Color32>) {
    ctx.data_mut(|data| data.insert_temp(Id::new(APPLIED), accent));
}

/// True on the first frame after the window regains focus.
pub(super) fn regained_focus(ctx: &Context) -> bool {
    let focused = ctx.input(|input| input.focused);
    let previous = ctx.data_mut(|data| {
        let previous = data.get_temp::<bool>(Id::new(FOCUSED));
        data.insert_temp(Id::new(FOCUSED), focused);
        previous
    });
    focused && previous == Some(false)
}

/// Styles selection and link colours from `accent`, or restores egui's
/// defaults when there is none.
pub(super) fn apply(visuals: &mut Visuals, accent: Option<Color32>) {
    let stock = if visuals.dark_mode {
        Visuals::dark()
    } else {
        Visuals::light()
    };
    let Some(accent) = accent else {
        visuals.selection = stock.selection;
        visuals.hyperlink_color = stock.hyperlink_color;
        return;
    };
    let background = visuals.panel_fill;
    let tint = if visuals.dark_mode {
        blend(accent, background, 0.45)
    } else {
        blend(accent, background, 0.6)
    };
    // A near-white or near-black custom accent would otherwise vanish into the
    // panel and hide which row is selected.
    let fill = contrast_adjusted(tint, background, FILL_CONTRAST);
    let label_seed = if visuals.dark_mode {
        blend(accent, Color32::WHITE, 0.7)
    } else {
        accent
    };
    visuals.selection.bg_fill = fill;
    visuals.selection.stroke.color = readable_on(label_seed, fill, SELECTED_TEXT_CONTRAST);
    visuals.hyperlink_color = contrast_adjusted(accent, background, LINK_CONTRAST);
}

/// `contrast_adjusted` stops after a fixed number of steps; fall back to black
/// or white so selected text is always readable on a mid-tone accent fill.
fn readable_on(seed: Color32, background: Color32, minimum: f32) -> Color32 {
    let adjusted = contrast_adjusted(seed, background, minimum);
    if contrast_ratio(adjusted, background) >= minimum {
        return adjusted;
    }
    if contrast_ratio(Color32::BLACK, background) >= contrast_ratio(Color32::WHITE, background) {
        Color32::BLACK
    } else {
        Color32::WHITE
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use egui::Color32;
    use objc2_app_kit::{NSColor, NSColorSpace};

    pub(super) fn read() -> Option<Color32> {
        // `controlAccentColor` is a dynamic colour whose space is unspecified;
        // components are only defined after converting to a concrete space.
        let color =
            NSColor::controlAccentColor().colorUsingColorSpace(&NSColorSpace::sRGBColorSpace())?;
        let channel = |value: f64| (value.clamp(0.0, 1.0) * 255.0).round() as u8;
        Some(Color32::from_rgb(
            channel(color.redComponent()),
            channel(color.greenComponent()),
            channel(color.blueComponent()),
        ))
    }
}

#[cfg(windows)]
mod platform {
    use egui::Color32;
    use windows_sys::Win32::Graphics::Dwm::DwmGetColorizationColor;

    pub(super) fn read() -> Option<Color32> {
        let mut color = 0_u32;
        let mut opaque = 0;
        // SAFETY: both out-pointers refer to live locals for the call.
        let result = unsafe { DwmGetColorizationColor(&mut color, &mut opaque) };
        if result < 0 {
            log::warn!("Reading the Windows accent colour failed: HRESULT {result:#x}");
            return None;
        }
        // 0xAARRGGBB; the alpha is DWM's glass opacity, not part of the accent.
        let [_, r, g, b] = color.to_be_bytes();
        Some(Color32::from_rgb(r, g, b))
    }
}

#[cfg(not(any(target_os = "macos", windows)))]
mod platform {
    pub(super) fn read() -> Option<egui::Color32> {
        None
    }
}

#[cfg(test)]
#[path = "system_accent_tests.rs"]
mod tests;
