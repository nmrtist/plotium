use super::*;

pub(super) fn feedback_banner(app: &mut PlotiumApp, ui: &mut Ui, dark: bool) {
    let paused = paused_results_notice::candidate(app, ui.ctx());
    let operation = pending_feedback(app);
    let mut activity_id = None;
    let (completion_order, outcome, summary, earlier, batch) =
        if let Some((_, order, outcome, summary, earlier)) = operation {
            paused_results_notice::hidden(app, ui.ctx());
            (Some(order), outcome, summary, earlier, None)
        } else {
            let activity = activity_results::banner_notice(app, ui.ctx());
            let activity = match activity {
                Some(activity_results::BannerNotice::Activity(notice))
                    if notice.outcome == OperationOutcome::Success
                        && paused_results_notice::suppress_success(app, ui.ctx(), notice.id) =>
                {
                    None
                }
                activity => activity,
            };
            let urgent = match &activity {
                Some(activity_results::BannerNotice::Import(n)) => {
                    n.outcome != OperationOutcome::Success
                }
                Some(activity_results::BannerNotice::Activity(n)) => {
                    n.outcome != OperationOutcome::Success
                }
                None => false,
            };
            if !urgent && let Some(count) = paused {
                paused_results_notice::show(app, ui, dark, count);
                return;
            }
            paused_results_notice::hidden(app, ui.ctx());
            match activity {
                Some(activity_results::BannerNotice::Import(import)) => {
                    (None, import.outcome, import.summary, 0, Some(import.batch))
                }
                Some(activity_results::BannerNotice::Activity(activity)) => {
                    activity_id = Some(activity.id);
                    (None, activity.outcome, activity.summary, 0, None)
                }
                None => return,
            }
        };
    let (icon, color) = match outcome {
        OperationOutcome::Failure => (
            egui_phosphor::regular::WARNING_OCTAGON,
            ui.visuals().error_fg_color,
        ),
        OperationOutcome::Warning => (
            egui_phosphor::regular::WARNING,
            if dark {
                Color32::from_rgb(245, 190, 72)
            } else {
                Color32::from_rgb(145, 86, 0)
            },
        ),
        OperationOutcome::Success => (egui_phosphor::regular::INFO, ui.visuals().text_color()),
    };
    egui::Panel::top("operation_feedback")
        .frame(
            egui::Frame::new()
                .fill(color.linear_multiply(if dark { 0.16 } else { 0.10 }))
                .inner_margin(egui::Margin::symmetric(12, 8))
                .stroke(Stroke::new(1.0_f32, color.linear_multiply(0.55))),
        )
        .show_separator_line(false)
        .show_inside(ui, |ui| {
            egui::containers::Sides::new()
                .shrink_left()
                .truncate()
                .show(
                    ui,
                    |ui| {
                        ui.colored_label(color, egui::RichText::new(icon).size(18.0));
                        ui.colored_label(color, &summary).on_hover_text(&summary);
                        if earlier > 0 {
                            ui.label(
                                egui::RichText::new(format!("+{earlier} earlier"))
                                    .color(ui.visuals().weak_text_color()),
                            )
                            .on_hover_text(
                                "Older unresolved reports — open Details to review them.",
                            );
                        }
                    },
                    |ui| {
                        let dismiss = ui.button("Dismiss");
                        let dismiss = if earlier > 0 {
                            dismiss.on_hover_text("Acknowledge this and the earlier reports")
                        } else {
                            dismiss
                        };
                        if dismiss.clicked() {
                            if let Some(batch) = batch {
                                import_feedback::dismiss_notice(app, ui.ctx(), batch);
                            } else if let Some(id) = activity_id {
                                activity_results::dismiss_notice(app, ui.ctx(), id);
                            } else {
                                app.session.ui.dismissed_feedback_order = completion_order;
                            }
                        }
                        if ui.button("Details").clicked() {
                            if let Some(batch) = batch {
                                commands::execute_without_clipboard(
                                    commands::CommandId::ImportResults,
                                    app,
                                    ui.ctx(),
                                );
                                import_feedback::open_results(app, ui.ctx(), Some(batch));
                            } else if activity_id.is_some() {
                                activity::open_results(app, ui.ctx());
                            } else {
                                activity::open_diagnostics(app, ui.ctx());
                            }
                        }
                    },
                );
        });
}
