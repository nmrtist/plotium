//! Short caller references, expanded to the exact identities the core plans
//! and audits with. Expansion is purely lexical for targets; table sources
//! resolve column names against the current table and report what they chose.
use super::*;
use plotium_core::state::{Dataset, PlotiumApp};
use serde_json::Map;

/// Expand short references in place and return the table sources that were
/// resolved from column names, in request order.
pub(crate) fn expand(app: &PlotiumApp, arguments: &mut Value) -> Result<Vec<Value>, Failure> {
    let mut resolved = Vec::new();
    walk(app, arguments, "", &mut resolved)?;
    Ok(resolved)
}

fn walk(
    app: &PlotiumApp,
    value: &mut Value,
    path: &str,
    resolved: &mut Vec<Value>,
) -> Result<(), Failure> {
    if let Some(target) = value.as_object().and_then(series_target) {
        *value = target;
        return Ok(());
    }
    match value {
        Value::Object(fields) => {
            for (key, child) in fields.iter_mut() {
                let path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                if key == "resource" {
                    complete_resource(child);
                } else if key == "source" && is_table_source(child) {
                    if resolve_source(app, child, &path)? {
                        resolved.push(child.clone());
                    }
                    continue;
                }
                walk(app, child, &path, resolved)?;
            }
        }
        Value::Array(items) => {
            for (index, item) in items.iter_mut().enumerate() {
                walk(app, item, &format!("{path}[{index}]"), resolved)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// `{"plot":"CANVAS/OBJECT","series":N}` names one Series of a plot.
fn series_target(fields: &Map<String, Value>) -> Option<Value> {
    if fields.len() != 2 {
        return None;
    }
    let plot = fields.get("plot")?.as_str().filter(|id| id.contains('/'))?;
    let series = fields.get("series")?.as_u64()?;
    let mut resource = json!({"id":plot,"kind":KIND_CANVAS_OBJECT});
    complete_resource(&mut resource);
    Some(json!({"resource":resource,"component":{"kind":"series","id":series}}))
}

/// Nested resource ids are `PARENT/.../LOCAL`; fill the parts a caller omitted.
fn complete_resource(resource: &mut Value) {
    let Some(fields) = resource.as_object_mut() else {
        return;
    };
    let Some(id) = fields.get("id").and_then(Value::as_str).map(str::to_owned) else {
        return;
    };
    if fields.contains_key("parent_id") || fields.contains_key("local_id") {
        return;
    }
    if let (Some((parent, _)), Some((_, local))) = (id.split_once('/'), id.rsplit_once('/')) {
        fields.insert("parent_id".into(), json!(parent));
        fields.insert("local_id".into(), json!(local));
    }
}

fn is_table_source(value: &Value) -> bool {
    value.get("dataset_id").is_some()
        && ["x", "y", "y_error", "x_column_id", "y_column_id"]
            .iter()
            .any(|key| value.get(key).is_some())
}

/// Complete a table curve source. Returns whether it named its columns, so
/// the plan can show what they resolved to.
fn resolve_source(app: &PlotiumApp, source: &mut Value, path: &str) -> Result<bool, Failure> {
    let fields = source.as_object_mut().expect("checked object");
    let short = ["x", "y", "y_error"]
        .iter()
        .any(|k| fields.contains_key(*k));
    if !short {
        // The full form may omit a missing error column.
        fields.entry("y_error_column_id").or_insert(Value::Null);
        return Ok(false);
    }
    for key in ["x_column_id", "y_column_id", "y_error_column_id"] {
        if fields.contains_key(key) {
            return Err(invalid(
                format!("{path}.{key}"),
                "Use either x/y/y_error or x_column_id/y_column_id/y_error_column_id, not both",
            ));
        }
    }
    let dataset = fields["dataset_id"]
        .as_str()
        .and_then(|id| serde_json::from_value(json!(id)).ok())
        .and_then(|id| app.doc.dataset_by_id(id));
    let Some(Dataset::Table(table)) = dataset else {
        return Err(invalid(
            format!("{path}.dataset_id"),
            "dataset_id must name a table dataset in this project",
        ));
    };
    let revision = &table.typed_state.envelope.revision;
    let columns = &revision.snapshot.schema.columns;
    let field = dataset
        .and_then(Dataset::default_field_id)
        .ok_or_else(|| invalid(format!("{path}.dataset_id"), "The table has no curve field"))?;
    fields.entry("field_id").or_insert(json!(field));
    fields.entry("table_id").or_insert(json!(revision.table_id));
    fields.entry("table_revision").or_insert(json!(revision.id));
    for (short, full) in [
        ("x", "x_column_id"),
        ("y", "y_column_id"),
        ("y_error", "y_error_column_id"),
    ] {
        let id = match fields.remove(short) {
            None if short == "y_error" => Value::Null,
            None => {
                return Err(invalid(format!("{path}.{short}"), "Missing column"));
            }
            Some(Value::Null) if short == "y_error" => Value::Null,
            Some(Value::String(wanted)) => {
                let matches: Vec<_> = columns
                    .iter()
                    .filter(|c| c.name == wanted || json!(c.id) == json!(wanted))
                    .collect();
                match matches.as_slice() {
                    [column] => json!(column.id),
                    found => {
                        let message = if found.is_empty() {
                            format!("No column is named or identified by \"{wanted}\"")
                        } else {
                            format!("Several columns are named \"{wanted}\"; use a ColumnId")
                        };
                        let mut error = invalid(format!("{path}.{short}"), message);
                        error.details.as_mut().expect("field details")["candidates"] = columns
                            .iter()
                            .map(|c| json!({"name":c.name,"column_id":c.id}))
                            .collect();
                        return Err(error);
                    }
                }
            }
            Some(_) => {
                return Err(invalid(
                    format!("{path}.{short}"),
                    "A column is a name or ColumnId string",
                ));
            }
        };
        fields.insert(full.into(), id);
    }
    Ok(true)
}

/// Plans show the sources chosen from column names so the caller can check them.
pub(super) fn annotate(name: &str, resolved: Vec<Value>, mut value: Value) -> Value {
    if name == "plotium_plan" && !resolved.is_empty() {
        value["resolved_sources"] = json!(resolved);
    }
    value
}

/// `{plot, series}`: one Series of a plot, by the plot's resource id.
pub(crate) fn series_schema() -> Value {
    json!({"type":"object","additionalProperties":false,"required":["plot","series"],
        "description":"Short Series target: the plot resource id CANVAS/OBJECT and the Series id.",
        "properties":{"plot":{"type":"string"},"series":{"type":"integer","minimum":0}}})
}

/// The served schema of `id`: the core schema with its short forms advertised.
#[cfg(test)]
pub(crate) fn widened(id: &str, schema: Value) -> Value {
    let mut constraints = Map::new();
    constraints.insert(id.into(), json!({ "schema": schema }));
    widen(&mut constraints);
    constraints[id]["schema"].clone()
}

/// Advertise the short forms on the operation schemas that accept them.
pub(crate) fn widen(constraints: &mut Map<String, Value>) {
    let either = |full: &Value, short: Value| json!({"anyOf":[full, short]});
    for (id, sources, targets) in [
        (
            PLOT_CREATE,
            Some("/properties/series/items/properties/source"),
            None,
        ),
        (
            "plot.series.append",
            Some("/properties/series/items/properties/source"),
            None,
        ),
        (
            "plot.series.bind",
            Some("/properties/edits/items/properties/source"),
            Some("/properties/edits/items/properties/target"),
        ),
        (
            "plot.series.materialize_default",
            None,
            Some("/properties/edits/items/properties/target"),
        ),
        (
            "properties.set_many",
            None,
            Some("/properties/writes/items/properties/target"),
        ),
    ] {
        let Some(schema) = constraints.get_mut(id).map(|c| &mut c["schema"]) else {
            continue;
        };
        if let Some(path) = sources {
            let full = schema.pointer_mut(path).expect("table source schema");
            let mut optional_error = full.clone();
            if let Some(required) = optional_error["required"].as_array_mut() {
                required.retain(|r| r != "y_error_column_id");
            }
            let column = json!({"type":"string","description":"Column name (exact, case-sensitive) or ColumnId"});
            let uuid = json!({"type":"string","format":"uuid"});
            let short = json!({"type":"object","additionalProperties":false,"required":["dataset_id","x","y"],
                "description":"Columns by name; field, table and current revision are filled in and reported in resolved_sources.",
                "properties":{"dataset_id":uuid,"x":column,"y":column,"y_error":{"type":["string","null"]},
                    "field_id":{"type":"integer","minimum":0},"table_id":uuid,"table_revision":uuid}});
            *full = either(&optional_error, short);
        }
        if let Some(path) = targets {
            let full = schema.pointer_mut(path).expect("series target schema");
            let widened = either(full, series_schema());
            *full = widened;
        }
    }
}

fn invalid(field: String, message: impl Into<String>) -> Failure {
    Failure::new("invalid_parameters", message).at_field(&field)
}

#[cfg(test)]
#[path = "short_refs_tests.rs"]
mod tests;
