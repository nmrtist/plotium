use super::{
    AutomationError, CapabilityId, ComponentRef, DataPreview, DocumentRevision, FrozenTargetSet,
    ProjectBlueprint, ResourceDescriptor, ResourceKindId, ResourceQuery, ResourceRef,
    SelectionReason, TargetRef, TargetSelector,
};
use crate::state::{Dataset, PlotiumApp};
use std::collections::BTreeMap;

mod mass_spec;
mod science;
pub use science::diffusion_analysis_ref;
mod targets;
pub use targets::freeze_targets;
mod selection;
mod statistics;
mod xas;
mod xps;
mod xrd;

use statistics::add_statistics;

pub const KIND_DATASET: &str = "plotium.dataset";
pub const KIND_APP: &str = "plotium.app";
pub const KIND_DOCUMENT: &str = "plotium.document";
pub const KIND_CANVAS: &str = "plotium.canvas";
pub const KIND_TABLE_ROW: &str = "plotium.table.row";
pub const KIND_TABLE_COLUMN: &str = "plotium.table.column";
pub const KIND_FIELD: &str = "plotium.field";
pub const KIND_ANALYSIS: &str = "plotium.analysis";
pub const KIND_CANVAS_OBJECT: &str = "plotium.canvas.object";
pub const KIND_EXTERNAL_INPUT: &str = "plotium.external_input";
/// The one document root inside a `PlotiumApp` target space.
pub const DOCUMENT_RESOURCE_ID: &str = "document";
pub const APP_RESOURCE_ID: &str = "app";

pub const CAP_RENAME: &str = "resource.rename";
pub const CAP_RENDER: &str = "figure.render";
pub const CAP_THEME: &str = "figure.theme";
pub const CAP_EXPORT: &str = "figure.export";
pub const CAP_PREVIEW: &str = "data.preview";
pub const CAP_TRANSFORM: &str = "data.transform";
pub const CAP_DIFFUSION_FIT: &str = "analysis.diffusion.fit";
pub const CAP_DIFFUSION_RESULT: &str = "analysis.diffusion.result";
pub const CAP_DIFFUSION_INPUT: &str = "analysis.diffusion.input";
pub const CAP_PROCESSING_SCHEME: &str = "processing.scheme";
/// The resource holds components the property catalog can address. It is the
/// admission gate for the `properties.*` tools, and it is a capability rather
/// than a resource-kind test on purpose: any future resource that grows
/// addressable components gains those tools by exposing this, and no encoding
/// or property registry ever grows a branch naming a data domain or an object
/// type (§1 principle 3).
pub const CAP_PROPERTY_CATALOG: &str = "properties.catalog";
pub const CAP_FIELD_SCALAR_GRID_2D_REGULAR: &str = "field.scalar_grid_2d.regular";
pub const CAP_FIELD_CURVE_1D: &str = "field.curve_1d";
pub const CAP_FIELD_TRACE_COLLECTION: &str = "field.trace_collection";
pub const CAP_FIELD_COLORED_RASTER_2D: &str = "field.colored_raster_2d";
pub const CAP_FIELD_SIGNED: &str = "field.signed";
pub const CAP_FIELD_NOISE_SCALE: &str = "field.noise_scale";
pub const CAP_FIELD_LOCATION_SCALE: &str = "field.location_scale";
pub const CAP_FIELD_BOUNDED: &str = "field.bounded";
/// Provider semantics consumed only by legacy chart builders. They keep those
/// builders from masquerading as universally applicable curve/grid encodings.
pub const CAP_FIELD_TABLE: &str = "field.tabular";
pub const CAP_FIELD_NMR_SIGNAL: &str = "field.nmr.signal";
pub const CAP_FIELD_NMR_CONTOUR: &str = "field.nmr.contour";
pub const CAP_FIELD_NMR_STACK: &str = "field.nmr.stack";
pub const CAP_FIELD_SWEEP_COLLECTION: &str = "field.sweep_collection";
pub const CAP_FIELD_FORCE_CURVE: &str = "field.force_curve";
pub const CAP_FIELD_AFM_MAP: &str = "field.afm.map";
pub const CAP_FIELD_REGION_SERIES: &str = "field.region_series";
pub const CAP_FIELD_MASS_CHROMATOGRAM: &str = "field.mass_spectrometry.chromatogram";
pub const CAP_FIELD_MASS_SPECTRUM: &str = "field.mass_spectrometry.spectrum";
pub const CAP_FIELD_XRD_PATTERN: &str = "field.xrd.pattern";
pub const CAP_FIELD_XAS_SPECTRUM: &str = "field.xas.spectrum";
pub const CAP_FIELD_XPS_SPECTRUM: &str = "field.xps.spectrum";

/// Capability-oriented resource access. New resource types can participate by
/// implementing this trait; query and tool orchestration do not dispatch on a
/// Rust data-domain enum.
pub trait ResourceProvider {
    fn component_exists(&self, _target: &TargetRef) -> bool {
        false
    }
    fn revision(&self) -> DocumentRevision;
    fn blueprint(&self) -> ProjectBlueprint;
    fn descriptors(&self) -> Vec<ResourceDescriptor>;
    fn current_selection(&self) -> Vec<ResourceRef>;
    fn preview(&self, target: &ResourceRef, limit: usize) -> Result<DataPreview, AutomationError>;
    fn render_preview(&self, target: &ResourceRef) -> Result<String, AutomationError>;

    fn inspect(&self, id: &str) -> Option<ResourceDescriptor> {
        self.descriptors()
            .into_iter()
            .find(|descriptor| descriptor.resource.id == id)
    }
}

pub struct ProjectResourceProvider<'a> {
    app: &'a PlotiumApp,
}

impl<'a> ProjectResourceProvider<'a> {
    pub fn new(app: &'a PlotiumApp) -> Self {
        Self { app }
    }

    fn dataset_descriptor(&self, index: usize, dataset: &Dataset) -> ResourceDescriptor {
        let id = dataset.resource_id();
        // Only emit lineage sources that still exist: an undo can leave a
        // dangling DatasetId in the document (kept for redo), but a descriptor
        // naming a resource `resolve_target` would reject is inconsistent.
        let lineage = dataset
            .lineage()
            .into_iter()
            .flat_map(|lineage| lineage.sources.iter())
            .filter(|source| self.app.doc.dataset_index(**source).is_some())
            .map(ToString::to_string)
            .collect();
        let mut capabilities = vec![cap(CAP_RENAME), cap(CAP_PREVIEW)];
        if matches!(
            dataset,
            Dataset::Nmr(_) | Dataset::Nmr2D(_) | Dataset::Xrd(_) | Dataset::Xps(_)
        ) {
            capabilities.push(cap(CAP_PROCESSING_SCHEME));
        }
        // Asked of the catalog, not of the variant: the gate is "does this
        // resource hold components the catalog can address", and the catalog is
        // the only thing that knows. A resource kind that grows addressable
        // components later gains the tools without a branch being added here.
        if crate::properties::has_addressable_components(dataset) {
            capabilities.push(cap(CAP_PROPERTY_CATALOG));
        }
        for capability in [CAP_DIFFUSION_INPUT, CAP_DIFFUSION_FIT, CAP_DIFFUSION_RESULT] {
            if dataset
                .field_descriptors()
                .iter()
                .any(|field| field.capabilities.contains(capability))
            {
                capabilities.push(cap(capability));
            }
        }
        let mut metadata = BTreeMap::new();
        metadata.insert("domain".to_owned(), format!("{:?}", dataset.domain()));
        metadata.insert("index_hint".to_owned(), index.to_string());
        let (dimensions, units, mut children) = match dataset {
            Dataset::Table(table) => {
                capabilities.push(cap(CAP_TRANSFORM));
                let connection = super::table_update::connection_table_freshness(self.app, id);
                metadata.insert(
                    "table_mcp_refreshable".into(),
                    connection.mcp_refreshable.to_string(),
                );
                metadata.insert(
                    "table_mcp_refresh_reason".into(),
                    connection.mcp_refresh_reason,
                );
                let freshness = connection.native;
                metadata.insert(
                    "table_freshness".into(),
                    format!("{:?}", freshness.status).to_lowercase(),
                );
                metadata.insert(
                    "table_refreshable".into(),
                    freshness.refreshable.to_string(),
                );
                metadata.insert("table_freshness_reason".into(), freshness.reason);
                let snapshot = &table.typed_state.envelope.revision.snapshot;
                let columns = snapshot
                    .schema
                    .columns
                    .iter()
                    .map(|column| child_ref(id, &column.id.to_string(), KIND_TABLE_COLUMN));
                metadata.insert("table_id".into(), snapshot.table_id.to_string());
                metadata.insert(
                    "table_revision".into(),
                    table.typed_state.envelope.revision.id.to_string(),
                );
                (
                    vec![
                        usize::try_from(snapshot.row_count).unwrap_or(usize::MAX),
                        snapshot.schema.columns.len(),
                    ],
                    snapshot
                        .schema
                        .columns
                        .iter()
                        .filter_map(|column| {
                            column.unit.as_ref().map(|unit| unit.display_unit.clone())
                        })
                        .collect(),
                    columns.collect(),
                )
            }
            Dataset::Nmr(nmr) => (
                vec![nmr.processed.values().len()],
                vec![match nmr.output_domain() {
                    plotium_io::Domain::Time => "s".to_owned(),
                    plotium_io::Domain::Frequency => "ppm".to_owned(),
                }],
                Vec::new(),
            ),
            Dataset::Nmr2D(nmr) => (
                vec![nmr.data.rows, nmr.data.cols],
                vec!["ppm".to_owned(), "ppm".to_owned()],
                Vec::new(),
            ),
            Dataset::Electrophysiology(recording) => (
                vec![recording.data.sweeps.len(), recording.data.channels.len()],
                vec!["s".to_owned()],
                Vec::new(),
            ),
            Dataset::Afm(afm) => {
                let dimensions = afm.data.forces.as_ref().map_or_else(
                    || {
                        afm.data
                            .images
                            .first()
                            .map_or_else(Vec::new, |image| vec![image.height, image.width])
                    },
                    |forces| {
                        vec![
                            forces.grid_height,
                            forces.grid_width,
                            forces.samples_per_curve,
                        ]
                    },
                );
                let units = afm
                    .data
                    .images
                    .iter()
                    .map(|image| image.scale.unit.clone())
                    .collect();
                (dimensions, units, Vec::new())
            }
            Dataset::MassSpec(dataset) => mass_spec::descriptor(dataset),
            Dataset::Xrd(dataset) => (vec![dataset.data.len()], vec!["deg".to_owned()], Vec::new()),
            Dataset::Xps(dataset) => xps::descriptor(dataset),
            Dataset::Xas(dataset) => (vec![dataset.scan.len()], vec!["eV".to_owned()], Vec::new()),
        };
        children.extend(
            dataset
                .field_descriptors()
                .into_iter()
                .map(|field| child_ref(id, &field.local_id, KIND_FIELD)),
        );
        if let Some(table) = dataset.as_table() {
            children.extend(
                table
                    .curve_fit_analyses
                    .iter()
                    .filter(|a| a.result.model.id == plotium_analysis::models::DIFFUSION_B_ID)
                    .map(|a| diffusion_analysis_ref(table.resource_id, a.id)),
            );
        }
        ResourceDescriptor {
            resource: top_ref(id, KIND_DATASET),
            name: dataset.display_name(),
            capabilities,
            children,
            dimensions,
            units,
            metadata,
            lineage,
            revision: self.revision(),
        }
    }

    fn canvas_descriptor(&self, index: usize) -> ResourceDescriptor {
        let canvas = &self.app.doc.canvases[index];
        let children = canvas
            .objects
            .iter()
            .map(|object| {
                child_ref(
                    canvas.resource_id,
                    &object.id.to_string(),
                    KIND_CANVAS_OBJECT,
                )
            })
            .collect();
        let mut metadata = BTreeMap::new();
        metadata.insert("index_hint".to_owned(), index.to_string());
        ResourceDescriptor {
            resource: top_ref(canvas.resource_id, KIND_CANVAS),
            name: canvas.name.clone(),
            capabilities: vec![
                cap(CAP_RENAME),
                cap(CAP_RENDER),
                cap(CAP_THEME),
                cap(CAP_EXPORT),
                cap(CAP_PROPERTY_CATALOG),
            ],
            children,
            dimensions: vec![canvas.objects.len()],
            units: vec!["mm".to_owned()],
            metadata,
            lineage: canvas
                .dataset_ids()
                .into_iter()
                .filter(|dataset| self.app.doc.dataset_index(*dataset).is_some())
                .map(|dataset| dataset.to_string())
                .collect(),
            revision: self.revision(),
        }
    }
}

impl ResourceProvider for ProjectResourceProvider<'_> {
    fn component_exists(&self, target: &TargetRef) -> bool {
        let targets = match target.component {
            Some(ComponentRef::Series(_)) => self.app.resource_series_targets(&target.resource),
            Some(ComponentRef::ProcessingStep(_)) => {
                self.app.resource_processing_step_targets(&target.resource)
            }
            None => return false,
        };
        targets.contains(target)
    }

    fn revision(&self) -> DocumentRevision {
        DocumentRevision(self.app.doc.automation_revision)
    }

    fn blueprint(&self) -> ProjectBlueprint {
        let mut counts = BTreeMap::new();
        for descriptor in self.descriptors() {
            *counts.entry(descriptor.resource.kind.0).or_insert(0) += 1;
        }
        let relationships = self
            .app
            .doc
            .datasets
            .iter()
            .flat_map(|dataset| {
                let child = dataset.resource_id();
                dataset
                    .lineage()
                    .into_iter()
                    .flat_map(|lineage| lineage.sources.iter())
                    .filter(|source| self.app.doc.dataset_index(**source).is_some())
                    .map(move |source| format!("{child} derives_from {source}"))
            })
            .collect();
        ProjectBlueprint {
            revision: self.revision(),
            resource_counts: counts,
            relationships,
            warnings: Vec::new(),
        }
    }

    fn descriptors(&self) -> Vec<ResourceDescriptor> {
        let root = |id: &str, kind: &str, name: &str| ResourceDescriptor {
            resource: ResourceRef {
                id: id.to_owned(),
                kind: ResourceKindId::new(kind),
                parent_id: None,
                local_id: None,
            },
            name: name.to_owned(),
            capabilities: vec![cap(CAP_PROPERTY_CATALOG)],
            children: Vec::new(),
            dimensions: Vec::new(),
            units: Vec::new(),
            metadata: BTreeMap::new(),
            lineage: Vec::new(),
            revision: self.revision(),
        };
        let mut descriptors = vec![
            root(APP_RESOURCE_ID, KIND_APP, "Plotium application"),
            root(DOCUMENT_RESOURCE_ID, KIND_DOCUMENT, "Plotium document"),
        ];
        for (index, dataset) in self.app.doc.datasets.iter().enumerate() {
            let parent = self.dataset_descriptor(index, dataset);
            if let Dataset::Table(table) = dataset {
                descriptors.extend(science::analysis_descriptors(table, self.revision()));
                let snapshot = &table.typed_state.envelope.revision.snapshot;
                for column in &snapshot.schema.columns {
                    descriptors.push(ResourceDescriptor {
                        resource: child_ref(
                            dataset.resource_id(),
                            &column.id.to_string(),
                            KIND_TABLE_COLUMN,
                        ),
                        name: column.name.clone(),
                        capabilities: vec![cap(CAP_PREVIEW)],
                        children: Vec::new(),
                        dimensions: vec![usize::try_from(snapshot.row_count).unwrap_or(usize::MAX)],
                        units: column
                            .unit
                            .as_ref()
                            .map(|unit| vec![unit.display_unit.clone()])
                            .unwrap_or_default(),
                        metadata: BTreeMap::from([
                            ("logical_type".into(), format!("{:?}", column.logical_type)),
                            ("nullable".into(), column.nullable.to_string()),
                        ]),
                        lineage: Vec::new(),
                        revision: self.revision(),
                    });
                }
            }
            for field in dataset.field_descriptors() {
                descriptors.push(ResourceDescriptor {
                    resource: child_ref(dataset.resource_id(), &field.local_id, KIND_FIELD),
                    name: field.name,
                    capabilities: field.capabilities.iter().cloned().collect(),
                    children: Vec::new(),
                    dimensions: field.dimensions,
                    units: field.units,
                    metadata: field.metadata.0,
                    lineage: Vec::new(),
                    revision: self.revision(),
                });
            }
            descriptors.push(parent);
        }
        for index in 0..self.app.doc.canvases.len() {
            let parent = self.canvas_descriptor(index);
            let canvas = &self.app.doc.canvases[index];
            for object in &canvas.objects {
                let capabilities = vec![cap(CAP_RENAME), cap(CAP_PROPERTY_CATALOG)];
                descriptors.push(ResourceDescriptor {
                    resource: child_ref(
                        canvas.resource_id,
                        &object.id.to_string(),
                        KIND_CANVAS_OBJECT,
                    ),
                    name: object.name.clone(),
                    capabilities,
                    children: Vec::new(),
                    dimensions: Vec::new(),
                    units: Vec::new(),
                    metadata: BTreeMap::new(),
                    lineage: object
                        .dataset_ids()
                        .into_iter()
                        .filter(|dataset| self.app.doc.dataset_index(*dataset).is_some())
                        .map(|dataset| dataset.to_string())
                        .collect(),
                    revision: self.revision(),
                });
            }
            descriptors.push(parent);
        }
        descriptors
    }

    fn current_selection(&self) -> Vec<ResourceRef> {
        selection::current(self.app)
    }

    fn preview(&self, target: &ResourceRef, limit: usize) -> Result<DataPreview, AutomationError> {
        let descriptor = self.inspect(&target.id).ok_or_else(|| {
            AutomationError::InvalidSelector(format!("resource {} does not exist", target.id))
        })?;
        let Some(dataset) = self.app.doc.datasets.iter().find(|dataset| {
            dataset.resource_id().to_string() == target.id
                || target.parent_id.as_deref() == Some(dataset.resource_id().to_string().as_str())
        }) else {
            return Ok(DataPreview {
                target: target.clone(),
                shape: descriptor.dimensions,
                values: serde_json::Value::Null,
                returned: 0,
                total: 0,
                truncated: false,
                statistics: BTreeMap::new(),
            });
        };
        preview_dataset(dataset, target, limit)
    }

    fn render_preview(&self, target: &ResourceRef) -> Result<String, AutomationError> {
        let canvas_id = if target.kind.0 == KIND_CANVAS {
            target.id.as_str()
        } else {
            target.parent_id.as_deref().unwrap_or("")
        };
        let canvas = self
            .app
            .doc
            .canvases
            .iter()
            .find(|canvas| canvas.resource_id.to_string() == canvas_id)
            .ok_or_else(|| {
                AutomationError::InvalidSelector("render.preview requires a canvas".to_owned())
            })?;
        Ok(crate::state::render_document_svg(canvas))
    }
}

pub fn search_resources(
    provider: &impl ResourceProvider,
    query: &ResourceQuery,
) -> FrozenTargetSet {
    let limit = query.limit.clamp(1, 500);
    let mut matches = provider
        .descriptors()
        .into_iter()
        .filter(|descriptor| matches_query(descriptor, query))
        .collect::<Vec<_>>();
    matches.sort_by(|a, b| a.resource.id.cmp(&b.resource.id));
    let total_matches = matches.len();
    let selected = matches
        .into_iter()
        .skip(query.offset)
        .take(limit)
        .collect::<Vec<_>>();
    FrozenTargetSet {
        component_targets: Vec::new(),
        revision: provider.revision(),
        targets: selected
            .iter()
            .map(|descriptor| descriptor.resource.clone())
            .collect(),
        reasons: selected
            .iter()
            .map(|descriptor| SelectionReason {
                resource_id: descriptor.resource.id.clone(),
                reason: query_reason(query),
            })
            .collect(),
        total_matches,
        truncated: query.offset + selected.len() < total_matches,
    }
}

fn matches_query(descriptor: &ResourceDescriptor, query: &ResourceQuery) -> bool {
    (query.kinds.is_empty() || query.kinds.contains(&descriptor.resource.kind))
        && query
            .capabilities
            .iter()
            .all(|required| descriptor.capabilities.contains(required))
        && query.name_contains.as_ref().is_none_or(|needle| {
            descriptor
                .name
                .to_lowercase()
                .contains(&needle.to_lowercase())
        })
        && query
            .units
            .iter()
            .all(|unit| descriptor.units.contains(unit))
        && query
            .metadata
            .iter()
            .all(|(key, value)| descriptor.metadata.get(key) == Some(value))
        && query
            .lineage_source
            .as_ref()
            .is_none_or(|source| descriptor.lineage.contains(source))
}

fn query_reason(query: &ResourceQuery) -> String {
    let mut clauses = Vec::new();
    if !query.kinds.is_empty() {
        clauses.push("kind");
    }
    if !query.capabilities.is_empty() {
        clauses.push("capability");
    }
    if query.name_contains.is_some() {
        clauses.push("name");
    }
    if !query.units.is_empty() {
        clauses.push("unit");
    }
    if !query.metadata.is_empty() {
        clauses.push("metadata");
    }
    if query.lineage_source.is_some() {
        clauses.push("lineage");
    }
    if clauses.is_empty() {
        "matched all resources".to_owned()
    } else {
        format!("matched {} query", clauses.join(" + "))
    }
}

fn preview_dataset(
    dataset: &Dataset,
    target: &ResourceRef,
    limit: usize,
) -> Result<DataPreview, AutomationError> {
    let limit = limit.clamp(1, 10_000);
    let mut statistics = BTreeMap::new();
    let (shape, values, total) = match dataset {
        Dataset::Table(table) => {
            if target.kind.0 == KIND_TABLE_COLUMN {
                let column = target
                    .local_id
                    .as_deref()
                    .and_then(|id| id.parse::<plotium_data::ColumnId>().ok())
                    .ok_or_else(|| {
                        AutomationError::InvalidSelector("invalid table column id".to_owned())
                    })?;
                let preview = table
                    .typed_rows(limit, &[column])
                    .map_err(AutomationError::Execution)?;
                let values = &preview.columns[0].values;
                add_typed_statistics(&mut statistics, values);
                (
                    vec![usize::try_from(preview.total_rows).unwrap_or(usize::MAX)],
                    serde_json::Value::Array(values.iter().map(scalar_json).collect()),
                    usize::try_from(preview.total_rows).unwrap_or(usize::MAX),
                )
            } else {
                let preview = table
                    .typed_rows(limit, &[])
                    .map_err(AutomationError::Execution)?;
                let values = (0..preview.row_ids.len())
                    .map(|row| {
                        let mut record =
                            vec![serde_json::Value::String(preview.row_ids[row].to_string())];
                        record.extend(preview.columns.iter().map(|column| {
                            column
                                .values
                                .get(row)
                                .map(scalar_json)
                                .unwrap_or(serde_json::Value::Null)
                        }));
                        record
                    })
                    .collect::<Vec<_>>();
                (
                    vec![
                        usize::try_from(preview.total_rows).unwrap_or(usize::MAX),
                        preview.columns.len(),
                    ],
                    serde_json::json!(values),
                    usize::try_from(preview.total_rows).unwrap_or(usize::MAX),
                )
            }
        }
        Dataset::Nmr(nmr) => {
            let values = nmr
                .processed
                .values()
                .iter()
                .map(|value| value.re)
                .collect::<Vec<_>>();
            add_statistics(&mut statistics, &values);
            (
                vec![values.len()],
                serde_json::json!(finite_slice(&values, limit)),
                values.len(),
            )
        }
        Dataset::Nmr2D(nmr) => (
            vec![nmr.data.rows, nmr.data.cols],
            serde_json::json!({"summary": dataset.summary()}),
            nmr.data.rows.saturating_mul(nmr.data.cols),
        ),
        Dataset::Electrophysiology(recording) => (
            vec![recording.data.sweeps.len(), recording.data.channels.len()],
            serde_json::json!({"summary": dataset.summary()}),
            recording.data.sweeps.len(),
        ),
        Dataset::Afm(afm) => {
            let shape = afm.data.forces.as_ref().map_or_else(
                || {
                    afm.data
                        .images
                        .first()
                        .map_or_else(Vec::new, |image| vec![image.height, image.width])
                },
                |forces| {
                    vec![
                        forces.grid_height,
                        forces.grid_width,
                        forces.samples_per_curve,
                    ]
                },
            );
            let total = shape.iter().copied().fold(1usize, usize::saturating_mul);
            (
                shape,
                serde_json::json!({"summary": dataset.summary()}),
                total,
            )
        }
        Dataset::MassSpec(dataset) => mass_spec::preview(dataset, target, limit, &mut statistics),
        Dataset::Xrd(dataset) => xrd::preview(dataset, limit, &mut statistics),
        Dataset::Xas(dataset) => xas::preview(dataset, limit, &mut statistics),
        Dataset::Xps(dataset) => xps::preview(dataset, target, limit, &mut statistics)?,
    };
    let returned = total.min(limit);
    Ok(DataPreview {
        target: target.clone(),
        shape,
        values,
        returned,
        total,
        truncated: returned < total,
        statistics,
    })
}

fn add_typed_statistics(out: &mut BTreeMap<String, f64>, values: &[plotium_data::ScalarValue]) {
    let finite = values.iter().filter_map(|value| match value {
        plotium_data::ScalarValue::Int64(value) => Some(*value as f64),
        plotium_data::ScalarValue::Float64(value) if value.is_finite() => Some(*value),
        _ => None,
    });
    add_statistics(out, &finite.collect::<Vec<_>>());
}

fn scalar_json(value: &plotium_data::ScalarValue) -> serde_json::Value {
    use plotium_data::ScalarValue;
    match value {
        ScalarValue::Null => serde_json::Value::Null,
        ScalarValue::Boolean(value) => serde_json::json!(value),
        ScalarValue::Int64(value) => serde_json::json!(value),
        ScalarValue::Float64(value) if value.is_nan() => serde_json::json!({"float": "NaN"}),
        ScalarValue::Float64(value) if *value == f64::INFINITY => {
            serde_json::json!({"float": "+Inf"})
        }
        ScalarValue::Float64(value) if *value == f64::NEG_INFINITY => {
            serde_json::json!({"float": "-Inf"})
        }
        ScalarValue::Float64(value) => serde_json::json!(value),
        ScalarValue::Utf8(value) => serde_json::json!(value),
        ScalarValue::Categorical(value) => serde_json::json!({"category_index": value}),
        ScalarValue::Date(value) => serde_json::json!({"date_days": value}),
        ScalarValue::Time(value) => serde_json::json!({"time_ns": value}),
        ScalarValue::Timestamp(value) => serde_json::json!({"timestamp_ns_utc": value}),
        ScalarValue::Duration(value) => serde_json::json!({"duration_ns": value}),
        ScalarValue::Extension { type_id, storage } => {
            serde_json::json!({"extension": type_id, "storage": scalar_json(storage)})
        }
    }
}

fn finite_slice(values: &[f64], limit: usize) -> Vec<serde_json::Value> {
    values
        .iter()
        .take(limit)
        .copied()
        .map(finite_json)
        .collect()
}

fn finite_json(value: f64) -> serde_json::Value {
    if value.is_finite() {
        serde_json::json!(value)
    } else {
        serde_json::Value::Null
    }
}

fn top_ref(id: impl ToString, kind: &str) -> ResourceRef {
    ResourceRef {
        id: id.to_string(),
        kind: ResourceKindId::new(kind),
        parent_id: None,
        local_id: None,
    }
}

pub(super) fn child_ref(parent: impl ToString, local: &str, kind: &str) -> ResourceRef {
    let parent = parent.to_string();
    ResourceRef {
        id: format!("{parent}/{local}"),
        kind: ResourceKindId::new(kind),
        parent_id: Some(parent),
        local_id: Some(local.to_owned()),
    }
}

/// The child reference that addresses one plot object of a page. Property
/// targets and descriptors must agree on this shape, so both build it here.
pub fn canvas_object_ref(
    canvas: crate::state::CanvasId,
    object: crate::state::ObjectId,
) -> ResourceRef {
    child_ref(canvas, &object.to_string(), KIND_CANVAS_OBJECT)
}

fn cap(id: &str) -> CapabilityId {
    CapabilityId::new(id)
}
