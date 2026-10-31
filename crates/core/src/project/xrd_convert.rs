use super::{EntryReader, ProjectError, ProjectLoadLimits, Result};
use plotium_io::XrdData;
use std::io::{Read, Write};

const MAGIC: &[u8; 8] = b"PXXRD1\0\0";
const VALUES_PER_CHUNK: usize = 4096;

pub(super) fn write(output: &mut impl Write, data: &XrdData) -> Result<()> {
    data.validate()
        .map_err(|error| ProjectError::Invalid(error.to_owned()))?;
    let limits = ProjectLoadLimits::default();
    validate_array_len(data.len(), limits)?;
    validate_materialized_size(data.len(), data.attenuation.is_some(), limits)?;

    let mut metadata = data.clone();
    metadata.two_theta_deg.clear();
    metadata.intensity.clear();
    if let Some(attenuation) = &mut metadata.attenuation {
        attenuation.clear();
    }
    let json = serde_json::to_vec(&metadata)?;
    if json.len() as u64 > limits.max_metadata_bytes {
        return Err(ProjectError::Invalid(
            "XRD metadata exceeds the configured limit".to_owned(),
        ));
    }

    let array_count = 2 + usize::from(data.attenuation.is_some());
    let arrays_bytes = data
        .len()
        .checked_mul(std::mem::size_of::<f64>())
        .and_then(|bytes| bytes.checked_mul(array_count))
        .ok_or_else(|| ProjectError::Invalid("XRD payload size overflows usize".to_owned()))?;
    let encoded_bytes = MAGIC
        .len()
        .checked_add(8)
        .and_then(|bytes| bytes.checked_add(json.len()))
        .and_then(|bytes| bytes.checked_add(array_count * 8))
        .and_then(|bytes| bytes.checked_add(arrays_bytes))
        .ok_or_else(|| ProjectError::Invalid("XRD payload size overflows usize".to_owned()))?;
    if encoded_bytes as u64 > limits.max_entry_bytes {
        return Err(ProjectError::Invalid(
            "XRD payload exceeds the configured project-entry limit".to_owned(),
        ));
    }

    output.write_all(MAGIC)?;
    write_len(output, json.len())?;
    output.write_all(&json)?;
    write_f64s(output, &data.two_theta_deg)?;
    write_f64s(output, &data.intensity)?;
    if let Some(attenuation) = &data.attenuation {
        write_f64s(output, attenuation)?;
    }
    Ok(())
}

pub(super) fn decode<R: Read>(input: &mut EntryReader<'_, R>) -> Result<XrdData> {
    let mut reader = Reader::new(input);
    if reader.read_array::<8>()? != *MAGIC {
        return Err(ProjectError::Invalid(
            "XRD payload has an invalid signature".to_owned(),
        ));
    }

    let metadata_len = reader.read_len("metadata length")?;
    if metadata_len > ProjectLoadLimits::default().max_metadata_bytes as usize {
        return Err(reader
            .input
            .invalid("XRD metadata exceeds the configured limit"));
    }
    let metadata = reader.read_bytes(metadata_len, "XRD metadata")?;
    let mut data: XrdData = serde_json::from_slice(&metadata)?;
    if !data.two_theta_deg.is_empty()
        || !data.intensity.is_empty()
        || data
            .attenuation
            .as_ref()
            .is_some_and(|values| !values.is_empty())
    {
        return Err(reader
            .input
            .invalid("XRD metadata contains inline scientific arrays"));
    }

    let points = reader.read_len("2theta length")?;
    validate_array_len(points, ProjectLoadLimits::default())?;
    validate_materialized_size(
        points,
        data.attenuation.is_some(),
        ProjectLoadLimits::default(),
    )?;
    data.two_theta_deg = reader.read_f64_values(points, "2theta")?;
    data.intensity = reader.read_f64s(Some(points), "intensity")?;
    if data.attenuation.is_some() {
        data.attenuation = Some(reader.read_f64s(Some(points), "attenuation")?);
    }
    Ok(data)
}

pub(super) fn validate_array_len(len: usize, limits: ProjectLoadLimits) -> Result<()> {
    if len > limits.max_collection_items {
        return Err(ProjectError::Invalid(format!(
            "XRD array contains {len} values, exceeding the {}-item limit",
            limits.max_collection_items
        )));
    }
    Ok(())
}

fn validate_materialized_size(
    len: usize,
    has_attenuation: bool,
    limits: ProjectLoadLimits,
) -> Result<()> {
    let array_count = 2 + usize::from(has_attenuation);
    let bytes = len
        .checked_mul(std::mem::size_of::<f64>())
        .and_then(|bytes| bytes.checked_mul(array_count))
        .ok_or_else(|| ProjectError::Invalid("XRD materialized size overflows usize".to_owned()))?;
    if bytes as u64 > limits.max_materialized_bytes {
        return Err(ProjectError::Invalid(format!(
            "XRD arrays require {bytes} bytes, exceeding the {}-byte materialized-data limit",
            limits.max_materialized_bytes
        )));
    }
    Ok(())
}

pub(super) fn write_len(output: &mut impl Write, len: usize) -> Result<()> {
    let len = u64::try_from(len)
        .map_err(|_| ProjectError::Invalid("XRD length exceeds u64".to_owned()))?;
    output.write_all(&len.to_le_bytes())?;
    Ok(())
}

pub(super) fn write_f64s(output: &mut impl Write, values: &[f64]) -> Result<()> {
    write_len(output, values.len())?;
    let mut buffer = Vec::with_capacity(VALUES_PER_CHUNK * std::mem::size_of::<f64>());
    for chunk in values.chunks(VALUES_PER_CHUNK) {
        buffer.clear();
        for value in chunk {
            buffer.extend_from_slice(&value.to_le_bytes());
        }
        output.write_all(&buffer)?;
    }
    Ok(())
}

pub(super) struct Reader<'a, 'p, R: Read> {
    input: &'a mut EntryReader<'p, R>,
}

impl<'a, 'p, R: Read> Reader<'a, 'p, R> {
    pub(super) fn new(input: &'a mut EntryReader<'p, R>) -> Self {
        Self { input }
    }

    pub(super) fn read_array<const N: usize>(&mut self) -> Result<[u8; N]> {
        self.input.require_bytes(N, "binary field")?;
        let mut bytes = [0_u8; N];
        self.input.read_exact(&mut bytes).map_err(|error| {
            self.input
                .invalid(format!("Binary payload is truncated: {error}"))
        })?;
        Ok(bytes)
    }

    pub(super) fn read_len(&mut self, label: &str) -> Result<usize> {
        usize::try_from(u64::from_le_bytes(self.read_array()?))
            .map_err(|_| self.input.invalid(format!("{label} exceeds usize")))
    }

    pub(super) fn read_bytes(&mut self, len: usize, label: &str) -> Result<Vec<u8>> {
        self.input.require_bytes(len, label)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(len)
            .map_err(|_| self.input.invalid(format!("could not reserve {label}")))?;
        bytes.resize(len, 0);
        self.input.read_exact(&mut bytes).map_err(|error| {
            self.input
                .invalid(format!("Binary payload is truncated: {error}"))
        })?;
        Ok(bytes)
    }

    pub(super) fn read_f64s(&mut self, expected: Option<usize>, label: &str) -> Result<Vec<f64>> {
        let len = self.read_len(label)?;
        validate_array_len(len, ProjectLoadLimits::default())?;
        if let Some(expected) = expected
            && expected != len
        {
            return Err(self.input.invalid(format!(
                "{label} length {len} does not match expected length {expected}"
            )));
        }
        self.read_f64_values(len, label)
    }

    fn read_f64_values(&mut self, len: usize, label: &str) -> Result<Vec<f64>> {
        let bytes = len
            .checked_mul(std::mem::size_of::<f64>())
            .ok_or_else(|| self.input.invalid(format!("{label} size overflows usize")))?;
        self.input.require_bytes(bytes, label)?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(len)
            .map_err(|_| self.input.invalid(format!("could not reserve {label}")))?;
        for _ in 0..len {
            values.push(f64::from_le_bytes(self.read_array()?));
        }
        Ok(values)
    }
}

#[cfg(test)]
#[path = "xrd_convert_tests.rs"]
mod tests;
