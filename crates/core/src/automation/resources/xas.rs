use super::add_statistics;
use crate::state::XasDataset;
use std::collections::BTreeMap;

pub(super) fn preview(
    dataset: &XasDataset,
    limit: usize,
    statistics: &mut BTreeMap<String, f64>,
) -> (Vec<usize>, serde_json::Value, usize) {
    let values = dataset.default_values();
    add_statistics(statistics, values);
    let preview = values
        .iter()
        .take(limit)
        .map(|value| {
            if value.is_finite() {
                serde_json::json!(value)
            } else {
                serde_json::Value::Null
            }
        })
        .collect::<Vec<_>>();
    (
        vec![values.len()],
        serde_json::Value::Array(preview),
        values.len(),
    )
}
