use plotium_render::screen::ScreenRenderDetail;
use std::time::Duration;

const STATE_ID: &str = "plotium.workspace_interactive_render";
const INTERACTIVE_SECONDS: f64 = 0.2;

#[derive(Clone, Copy, Default)]
struct InteractiveRenderState {
    until: f64,
}

pub(super) fn mark_workspace_navigation(ctx: &egui::Context, now: f64) {
    let id = crate::ui::viewport_local_id(ctx, STATE_ID);
    ctx.data_mut(|data| {
        data.insert_temp(
            id,
            InteractiveRenderState {
                until: now + INTERACTIVE_SECONDS,
            },
        );
    });
    ctx.request_repaint_after(Duration::from_millis(200));
}

pub(super) fn workspace_render_detail(ctx: &egui::Context, now: f64) -> ScreenRenderDetail {
    let id = crate::ui::viewport_local_id(ctx, STATE_ID);
    let state = ctx.data_mut(|data| data.get_temp::<InteractiveRenderState>(id));
    let Some(state) = state else {
        return ScreenRenderDetail::Full;
    };
    if now < state.until {
        ctx.request_repaint_after(Duration::from_secs_f64(state.until - now));
        ScreenRenderDetail::Interactive
    } else {
        ctx.data_mut(|data| data.remove_temp::<InteractiveRenderState>(id));
        ScreenRenderDetail::Full
    }
}

/// `size` for board text that scales with the camera, which must not request
/// a new font size every frame while the camera moves.
pub(super) fn workspace_font_size(ctx: &egui::Context, size: f32) -> f32 {
    match workspace_render_detail(ctx, ctx.input(|input| input.time)) {
        ScreenRenderDetail::Full => size,
        ScreenRenderDetail::Interactive => plotium_render::rich_text::navigation_font_size(size),
    }
}

/// The caption font under a page; painting and hit testing must agree on it.
pub(super) fn board_caption_font(ctx: &egui::Context, zoom: f32) -> egui::FontId {
    egui::FontId::proportional(workspace_font_size(ctx, (11.0 * zoom).clamp(7.0, 28.0)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_navigation_detail_expires_after_quiet_period() {
        let ctx = egui::Context::default();
        assert_eq!(
            workspace_render_detail(&ctx, 10.0),
            ScreenRenderDetail::Full
        );
        mark_workspace_navigation(&ctx, 10.0);
        assert_eq!(
            workspace_render_detail(&ctx, 10.199),
            ScreenRenderDetail::Interactive
        );
        assert_eq!(
            workspace_render_detail(&ctx, 10.2),
            ScreenRenderDetail::Full
        );
    }
    #[test]
    fn board_text_snaps_only_during_navigation() {
        let ctx = egui::Context::default();
        assert_eq!(workspace_font_size(&ctx, 12.3), 12.3);
        mark_workspace_navigation(&ctx, 0.0);
        assert_eq!(
            workspace_font_size(&ctx, 12.3),
            plotium_render::rich_text::navigation_font_size(12.3)
        );
    }

    #[test]
    fn navigating_one_viewport_keeps_the_other_at_full_detail() {
        let ctx = egui::Context::default();
        let child = egui::ViewportId::from_hash_of("preview");
        ctx.set_embed_viewports(false);
        let mut input = egui::RawInput::default();
        input.viewports.insert(
            child,
            egui::ViewportInfo {
                parent: Some(egui::ViewportId::ROOT),
                ..Default::default()
            },
        );
        let _ = ctx.run_ui(input.clone(), |ui| {
            mark_workspace_navigation(ui.ctx(), 10.0);
            ui.ctx()
                .show_viewport_deferred(child, egui::ViewportBuilder::default(), |_, _| {});
        });
        input.viewport_id = child;
        let _ = ctx.run_ui(input.clone(), |ui| {
            assert_eq!(
                workspace_render_detail(ui.ctx(), 10.1),
                ScreenRenderDetail::Full
            );
        });
        input.viewport_id = egui::ViewportId::ROOT;
        let _ = ctx.run_ui(input, |ui| {
            assert_eq!(
                workspace_render_detail(ui.ctx(), 10.1),
                ScreenRenderDetail::Interactive
            );
        });
    }
}
