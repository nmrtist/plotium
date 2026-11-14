//! Chrome motion: durations, spring parameters, and the reduced-motion policy.
//!
//! Every animation state is keyed per viewport, so two windows showing the
//! same canvas never drive one another's springs. Animations request repaints
//! only while they are moving, and only for the viewport that owns them.

use egui::{Color32, Id};
use plotium_core::settings::ReduceMotion;

/// Hover colour transitions. Presses show at once.
pub(crate) const HOVER_SECONDS: f32 = 0.10;
/// Dialog and command palette fade-in.
pub(crate) const APPEAR_SECONDS: f32 = 0.16;
/// Scale a dialog grows from while it fades in.
pub(crate) const APPEAR_SCALE_FROM: f32 = 0.96;
/// Spring period in seconds and damping ratio for sliding selections and
/// board camera glides.
const SPRING_RESPONSE: f32 = 0.34;
const SPRING_DAMPING: f32 = 0.82;
/// How often the platform accessibility setting is re-read while frames run.
const SYSTEM_POLL_SECONDS: f64 = 2.0;

const POLICY_ID: &str = "plotium_motion_reduced";
const SYSTEM_ID: &str = "plotium_motion_system";

/// Applies the preference once per frame. The platform setting is re-read
/// every few seconds so a change in the system settings takes effect without
/// restarting.
pub(crate) fn sync(ctx: &egui::Context, preference: ReduceMotion) {
    let reduced = match preference {
        ReduceMotion::On => true,
        ReduceMotion::Off => false,
        ReduceMotion::System => system_reduce_motion_cached(ctx),
    };
    let applied = ctx.data(|data| data.get_temp::<bool>(Id::new(POLICY_ID)));
    if applied == Some(reduced) {
        return;
    }
    let defaults = egui::Style::default();
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        ctx.style_mut_of(theme, |style| {
            if reduced {
                style.animation_time = 0.0;
                style.scroll_animation = egui::style::ScrollAnimation::none();
            } else {
                style.animation_time = defaults.animation_time;
                style.scroll_animation = defaults.scroll_animation;
            }
        });
    }
    ctx.data_mut(|data| data.insert_temp(Id::new(POLICY_ID), reduced));
}

pub(crate) fn reduced(ctx: &egui::Context) -> bool {
    ctx.data(|data| data.get_temp::<bool>(Id::new(POLICY_ID)))
        .unwrap_or(false)
}

fn system_reduce_motion_cached(ctx: &egui::Context) -> bool {
    let now = ctx.input(|input| input.time);
    let cached = ctx.data(|data| data.get_temp::<(f64, bool)>(Id::new(SYSTEM_ID)));
    if let Some((read_at, value)) = cached
        && now - read_at < SYSTEM_POLL_SECONDS
        && now >= read_at
    {
        return value;
    }
    let value = system_reduce_motion();
    ctx.data_mut(|data| data.insert_temp(Id::new(SYSTEM_ID), (now, value)));
    value
}

#[cfg(target_os = "macos")]
fn system_reduce_motion() -> bool {
    objc2_app_kit::NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceMotion()
}

#[cfg(windows)]
fn system_reduce_motion() -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SPI_GETCLIENTAREAANIMATION, SystemParametersInfoW,
    };

    let mut animations: windows_sys::core::BOOL = 1;
    // SAFETY: SPI_GETCLIENTAREAANIMATION writes one BOOL through the pointer.
    let ok = unsafe {
        SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            (&raw mut animations).cast(),
            0,
        )
    };
    if ok == 0 {
        static WARNED: std::sync::Once = std::sync::Once::new();
        WARNED.call_once(|| {
            log::warn!(
                "could not read the Windows animation setting: {}",
                std::io::Error::last_os_error()
            )
        });
        return false;
    }
    animations == 0
}

#[cfg(not(any(target_os = "macos", windows)))]
fn system_reduce_motion() -> bool {
    false
}

#[derive(Clone, Copy)]
struct Spring {
    pos: f32,
    vel: f32,
}

/// A damped spring integrated once per frame. Re-targeting mid-flight carries
/// the momentum, so a quick re-click redirects the glide instead of snapping.
/// With reduced motion the value lands on `target` immediately.
pub(crate) fn spring(ctx: &egui::Context, id: Id, target: f32, dt: f32) -> f32 {
    if reduced(ctx) {
        ctx.data_mut(|d| {
            d.insert_temp(
                id,
                Spring {
                    pos: target,
                    vel: 0.0,
                },
            )
        });
        return target;
    }
    let omega = std::f32::consts::TAU / SPRING_RESPONSE;
    let k = omega * omega;
    let c = 2.0 * SPRING_DAMPING * omega;
    let mut s = ctx
        .data_mut(|d| d.get_temp::<Spring>(id))
        .unwrap_or(Spring {
            pos: target,
            vel: 0.0,
        });
    let dt = dt.clamp(0.0, 1.0 / 30.0);
    let accel = -k * (s.pos - target) - c * s.vel;
    s.vel += accel * dt;
    s.pos += s.vel * dt;
    if (s.pos - target).abs() < 5e-4 && s.vel.abs() < 5e-4 {
        s.pos = target;
        s.vel = 0.0;
    } else {
        ctx.request_repaint();
    }
    ctx.data_mut(|d| d.insert_temp(id, s));
    s.pos
}

/// Seed a spring's stored position without disturbing its velocity, so the next
/// `spring` call glides from `pos` (the current value) rather than snapping to
/// a fresh target. Preserving velocity lets a re-seed mid-flight keep momentum.
pub(crate) fn seed_spring(ctx: &egui::Context, id: Id, pos: f32) {
    let vel = ctx
        .data_mut(|d| d.get_temp::<Spring>(id))
        .map_or(0.0, |s| s.vel);
    ctx.data_mut(|d| d.insert_temp(id, Spring { pos, vel }));
}

/// Hover emphasis from 0 to 1. It animates only for a moment after the
/// pointer enters or leaves, so an idle hover costs no frames.
pub(crate) fn hover(ctx: &egui::Context, id: Id, hovered: bool) -> f32 {
    let seconds = if reduced(ctx) { 0.0 } else { HOVER_SECONDS };
    ctx.animate_bool_with_time(id.with("hover").with(ctx.viewport_id()), hovered, seconds)
}

/// Eased progress from 0 to 1 since the surface `id` started showing. A
/// surface counts as newly shown when it was absent on the previous pass.
pub(crate) fn appear(ctx: &egui::Context, id: Id) -> f32 {
    let key = id.with("appear").with(ctx.viewport_id());
    let pass = ctx.cumulative_pass_nr();
    let now = ctx.input(|input| input.time);
    let started = ctx.data_mut(|data| {
        let started = match data.get_temp::<(u64, f64)>(key) {
            Some((last_pass, started)) if last_pass + 1 >= pass && started <= now => started,
            _ => now,
        };
        data.insert_temp(key, (pass, started));
        started
    });
    if reduced(ctx) {
        return 1.0;
    }
    let t = ((now - started) as f32 / APPEAR_SECONDS).clamp(0.0, 1.0);
    if t < 1.0 {
        ctx.request_repaint();
    }
    egui::emath::easing::cubic_out(t)
}

/// Scales a layer by `scale` around `pivot`, both in layer coordinates.
pub(crate) fn scale_about(pivot: egui::Pos2, scale: f32) -> egui::emath::TSTransform {
    use egui::emath::TSTransform;
    TSTransform::from_translation(pivot.to_vec2())
        * TSTransform::from_scaling(scale)
        * TSTransform::from_translation(-pivot.to_vec2())
}

pub(crate) fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_premultiplied(
        l(a.r(), b.r()),
        l(a.g(), b.g()),
        l(a.b(), b.b()),
        l(a.a(), b.a()),
    )
}

#[cfg(test)]
#[path = "motion_tests.rs"]
mod tests;
