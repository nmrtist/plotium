use super::*;
use std::mem::size_of;

pub(super) fn validate<'a>(
    app: &'a PlotiumApp,
    p: &FigureDiagnosticParams,
) -> Result<&'a CanvasDocument, AutomationError> {
    let c = app
        .doc
        .canvases
        .iter()
        .find(|c| TargetRef::resource(c.resource_id.into()) == p.canvas)
        .ok_or_else(|| {
            invalid("Use an existing whole canvas with its complete target identity.")
        })?;
    if p.limit == 0 || p.limit > 200 || p.checks.is_empty() || p.checks.len() > 5 {
        return Err(invalid("Use 1..=200 rows and 1..=5 checks."));
    }
    for (i, k) in p.checks.iter().enumerate() {
        if p.checks[..i].contains(k) {
            return Err(invalid("Duplicate checks."));
        }
    }
    let [w, h] = p.output.size_mm;
    let [cw, ch] = c.size_mm;
    if [w, h, cw, ch].iter().any(|v| !v.is_finite() || *v <= 0.)
        || (f64::from(h) - f64::from(ch) * f64::from(w) / f64::from(cw)).abs() > 0.001
    {
        return Err(invalid(
            "Output size must be finite, positive, and retain the canvas aspect ratio.",
        ));
    }
    let scale = w / cw;
    if !scale.is_finite()
        || scale <= 0.
        || ![w * MM_TO_PT, h * MM_TO_PT].iter().all(|v| v.is_finite())
    {
        return Err(invalid("Output geometry overflow."));
    }
    if p.targets
        .as_ref()
        .is_some_and(|t| t.is_empty() || t.len() > 200)
        || p.overlap_targets.len() > 200
    {
        return Err(invalid(
            "Use 1..=200 explicit targets and at most 200 overlap targets.",
        ));
    }
    for (targets, whole) in [
        (p.targets.as_deref().unwrap_or(&[]), false),
        (p.overlap_targets.as_slice(), true),
    ] {
        let mut seen = BTreeSet::new();
        for t in targets {
            if !seen.insert(t.describe()) {
                return Err(invalid("Duplicate target."));
            }
            let o = c
                .objects
                .iter()
                .find(|o| canvas_object_ref(c.resource_id, o.id) == t.resource)
                .ok_or_else(|| {
                    invalid("Target must belong to this canvas with its full content identity.")
                })?;
            if let Some(component) = t.component {
                if whole {
                    return Err(invalid("Overlap targets must be whole contents."));
                }
                let ComponentRef::Series(id) = component else {
                    return Err(invalid("Only Series components are supported."));
                };
                if !o.plot().is_some_and(|plot| {
                    app.display_binding(plot.display_owner, &plot.binding)
                        .series
                        .iter()
                        .any(|s| s.id == id)
                }) {
                    return Err(invalid(
                        "Series target is not in this plot's current display binding.",
                    ));
                }
            }
        }
    }
    Ok(c)
}
impl FigureDiagnosticScan {
    pub fn capture(
        app: &PlotiumApp,
        revision: DocumentRevision,
        params: FigureDiagnosticParams,
    ) -> Result<Self, AutomationError> {
        if revision.0 != app.doc.automation_revision {
            return Err(AutomationError::StaleRevision {
                expected: revision.0,
                actual: app.doc.automation_revision,
            });
        }
        let c = validate(app, &params)?;
        let [w, h] = c.size_pt();
        let mut items = VecDeque::new();
        let mut overlap_frames = Vec::new();
        let mut bytes = 0usize;
        let mut text_bytes = 0usize;
        let mut asset_bytes = 0usize;
        let mut assets = BTreeSet::new();
        for o in &c.objects {
            let target = TargetRef::resource(canvas_object_ref(c.resource_id, o.id));
            let selected = params
                .targets
                .as_ref()
                .is_none_or(|ts| ts.iter().any(|t| t.resource == target.resource));
            let overlap = params.overlap_targets.contains(&target);
            if !selected && !overlap {
                continue;
            }
            let frame = c
                .content_page_frame(o.id)
                .ok_or_else(|| invalid("Content frame unavailable."))?
                .rect();
            let parent = c.parent_panel(o.id).and_then(|p| c.panel(p));
            let visible = o.visible && parent.is_none_or(|p| p.visible);
            if !finite(frame)
                || !finite(geometry::scale(
                    frame,
                    params.output.size_mm[0] / c.size_mm[0],
                ))
            {
                return Err(invalid(
                    "Content frame overflows the requested output coordinates.",
                ));
            }
            if overlap && visible {
                let clipped = match &o.kind {
                    ContentKind::RasterImage(i) if i.opacity <= 0. => None,
                    ContentKind::RasterImage(_) => {
                        parent.filter(|p| p.clip_children).map_or(Some(frame), |p| {
                            geometry::intersection(frame, p.frame.rect())
                        })
                    }
                    _ => Some(frame),
                };
                if let Some(r) =
                    clipped.and_then(|r| geometry::intersection(r, Rect::new(0., 0., w, h)))
                {
                    overlap_frames.push((target.clone(), r));
                }
            }
            if !selected {
                continue;
            }
            bytes = bytes.saturating_add(2048);
            let kind = match &o.kind {
                ContentKind::Plot(p) => {
                    let f = p.figure();
                    if f.series.len().saturating_add(f.polygons.len()) > 4096 {
                        return Err(budget(
                            "More than 4096 rendered runs/polygons in one plot; diagnostic layout budget exceeded.",
                        ));
                    }
                    let (figure_bytes, text) = figure_cost(f);
                    bytes = bytes.saturating_add(figure_bytes);
                    text_bytes = text_bytes.saturating_add(text);
                    check_budget(bytes, text_bytes)?;
                    let binding = app.display_binding(p.display_owner, &p.binding);
                    let selected_series = params
                        .targets
                        .as_ref()
                        .filter(|ts| !ts.contains(&target))
                        .map(|ts| {
                            ts.iter()
                                .filter(|t| t.resource == target.resource)
                                .filter_map(|t| match t.component {
                                    Some(ComponentRef::Series(id)) => Some(id),
                                    _ => None,
                                })
                                .collect()
                        });
                    SnapshotKind::Plot {
                        figure: Box::new(f.clone()),
                        selected_series,
                        declared: binding
                            .series
                            .iter()
                            .map(|s| {
                                let source = super::super::document::source::binding_status(app, s)
                                    .to_owned();
                                (s.id, s.visible, source)
                            })
                            .collect(),
                    }
                }
                ContentKind::Text(t) => {
                    bytes = bytes.saturating_add(t.text.len());
                    text_bytes = text_bytes.saturating_add(t.text.len());
                    check_budget(bytes, text_bytes)?;
                    SnapshotKind::Text {
                        text: t.text.clone(),
                        font: t.font_size,
                        align: t.align.to_render(),
                        part: "text",
                    }
                }
                ContentKind::Shape(s) => SnapshotKind::Shape {
                    bounds: geometry::shape_bounds(frame, s.shape.to_render(), s.stroke_width),
                },
                ContentKind::RasterImage(i) => {
                    let mut pixels = None;
                    let status = if let Err(e) = i.validate() {
                        format!("invalid_asset_geometry: {e}")
                    } else if let Some(a) = app.doc.assets.get(&i.asset) {
                        if assets.insert(i.asset) {
                            asset_bytes = asset_bytes.saturating_add(a.bytes.len());
                        }
                        if asset_bytes > PREVIEW_ASSET_BYTES {
                            "asset_probe_budget_exceeded".into()
                        } else {
                            match plotium_io::image::probe(&a.bytes) {
                                Ok(probe) => {
                                    let size = if i.page_index == 0 {
                                        Some([probe.width, probe.height])
                                    } else {
                                        plotium_io::image::tiff_page_dimensions(
                                            &a.bytes,
                                            i.page_index,
                                        )
                                    };
                                    pixels = size;
                                    if size.is_some() {
                                        "available_geometry_only_alpha_not_tested".into()
                                    } else {
                                        "asset_page_unavailable".into()
                                    }
                                }
                                Err(e) => format!("asset_unavailable: {e}"),
                            }
                        }
                    } else {
                        "asset_missing".into()
                    };
                    let bounds = pixels.and_then(|p| {
                        geometry::raster_bounds(
                            frame,
                            p,
                            i.crop,
                            match i.rotation {
                                QuarterTurn::Zero => 0,
                                QuarterTurn::Clockwise90 => 1,
                                QuarterTurn::Clockwise180 => 2,
                                QuarterTurn::Clockwise270 => 3,
                            },
                            match i.fit {
                                ImageFit::Contain => plotium_render::RasterFit::Contain,
                                ImageFit::Cover => plotium_render::RasterFit::Cover,
                                ImageFit::Stretch => plotium_render::RasterFit::Stretch,
                            },
                        )
                    });
                    SnapshotKind::Raster {
                        bounds,
                        clip: parent.filter(|p| p.clip_children).map(|p| p.frame.rect()),
                        status,
                    }
                }
            };
            check_budget(bytes, text_bytes)?;
            let effective_visible =
                visible && !matches!(&o.kind,ContentKind::RasterImage(i) if i.opacity<=0.);
            items.push_back(SnapshotItem {
                target,
                frame,
                visible: effective_visible,
                kind,
            });
        }
        // Panel labels have existing panel identities; their text is not a new content ID.
        if params.targets.is_none() {
            for panel in &c.panels {
                if !c.panel_label_is_displayed(panel.id) {
                    continue;
                }
                let label = c.panel_label_text(panel);
                if label.is_empty() {
                    continue;
                }
                text_bytes = text_bytes.saturating_add(label.len());
                bytes = bytes.saturating_add(2048 + label.len());
                check_budget(bytes, text_bytes)?;
                let mut frame = panel.frame.rect();
                frame.left += panel.label.position[0];
                frame.top += panel.label.position[1];
                items.push_back(SnapshotItem {
                    target: TargetRef::resource(panel_resource(c, panel)),
                    frame,
                    visible: true,
                    kind: SnapshotKind::Text {
                        text: label,
                        font: panel.label.font_size,
                        align: plotium_render::OverlayAlign::Left,
                        part: "panel.label",
                    },
                });
            }
        }
        let mut overlaps = VecDeque::new();
        if params.checks.contains(&DiagnosticCheck::ContentOverlap) {
            for (i, (a, ra)) in overlap_frames.iter().enumerate() {
                for (b, rb) in &overlap_frames[i + 1..] {
                    overlaps.push_back((a.clone(), *ra, b.clone(), *rb));
                }
            }
        }
        bytes = bytes.saturating_add(overlaps.len() * 1024);
        check_budget(bytes, text_bytes)?;
        let mut scan = Self {
            project_context: app.session.project_context,
            revision,
            scale: params.output.size_mm[0] / c.size_mm[0],
            params,
            page: Rect::new(0., 0., w, h),
            items,
            curves: VecDeque::new(),
            overlaps,
            pending: VecDeque::new(),
            scanned_objects: 0,
            scanned_points: 0,
            scanned_pairs: 0,
            unavailable: 0,
            unsupported: 0,
            snapshot_bytes: bytes,
            font_status: None,
        };
        if scan.checks(DiagnosticCheck::ContentOverlap) && scan.params.overlap_targets.len() < 2 {
            scan.coverage(&scan.params.canvas.clone(),"content_overlap","not_applicable","Supply at least two explicit whole-content overlap_targets; GUI selection is never used.");
        }
        Ok(scan)
    }
}
fn check_budget(bytes: usize, text: usize) -> Result<(), AutomationError> {
    if bytes > DIAGNOSTIC_SNAPSHOT_BYTES {
        return Err(budget(
            "Snapshot exceeds 64 MiB; query fewer exact targets.",
        ));
    }
    if text > DIAGNOSTIC_TEXT_BYTES {
        return Err(budget("Text exceeds 1 MiB; query fewer exact targets."));
    }
    Ok(())
}
pub(super) fn finite(r: Rect) -> bool {
    [r.left, r.top, r.width, r.height, r.right(), r.bottom()]
        .iter()
        .all(|v| v.is_finite())
        && r.width > 0.
        && r.height > 0.
}
pub(super) fn figure_cost(f: &plotium_figure::Figure) -> (usize, usize) {
    let mut text = f.title.len() + f.guide_title.len() + f.x.label.len() + f.y.label.len();
    let mut bytes = size_of::<plotium_figure::Figure>();
    for axis in [&f.x, &f.y] {
        text += axis.display.as_ref().map_or(0, |d| d.unit.len());
        if let Some(c) = &axis.categories {
            bytes += c.len() * size_of::<String>();
            text += c.iter().map(String::len).sum::<usize>();
        }
    }
    for s in &f.series {
        bytes = bytes.saturating_add(
            size_of::<plotium_figure::Series>() + s.points.len() * size_of::<[f64; 2]>(),
        );
        text += s.name.len();
    }
    for p in &f.polygons {
        bytes = bytes.saturating_add(
            size_of::<plotium_figure::Polygon>() + p.points.len() * size_of::<[f64; 2]>(),
        );
        text += p.name.len();
    }
    for c in &f.contours {
        bytes = bytes.saturating_add(
            size_of::<plotium_figure::Contour>() + c.segments.len() * size_of::<[[f64; 2]; 2]>(),
        );
    }
    if let Some(h) = &f.heatmap {
        bytes = bytes.saturating_add(h.values.len() * size_of::<f32>());
    }
    for t in [&f.top_projection, &f.left_projection]
        .into_iter()
        .flatten()
    {
        bytes = bytes.saturating_add(t.points.len() * size_of::<[f64; 2]>());
    }
    text += f.annotations.iter().map(|a| a.text.len()).sum::<usize>()
        + f.range_annotations
            .iter()
            .map(|a| a.label.len())
            .sum::<usize>()
        + f.integral_curves
            .iter()
            .map(|a| a.label.len())
            .sum::<usize>();
    bytes += f.annotations.len() * size_of::<plotium_figure::Annotation>()
        + f.range_annotations.len() * size_of::<plotium_figure::RangeAnnotation>()
        + f.integral_curves.len() * size_of::<plotium_figure::IntegralCurve>()
        + f.error_bars.len() * size_of::<plotium_figure::ErrorBar>();
    (bytes.saturating_add(text), text)
}
