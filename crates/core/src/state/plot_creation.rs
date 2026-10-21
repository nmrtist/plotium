//! Atomic native single-plot creation, independent of current desktop selection.
use super::*;
use crate::actions::NativePlotInsertion;
use crate::automation::{
    DocumentRevision, TargetRef, canvas_object_ref, document_root_target, panel_resource,
};
use plotium_figure::{LineEncoding, SeriesEncoding, SeriesKind};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CurveKind {
    Line,
    Scatter,
}
impl CurveKind {
    fn native(self) -> SeriesKind {
        match self {
            Self::Line => SeriesKind::Line,
            Self::Scatter => SeriesKind::Points,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NewCurve {
    pub kind: CurveKind,
    pub source: TableCurveSource,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NewPlotCanvas {
    pub name: String,
    /// Exactly one of `size_mm` and `size_preset` (`SizePreset::id`).
    #[serde(default)]
    pub size_mm: Option<[f32; 2]>,
    #[serde(default)]
    pub size_preset: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlotCreateParams {
    pub parent: TargetRef,
    pub name: String,
    pub canvas: Option<NewPlotCanvas>,
    pub panel_name: Option<String>,
    pub panel_frame_pt: Option<[f32; 4]>,
    pub series: Vec<NewCurve>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CreatedCurveDefaults {
    pub target: TargetRef,
    pub kind: CurveKind,
    pub line_width_pt: f32,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlotCreationSummary {
    pub parent: TargetRef,
    pub canvas: TargetRef,
    pub panel: TargetRef,
    pub plot: TargetRef,
    pub created: Vec<TargetRef>,
    pub size_mm: [f32; 2],
    /// Built-in size preset id of a newly created canvas, if one was requested.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_preset: Option<String>,
    /// Panel-local content rectangle, in points.
    pub frame_pt: [f32; 4],
    /// Panel rectangle in canvas/page points, independent of the content-local origin.
    pub panel_frame_pt: [f32; 4],
    pub coordinate_units: CurveCoordinates,
    pub series_defaults: Vec<CreatedCurveDefaults>,
}
fn name(value: &str) -> Result<(), String> {
    if value.trim().is_empty() || value.len() > 1024 {
        Err("Names must contain 1–1024 UTF-8 bytes and not be blank.".into())
    } else {
        Ok(())
    }
}
impl PlotiumApp {
    pub fn plan_plot_creation(
        &self,
        revision: DocumentRevision,
        params: &PlotCreateParams,
    ) -> Result<CurveEditPlan, String> {
        if revision.0 != self.doc.automation_revision {
            return Err("Document revision is stale.".into());
        }
        name(&params.name)?;
        if params.series.is_empty() || params.series.len() > 200 {
            return Err("Create one plot with 1–200 explicit curves.".into());
        }
        let (mut page, existing_panel, new_canvas) = self.plot_creation_parent(params)?;
        let new_panel = existing_panel.is_none();
        let frame = if let Some(id) = existing_panel {
            page.panel(id).expect("resolved panel").frame
        } else if let Some([x, y, w, h]) = params.panel_frame_pt {
            ObjectFrame {
                x,
                y,
                width: w,
                height: h,
            }
        } else {
            // Use the same native empty-page placement as GUI drag-tiling. The protocol
            // carries creation intent, not a second grid/margin/avoidance algorithm.
            let [w, h] = page.size_pt();
            let [top, right, bottom, left] = page.layout.margin_mm.map(|m| m * MM_TO_PT);
            if ![top, right, bottom, left]
                .into_iter()
                .all(|v| v.is_finite() && v >= 0.)
                || left + right >= w
                || top + bottom >= h
            {
                return Err("Page margins leave no finite positive plot area.".into());
            }
            crate::layout::compute_tiling_plan_for_items(
                page.size_pt(),
                &page.layout,
                &[],
                crate::layout::LayoutItem {
                    id: page.next_object_id,
                    insets: [0.; 4],
                },
                [w * 0.5, h * 0.5],
            )
            .newcomer
        };
        validate_frame(frame, "plot")?;
        let object_id = page.next_object_id;
        page.next_object_id = object_id
            .try_advance(1)
            .ok_or("Content identity allocator is exhausted.")?;
        let mut binding = DataBinding {
            coordinates: None,
            series: Vec::new(),
        };
        for (index, curve) in params.series.iter().enumerate() {
            self.check_table_curve_revision(&curve.source)?;
            let mut sb = SeriesBinding::with_source(curve.source.source());
            sb.id = SeriesId::new(index as u64);
            sb.encoding = SeriesEncoding::Line(LineEncoding {
                kind: curve.kind.native(),
                ..LineEncoding::default()
            });
            binding.series.push(sb);
        }
        self.check_curve_row_budget([&binding])?;
        let (table, _) = self.table_curve_sources(binding.series[0].source)?;
        let columns = binding.series[0]
            .source
            .columns()
            .expect("all newly created curves are explicit");
        let coordinates = table_curve_coordinates(table, columns)?;
        binding.coordinates = Some(coordinates.clone());
        let stack = StackSpec::default();
        // The same full-row extraction, unit conversions and renderer used by R4.
        let figure = self.build_explicit_binding_figure(
            &binding,
            &stack,
            [frame.width / MM_TO_PT, frame.height / MM_TO_PT],
        )?;
        let mut viewport = CanvasViewport::from_figure(&figure);
        // Start with the same automatic-range policy used by later presentation
        // edits, so changing line width does not cause a first-edit axis jump.
        viewport.reset_y(&figure);
        let bindings = self.curve_binding_summaries(page.resource_id, object_id, &binding)?;
        let defaults = bindings
            .iter()
            .zip(&params.series)
            .zip(&binding.series)
            .map(|((b, c), sb)| CreatedCurveDefaults {
                target: b.target.clone(),
                kind: c.kind,
                line_width_pt: match &sb.encoding {
                    SeriesEncoding::Line(line) => line.width.get(),
                    _ => unreachable!("new curves are line encodings"),
                },
            })
            .collect();
        let mut object = CanvasObject {
            id: object_id,
            name: params.name.clone(),
            frame: ObjectFrame {
                x: 0.,
                y: 0.,
                width: frame.width,
                height: frame.height,
            },
            locked: false,
            visible: true,
            kind: CanvasObjectKind::Plot(Box::new(PlotObject::new(
                None,
                SeriesId::new(binding.series.len() as u64),
                binding,
                ChartSpec {
                    type_id: "table_line".into(),
                    ..ChartSpec::default()
                },
                stack,
                AxisProjections::default(),
                AxisOverrides::default(),
                figure,
                viewport,
            ))),
        };
        object.plot_mut().expect("new native plot").apply_viewport();
        let panel_id = if let Some(id) = existing_panel {
            id
        } else {
            name(params.panel_name.as_deref().unwrap_or(&params.name))?;
            if page.next_panel_label_slot == u64::MAX {
                return Err("Panel label allocator is exhausted.".into());
            }
            page.create_panel(
                params
                    .panel_name
                    .clone()
                    .unwrap_or_else(|| params.name.clone()),
                frame,
            )
        };
        let before_panel = page
            .panel(panel_id)
            .expect("created or resolved panel")
            .clone();
        page.panel_mut(panel_id)
            .expect("resolved panel")
            .item_order
            .push(object_id);
        page.objects.push(object.clone());
        page.validate_structure()?;
        let canvas_target = TargetRef::resource(page.resource_id.into());
        let panel_target = TargetRef::resource(panel_resource(
            &page,
            page.panel(panel_id).expect("resolved panel"),
        ));
        let plot_target = TargetRef::resource(canvas_object_ref(page.resource_id, object_id));
        let mut created = Vec::new();
        if new_canvas {
            created.push(canvas_target.clone());
        }
        if new_panel {
            created.push(panel_target.clone());
        }
        created.push(plot_target.clone());
        created.extend(bindings.iter().map(|b| b.target.clone()));
        let summary = PlotCreationSummary {
            parent: params.parent.clone(),
            canvas: canvas_target,
            panel: panel_target,
            plot: plot_target,
            created,
            size_mm: page.size_mm,
            size_preset: page.size_preset_id.clone().filter(|_| new_canvas),
            frame_pt: [0., 0., frame.width, frame.height],
            panel_frame_pt: [frame.x, frame.y, frame.width, frame.height],
            coordinate_units: coordinates,
            series_defaults: defaults,
        };
        let insertion = if new_canvas {
            page.board_pos = next_board_frame_pos(self, page.size_pt());
            NativePlotInsertion::NewCanvas {
                page: Box::new(page),
                active_before: self
                    .session
                    .active_canvas
                    .and_then(|i| self.doc.canvases.get(i))
                    .map(|c| c.resource_id),
            }
        } else {
            NativePlotInsertion::ExistingCanvas {
                canvas: page.resource_id,
                object: Box::new(object),
                panel: Box::new(if new_panel {
                    page.panel(panel_id).expect("new panel").clone()
                } else {
                    before_panel
                }),
                new_panel,
                additional_panel: params.panel_frame_pt.is_some(),
                next_label_slot: page.next_panel_label_slot,
            }
        };
        Ok(CurveEditPlan::for_creation(
            revision, bindings, summary, insertion,
        ))
    }

    fn plot_creation_parent(
        &self,
        params: &PlotCreateParams,
    ) -> Result<(CanvasDocument, Option<PanelId>, bool), String> {
        if params.parent == document_root_target() {
            let (canvas_name, size, preset) = match &params.canvas {
                None => (params.name.as_str(), DEFAULT_CANVAS_SIZE_MM, None),
                Some(c) => match (c.size_mm, c.size_preset.as_deref()) {
                    (Some(size), None) => (c.name.as_str(), size, None),
                    (None, Some(id)) => {
                        let preset = super::figure_edit::size_preset_by_id(id)?;
                        (c.name.as_str(), preset.size_mm(), Some(preset))
                    }
                    _ => {
                        return Err(
                            "A new canvas takes exactly one of size_mm or size_preset.".into()
                        );
                    }
                },
            };
            name(canvas_name)?;
            if !size
                .into_iter()
                .all(|x| x.is_finite() && (10.0..=1000.0).contains(&x))
            {
                return Err(
                    "Canvas width and height must be finite and between 10 and 1000 mm.".into(),
                );
            }
            let mut page = CanvasDocument::new(canvas_name.into(), size);
            if let Some(preset) = preset {
                page.size_preset_id = Some(preset.id.to_owned());
                page.layout.margin_mm = preset.default_margin_mm();
            }
            return Ok((page, None, true));
        }
        if params.parent.component.is_some() || params.canvas.is_some() {
            return Err(
                "Existing parents require a whole canvas/panel target and no canvas options."
                    .into(),
            );
        }
        for page in &self.doc.canvases {
            let canvas_match = params.parent == TargetRef::resource(page.resource_id.into());
            let panel_match = page
                .panels
                .iter()
                .find(|p| params.parent == TargetRef::resource(panel_resource(page, p)));
            if !canvas_match && panel_match.is_none() {
                continue;
            }
            if params.panel_frame_pt.is_none() && page.objects.iter().any(|o| o.plot().is_some()) {
                return Err(
                    "Create a single plot in an empty canvas/panel, not a second plot.".into(),
                );
            }
            if canvas_match {
                if params.panel_frame_pt.is_none() && !page.panels.is_empty() {
                    return Err(
                        "Canvas already has a panel; target that empty panel explicitly.".into(),
                    );
                }
                return Ok((page.clone(), None, false));
            }
            if params.panel_frame_pt.is_some() {
                return Err(
                    "panel_frame_pt creates a new Panel on a canvas, not an existing Panel resize."
                        .into(),
                );
            }
            let panel = panel_match.expect("matched parent");
            if page.panels.len() != 1
                || panel.locked
                || !panel.visible
                || !panel.item_order.is_empty()
                || panel.layout != PanelLayout::Free
                || params.panel_name.is_some()
            {
                return Err(
                    "Target one empty, visible, unlocked free-layout panel without panel_name."
                        .into(),
                );
            }
            return Ok((page.clone(), Some(panel.id), false));
        }
        Err(
            "Parent identity is missing or forged; discover an exact document/canvas/panel target."
                .into(),
        )
    }
}
