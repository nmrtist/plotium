use super::*;
use crate::actions::Action;
use std::sync::Arc;

impl PlotiumApp {
    pub fn duplicate_afm_fit_plot(&mut self, target: AfmFitTarget) -> Result<(), String> {
        let fit = self
            .afm_fit_result(target)
            .filter(|fit| fit.converged)
            .ok_or("Run a converged AFM fit before duplicating the plot.")?;
        let ci = self
            .doc
            .canvas_index(target.canvas)
            .ok_or("The source canvas is unavailable.")?;
        let source = &self.doc.canvases[ci];
        let mut object = source
            .object(target.object)
            .cloned()
            .ok_or("The source plot is unavailable.")?;
        let name = format!("{} - {} Fit", object.name, fit.branch.name());
        let mut page = CanvasDocument::new(
            name,
            [
                object.frame.width / MM_TO_PT,
                object.frame.height / MM_TO_PT,
            ],
        );
        page.background = source.background;
        object.id = page.allocate_object_id();
        object.frame.x = 0.0;
        object.frame.y = 0.0;
        object.locked = false;
        let mut fit = fit.clone();
        Arc::make_mut(&mut fit).shown = true;
        object
            .plot_mut()
            .ok_or("The source plot is unavailable.")?
            .set_afm_force_fits(vec![fit]);
        let object_id = object.id;
        page.objects.push(object);
        page.create_panel_for_plot(object_id)
            .ok_or("Could not create the AFM fit panel.")?;
        let index = self.doc.canvases.len();
        self.try_execute_action(Action::InsertCanvas {
            index,
            canvas: Box::new(page),
            active_before: self.session.active_canvas,
            activate: true,
            auto_place: true,
        })
        .map_err(|error| error.to_string())?;
        self.reveal_board_frame(FrameRef::Page(index));
        Ok(())
    }

    pub fn create_afm_fit_output(
        &mut self,
        target: AfmFitTarget,
        residual_plot: bool,
    ) -> Result<(), String> {
        let fit = self
            .afm_fit_result(target)
            .ok_or("Run AFM Fit before creating an output.")?;
        let source = fit.source;
        let name = format!(
            "{} - {} {}",
            self.doc
                .dataset_by_id(source)
                .ok_or("The AFM source is unavailable.")?
                .display_name(),
            fit.branch.name(),
            if residual_plot {
                "Residuals"
            } else {
                "Fit Data"
            },
        );
        let column = |name: &str, value: fn([f64; 3]) -> f64| FloatSeries {
            name: name.into(),
            unit: fit.y_unit.clone(),
            values: fit
                .rows
                .iter()
                .copied()
                .map(|row| Some(value(row)))
                .collect(),
            uncertainty: None,
            fit: None,
        };
        let mut series = vec![column("Residual", |[_, observed, predicted]| {
            observed - predicted
        })];
        if !residual_plot {
            series.insert(0, column(&fit.y_name, |[_, observed, _]| observed));
            series.insert(1, column("Predicted", |[_, _, predicted]| predicted));
        }
        let mut table = materialized_float_series_table(
            (
                fit.x_name.clone(),
                fit.x_unit.clone(),
                fit.rows.iter().map(|[x, _, _]| Some(*x)).collect(),
            ),
            series,
            "plotium.afm.fit-output.v1",
        )
        .map_err(|error| format!("Could not create AFM fit output: {error}"))?;
        table.name = Some(name.clone());
        table.lineage = Some(DatasetLineage::new(
            DerivationKind::AfmForceBranchTable,
            [source],
        ));
        if residual_plot {
            let index = self.doc.canvases.len();
            let y_label = AfmCurveAxis {
                name: "Residual",
                unit: &fit.y_unit,
            }
            .label();
            let dataset = Dataset::Table(Box::new(table));
            let mut canvas = crate::workflow::build_default_canvas_for_dataset(
                &dataset,
                self.doc.datasets.len(),
                name.clone(),
                DEFAULT_CANVAS_SIZE_MM,
            );
            let plot = canvas
                .objects
                .first_mut()
                .and_then(CanvasObject::plot_mut)
                .ok_or("Could not prepare the AFM residual plot.")?;
            plot.axis_overrides.y_label = Some(y_label);
            plot.apply_axis_overrides();
            let mut action = Action::insert_dataset_with_default_canvas(
                self,
                dataset,
                name,
                DEFAULT_CANVAS_SIZE_MM,
            );
            let Action::InsertDatasetWithCanvas {
                prepared_canvas, ..
            } = &mut action
            else {
                return Err("Could not prepare the AFM residual plot action.".into());
            };
            *prepared_canvas = Some(Box::new(canvas));
            self.try_execute_action(action)
                .map_err(|error| error.to_string())?;
            self.reveal_board_frame(FrameRef::Page(index));
        } else {
            let size = table.board_rect_pt();
            table.board_pos = next_board_frame_pos(self, [size.width, size.height]);
            let id = table.resource_id;
            self.try_execute_action(Action::CreateTableData {
                dataset: Box::new(Dataset::Table(Box::new(table))),
            })
            .map_err(|error| error.to_string())?;
            let index = self
                .doc
                .dataset_index(id)
                .ok_or("The AFM fit table is unavailable.")?;
            self.reveal_board_frame(FrameRef::Sheet(index));
            self.session.ui.sheet_open = Some(index);
        }
        Ok(())
    }

    pub fn zoom_to_afm_fit_range(&mut self, target: AfmFitTarget) -> Result<(), String> {
        let fit = self
            .afm_fit_result(target)
            .ok_or("Run AFM Fit before zooming to its range.")?;
        let ci = self
            .doc
            .canvas_index(target.canvas)
            .ok_or("The source canvas is unavailable.")?;
        let plot = self
            .afm_fit_plot(target)
            .ok_or("The source plot is unavailable.")?;
        let mut viewport = plot.viewport.clone();
        let range = fit.range();
        if range.min >= range.max || !range.min.is_finite() || !range.max.is_finite() {
            return Err("The fitted range is invalid.".into());
        }
        viewport.view_x = range.clamp_in(viewport.full_x, plot.figure().x.scale);
        if viewport.auto_y {
            viewport.refresh_auto_y(plot.figure());
        }
        self.try_execute_action(Action::set_object_viewport(
            ci,
            target.object,
            plot.viewport.clone(),
            viewport,
        ))
        .map_err(|error| error.to_string())?;
        Ok(())
    }
}
