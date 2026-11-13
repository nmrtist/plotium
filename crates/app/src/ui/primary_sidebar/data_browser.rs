use plotium_core::data::ColumnId;
use plotium_core::state::{Dataset, DatasetId, ExtractionId, PlotiumApp};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, PartialEq)]
pub(super) struct DataTree {
    pub roots: Vec<DatasetNode>,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct DatasetNode {
    pub dataset: usize,
    pub linked_reference: bool,
    pub cycle_cut: bool,
    pub matches_filter: bool,
    pub analysis: Vec<AnalysisItem>,
    pub derived: Vec<DatasetNode>,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct AnalysisItem {
    pub kind: AnalysisKind,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum AnalysisKind {
    Peak(u64),
    Integral(u64),
    Region(plotium_core::state::RegionId),
    LineFit(u64),
    Multiplet(u64),
    Craft(u64),
    CurveFitResponse(ColumnId),
    MassSpectrum(ExtractionId),
}

impl AnalysisKind {
    pub fn key(&self, dataset: usize) -> String {
        match self {
            Self::Peak(id) => format!("peak:{dataset}:{id}"),
            Self::Integral(id) => format!("integral:{dataset}:{id}"),
            Self::Region(id) => format!("region:{dataset}:{id}"),
            Self::LineFit(id) => format!("line_fit:{dataset}:{id}"),
            Self::Multiplet(id) => format!("multiplet:{dataset}:{id}"),
            Self::Craft(id) => format!("craft:{dataset}:{id}"),
            Self::CurveFitResponse(column) => format!("curve_fit_response:{dataset}:{column}"),
            Self::MassSpectrum(id) => format!("mass_spectrum:{dataset}:{id}"),
        }
    }

    pub fn type_label(&self) -> &'static str {
        match self {
            Self::Peak(_) => "Peak",
            Self::Integral(_) => "Integral",
            Self::Region(_) => "Region",
            Self::LineFit(_) => "Peak fit",
            Self::Multiplet(_) => "Multiplet",
            Self::Craft(_) => "CRAFT run",
            Self::CurveFitResponse(_) => "Curve fit response",
            Self::MassSpectrum(_) => "Extracted mass spectrum",
        }
    }
}

impl DataTree {
    pub fn build(app: &PlotiumApp) -> Self {
        let count = app.doc.datasets.len();
        // This tree is rebuilt every visible frame; resolve source ids through a
        // one-shot map instead of a linear `dataset_index` scan per lineage edge.
        let index_of: HashMap<DatasetId, usize> = app
            .doc
            .datasets
            .iter()
            .enumerate()
            .map(|(i, dataset)| (dataset.resource_id(), i))
            .collect();
        let mut children = vec![Vec::new(); count];
        for (derived, dataset) in app.doc.datasets.iter().enumerate() {
            if let Some(lineage) = dataset.lineage() {
                for &source in &lineage.sources {
                    if let Some(source) = index_of.get(&source).copied()
                        && source != derived
                    {
                        children[source].push(derived);
                    }
                }
            }
        }

        let mut roots: Vec<usize> =
            app.doc
                .datasets
                .iter()
                .enumerate()
                .filter(|(di, dataset)| {
                    dataset.lineage().is_none_or(|lineage| {
                        lineage.sources.is_empty()
                            || lineage.sources.iter().all(|&source| {
                                index_of.get(&source).copied().is_none_or(|i| i == *di)
                            })
                    })
                })
                .map(|(di, _)| di)
                .collect();

        // Corrupt in-memory graphs can consist only of a cycle. Keep every
        // dataset reachable even then; recursion below cuts repeated path nodes.
        let mut reachable = HashSet::new();
        for &root in &roots {
            mark_reachable(root, &children, &mut reachable);
        }
        for di in 0..count {
            if !reachable.contains(&di) {
                roots.push(di);
                mark_reachable(di, &children, &mut reachable);
            }
        }

        Self {
            roots: roots
                .into_iter()
                .map(|di| build_node(app, di, &children, &mut Vec::new()))
                .collect(),
        }
    }

    pub fn filtered(&self, app: &PlotiumApp, query: &str) -> Self {
        let terms: Vec<_> = query.split_whitespace().map(str::to_lowercase).collect();
        let tag = app.session.ui.data_browser_tag.as_deref();
        let batch = app.session.ui.data_browser_batch;
        if terms.is_empty() && tag.is_none() && batch.is_none() {
            return self.clone();
        }
        let mut batch_members = HashSet::new();
        let mut paths = HashMap::new();
        for import in app.session.data_imports.batches() {
            for item in &import.items {
                if let Ok(target) = &item.result {
                    if Some(import.id) == batch {
                        batch_members.insert(target.dataset);
                    }
                    if !terms.is_empty() {
                        paths.insert(target.dataset, item.path.to_string_lossy());
                    }
                }
            }
        }
        let matches: Vec<_> = app
            .doc
            .datasets
            .iter()
            .map(|dataset| {
                let path = paths.get(&dataset.resource_id());
                let eligible = (batch.is_none() || batch_members.contains(&dataset.resource_id()))
                    && tag.is_none_or(|tag| dataset.organization().has_tag(tag));
                let query_match =
                    eligible && matches_dataset(dataset, &terms, path.map(|p| p.as_ref()));
                (eligible, query_match)
            })
            .collect();
        Self {
            roots: self
                .roots
                .iter()
                .filter_map(|node| filter_node(node, &matches, &terms))
                .collect(),
        }
    }

    /// Dataset rows in the exact order the current tree renders them. Linked
    /// references are one logical selectable item and therefore appear once.
    pub(super) fn visible_datasets(&self, app: &PlotiumApp, filtering: bool) -> Vec<usize> {
        fn visit(
            node: &DatasetNode,
            app: &PlotiumApp,
            filtering: bool,
            out: &mut Vec<usize>,
            seen: &mut HashSet<usize>,
        ) {
            if node.matches_filter && seen.insert(node.dataset) {
                out.push(node.dataset);
            }
            let dataset_open = filtering
                || !app
                    .session
                    .ui
                    .data_browser_collapsed_datasets
                    .contains(&node.dataset);
            let derived_open = filtering
                || !app
                    .session
                    .ui
                    .data_browser_collapsed_derived
                    .contains(&node.dataset);
            if dataset_open && derived_open && !node.cycle_cut {
                for child in &node.derived {
                    visit(child, app, filtering, out, seen);
                }
            }
        }

        let mut visible = Vec::new();
        let mut seen = HashSet::new();
        for root in &self.roots {
            visit(root, app, filtering, &mut visible, &mut seen);
        }
        visible
    }
}

fn mark_reachable(di: usize, children: &[Vec<usize>], seen: &mut HashSet<usize>) {
    if !seen.insert(di) {
        return;
    }
    for &child in &children[di] {
        mark_reachable(child, children, seen);
    }
}

fn build_node(
    app: &PlotiumApp,
    di: usize,
    children: &[Vec<usize>],
    path: &mut Vec<usize>,
) -> DatasetNode {
    if path.contains(&di) {
        return DatasetNode {
            dataset: di,
            linked_reference: is_linked(app, di),
            cycle_cut: true,
            matches_filter: true,
            analysis: Vec::new(),
            derived: Vec::new(),
        };
    }
    path.push(di);
    let derived = children[di]
        .iter()
        .copied()
        .map(|child| build_node(app, child, children, path))
        .collect();
    path.pop();
    DatasetNode {
        dataset: di,
        linked_reference: is_linked(app, di),
        cycle_cut: false,
        matches_filter: true,
        analysis: analysis_items(&app.doc.datasets[di]),
        derived,
    }
}

fn is_linked(app: &PlotiumApp, di: usize) -> bool {
    app.doc.datasets[di]
        .lineage()
        .is_some_and(|lineage| lineage.sources.len() > 1)
}

fn analysis_items(dataset: &Dataset) -> Vec<AnalysisItem> {
    let mut result = Vec::new();
    let time_nmr = dataset
        .as_nmr()
        .is_some_and(|nmr| nmr.output_domain() == plotium_io::Domain::Time);
    if !time_nmr && let Some(peaks) = dataset.peaks() {
        result.extend(peaks.marks.iter().map(|peak| {
            AnalysisItem {
                kind: AnalysisKind::Peak(peak.id),
                label: peak
                    .label
                    .clone()
                    .filter(|label| !label.trim().is_empty())
                    .unwrap_or_else(|| format!("Peak {:.3}", peak.x)),
            }
        }));
    }
    if !time_nmr && let Some(nmr) = dataset.as_nmr() {
        result.extend(nmr.integrals.iter().map(|integral| AnalysisItem {
            kind: AnalysisKind::Integral(integral.id),
            label: format!("Integral {:.3}–{:.3}", integral.start_ppm, integral.end_ppm),
        }));
    }
    if let Some(nmr2d) = dataset.as_nmr2d() {
        result.extend(nmr2d.integrals.iter().map(|integral| AnalysisItem {
            kind: AnalysisKind::Integral(integral.id),
            label: if integral.name.trim().is_empty() {
                format!("Integral #{}", integral.id + 1)
            } else {
                integral.name.clone()
            },
        }));
    }
    if let Some(state) = dataset.region_analysis() {
        let unit = dataset.region_axis_unit().unwrap_or("");
        result.extend(state.regions.iter().map(|region| AnalysisItem {
            kind: AnalysisKind::Region(region.id),
            label: region.column_name(unit),
        }));
    }
    if !time_nmr {
        result.extend(dataset.line_fits().iter().map(|fit| AnalysisItem {
            kind: AnalysisKind::LineFit(fit.id),
            label: format!("Peak fit #{} ({:.3}–{:.3})", fit.id + 1, fit.lo, fit.hi),
        }));
        result.extend(dataset.multiplets().iter().map(|multiplet| AnalysisItem {
            kind: AnalysisKind::Multiplet(multiplet.id),
            label: multiplet.descriptor(),
        }));
    }
    if let Some(nmr) = dataset.as_nmr() {
        result.extend(nmr.craft_runs.iter().map(|run| AnalysisItem {
            kind: AnalysisKind::Craft(run.id.0),
            label: format!(
                "CRAFT #{} ({} components)",
                run.id.0 + 1,
                run.components.len()
            ),
        }));
    }
    if let Some(table) = dataset.as_table() {
        result.extend(
            table
                .series_bindings
                .iter()
                .enumerate()
                .filter(|(_, binding)| binding.fit.is_some())
                .map(|(_, binding)| AnalysisItem {
                    kind: AnalysisKind::CurveFitResponse(binding.value_column),
                    label: format!(
                        "{} fit",
                        table
                            .typed_state
                            .envelope
                            .revision
                            .snapshot
                            .schema
                            .column(binding.value_column)
                            .map_or("Value", |value| value.name.as_str())
                    ),
                }),
        );
    }
    if let Some(mass_spec) = dataset.as_mass_spec() {
        result.extend(
            mass_spec
                .extracted_spectra
                .iter()
                .map(|extraction| AnalysisItem {
                    kind: AnalysisKind::MassSpectrum(extraction.id),
                    label: plotium_core::state::extraction_title(&mass_spec.run, extraction),
                }),
        );
    }
    result
}

pub(super) fn filtering(app: &PlotiumApp) -> bool {
    !app.session.ui.data_browser_filter.trim().is_empty()
        || app.session.ui.data_browser_tag.is_some()
        || app.session.ui.data_browser_batch.is_some()
}

fn matches_dataset(dataset: &Dataset, terms: &[String], import_path: Option<&str>) -> bool {
    if terms.is_empty() {
        return true;
    }
    let identity = dataset.acquisition_identity();
    let summary = dataset.summary();
    let fields = [
        dataset.name().as_deref().unwrap_or(""),
        dataset.compact_category(),
        dataset.kind_label(),
        summary.as_str(),
        dataset.dimension_label().unwrap_or(""),
        dataset.experiment_label().unwrap_or(""),
        dataset.source_identifier(),
        &identity.source_label,
        identity.subject.as_deref().unwrap_or(""),
        import_path.unwrap_or(""),
        dataset
            .lineage()
            .map_or("Original data", |l| l.kind.label()),
    ]
    .into_iter()
    .chain(dataset.organization().tags().iter().map(String::as_str))
    .chain(
        dataset
            .as_table()
            .into_iter()
            .flat_map(|table| &table.import_sources)
            .filter_map(|source| source.name.as_deref()),
    )
    .map(str::to_lowercase)
    .collect::<Vec<_>>();
    terms
        .iter()
        .all(|term| fields.iter().any(|field| field.contains(term)))
}

fn filter_node(
    node: &DatasetNode,
    matches: &[(bool, bool)],
    terms: &[String],
) -> Option<DatasetNode> {
    let (eligible, self_matches) = matches[node.dataset];
    let analysis: Vec<_> = node
        .analysis
        .iter()
        .filter(|item| {
            eligible
                && (self_matches || {
                    let text = format!("{} {}", item.label, item.kind.type_label()).to_lowercase();
                    terms.iter().all(|term| text.contains(term))
                })
        })
        .cloned()
        .collect();
    let derived: Vec<_> = node
        .derived
        .iter()
        .filter_map(|child| filter_node(child, matches, terms))
        .collect();
    (self_matches || !analysis.is_empty() || !derived.is_empty()).then_some(DatasetNode {
        dataset: node.dataset,
        linked_reference: node.linked_reference,
        cycle_cut: node.cycle_cut,
        matches_filter: eligible && (self_matches || !analysis.is_empty()),
        analysis,
        derived,
    })
}

pub(super) fn reveal_active_path(app: &mut PlotiumApp) {
    let active = app.active_dataset();
    if app.session.ui.data_browser_last_active == active {
        return;
    }
    app.session.ui.data_browser_last_active = active;
    let Some(active) = active else { return };
    let mut visiting = HashSet::new();
    reveal_sources(app, active, &mut visiting);
}

fn reveal_sources(app: &mut PlotiumApp, di: usize, visiting: &mut HashSet<usize>) {
    if !visiting.insert(di) {
        return;
    }
    app.session.ui.data_browser_collapsed_datasets.remove(&di);
    let sources = app.doc.datasets[di]
        .lineage()
        .map(|lineage| lineage.sources.clone())
        .unwrap_or_default();
    for source in sources {
        if let Some(source) = app.doc.dataset_index(source) {
            app.session
                .ui
                .data_browser_collapsed_datasets
                .remove(&source);
            app.session
                .ui
                .data_browser_collapsed_derived
                .remove(&source);
            reveal_sources(app, source, visiting);
        }
    }
    visiting.remove(&di);
}

pub(super) fn sources_tooltip(app: &PlotiumApp, di: usize) -> Option<String> {
    let lineage = app.doc.datasets.get(di)?.lineage()?;
    let names: Vec<_> = lineage
        .sources
        .iter()
        .filter_map(|&source| app.doc.dataset_index(source))
        .filter_map(|source| app.doc.datasets.get(source))
        .map(Dataset::display_name)
        .collect();
    Some(format!(
        "{}\nSources: {}",
        lineage.kind.label(),
        names.join(", ")
    ))
}

#[cfg(test)]
#[path = "data_browser_tests.rs"]
mod tests;
