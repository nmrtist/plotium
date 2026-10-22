//! Append explicit curves without replacing existing plot or Series identities.
use super::*;
use plotium_figure::{LineEncoding, SeriesEncoding};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CurveAppendParams {
    pub plot: TargetRef,
    pub series: Vec<NewCurve>,
}
impl PlotiumApp {
    pub fn plan_append_curves(
        &self,
        revision: DocumentRevision,
        params: &CurveAppendParams,
    ) -> Result<CurveEditPlan, String> {
        if revision.0 != self.doc.automation_revision {
            return Err("Document revision is stale.".into());
        }
        if params.plot.component.is_some() || params.series.is_empty() || params.series.len() > 200
        {
            return Err("Supply one whole plot and 1–200 explicit curves to append.".into());
        }
        let (canvas, object) = self
            .doc
            .canvases
            .iter()
            .find_map(|c| {
                c.objects
                    .iter()
                    .find(|o| params.plot.resource == canvas_object_ref(c.resource_id, o.id))
                    .map(|o| (c.resource_id, o.id))
            })
            .ok_or("Plot identity is missing or forged.")?;
        let before = self.curve_plot(canvas, object)?.clone();
        if before.binding.coordinates.is_none() || before.display_owner.is_some() {
            return Err(
                "Append curves to an explicit-coordinate plot; materialize default curves first."
                    .into(),
            );
        }
        if before
            .binding
            .series
            .len()
            .saturating_add(params.series.len())
            > 200
        {
            return Err("An explicit plot supports at most 200 curves.".into());
        }
        let mut change = CurvePlotChange {
            canvas,
            object,
            before: Box::new(before.clone()),
            after: Box::new(before),
        };
        for curve in &params.series {
            self.check_table_curve_revision(&curve.source)?;
            let id = change.after.next_series_id;
            change.after.next_series_id = id
                .try_advance(1)
                .ok_or("Series identity allocator is exhausted.")?;
            let mut series = SeriesBinding::with_source(curve.source.source());
            series.id = id;
            series.encoding = SeriesEncoding::Line(LineEncoding {
                kind: curve.kind.native(),
                color: plotium_figure::ColorSource::Explicit(
                    self.next_stack_color(&change.after.binding),
                ),
                ..LineEncoding::default()
            });
            change.after.binding.series.push(series);
        }
        self.check_curve_row_budget([&change.after.binding])?;
        self.finish_curve_candidate(&mut change)?;
        let bindings = self.curve_binding_summaries(canvas, object, &change.after.binding)?;
        let appended = bindings
            .iter()
            .skip(change.before.binding.series.len())
            .map(|b| b.target.clone())
            .collect();
        Ok(CurveEditPlan {
            expected_revision: revision,
            bindings,
            changes: vec![change],
            creation: None,
            insertion: None,
            appended,
        })
    }
}
