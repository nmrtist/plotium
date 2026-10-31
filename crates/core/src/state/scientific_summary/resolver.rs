use super::{CanvasScientificSummary, ScientificSummary, SummaryLine, SummaryPart};
use crate::state::{
    CanvasDocument, Dataset, DatasetId, FieldDescriptor, PlotObject, PlotiumApp, SeriesBinding,
};

pub trait ScientificSummaryProvider {
    fn scientific_summary(
        &self,
        app: &PlotiumApp,
        field: &FieldDescriptor,
        series: &SeriesBinding,
        plot: &PlotObject,
    ) -> ScientificSummary;
}

impl PlotiumApp {
    pub fn canvas_scientific_summary(&self, canvas_index: usize) -> CanvasScientificSummary {
        let Some(canvas) = self.doc.canvases.get(canvas_index) else {
            return CanvasScientificSummary::default();
        };
        canvas_summary(self, canvas)
    }
}

fn canvas_summary(app: &PlotiumApp, canvas: &CanvasDocument) -> CanvasScientificSummary {
    let panels = canvas
        .panel_reading_order()
        .into_iter()
        .filter_map(|panel_id| {
            let panel = canvas.panel(panel_id)?;
            if !panel.visible {
                return None;
            }
            let summaries = panel
                .item_order
                .iter()
                .filter_map(|id| canvas.object(*id))
                .filter(|object| object.visible)
                .filter_map(|object| object.plot())
                .filter_map(|plot| plot_summary(app, plot))
                .collect::<Vec<_>>();
            combine_summaries(summaries).map(|summary| (panel_id, summary))
        })
        .collect::<Vec<_>>();

    if panels.is_empty() {
        return CanvasScientificSummary::default();
    }
    if panels.len() == 1 {
        return CanvasScientificSummary {
            lines: vec![SummaryLine {
                panel_label: None,
                parts: panels[0].1.parts(),
            }],
        };
    }

    let all_parts = panels
        .iter()
        .map(|(_, summary)| summary.parts())
        .collect::<Vec<_>>();
    let common_len = longest_common_prefix(&all_parts);
    let mut lines = Vec::new();
    if common_len > 0 {
        lines.push(SummaryLine {
            panel_label: None,
            parts: all_parts[0][..common_len].to_vec(),
        });
    }
    for (panel_index, ((panel_id, _), parts)) in panels.iter().zip(all_parts).enumerate() {
        let remainder = parts[common_len..].to_vec();
        if remainder.is_empty() {
            continue;
        }
        lines.push(SummaryLine {
            panel_label: Some(if canvas.panel_label_is_displayed(*panel_id) {
                canvas
                    .panel(*panel_id)
                    .and_then(|panel| panel.item_order.first())
                    .and_then(|id| canvas.panel_letter(*id))
                    .unwrap_or_else(|| format!("Panel {}", panel_index + 1))
            } else {
                format!("Panel {}", panel_index + 1)
            }),
            parts: remainder,
        });
    }
    CanvasScientificSummary { lines }
}

fn plot_summary(app: &PlotiumApp, plot: &PlotObject) -> Option<ScientificSummary> {
    let summaries = plot
        .binding
        .series
        .iter()
        .filter(|series| series.visible)
        .filter_map(|series| {
            let dataset = app
                .doc
                .dataset_index(series.source.resource)
                .and_then(|index| app.doc.datasets.get(index))?;
            let field = dataset.field_descriptor(series.source.field)?;
            Some(dataset.scientific_summary(app, &field, series, plot))
        })
        .collect::<Vec<_>>();
    combine_summaries(summaries)
}

fn combine_summaries(summaries: Vec<ScientificSummary>) -> Option<ScientificSummary> {
    let first = summaries.first()?.clone();
    if summaries.len() == 1 {
        return Some(first);
    }
    Some(ScientificSummary {
        subject: combine_part(&summaries, |summary| &summary.subject, "subjects"),
        observation: combine_part(&summaries, |summary| &summary.observation, "observations"),
        context: combine_optional_part(&summaries, |summary| summary.context.as_ref(), "contexts"),
    })
}

fn combine_part(
    summaries: &[ScientificSummary],
    part: impl Fn(&ScientificSummary) -> &SummaryPart,
    plural: &str,
) -> SummaryPart {
    combine_unique_parts(
        summaries.iter().map(|summary| part(summary).clone()),
        plural,
    )
    .expect("a combined required summary part has at least one value")
}

fn combine_unique_parts(
    parts: impl IntoIterator<Item = SummaryPart>,
    plural: &str,
) -> Option<SummaryPart> {
    let mut unique = Vec::<SummaryPart>::new();
    for candidate in parts {
        if !unique
            .iter()
            .any(|value| value.semantic_key == candidate.semantic_key)
        {
            unique.push(candidate);
        }
    }
    Some(match unique.as_slice() {
        [] => return None,
        [only] => only.clone(),
        [left, right] => SummaryPart::new(
            format!("{}+{}", left.semantic_key, right.semantic_key),
            format!("{} + {}", left.text, right.text),
        ),
        many => SummaryPart::new(
            format!("{plural}:{}", many.len()),
            format!("{} {plural}", many.len()),
        ),
    })
}

fn combine_optional_part<'a>(
    summaries: &'a [ScientificSummary],
    part: impl Fn(&'a ScientificSummary) -> Option<&'a SummaryPart>,
    plural: &str,
) -> Option<SummaryPart> {
    combine_unique_parts(summaries.iter().filter_map(part).cloned(), plural)
}

fn longest_common_prefix(parts: &[Vec<SummaryPart>]) -> usize {
    let Some(first) = parts.first() else { return 0 };
    (0..first.len())
        .take_while(|&index| {
            parts.iter().all(|candidate| {
                candidate
                    .get(index)
                    .is_some_and(|part| part.semantic_key == first[index].semantic_key)
            })
        })
        .count()
}

impl ScientificSummaryProvider for Dataset {
    fn scientific_summary(
        &self,
        app: &PlotiumApp,
        field: &FieldDescriptor,
        series: &SeriesBinding,
        plot: &PlotObject,
    ) -> ScientificSummary {
        provider_summary(self, app, field, series, plot)
    }
}

fn provider_summary(
    dataset: &Dataset,
    app: &PlotiumApp,
    field: &FieldDescriptor,
    series: &SeriesBinding,
    plot: &PlotObject,
) -> ScientificSummary {
    let subject = subject_part(dataset, app);
    let mut summary = match dataset {
        Dataset::Nmr(data) => ScientificSummary {
            subject,
            observation: nmr_1d_observation(data),
            context: acquisition_part(dataset, None),
        },
        Dataset::Nmr2D(data) => ScientificSummary {
            subject,
            observation: nmr_2d_observation(data),
            context: acquisition_part(dataset, data.data.experiment.as_deref()),
        },
        Dataset::Table(data) => ScientificSummary {
            subject,
            observation: series
                .source
                .columns()
                .and_then(|columns| {
                    let schema = &data.typed_state.envelope.revision.snapshot.schema;
                    let x = &schema.column(columns.x_column_id)?.name;
                    let y = &schema.column(columns.y_column_id)?.name;
                    Some(SummaryPart::new(
                        format!("table:{}:vs:{}", normalize_key(y), normalize_key(x)),
                        format!("{y} vs {x}"),
                    ))
                })
                .unwrap_or_else(|| table_observation(data, plot)),
            context: table_chart_context(dataset, plot),
        },
        Dataset::Electrophysiology(data) => ScientificSummary {
            subject,
            observation: field.scientific_observation.clone(),
            context: acquisition_part(dataset, data.data.protocol.as_deref()),
        },
        Dataset::Afm(data) => ScientificSummary {
            subject,
            observation: field.scientific_observation.clone(),
            context: afm_context(data, field),
        },
        Dataset::MassSpec(data) => ScientificSummary {
            subject,
            observation: field.scientific_observation.clone(),
            context: mass_spec_context(data, field.id),
        },
        Dataset::Xrd(data) => ScientificSummary {
            subject,
            observation: SummaryPart::new("xrd:powder", "Powder XRD"),
            context: data
                .data
                .target
                .as_ref()
                .map(|target| SummaryPart::new(format!("xrd:target:{target}"), target.clone()))
                .or_else(|| {
                    data.data.wavelength_angstrom.map(|value| {
                        SummaryPart::new(
                            format!("xrd:wavelength:{value}"),
                            format!("λ {value:.4} Å"),
                        )
                    })
                }),
        },
        Dataset::Xps(data) => xps_summary(data, field, subject),
        Dataset::Xas(data) => ScientificSummary {
            subject,
            observation: field.scientific_observation.clone(),
            context: data.scan.element.as_ref().map(|element| {
                let edge = data.scan.edge.as_deref().unwrap_or_default();
                let text = format!("{element} {edge}");
                SummaryPart::new(format!("xas:edge:{text}"), text.trim_end().to_owned())
            }),
        },
    };
    if let Some(item_context) = trace_item_context(dataset, field, series) {
        summary.context = merge_context(summary.context, item_context);
    }
    summary
}

fn subject_part(dataset: &Dataset, app: &PlotiumApp) -> SummaryPart {
    let identity = dataset.acquisition_identity();
    let inherited = dataset
        .lineage()
        .and_then(|lineage| inherited_subject(app, &lineage.sources));
    let text = identity
        .subject
        .clone()
        .or(inherited)
        .or_else(|| dataset.name())
        .unwrap_or_else(|| identity.source_label.clone());
    SummaryPart::new(format!("subject:{}", normalize_key(&text)), text)
}

fn nmr_2d_observation(data: &crate::state::Nmr2DDataset) -> SummaryPart {
    let direct = data.data.direct.nucleus.trim();
    let indirect = data.data.indirect.nucleus.trim();
    SummaryPart::new(
        format!("nmr:{direct}:{indirect}"),
        format!(
            "{}–{}",
            crate::figures::format_nucleus(direct),
            crate::figures::format_nucleus(indirect)
        ),
    )
}

fn nmr_1d_observation(data: &crate::state::NmrDataset) -> SummaryPart {
    let domain = data.output_domain();
    SummaryPart::new(
        format!("nmr:{domain:?}:{}", data.data.nucleus()),
        if domain == plotium_io::Domain::Time {
            format!("{} FID", data.data.nucleus())
        } else {
            data.data.nucleus().to_owned()
        },
    )
}

fn inherited_subject(app: &PlotiumApp, sources: &[DatasetId]) -> Option<String> {
    let mut values = sources.iter().filter_map(|id| {
        app.doc
            .dataset_index(*id)
            .and_then(|index| app.doc.datasets.get(index))
            .map(|dataset| {
                dataset
                    .acquisition_identity()
                    .subject
                    .clone()
                    .unwrap_or_else(|| dataset.acquisition_identity().source_label.clone())
            })
    });
    let first = values.next()?;
    values
        .all(|value| normalize_key(&value) == normalize_key(&first))
        .then_some(first)
}

fn acquisition_part(dataset: &Dataset, preferred: Option<&str>) -> Option<SummaryPart> {
    preferred
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .or_else(|| dataset.acquisition_identity().acquisition.clone())
        .map(|text| SummaryPart::new(format!("acquisition:{}", normalize_key(&text)), text))
}

fn table_observation(data: &crate::state::TableDataset, plot: &PlotObject) -> SummaryPart {
    let columns = &data.typed_state.envelope.revision.snapshot.schema.columns;
    let name = |id| {
        columns
            .iter()
            .find(|column| column.id == id)
            .map(|column| column.name.clone())
    };
    let x = data.x_binding.and_then(name);
    let all_y = data
        .series_bindings
        .iter()
        .filter_map(|binding| name(binding.value_column))
        .collect::<Vec<_>>();
    let selected = plot.chart.column.and_then(name).map(|column| vec![column]);
    let multi_column = matches!(
        plot.chart.type_id.as_str(),
        "" | "table_line"
            | "table_bar_grouped"
            | "table_box"
            | "table_violin"
            | "table_heatmap"
            | "table_surface"
    );
    let y = if multi_column {
        all_y
    } else {
        selected.unwrap_or_else(|| all_y.into_iter().take(1).collect())
    };
    let y_text = y.join(" + ");
    if y_text.is_empty() {
        return SummaryPart::new("table:data", "Data table");
    }
    let y_key = y
        .iter()
        .map(|value| normalize_key(value))
        .collect::<Vec<_>>()
        .join("+");
    if matches!(
        plot.chart.type_id.as_str(),
        "table_box" | "table_violin" | "table_histogram"
    ) {
        return SummaryPart::new(format!("table:{y_key}"), y_text);
    }
    match x {
        Some(x) => SummaryPart::new(
            format!("table:{y_key}:vs:{}", normalize_key(&x)),
            format!("{y_text} vs {x}"),
        ),
        None => SummaryPart::new(format!("table:{y_key}"), y_text),
    }
}

fn table_chart_context(dataset: &Dataset, plot: &PlotObject) -> Option<SummaryPart> {
    let name = crate::state::resolved_chart_type(dataset.domain(), &plot.chart.type_id).name;
    let display = name.strip_suffix(" chart").unwrap_or(name);
    (!display.eq_ignore_ascii_case("line"))
        .then(|| SummaryPart::new(format!("chart:{}", plot.chart.type_id), display))
}

fn trace_item_context(
    dataset: &Dataset,
    field: &FieldDescriptor,
    series: &SeriesBinding,
) -> Option<SummaryPart> {
    let item_id = series.source.item()?;
    let item = dataset.trace_collection(field.id)?.item(item_id)?;
    let label = item.automatic_label()?;
    Some(SummaryPart::new(
        format!("trace-item:{}:{item_id}", dataset.resource_id()),
        label,
    ))
}

fn merge_context(existing: Option<SummaryPart>, item: SummaryPart) -> Option<SummaryPart> {
    match existing {
        None => Some(item),
        Some(existing) => Some(SummaryPart::new(
            format!("{}+{}", existing.semantic_key, item.semantic_key),
            format!("{} + {}", existing.text, item.text),
        )),
    }
}

fn afm_context(data: &crate::state::AfmDataset, field: &FieldDescriptor) -> Option<SummaryPart> {
    if field.local_id == "afm.force_curve" {
        let [x, y] = data.selected_pixel;
        return Some(SummaryPart::new(
            format!("afm:pixel:{x}:{y}"),
            format!("Pixel ({x}, {y})"),
        ));
    }
    data.data
        .images
        .iter()
        .zip(data.image_field_keys.iter())
        .find(|(_, key)| key.as_str() == field.local_id)
        .and_then(|(channel, _)| match channel.frame_direction {
            plotium_io::AfmFrameDirection::Trace => Some(("afm:trace", "Trace")),
            plotium_io::AfmFrameDirection::Retrace => Some(("afm:retrace", "Retrace")),
            plotium_io::AfmFrameDirection::Unknown => None,
        })
        .map(|(key, text)| SummaryPart::new(key, text))
}

fn mass_spec_context(
    data: &crate::state::MassSpecDataset,
    field: crate::state::FieldId,
) -> Option<SummaryPart> {
    let provenance = data
        .chromatogram_provenance_for_field(field)
        .map(|provenance| {
            SummaryPart::new(
                format!(
                    "mass:chromatogram-provenance:{}",
                    provenance.machine_label()
                ),
                provenance.display_label(),
            )
        });
    let Some(stream_id) = data
        .chromatogram_stream_for_field(field)
        .or_else(|| data.spectrum_stream_for_field(field))
    else {
        return provenance;
    };
    let Some(stream) = data.run.stream(stream_id) else {
        return provenance;
    };
    let polarity = match stream.polarity() {
        plotium_io::Polarity::Positive => "positive",
        plotium_io::Polarity::Negative => "negative",
        plotium_io::Polarity::Unknown => "",
    };
    let level = stream.spectra.first().map(|spectrum| spectrum.ms_level);
    let text = match (polarity.is_empty(), level) {
        (false, Some(level)) => format!("{polarity} MS{level}"),
        (false, None) => polarity.to_owned(),
        (true, Some(level)) => format!("MS{level}"),
        (true, None) => return provenance,
    };
    let acquisition = SummaryPart::new(format!("mass:{}:{:?}", polarity, level), text);
    provenance
        .map(|provenance| merge_context(Some(acquisition.clone()), provenance).unwrap())
        .or(Some(acquisition))
}

fn xps_summary(
    data: &crate::state::XpsDataset,
    field: &FieldDescriptor,
    fallback_subject: SummaryPart,
) -> ScientificSummary {
    let region = data
        .experiment
        .regions
        .iter()
        .find(|region| data.field_for_region(region.id) == Some(field.id));
    let subject = region
        .and_then(|region| {
            data.experiment
                .measurements
                .iter()
                .find(|measurement| measurement.id == region.measurement)
        })
        .map(|measurement| {
            SummaryPart::new(
                xps_local_key(data.resource_id, "measurement", measurement.id.0),
                measurement.label.clone(),
            )
        })
        .unwrap_or(fallback_subject);
    let observation = region
        .map(|region| {
            SummaryPart::new(
                xps_local_key(data.resource_id, "region", region.id.0),
                region.name.clone(),
            )
        })
        .unwrap_or_else(|| field.scientific_observation.clone());
    let context = region.and_then(|region| {
        let shift = data
            .measurement_shifts
            .get(&region.measurement)
            .copied()
            .unwrap_or(0.0);
        if shift.abs() > f64::EPSILON {
            return Some(SummaryPart::new(
                format!("xps:shift:{shift}"),
                format!("Energy shift {shift:+.2} eV"),
            ));
        }
        region
            .metadata
            .get("anode")
            .or_else(|| data.experiment.metadata.get("anode"))
            .map(|anode| {
                SummaryPart::new(format!("xps:anode:{}", normalize_key(anode)), anode.clone())
            })
    });
    ScientificSummary {
        subject,
        observation,
        context,
    }
}

fn xps_local_key(dataset: DatasetId, kind: &str, local_id: u64) -> String {
    format!("xps:{dataset}:{kind}:{local_id}")
}

fn normalize_key(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

#[cfg(test)]
#[path = "resolver_tests.rs"]
mod tests;
