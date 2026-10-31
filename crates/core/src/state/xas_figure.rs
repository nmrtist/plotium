//! The XANES, chi(k) and |chi(R)| panels that a XAS paper figure usually shows.
use super::xas::{CHI_R, NORM, SUMMARY_FIELD_KEYS};
use super::*;
use crate::actions::Action;

const CANVAS_SIZE_MM: [f32; 2] = [183.0, 62.0];
const GAP_PT: f32 = 12.0;
/// The XANES window around E0, in eV.
const XANES_WINDOW: (f64, f64) = (-30.0, 70.0);
const R_WINDOW: (f64, f64) = (0.0, 6.0);

impl PlotiumApp {
    /// Adds a three-panel canvas for the dataset at `index`, one Undo step.
    pub fn create_xas_summary_canvas(&mut self, index: usize) -> Result<(), String> {
        let dataset = self
            .doc
            .datasets
            .get(index)
            .ok_or("The XAS dataset is no longer available.")?
            .clone();
        let xas = dataset.as_xas().ok_or("Select an XAS dataset first.")?;
        let e0 = xas
            .processed
            .edge
            .as_ref()
            .ok_or("Normalize the scan before building the figure.")?
            .e0;
        if xas.processed.exafs.is_none() {
            return Err("The figure needs chi(k): the scan has no EXAFS region.".into());
        }
        let name = format!("{} summary", crate::workflow::dataset_title(&dataset));
        let mut page = CanvasDocument::new(name, CANVAS_SIZE_MM);
        let [width, height] = page.size_pt();
        let panel_width = (width - 2.0 * GAP_PT) / 3.0;
        for (position, key) in SUMMARY_FIELD_KEYS.into_iter().enumerate() {
            let field = xas
                .field_for_curve(key)
                .ok_or("The XAS result is incomplete.")?;
            let title = dataset
                .field_descriptor(field)
                .map_or_else(|| key.to_owned(), |descriptor| descriptor.name);
            let id = page.allocate_object_id();
            let x = position as f32 * (panel_width + GAP_PT);
            let frame = ObjectFrame::new(x, 0.0, panel_width, height);
            let mut object = crate::workflow::build_plot_object(&dataset, index, frame, id, title);
            let plot = object.plot_mut().ok_or("The plot could not be created.")?;
            let mut series = SeriesBinding::from_field_all(&dataset, field);
            for (index, item) in series.iter_mut().enumerate() {
                item.id = SeriesId::new(index as u64);
            }
            plot.next_series_id = SeriesId::new(series.len() as u64);
            plot.binding = DataBinding {
                coordinates: None,
                series,
            };
            // Each panel is pinned to its field, whatever the dataset displays.
            plot.display_owner = None;
            plot.axis_overrides.x_range = match key {
                NORM => Some(AxisRange::new(e0 + XANES_WINDOW.0, e0 + XANES_WINDOW.1)),
                CHI_R => Some(AxisRange::new(R_WINDOW.0, R_WINDOW.1)),
                _ => None,
            };
            let (binding, chart, stack, size) = (
                plot.binding.clone(),
                plot.chart.clone(),
                plot.stack,
                [frame.width / MM_TO_PT, frame.height / MM_TO_PT],
            );
            let figure = self.build_binding_figure(&binding, &chart, &stack, size);
            plot.adopt_rebuilt_figure(figure);
            page.objects.push(object);
            page.create_panel_for_plot(id)
                .ok_or("The plot panel could not be created.")?;
        }
        self.execute_action(Action::InsertCanvas {
            index: self.doc.canvases.len(),
            canvas: Box::new(page),
            active_before: self.session.active_canvas,
            auto_place: true,
        });
        Ok(())
    }
}
