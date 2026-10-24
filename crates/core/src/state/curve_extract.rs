//! Strict, row-aligned extraction shared by binding validation and native rendering.
use super::curve_layout::{LabelStyle, MarkKind, Occupant, Overlay, Role};
use super::*;
use plotium_data::{LogicalType, RowId, ScalarValue};
use plotium_figure::{ErrorBar, Figure, Series, SeriesEncoding, SeriesKind};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CurveDiagnostics {
    pub total_rows: usize,
    pub plotted_rows: usize,
    pub missing_xy: usize,
    pub non_finite_xy: usize,
    pub missing_error: usize,
    pub affected_rows: Vec<RowId>,
}
impl CurveDiagnostics {
    fn note(&mut self, row: RowId) {
        if self.affected_rows.len() < 20 && self.affected_rows.last() != Some(&row) {
            self.affected_rows.push(row);
        }
    }
}

pub struct ExtractedTableCurve {
    /// For a text x column, x values are indices into `categories`.
    pub runs: Vec<Vec<[f64; 2]>>,
    /// Distinct x texts in first-occurrence row order; `None` for numeric x.
    pub categories: Option<Vec<String>>,
    pub errors: Vec<ErrorBar>,
    pub x_label: String,
    pub y_label: String,
    pub diagnostics: CurveDiagnostics,
}

fn number(value: &ScalarValue) -> Result<Option<f64>, String> {
    match value {
        ScalarValue::Null => Ok(None),
        ScalarValue::Int64(v) => {
            let n = *v as f64;
            if n as i128 != i128::from(*v) {
                return Err(format!(
                    "Integer {v} cannot be represented exactly in curve coordinates."
                ));
            }
            Ok(Some(n))
        }
        ScalarValue::Float64(v) => Ok(Some(*v)),
        _ => Err("Curve columns must be Int64 or Float64.".into()),
    }
}

pub fn extract_table_curve(
    table: &TableDataset,
    columns: TableCurveColumns,
    coordinates: &CurveCoordinates,
) -> Result<ExtractedTableCurve, String> {
    let revision = &table.typed_state.envelope.revision;
    if columns.table_id != revision.table_id {
        return Err("Curve TableId is stale.".into());
    }
    let schema = &revision.snapshot.schema;
    let mut projection = vec![columns.x_column_id, columns.y_column_id];
    projection.extend(columns.y_error_column_id);
    projection.sort();
    projection.dedup();
    let text_x = schema
        .column(columns.x_column_id)
        .is_some_and(|c| c.logical_type == LogicalType::Utf8);
    for id in &projection {
        let c = schema
            .column(*id)
            .ok_or_else(|| format!("Curve column {id} is absent."))?;
        let numeric = matches!(c.logical_type, LogicalType::Int64 | LogicalType::Float64);
        if !numeric && !(text_x && *id == columns.x_column_id) {
            return Err(format!(
                "Curve column {id} must be Int64 or Float64 (an x may also be Utf8 text)."
            ));
        }
    }
    let source_units = table_curve_coordinates(table, columns)?;
    // Category slots have no unit to convert.
    let x_conversion = (!text_x)
        .then(|| {
            super::curve_binding::CurveUnitConversion::new(&source_units.x, &coordinates.x, false)
        })
        .transpose()?;
    let y_conversion =
        super::curve_binding::CurveUnitConversion::new(&source_units.y, &coordinates.y, false)?;
    let error_unit = columns
        .y_error_column_id
        .and_then(|id| schema.column(id))
        .and_then(|c| c.unit.clone());
    let error_conversion = columns
        .y_error_column_id
        .map(|_| super::curve_binding::CurveUnitConversion::new(&error_unit, &coordinates.y, true))
        .transpose()?;
    let label = |id, unit: &Option<plotium_data::UnitSpec>| {
        let name = &schema
            .column(id)
            .expect("projection columns were checked")
            .name;
        unit.as_ref()
            .map_or_else(|| name.clone(), |u| format!("{name} ({})", u.display_unit))
    };
    let mut out = ExtractedTableCurve {
        runs: Vec::new(),
        categories: text_x.then(Vec::new),
        errors: Vec::new(),
        x_label: label(columns.x_column_id, &coordinates.x),
        y_label: label(columns.y_column_id, &coordinates.y),
        diagnostics: CurveDiagnostics::default(),
    };
    let rows = table.typed_rows(usize::MAX, &projection)?;
    let values = |id| -> Result<&Vec<ScalarValue>, String> {
        rows.columns
            .iter()
            .find(|c| c.schema.id == id)
            .map(|c| &c.values)
            .ok_or_else(|| format!("Missing projected column {id}."))
    };
    let x = values(columns.x_column_id)?;
    let y = values(columns.y_column_id)?;
    let error = columns.y_error_column_id.map(values).transpose()?;
    if x.len() != rows.row_ids.len()
        || y.len() != x.len()
        || error.is_some_and(|v| v.len() != x.len())
    {
        return Err("Curve columns are not row-aligned.".into());
    }
    out.diagnostics.total_rows = rows.row_ids.len();
    let mut slots = std::collections::HashMap::new();
    let mut run = Vec::new();
    for (i, row) in rows.row_ids.iter().copied().enumerate() {
        let at = |e: String| format!("Row {row}: {e}");
        let px = match (&mut out.categories, &x[i]) {
            (None, value) => number(value).map_err(at)?,
            (Some(_), ScalarValue::Null) => None,
            (Some(categories), ScalarValue::Utf8(text)) => {
                Some(*slots.entry(text.clone()).or_insert_with(|| {
                    categories.push(text.clone());
                    categories.len() - 1
                }) as f64)
            }
            (Some(_), _) => return Err(at("Category x values must be text.".into())),
        };
        let py = number(&y[i]).map_err(at)?;
        let sigma = error
            .map(|v| number(&v[i]))
            .transpose()
            .map_err(at)?
            .flatten();
        if sigma.is_some_and(|v| !v.is_finite() || v < 0.0) {
            return Err(format!(
                "Row {row}: Y error must be a finite non-negative absolute amplitude."
            ));
        }
        if error.is_some() && sigma.is_none() {
            out.diagnostics.missing_error += 1;
            out.diagnostics.note(row);
        }
        let missing = px.is_none() || py.is_none();
        let nonfinite = px.is_some_and(|v| !v.is_finite()) || py.is_some_and(|v| !v.is_finite());
        if missing || nonfinite {
            out.diagnostics.missing_xy += usize::from(missing);
            out.diagnostics.non_finite_xy += usize::from(nonfinite);
            out.diagnostics.note(row);
            if !run.is_empty() {
                out.runs.push(std::mem::take(&mut run));
            }
            continue;
        }
        let px = px.expect("finite pair");
        let point = [
            match &x_conversion {
                Some(conversion) => conversion.apply(px).map_err(at)?,
                None => px,
            },
            y_conversion.apply(py.expect("finite pair")).map_err(at)?,
        ];
        if let Some(sigma) = sigma {
            let sigma = error_conversion
                .as_ref()
                .expect("selected error column")
                .apply(sigma)
                .map_err(at)?;
            if !(point[1] - sigma).is_finite() || !(point[1] + sigma).is_finite() {
                return Err(format!("Row {row}: error extent overflowed."));
            }
            out.errors.push(ErrorBar::symmetric(point, sigma));
        }
        run.push(point);
        out.diagnostics.plotted_rows += 1;
    }
    if !run.is_empty() {
        out.runs.push(run);
    }
    Ok(out)
}

impl PlotiumApp {
    pub fn build_explicit_binding_figure(
        &self,
        binding: &DataBinding,
        stack: &StackSpec,
        size_mm: [f32; 2],
    ) -> Result<Figure, String> {
        let coordinates = binding
            .coordinates
            .as_ref()
            .ok_or("Explicit curves require plot coordinates.")?;
        let mut figure = Figure::new("", Axis::new("x", 0., 1.), Axis::new("y", 0., 1.));
        let mut xmin = f64::INFINITY;
        let mut xmax = f64::NEG_INFINITY;
        let mut ymin = f64::INFINITY;
        let mut ymax = f64::NEG_INFINITY;
        let mut label_set = false;
        let mut x_label_set = false;
        let mut prepared = Vec::new();
        let mut global_peak = 0.0_f64;
        let mut first_span = None;
        // Whether the plot's x is text categories, fixed by its first curve.
        let mut categorical = None;
        let mut categories = super::curve_bars::CategorySlots::default();
        // Right-axis curves are converted into the right axis's own unit.
        let right_coordinates = coordinates.for_axis(plotium_figure::YAxisSide::Right);
        let mut right_label = None;
        for (series_index, sb) in binding.series.iter().enumerate() {
            let SeriesEncoding::Line(line) = &sb.encoding else {
                return Err("Column bindings require a line/point encoding.".into());
            };
            if matches!(line.kind, SeriesKind::Stick) {
                return Err(
                    "Explicit column curves support line, points, bars, boxes and violins.".into(),
                );
            }
            if !line.scale.is_finite() {
                return Err("Curve scale must be finite.".into());
            }
            let aux_line_pt = self.doc.style_library.figure_typography.aux_line_pt;
            let right = line.appearance.y_axis.is_right();
            if right && !super::right_axis_supports(line.kind) {
                return Err(super::right_axis::RIGHT_AXIS_KINDS.into());
            }
            if right && coordinates.transposed {
                return Err(super::right_axis::HORIZONTAL_ONE_VALUE_AXIS.into());
            }
            let role = match line.kind {
                SeriesKind::Bar => Role::Occupant(MarkKind::Bar),
                SeriesKind::Box => Role::Occupant(MarkKind::Box),
                SeriesKind::Violin => Role::Occupant(MarkKind::Violin),
                _ => Role::Overlay(line.spread.map_or(0.0, |s| s.get())),
            };
            let style = super::curve_bars::BarStyle::of(line, aux_line_pt);
            let occupant = matches!(role, Role::Occupant(_));
            let distribution = matches!(role, Role::Occupant(MarkKind::Box | MarkKind::Violin));
            let (table, selections) = self.table_curve_sources(sb.source)?;
            for (index, columns) in selections.iter().copied().enumerate() {
                let target = if right {
                    &right_coordinates
                } else {
                    coordinates
                };
                let mut extracted = extract_table_curve(table, columns, target)?;
                let text_x = extracted.categories.is_some();
                if distribution && columns.y_error_column_id.is_some() {
                    return Err(
                        "Box and violin series summarise raw samples; give them no y error column."
                            .into(),
                    );
                }
                if *categorical.get_or_insert(text_x) != text_x {
                    return Err(
                        "All series in one plot must share a text or a numeric x column.".into(),
                    );
                }
                if let Some(local) = &extracted.categories {
                    categories.remap(local, &mut extracted.runs, &mut extracted.errors);
                }
                if !x_label_set {
                    figure.x.label = std::mem::take(&mut extracted.x_label);
                    x_label_set = true;
                }
                if right {
                    right_label.get_or_insert(extracted.y_label);
                } else if !label_set {
                    figure.y.label = extracted.y_label;
                    label_set = true;
                }
                if !sb.visible {
                    continue;
                }
                let mut part = Figure::new("", Axis::new("x", 0., 1.), Axis::new("y", 0., 1.));
                let name = sb.label.clone().unwrap_or_else(|| {
                    table
                        .typed_state
                        .envelope
                        .revision
                        .snapshot
                        .schema
                        .column(columns.y_column_id)
                        .expect("validated column")
                        .name
                        .clone()
                });
                for points in extracted.runs {
                    let mut curve = Series::line(name.clone(), points);
                    curve.kind = if occupant || sb.source.columns().is_some() {
                        line.kind
                    } else {
                        SeriesKind::Points
                    };
                    part.series.push(curve);
                }
                part.error_bars = extracted.errors;
                self.apply_series_binding_style(&mut part, sb);
                if selections.len() > 1 {
                    let color = table.default_curve_color(index, columns.y_column_id);
                    for s in &mut part.series {
                        s.color = color;
                    }
                    for e in &mut part.error_bars {
                        e.color = color;
                    }
                }
                let peak = part
                    .series
                    .iter()
                    .flat_map(|s| &s.points)
                    .fold(0.0_f64, |m, p| m.max(p[1].abs()));
                let factor = if stack.normalize && peak > 0.0 {
                    1.0 / peak
                } else {
                    1.0
                };
                for series in &mut part.series {
                    for p in &mut series.points {
                        p[1] *= factor;
                    }
                }
                for e in &mut part.error_bars {
                    e.center[1] *= factor;
                    e.negative *= factor.abs();
                    e.positive *= factor.abs();
                }
                global_peak = global_peak.max(peak * factor);
                if first_span.is_none() {
                    let mut lo = f64::INFINITY;
                    let mut hi = f64::NEG_INFINITY;
                    for p in part.series.iter().flat_map(|s| &s.points) {
                        lo = lo.min(p[0]);
                        hi = hi.max(p[0]);
                    }
                    if lo.is_finite() {
                        first_span = Some((hi - lo).max(f64::MIN_POSITIVE));
                    }
                }
                prepared.push((series_index, part, role, line.value_labels, style));
            }
        }
        let (mut occupants, mut overlays) = (Vec::new(), Vec::new());
        let labels = prepared.iter().any(|p| p.3);
        for (index, mut part, role, value_labels, style) in prepared {
            let offset = matches!(stack.mode, StackMode::Offset);
            // Bars stay on their slots; shear would skew the slot width.
            let xoff = if offset && matches!(role, Role::Overlay(_)) {
                index as f64 * stack.shear_x * first_span.unwrap_or(1.0)
            } else {
                0.0
            };
            let yoff = if offset {
                index as f64 * stack.spacing_y * global_peak
            } else {
                0.0
            };
            for series in &mut part.series {
                series.display_baseline += yoff;
                if stack.active == Some(index) {
                    series.width = series.width.max(1.0) * 2.0;
                }
                for p in &mut series.points {
                    p[0] += xoff;
                    p[1] += yoff;
                    let [x, y] = *p;
                    if !x.is_finite() || !y.is_finite() {
                        return Err("Curve presentation overflowed.".into());
                    }
                    xmin = xmin.min(x);
                    xmax = xmax.max(x);
                    ymin = ymin.min(y);
                    ymax = ymax.max(y);
                }
            }
            for e in &mut part.error_bars {
                e.display_baseline += yoff;
                e.center[0] += xoff;
                e.center[1] += yoff;
                if stack.active == Some(index) {
                    e.width = e.width.max(1.0) * 2.0;
                }
                if !(e.center[1] - e.negative).is_finite()
                    || !(e.center[1] + e.positive).is_finite()
                {
                    return Err("Curve error presentation overflowed.".into());
                }
                ymin = ymin.min(e.center[1] - e.negative);
                ymax = ymax.max(e.center[1] + e.positive);
            }
            match role {
                Role::Occupant(kind) => occupants.push(Occupant {
                    kind,
                    series: part.series,
                    errors: part.error_bars,
                    style,
                    baseline: yoff,
                    labels: value_labels,
                }),
                Role::Overlay(spread) => overlays.push(Overlay {
                    series: part.series,
                    errors: part.error_bars,
                    spread,
                    labels: value_labels,
                    baseline: yoff,
                }),
            }
        }
        let categorical = categorical == Some(true);
        let has_bars = occupants.iter().any(|o| o.kind == MarkKind::Bar);
        let typography = self.doc.style_library.figure_typography;
        let label_style = LabelStyle {
            size: typography.tick_pt,
            color: typography.legend_color,
        };
        if let Some([x0, x1, y0, y1]) =
            super::curve_layout::lower(&mut figure, occupants, overlays, categorical, label_style)?
        {
            xmin = xmin.min(x0);
            xmax = xmax.max(x1);
            ymin = ymin.min(y0);
            ymax = ymax.max(y1);
        }
        if categorical {
            let label = std::mem::take(&mut figure.x.label);
            figure.x = Axis::categorical(label, categories.into_names());
        } else if xmin.is_finite() {
            let pad = ((xmax - xmin) * 0.02).max(1e-12);
            figure.x.min = xmin - pad;
            figure.x.max = xmax + pad;
        }
        if ymin.is_finite() {
            let pad = ((ymax - ymin) * 0.05).max(1e-12);
            // Value labels need room past the marks' ends, on their side.
            let labelled = |anchor| figure.annotations.iter().any(|a| a.anchor == anchor);
            // On its side a label's length, not its height, runs along values.
            let label_room = if coordinates.transposed { 4.0 } else { 2.4 };
            let room = |side| {
                if labels && labelled(side) {
                    label_room * pad
                } else {
                    pad
                }
            };
            // Bars rest on a zero baseline, which stays the axis floor.
            figure.y.min = if has_bars && ymin == 0.0 {
                0.0
            } else {
                ymin - room(plotium_figure::TextAnchor::Below)
            };
            figure.y.max = ymax + room(plotium_figure::TextAnchor::Above);
        }
        if coordinates.transposed {
            super::curve_layout::transpose(&mut figure);
        }
        if ![figure.x.min, figure.x.max, figure.y.min, figure.y.max]
            .into_iter()
            .all(f64::is_finite)
        {
            return Err("Curve bounds overflowed.".into());
        }
        if let Some(label) = right_label {
            figure.y2 = Some(Axis::new(label, 0., 1.));
        }
        super::right_axis::split_y_axes(&mut figure);
        Ok(self.normalize_binding_figure(figure, size_mm))
    }
}

impl TableDataset {
    pub fn default_curve_color(
        &self,
        index: usize,
        column: plotium_data::ColumnId,
    ) -> plotium_figure::Color {
        self.provenance
            .as_ref()
            .and_then(|p| p.regions.iter().find(|r| r.column == column))
            .map(|r| plotium_figure::Color::rgb(r.color[0], r.color[1], r.color[2]))
            .unwrap_or_else(|| super::table::series_color(index))
    }
}

impl PlotiumApp {
    /// Rows at each distinct x of a table-backed explicit curve, one entry
    /// per x of each column selection.
    pub(crate) fn curve_rows_per_x(
        &self,
        binding: &DataBinding,
        series: &SeriesBinding,
    ) -> Result<Vec<usize>, String> {
        let coordinates = binding
            .coordinates
            .as_ref()
            .ok_or("The plot has no explicit coordinates.")?;
        let (table, selections) = self.table_curve_sources(series.source)?;
        let mut sizes = Vec::new();
        for columns in selections {
            let curve = extract_table_curve(table, columns, coordinates)?;
            let mut counts: std::collections::HashMap<u64, usize> = Default::default();
            for point in curve.runs.iter().flatten() {
                *counts.entry((point[0] + 0.0).to_bits()).or_default() += 1;
            }
            sizes.extend(counts.into_values());
        }
        Ok(sizes)
    }
}
