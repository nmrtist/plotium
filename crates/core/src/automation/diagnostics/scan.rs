use super::*;

impl FigureDiagnosticScan {
    /// One bounded batch. Pages may contain coverage rows without issues. An
    /// empty page with `scan_complete=false` means continue, never "no issues".
    pub fn next_page(&mut self, cancelled: impl Fn() -> bool) -> Result<Value, AutomationError> {
        if cancelled() {
            return Err(AutomationError::DiagnosticCancelled);
        }
        if self.font_status.is_none() {
            self.font_status = Some(crate::export::diagnostic_font_status());
        }
        let mut objects = 0usize;
        let mut points = 0usize;
        let mut rows = Vec::new();
        loop {
            if cancelled() {
                return Err(AutomationError::DiagnosticCancelled);
            }
            if let Some(row) = self.pending.pop_front() {
                rows.push(row);
                if serde_json::to_vec(&self.page_value(&rows, objects, points))
                    .map_err(|e| AutomationError::Execution(e.to_string()))?
                    .len()
                    > DIAGNOSTIC_PAGE_BYTES
                {
                    self.pending.push_front(rows.pop().expect("just appended"));
                    if rows.is_empty() {
                        return Err(AutomationError::ResponseTooLarge);
                    }
                    break;
                }
                if rows.len() >= self.params.limit {
                    break;
                }
                continue;
            }
            if let Some(mut work) = self.curves.pop_front() {
                if points >= DIAGNOSTIC_POINTS {
                    self.curves.push_front(work);
                    break;
                }
                let mut current = work.current.take().or_else(|| {
                    work.runs
                        .pop_front()
                        .map(|s| (s, geometry::CurveScan::default()))
                });
                if let Some((series, mut cursor)) = current.take() {
                    let start = cursor.next_point;
                    let proj = plotium_render::Projector {
                        fig: &work.figure,
                        plot: work.plot,
                        top_band: None,
                        left_band: None,
                    };
                    let clip = geometry::intersection(
                        work.plot,
                        geometry::translate(self.page, -work.frame.left, -work.frame.top),
                    );
                    let result = geometry::scan_curve(
                        &series,
                        &proj,
                        clip,
                        work.legend,
                        &mut cursor,
                        DIAGNOSTIC_POINTS - points,
                        &cancelled,
                    );
                    if result.cancelled {
                        return Err(AutomationError::DiagnosticCancelled);
                    }
                    points += result.visited;
                    self.scanned_points += result.visited;
                    let target = TargetRef {
                        resource: work.target.resource.clone(),
                        component: series
                            .origin
                            .map(|o| ComponentRef::Series(SeriesId::new(o.0))),
                    };
                    let to_page = |r: Rect| geometry::translate(r, work.frame.left, work.frame.top);
                    let evidence = json!({"point_range":[start,cursor.next_point],"scanned_points":result.visited,"total_points_in_run":series.points.len(),"visible_primitives":result.visible,"clipped_primitives":result.clipped,"intersecting_primitives":result.intersections,"clip":clip.map(|r|self.rect(to_page(r))),"clip_sources":["plot_data_area","output_page"],"legend_bounds":work.legend.map(|r|self.rect(to_page(r))),"intersection":result.first_intersection.map(|r|self.rect(to_page(r))),"clipped_bounds":result.first_clipped.map(|r|self.rect(to_page(r))),"sampling":false,"stroke_boundary":"stroke_envelope","legend_boundary":"layout_rectangle","other_target":work.target,"other_component":"guide.legend"});
                    self.emit(json!({"kind":"layout","target":target,"component":"curve","measurement_basis":"renderer_projected_primitives","evidence":evidence}));
                    if result.intersections > 0 && self.checks(DiagnosticCheck::LegendOcclusion) {
                        self.issue(
                            &target,
                            "curve",
                            "legend_occlusion_potential",
                            "potential_visual_conflict",
                            evidence.clone(),
                        );
                    }
                    if result.clipped > 0 && self.checks(DiagnosticCheck::Clipping) {
                        self.issue(
                            &target,
                            "curve",
                            "curve_primitives_clipped",
                            "geometry",
                            evidence,
                        );
                    }
                    if result.invalid > 0 {
                        self.coverage(&target,"curve_primitives","unavailable","Non-finite projected geometry was excluded; no complete absence claim is possible.");
                    }
                    if series.points.is_empty() {
                        self.coverage(&target, "curve_primitives", "not_applicable", "empty_data");
                    }
                    if result.finished {
                        self.coverage(
                            &target,
                            "curve_primitives",
                            "complete",
                            "All points in this rendered run were visited without sampling.",
                        );
                    } else {
                        work.current = Some((series, cursor));
                    }
                    if work.current.is_some() || !work.runs.is_empty() {
                        self.curves.push_front(work);
                    }
                    continue;
                }
            }
            if objects < DIAGNOSTIC_OBJECTS
                && let Some(item) = self.items.pop_front()
            {
                self.scan_item(item);
                objects += 1;
                self.scanned_objects += 1;
                continue;
            }
            if !self.items.is_empty() {
                break;
            }
            if let Some((a, ra, b, rb)) = self.overlaps.pop_front() {
                self.scanned_pairs += 1;
                if let Some(r) = geometry::intersection(ra, rb) {
                    self.issue(&a,"content.frame","content_frame_overlap","geometry",json!({"other_target":b,"bounds":self.rect(ra),"other_bounds":self.rect(rb),"intersection":self.rect(r),"interpretation":"frame_geometry_not_pixel_occlusion"}));
                }
                continue;
            }
            break;
        }
        Ok(self.page_value(&rows, objects, points))
    }
    fn page_value(&self, rows: &[Value], objects: usize, points: usize) -> Value {
        let complete = self.is_complete();
        json!({"project_context":self.project_context,"basis_revision":self.revision,"canvas":self.params.canvas,
            "output":{"size_mm":self.params.output.size_mm,"native_size_pt":[self.page.width,self.page.height],"size_pt":[self.page.width*self.scale,self.page.height*self.scale],"scale":self.scale,"coordinate_space":"output_page_pt","origin":"top_left","x_direction":"right","y_direction":"down","points_per_inch":72,"scaling":"uniform_native_page_no_reflow","renderer":"native_svg_png_layout","font_status":self.font_status},
            "items":rows,"coverage":{"requested_checks":self.params.checks,"supported_checks":all_checks(),"unsupported_checks":["non_line_scatter_internal_geometry","error_bar_clipping_occlusion","image_alpha_pixel_occlusion","scientific_derived_annotation_detail","exact_cross_backend_glyph_bounds"],"status":if !complete{"partial"}else if self.unavailable>0||self.unsupported>0{"limited"}else{"complete"},"scan_complete":complete,"scanned_objects":self.scanned_objects,"scanned_points":self.scanned_points,"scanned_overlap_pairs":self.scanned_pairs,"unavailable_items":self.unavailable,"unsupported_items":self.unsupported,"sampling":false},
            "budget":{"limits":figure_diagnostic_limits(),"snapshot_bytes":self.snapshot_bytes,"batch_objects":objects,"batch_points":points},"truncated":!complete,"continuation_required":!complete,
            "preview_request":{"tool":"plotium_render_preview","arguments":{"expected_project_context":self.project_context,"expected_revision":self.revision,"target":self.params.canvas,"max_edge":1440},"mapping":"PNG pixel x/y = output_page_pt x/y * returned width_px/height_px divided by output page width_pt/height_pt. Editor zoom is unrelated."},
            "read_policy":{"modifies_content":false,"advances_revision":false,"audit":"none","persists_diagnostic_geometry":false}})
    }
    fn box_observation(
        &mut self,
        target: &TargetRef,
        part: &str,
        bounds: Rect,
        (clip, clip_sources): (Option<Rect>, &[&str]),
        basis: &str,
        container: Option<Rect>,
    ) {
        if !capture::finite(bounds) || !capture::finite(geometry::scale(bounds, self.scale)) {
            self.coverage(
                target,
                part,
                "unavailable",
                "Non-finite or degenerate measurement.",
            );
            return;
        }
        let visible = clip.and_then(|c| geometry::intersection(c, bounds));
        let evidence = json!({"bounds":self.rect(bounds),"clip":clip.map(|r|self.rect(r)),"clip_sources":clip_sources,"visible_bounds":visible.map(|r|self.rect(r)),"container":container.map(|r|self.rect(r))});
        self.emit(json!({"kind":"layout","target":target,"component":part,"measurement_basis":basis,"evidence":evidence}));
        if self.checks(DiagnosticCheck::Bounds) && !geometry::contains(self.page, bounds) {
            self.issue(target, part, "out_of_canvas", basis, evidence.clone());
        }
        if part != "content.frame"
            && self.checks(DiagnosticCheck::Clipping)
            && !clip.is_some_and(|c| geometry::contains(c, bounds))
        {
            self.issue(
                target,
                part,
                if part == "shape" {
                    "shape_clipped"
                } else if visible.is_some() {
                    "partially_clipped"
                } else {
                    "fully_clipped"
                },
                basis,
                evidence.clone(),
            );
        }
        if self.checks(DiagnosticCheck::TextOverflow)
            && basis == "text_estimate"
            && container.is_some_and(|c| !geometry::contains(c, bounds))
        {
            self.issue(target, part, "text_container_overflow", basis, evidence);
        }
    }
    fn scan_item(&mut self, item: SnapshotItem) {
        let SnapshotItem {
            target,
            frame,
            visible,
            kind,
        } = item;
        if !visible {
            self.coverage(
                &target,
                "all_requested",
                "not_applicable",
                "hidden_by_content_or_parent_or_zero_opacity",
            );
            return;
        }
        let clip = match &kind {
            SnapshotKind::Raster { clip: Some(c), .. } => geometry::intersection(self.page, *c),
            _ => Some(self.page),
        };
        let clip_sources = if matches!(&kind, SnapshotKind::Raster { clip: Some(_), .. }) {
            vec!["output_page", "raster_frame", "parent_panel_clip_children"]
        } else {
            vec!["output_page"]
        };
        if !matches!(
            &kind,
            SnapshotKind::Text {
                part: "panel.label",
                ..
            }
        ) {
            self.box_observation(
                &target,
                "content.frame",
                frame,
                (clip, &clip_sources),
                "geometry",
                None,
            );
        }
        match kind {
            SnapshotKind::Text {
                text,
                font,
                align,
                part,
            } => {
                self.text_font_coverage(&target);
                if let Some(r) = geometry::overlay_text_bounds(frame, &text, font, align) {
                    self.box_observation(
                        &target,
                        part,
                        r,
                        (Some(self.page), &["output_page"]),
                        "text_estimate",
                        (part == "text").then_some(frame),
                    );
                } else {
                    self.coverage(&target, "text", "not_applicable", "empty_text");
                }
            }
            SnapshotKind::Raster { status, bounds, .. } => {
                self.coverage(
                    &target,
                    "raster",
                    if bounds.is_some() {
                        "complete"
                    } else {
                        "unavailable"
                    },
                    &status,
                );
                if let Some(bounds) = bounds {
                    let actual_clip = clip.and_then(|c| geometry::intersection(c, frame));
                    self.box_observation(
                        &target,
                        "raster.viewport",
                        bounds,
                        (
                            actual_clip,
                            &[
                                "output_page",
                                "raster_frame",
                                "optional_parent_clip_children",
                            ],
                        ),
                        "native_raster_viewport_decode_unverified",
                        None,
                    );
                    self.coverage(&target,"raster_pixels","unsupported","Header-based viewport geometry; decoding and alpha-pixel coverage are unverified. Use native preview to verify asset rendering.");
                }
            }
            SnapshotKind::Shape { bounds } => {
                self.box_observation(
                    &target,
                    "shape",
                    bounds,
                    (Some(self.page), &["output_page"]),
                    "renderer_shape_extrema",
                    None,
                );
                self.coverage(&target,"shape","complete","Native shape extrema and actual page clip; bounds intersection is not opaque-pixel occlusion.");
            }
            SnapshotKind::Plot {
                mut figure,
                selected_series,
                declared,
            } => {
                let guide = plotium_render::guide_geometry(&figure, [frame.width, frame.height]);
                let move_rect = |r| geometry::translate(r, frame.left, frame.top);
                for (part, r) in [
                    ("guide.legend", guide.legend),
                    ("guide.color_scale", guide.color_scale),
                ] {
                    if let Some(r) = r {
                        self.box_observation(
                            &target,
                            part,
                            move_rect(r),
                            (Some(self.page), &["output_page"]),
                            "renderer_layout_text_width_estimate",
                            None,
                        );
                    }
                }
                if guide.legend.is_none() && self.checks(DiagnosticCheck::LegendOcclusion) {
                    self.coverage(
                        &target,
                        "legend_occlusion",
                        "not_applicable",
                        guide.not_rendered_reason.unwrap_or("no_legend"),
                    );
                }
                self.text_font_coverage(&target);
                for t in geometry::figure_text_boxes(&figure, [frame.width, frame.height]) {
                    self.box_observation(
                        &target,
                        &t.component,
                        move_rect(t.bounds),
                        (Some(self.page), &["output_page"]),
                        "text_estimate",
                        t.container.map(move_rect),
                    );
                }
                if !figure.polygons.is_empty()
                    || !figure.contours.is_empty()
                    || figure.heatmap.is_some()
                    || !figure.integral_curves.is_empty()
                    || figure.text_annotations().next().is_some()
                    || !figure.range_annotations.is_empty()
                    || figure.top_projection.is_some()
                    || figure.left_projection.is_some()
                {
                    self.coverage(&target,"plot_internal_geometry","unsupported","Non-line/scatter primitives and derived annotations are not scanned for clipping or occlusion; frame and guide observations remain available.");
                }
                if !figure.error_bars.is_empty() {
                    self.coverage(
                        &target,
                        "error_bar_clipping_occlusion",
                        "unsupported",
                        "Error-bar internal clipping/occlusion is unsupported; ordinary line/point coverage excludes whiskers.",
                    );
                }
                let rendered_origins = figure
                    .series
                    .iter()
                    .filter_map(|s| s.origin.map(|o| SeriesId::new(o.0)))
                    .collect::<BTreeSet<_>>();
                if declared.is_empty() && figure.series.is_empty() {
                    self.coverage(
                        &target,
                        "curve_primitives",
                        "not_applicable",
                        "empty_rendered_data",
                    );
                }
                for (id, visible, source) in &declared {
                    if selected_series.as_ref().is_some_and(|s| !s.contains(id)) {
                        continue;
                    }
                    let st = TargetRef {
                        resource: target.resource.clone(),
                        component: Some(ComponentRef::Series(*id)),
                    };
                    if !*visible {
                        self.coverage(&st, "curve_primitives", if rendered_origins.contains(id){"unavailable"}else{"not_applicable"}, if rendered_origins.contains(id){"hidden_binding_has_native_fallback_rendering; not included in Series occlusion coverage"}else{"hidden_series"});
                    } else if source != "resolved" && source != "incompatible_encoding" {
                        self.coverage(&st,"curve_primitives","unavailable",&format!("{source}: native source is unavailable. Cached rendered geometry, if present, is reported separately."));
                    } else if source == "incompatible_encoding" {
                        self.coverage(
                            &st,
                            "curve_primitives",
                            "unsupported",
                            "This encoding is not an ordinary line/scatter source.",
                        );
                    } else if !rendered_origins.contains(id)
                        && figure.series.iter().any(|s| s.origin.is_none())
                    {
                        self.coverage(&st,"curve_primitives","unavailable","Rendered runs have no verified native provenance; inspect the owning plot and preview before editing.");
                    } else if !rendered_origins.contains(id) {
                        self.coverage(
                            &st,
                            "curve_primitives",
                            "not_applicable",
                            "empty_rendered_data: no native runs for this visible Series",
                        );
                    }
                }
                let mut runs = VecDeque::new();
                for series in std::mem::take(&mut figure.series) {
                    let Some(origin) = series.origin else {
                        self.coverage(&target,"curve_primitives","unsupported","Rendered curve has no native Series provenance; it is not assigned an invented identity.");
                        continue;
                    };
                    let id = SeriesId::new(origin.0);
                    if selected_series.as_ref().is_some_and(|s| !s.contains(&id)) {
                        continue;
                    }
                    // A builder may render a primary fallback even for a hidden binding.
                    // Diagnostics never promote that fallback to visible Series coverage.
                    if !declared.iter().any(|(sid, v, _)| *sid == id && *v) {
                        continue;
                    }
                    if series.kind == plotium_figure::SeriesKind::Stick {
                        self.coverage(
                            &target,
                            "curve_primitives",
                            "unsupported",
                            "Stick encoding is not an ordinary line/scatter curve.",
                        );
                        continue;
                    }
                    if self.checks(DiagnosticCheck::Clipping)
                        || self.checks(DiagnosticCheck::LegendOcclusion)
                    {
                        runs.push_back(series);
                    }
                }
                if !runs.is_empty() {
                    self.curves.push_back(CurveWork {
                        target,
                        frame,
                        plot: guide.plot,
                        legend: guide.legend,
                        figure: std::sync::Arc::from(figure),
                        runs,
                        current: None,
                    });
                }
            }
        }
    }
    fn text_font_coverage(&mut self, target: &TargetRef) {
        if self.font_status == Some("missing") {
            self.coverage(target,"text","unavailable","No native export fonts are available. Bounds remain estimates; rendered text is unverified.");
        } else {
            self.coverage(target,"text","complete","Native width/line-spacing estimates; glyph coverage and exact cross-backend ink bounds are unverified.");
        }
    }
}
