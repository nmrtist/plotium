use super::convert::{DatasetBlob, DatasetObjects};
use super::xrd_convert::{Reader, validate_array_len, write_f64s, write_len};
use super::{
    Classification, DataObject, Dataset, EntryReader, Payload, ProjectError, ProjectLoadLimits,
    RecipeObject, RecipeParameters, Result, STORAGE_XAS_V1, read_entry,
};
use crate::state::{XasDataset, XasSignal};
use plotium_io::xas::XasScan;
use plotium_processing::xas::XasProcessing;
use std::fs::File;
use std::io::{Read, Write};

const MAGIC: &[u8; 8] = b"PXXAS1\0\0";
const EXTENSION: &str = "plotium.xas";
const TECHNIQUE: &str = "xas";

#[derive(serde::Serialize, serde::Deserialize)]
struct Recipe {
    signal: Option<XasSignal>,
    processing: XasProcessing,
}

pub(super) fn to_objects<'a>(
    xas: &'a XasDataset,
    data_id: &str,
    recipe_id: &str,
) -> DatasetObjects<'a> {
    let classification = |object: &str| Classification {
        domain: "spectroscopy".to_owned(),
        technique: Some(TECHNIQUE.to_owned()),
        object: object.to_owned(),
    };
    let data = DataObject {
        id: data_id.to_owned(),
        role: "data".to_owned(),
        classification: classification("scan"),
        label: xas.name.clone(),
        dimensions: Vec::new(),
        payload: Payload {
            storage: STORAGE_XAS_V1.to_owned(),
            blob: format!("objects/{data_id}/data.bin"),
            shape: vec![xas.scan.columns.len(), xas.scan.len()],
            domain: "energy".to_owned(),
        },
        extensions: serde_json::json!({ "plotium.fields": &xas.field_catalog }),
    };
    let recipe = RecipeObject {
        id: recipe_id.to_owned(),
        role: "recipe".to_owned(),
        classification: classification("processing_recipe"),
        input: data_id.to_owned(),
        parameters: RecipeParameters::default(),
        extensions: serde_json::json!({ EXTENSION: Recipe {
            signal: xas.signal,
            processing: xas.params,
        }}),
    };
    DatasetObjects::primary(data, DatasetBlob::Xas(&xas.scan), recipe)
}

pub(super) fn matches(data: &DataObject) -> bool {
    data.classification.domain == "spectroscopy"
        && data.classification.technique.as_deref() == Some(TECHNIQUE)
}

pub(super) fn from_objects(
    zip: &mut zip::ZipArchive<File>,
    data: &DataObject,
    recipe: &RecipeObject,
) -> Result<Dataset> {
    if data.payload.storage != STORAGE_XAS_V1 {
        return Err(ProjectError::Unsupported(format!(
            "XAS payload storage {}",
            data.payload.storage
        )));
    }
    // Named generic decoder functions do not satisfy the higher-ranked
    // lifetime required by `ZipFile`; the closure reborrows each entry.
    #[allow(clippy::redundant_closure)]
    let scan = read_entry(
        zip,
        &data.payload.blob,
        "XAS payload",
        ProjectLoadLimits::default().max_entry_bytes,
        |reader| decode(reader),
    )?;
    if data.payload.shape.as_slice() != [scan.columns.len(), scan.len()] {
        return Err(ProjectError::Invalid(format!(
            "XAS payload shape {:?} does not match the decoded scan",
            data.payload.shape,
        )));
    }
    let mut dataset = XasDataset::load(scan);
    dataset.acquisition_identity = super::convert::read_acquisition_identity(data)?;
    dataset.field_catalog = super::convert::read_field_catalog(data)?;
    dataset.name = data.label.clone();
    let stored: Recipe = recipe
        .extensions
        .get(EXTENSION)
        .cloned()
        .map(serde_json::from_value)
        .transpose()?
        .ok_or_else(|| ProjectError::Invalid("XAS recipe is missing".to_owned()))?;
    dataset.apply_processing(stored.signal, stored.processing);
    let dataset = Dataset::Xas(Box::new(dataset));
    dataset
        .validate_field_catalog()
        .map_err(ProjectError::Invalid)?;
    Ok(dataset)
}

pub(super) fn write(output: &mut impl Write, scan: &XasScan) -> Result<()> {
    scan.validate()
        .map_err(|error| ProjectError::Invalid(error.to_owned()))?;
    let limits = ProjectLoadLimits::default();
    validate_array_len(scan.len(), limits)?;
    let mut metadata = scan.clone();
    metadata
        .columns
        .iter_mut()
        .for_each(|column| column.values.clear());
    let json = serde_json::to_vec(&metadata)?;
    if json.len() as u64 > limits.max_metadata_bytes {
        return Err(ProjectError::Invalid(
            "XAS metadata exceeds the configured limit".to_owned(),
        ));
    }
    output.write_all(MAGIC)?;
    write_len(output, json.len())?;
    output.write_all(&json)?;
    for column in &scan.columns {
        write_f64s(output, &column.values)?;
    }
    Ok(())
}

pub(super) fn decode<R: Read>(input: &mut EntryReader<'_, R>) -> Result<XasScan> {
    let mut reader = Reader::new(input);
    if reader.read_array::<8>()? != *MAGIC {
        return Err(ProjectError::Invalid(
            "XAS payload has an invalid signature".to_owned(),
        ));
    }
    let metadata_len = reader.read_len("metadata length")?;
    if metadata_len > ProjectLoadLimits::default().max_metadata_bytes as usize {
        return Err(ProjectError::Invalid(
            "XAS metadata exceeds the configured limit".to_owned(),
        ));
    }
    let metadata = reader.read_bytes(metadata_len, "XAS metadata")?;
    let mut scan: XasScan = serde_json::from_slice(&metadata)?;
    for column in &mut scan.columns {
        column.values = reader.read_f64s(None, "XAS column")?;
    }
    scan.validate()
        .map_err(|error| ProjectError::Invalid(error.to_owned()))?;
    Ok(scan)
}

#[cfg(test)]
#[path = "xas_convert_tests.rs"]
mod tests;
