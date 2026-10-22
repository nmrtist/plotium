use super::*;

pub(crate) fn handle_and_paint_craft_result(
    app: &mut PlotiumApp,
    ci: usize,
    object_id: ObjectId,
    plot: PlotRect,
    painter: &egui::Painter,
    ui: &Ui,
) {
    let Some(plotium_core::state::CanvasAnalysisBinding::Craft { dataset, run }) =
        app.doc.canvases[ci].analysis_binding
    else {
        return;
    };
    let overview = app.doc.canvases[ci]
        .x_viewport_links
        .first()
        .and_then(|group| group.members.first())
        .copied();
    if overview != Some(object_id) {
        return;
    }
    let Some((nmr, stored)) = app
        .doc
        .dataset_by_id(dataset)
        .and_then(Dataset::as_nmr)
        .and_then(|nmr| Some((nmr, nmr.craft_run(run)?)))
    else {
        return;
    };
    if stored.is_stale_for(&nmr.data, nmr.craft_reference()) {
        painter.text(
            Pos2::new(plot.left + 6.0, plot.top + 6.0),
            egui::Align2::LEFT_TOP,
            "Stale CRAFT run",
            egui::FontId::proportional(10.0),
            ui.visuals().warn_fg_color,
        );
    }
    let Some(figure) = app.doc.canvases[ci]
        .object(object_id)
        .and_then(|object| object.plot())
        .map(|plot| plot.figure())
    else {
        return;
    };
    paint_craft_ranges(CraftRangePaintContext {
        app,
        dataset,
        run,
        stored,
        plot,
        figure,
        painter,
        ui,
    });
    if let Some(selected) = app.session.ui.craft_selected_component
        && let Some(component) = stored
            .components
            .iter()
            .find(|component| component.id == selected)
    {
        let x = to_screen_x(component.chemical_shift_ppm, plot, &figure.x);
        painter.text(
            Pos2::new(x, plot.top + 6.0),
            egui::Align2::CENTER_TOP,
            format!("{:.5} ppm", component.chemical_shift_ppm),
            egui::FontId::proportional(10.0),
            ui.visuals().selection.stroke.color,
        );
    }
    let (clicked, pointer) =
        ui.input(|input| (input.pointer.primary_clicked(), input.pointer.hover_pos()));
    if !clicked {
        return;
    }
    let Some(pointer) = pointer.filter(|pointer| plot_contains(plot, *pointer)) else {
        return;
    };
    let selected = stored
        .components
        .iter()
        .map(|component| {
            let x = to_screen_x(component.chemical_shift_ppm, plot, &figure.x);
            (component, (x - pointer.x).abs())
        })
        .filter(|(_, distance)| *distance <= 8.0)
        .min_by(|left, right| left.1.total_cmp(&right.1))
        .map(|(component, _)| (component.id, component.region));
    if let Some((component, region)) = selected {
        app.session.ui.craft_task_dataset = Some(dataset);
        app.session.ui.craft_selected_run = Some(run);
        app.session.ui.craft_selected_component = Some(component);
        app.session.ui.craft_component_region = Some(region);
        app.session.ui.craft_result_tab = plotium_core::state::CraftResultTab::Components;
        app.session.ui.craft_task_page = plotium_core::state::CraftTaskPage::Results;
        app.session
            .ui
            .open_task_tab(plotium_core::state::TaskDockTab::Craft);
    }
}

struct CraftRangePaintContext<'a> {
    app: &'a PlotiumApp,
    dataset: plotium_core::state::DatasetId,
    run: plotium_core::state::CraftRunId,
    stored: &'a plotium_core::state::StoredCraftRun,
    plot: PlotRect,
    figure: &'a plotium_figure::Figure,
    painter: &'a egui::Painter,
    ui: &'a Ui,
}

fn paint_craft_ranges(context: CraftRangePaintContext<'_>) {
    let CraftRangePaintContext {
        app,
        dataset,
        run,
        stored,
        plot,
        figure,
        painter,
        ui,
    } = context;
    let carrier = stored
        .provenance
        .invocation
        .reference
        .effective_carrier_ppm();
    let observe = stored
        .provenance
        .invocation
        .reference
        .reference_frequency_mhz;
    let modeling = stored
        .diagnostics
        .modeling_windows
        .iter()
        .map(|window| {
            (
                carrier + window.modeling_band_hz.0 / observe,
                carrier + window.modeling_band_hz.1 / observe,
            )
        })
        .collect::<Vec<_>>();
    let regions = stored
        .region_summaries
        .iter()
        .map(|region| (region.start_ppm, region.end_ppm))
        .collect::<Vec<_>>();
    let report_segments = app
        .session
        .ui
        .craft_selected_report
        .and_then(|id| app.doc.report(id))
        .filter(|record| {
            record.source
                == plotium_core::state::ReportSource {
                    dataset,
                    craft_run: run,
                }
        })
        .and_then(|record| {
            serde_json::from_value::<plotium_processing::craft::CraftAmplitudeReport>(
                record.snapshot.clone(),
            )
            .ok()
        })
        .map(|report| {
            report
                .segments
                .into_iter()
                .map(|segment| {
                    (
                        carrier + segment.start_hz / observe,
                        carrier + segment.end_hz / observe,
                    )
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    paint_range_track(
        &modeling,
        plot.top + 2.0,
        plot,
        figure,
        painter,
        ui.visuals().weak_text_color().linear_multiply(0.45),
    );
    paint_range_track(
        &regions,
        plot.top + 7.0,
        plot,
        figure,
        painter,
        ui.visuals().selection.stroke.color.linear_multiply(0.75),
    );
    paint_range_track(
        &report_segments,
        plot.top + 12.0,
        plot,
        figure,
        painter,
        ui.visuals().warn_fg_color.linear_multiply(0.75),
    );
}

fn paint_range_track(
    ranges: &[(f64, f64)],
    y: f32,
    plot: PlotRect,
    figure: &plotium_figure::Figure,
    painter: &egui::Painter,
    color: egui::Color32,
) {
    for &(left, right) in ranges {
        let first = to_screen_x(left, plot, &figure.x);
        let second = to_screen_x(right, plot, &figure.x);
        let rect = egui::Rect::from_min_max(
            Pos2::new(first.min(second).max(plot.left), y),
            Pos2::new(first.max(second).min(plot.right()), y + 3.0),
        );
        if rect.is_positive() {
            painter.rect_filled(rect, 0.0, color);
        }
    }
}
