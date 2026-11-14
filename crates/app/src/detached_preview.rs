//! Opt-in probe for a native, read-only window over a live canvas.

use std::sync::{Arc, Mutex};

use egui::{Context, Id, Pos2, Rect, Vec2, ViewportCommand, ViewportId};
use plotium_core::state::{CanvasId, PlotiumApp, ProjectContextId};
use plotium_render::{DocumentViewport, screen};

#[derive(Clone, Copy)]
struct Target {
    project: ProjectContextId,
    canvas: CanvasId,
}

#[derive(Default)]
pub(crate) struct DetachedPreview {
    target: Option<Target>,
    revision: Option<(u64, u64)>,
    camera: Arc<Mutex<Option<Camera>>>,
}

#[derive(Clone, Copy)]
struct Camera {
    zoom: f32,
    center: Vec2,
}

pub(crate) fn viewport_id() -> ViewportId {
    ViewportId::from_hash_of("plotium.detached_preview")
}

fn request_id() -> Id {
    Id::new("plotium.detached_preview.open")
}

pub(crate) fn active_canvas(app: &PlotiumApp) -> Option<CanvasId> {
    let index = app.session.active_canvas?;
    Some(app.doc.canvases.get(index)?.resource_id)
}

pub(crate) fn request_open(app: &PlotiumApp, ctx: &Context, canvas: Option<CanvasId>) {
    if let Some(canvas) = canvas.or_else(|| active_canvas(app)) {
        ctx.data_mut(|data| {
            data.insert_temp(
                request_id(),
                Some(Target {
                    project: app.session.project_context,
                    canvas,
                }),
            );
        });
        ctx.request_repaint_of(ViewportId::ROOT);
    }
}

fn canvas_index(app: &PlotiumApp, target: Target) -> Option<usize> {
    if app.session.project_context != target.project {
        return None;
    }
    app.doc
        .canvases
        .iter()
        .position(|canvas| canvas.resource_id == target.canvas)
}

impl DetachedPreview {
    #[cfg(target_os = "macos")]
    pub(crate) fn is_focused(&self, ctx: &Context) -> bool {
        self.target.is_some()
            && ctx.input(|input| {
                input
                    .raw
                    .viewports
                    .get(&viewport_id())
                    .and_then(|view| view.focused)
                    == Some(true)
                    && input.viewport().focused != Some(true)
            })
    }

    pub(crate) fn close(&mut self) {
        self.target = None;
        self.revision = None;
    }

    pub(crate) fn refresh(&mut self, app: &PlotiumApp, ctx: &Context) {
        if self.target.is_none() {
            return;
        }
        let revision = (app.doc.edit_generation, app.doc.automation_revision);
        if self.revision.replace(revision) != Some(revision) {
            ctx.request_repaint_of(viewport_id());
            ctx.request_repaint_of(ViewportId::ROOT);
        }
    }

    pub(crate) fn register(
        &mut self,
        app: &PlotiumApp,
        ctx: &Context,
        shared: &Arc<Mutex<PlotiumApp>>,
    ) {
        // The deferred callback borrows the same state on a separate pass.
        // An embedded callback would run now, while the root holds the lock.
        if ctx.embed_viewports() {
            return;
        }
        if let Some(target) =
            ctx.data_mut(|data| data.remove_temp::<Option<Target>>(request_id()).flatten())
        {
            self.target = Some(target);
            self.camera = Arc::default();
            self.revision = None;
            ctx.send_viewport_cmd_to(viewport_id(), ViewportCommand::Focus);
        }
        let Some(target) = self.target else { return };
        let Some(index) = canvas_index(app, target) else {
            self.close();
            return;
        };
        let title = format!("{} — Plotium Canvas Preview", app.doc.canvases[index].name);
        let shared = Arc::clone(shared);
        let camera = Arc::clone(&self.camera);
        ctx.show_viewport_deferred(
            viewport_id(),
            egui::ViewportBuilder::default()
                .with_title(title)
                .with_inner_size([820.0, 580.0]),
            move |ui, _| {
                if ui.input(|input| input.viewport().close_requested()) {
                    return;
                }
                let app = shared.lock().expect("application state lock poisoned");
                let Some(index) = canvas_index(&app, target) else {
                    return;
                };
                let mut camera = camera.lock().expect("preview camera lock poisoned");
                render(&app, index, &mut camera, ui);
            },
        );
        self.refresh(app, ctx);
    }
}

fn render(app: &PlotiumApp, index: usize, camera: &mut Option<Camera>, ui: &mut egui::Ui) {
    egui::Panel::top("preview_controls").show_inside(ui, |ui| {
        ui.horizontal(|ui| {
            if ui.button("Fit Canvas").clicked() {
                *camera = None;
            }
            let fullscreen = ui.input(|input| input.viewport().fullscreen.unwrap_or(false));
            if ui
                .button(if fullscreen {
                    "Exit Full Screen"
                } else {
                    "Full Screen"
                })
                .clicked()
            {
                ui.ctx()
                    .send_viewport_cmd(ViewportCommand::Fullscreen(!fullscreen));
            }
            ui.weak("Read only · Drag to pan · Scroll or pinch to zoom");
        });
    });
    egui::CentralPanel::default().show_inside(ui, |ui| {
        let area = ui.available_rect_before_wrap();
        let response = ui.allocate_rect(area, egui::Sense::click_and_drag());
        let document = crate::ui::canvas::image_painting::canvas_document(app, index);
        let size = egui::vec2(document.width, document.height);
        if !size.is_finite()
            || size.x <= 0.0
            || size.y <= 0.0
            || area.width() <= 24.0
            || area.height() <= 24.0
        {
            return;
        }
        let fit = Camera {
            zoom: ((area.width() - 24.0) / size.x).min((area.height() - 24.0) / size.y),
            center: size * 0.5,
        };
        if response.double_clicked() {
            *camera = None;
        }
        let drag = response.drag_delta();
        let (pan, zoom) = if response.hovered() {
            ui.input(preview_scroll)
        } else {
            (Vec2::ZERO, 1.0)
        };
        if drag != Vec2::ZERO || pan != Vec2::ZERO || (zoom - 1.0).abs() > 0.001 {
            let camera = camera.get_or_insert(fit);
            camera.center -= (drag + pan) / camera.zoom;
            if zoom.is_finite() && (zoom - 1.0).abs() > 0.001 {
                let pointer = response.hover_pos().unwrap_or(area.center());
                let anchor = pointer - area.center();
                let next_zoom = (camera.zoom * zoom).clamp(fit.zoom * 0.05, fit.zoom * 100.0);
                camera.center += anchor / camera.zoom - anchor / next_zoom;
                camera.zoom = next_zoom;
            }
        }
        let camera = camera.unwrap_or(fit);
        let top_left: Pos2 = area.center() - camera.center * camera.zoom;
        let page = Rect::from_min_size(top_left, size * camera.zoom);
        screen::paint_document_for_editor_with_detail(
            &ui.painter_at(area),
            plotium_render::Rect::new(page.left(), page.top(), page.width(), page.height()),
            &document,
            DocumentViewport {
                zoom: camera.zoom,
                pan: [0.0, 0.0],
            },
            screen::ScreenRenderDetail::Full,
        );
    });
}

fn preview_scroll(input: &egui::InputState) -> (Vec2, f32) {
    let pinch = input.zoom_delta();
    if (pinch - 1.0).abs() > 0.001 {
        return (Vec2::ZERO, pinch);
    }
    // macOS trackpad swipes arrive as pixel deltas. Mouse wheel notches zoom.
    if cfg!(target_os = "macos")
        && input.events.iter().any(|event| {
            matches!(
                event,
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    ..
                }
            )
        })
    {
        return (input.smooth_scroll_delta, 1.0);
    }
    (Vec2::ZERO, (input.smooth_scroll_delta.y * 0.008).exp())
}
