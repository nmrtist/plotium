use super::*;

pub(super) fn read(data: &DataObject) -> Result<crate::state::FieldCatalog> {
    let value = data
        .extensions
        .get("plotium.fields")
        .cloned()
        .ok_or_else(|| {
            ProjectError::Invalid(format!(
                "dataset {} is missing its mandatory plotium.fields identity catalog",
                data.id
            ))
        })?;
    serde_json::from_value(value).map_err(|error| {
        ProjectError::Invalid(format!(
            "dataset {} has an invalid plotium.fields identity catalog: {error}",
            data.id
        ))
    })
}

pub(super) fn validate_series(
    dataset: &Dataset,
    field: crate::state::FieldId,
    encoding: &plotium_figure::SeriesEncoding,
    context: &str,
) -> Result<()> {
    if !dataset.has_field(field) {
        return Err(ProjectError::Invalid(format!(
            "{context} references missing field {field}"
        )));
    }
    if !dataset.supports_encoding(field, encoding) {
        return Err(ProjectError::Invalid(format!(
            "{context} uses an encoding not supported by field {field}"
        )));
    }
    if let plotium_figure::SeriesEncoding::Line(line) = encoding
        && line
            .appearance
            .marker_size
            .is_some_and(|s| !(0.1..=20.).contains(&s.get()))
    {
        return Err(ProjectError::Invalid(format!(
            "{context} marker diameter must be between 0.1 and 20 pt"
        )));
    }
    if let plotium_figure::SeriesEncoding::Line(line) = encoding
        && line
            .appearance
            .marker_edge_width
            .is_some_and(|w| w.get() > 5.)
    {
        return Err(ProjectError::Invalid(format!(
            "{context} marker edge width must be at most 5 pt"
        )));
    }
    if let plotium_figure::SeriesEncoding::Line(line) = encoding
        && !line.scale.is_finite()
    {
        return Err(ProjectError::Invalid(format!(
            "{context} uses a non-finite line scale"
        )));
    }
    Ok(())
}

pub(super) fn validate_series_source(
    dataset: &Dataset,
    field: crate::state::FieldId,
    item: Option<plotium_data::TraceItemId>,
    encoding: &plotium_figure::SeriesEncoding,
    context: &str,
) -> Result<()> {
    validate_series(dataset, field, encoding, context)?;
    match (item, dataset.trace_collection(field)) {
        (Some(item), Some(collection)) if collection.item(item).is_some() => Ok(()),
        (Some(item), _) => Err(ProjectError::Invalid(format!(
            "{context} references unknown trace item {item}"
        ))),
        (None, Some(_)) => Err(ProjectError::Invalid(format!(
            "{context} addresses a trace collection as a scalar field"
        ))),
        (None, None) => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_field_or_inapplicable_encoding_is_rejected_before_serialization() {
        let source = plotium_io::NmrData {
            points: vec![num_complex::Complex64::new(1.0, 0.0); 4],
            domain: plotium_io::Domain::Frequency,
            spectral_width_hz: 4_000.0,
            observe_freq_mhz: 400.0,
            carrier_ppm: 0.0,
            nucleus: "1H".to_owned(),
            source: "field validation".to_owned(),
            group_delay: 0.0,
        };
        let dataset = Dataset::Nmr(Box::new(crate::nmr_test_support::load_1d(source).unwrap()));
        let field = dataset.default_field_id().unwrap();
        let error = validate_series(
            &dataset,
            field,
            &plotium_figure::SeriesEncoding::Heatmap(plotium_figure::HeatmapSpec::default()),
            "test series",
        )
        .unwrap_err();
        assert!(error.to_string().contains("encoding not supported"));

        let error = validate_series(
            &dataset,
            crate::state::FieldId::new(field.get() + 1),
            &plotium_figure::SeriesEncoding::default(),
            "test series",
        )
        .unwrap_err();
        assert!(error.to_string().contains("missing field"));
    }
}
