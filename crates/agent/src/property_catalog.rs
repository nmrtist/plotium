use crate::{Failure, session::property_catalog};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeSet;

#[derive(Default, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Detail {
    #[default]
    Compact,
    Full,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Query {
    keys: Option<Vec<String>>,
    scope: Option<String>,
    #[serde(default)]
    detail: Detail,
}

pub(crate) fn schema() -> Value {
    let catalog = property_catalog();
    let rows = catalog["properties"].as_array().expect("catalog rows");
    json!({"keys":{"type":"array","minItems":1,"maxItems":crate::session::PROPERTIES.len(),"uniqueItems":true,"items":{"type":"string"}},
        "scope":{"type":"string","enum":scopes(rows)},"detail":{"type":"string","enum":["compact","full"],"default":"compact"}})
}

fn scopes(rows: &[Value]) -> BTreeSet<String> {
    rows.iter()
        .map(|row| row["scope"].as_str().expect("catalog scope").to_owned())
        .collect()
}

pub(crate) fn respond(arguments: Value) -> Result<Value, Failure> {
    for (field, valid) in [
        ("keys", arguments.get("keys").is_none_or(Value::is_array)),
        ("scope", arguments.get("scope").is_none_or(Value::is_string)),
    ] {
        if !valid {
            return Err(
                Failure::new("invalid_parameters", format!("Invalid {field} type")).at_field(field),
            );
        }
    }
    let query: Query = serde_json::from_value(arguments)
        .map_err(|e| Failure::new("invalid_parameters", e.to_string()))?;
    let catalog = property_catalog();
    let rows = catalog["properties"].as_array().expect("catalog rows");
    if let Some(scope) = &query.scope
        && !scopes(rows).contains(scope)
    {
        let mut e = Failure::new("invalid_parameters", "Unknown property scope").at_field("scope");
        e.details.as_mut().expect("field details")["scopes"] = json!(scopes(rows));
        return Err(e);
    }
    if let Some(keys) = &query.keys {
        if keys.is_empty()
            || keys.len() > rows.len()
            || keys.iter().collect::<BTreeSet<_>>().len() != keys.len()
        {
            return Err(Failure::new(
                "invalid_parameters",
                "keys must be nonempty, unique and within the supported property count",
            )
            .at_field("keys"));
        }
        for key in keys {
            let Some(row) = rows.iter().find(|row| row["id"] == *key) else {
                return Err(Failure::new(
                    "unsupported_property",
                    format!("Unsupported property {key}"),
                )
                .at_field("keys"));
            };
            if query
                .scope
                .as_ref()
                .is_some_and(|scope| row["scope"] != *scope)
            {
                return Err(Failure::new(
                    "invalid_parameters",
                    format!("Property {key} does not belong to the requested scope"),
                )
                .at_field("scope"));
            }
        }
    }
    let properties: Vec<Value> = rows
        .iter()
        .filter(|row| {
            query
                .keys
                .as_ref()
                .is_none_or(|keys| keys.iter().any(|key| row["id"] == *key))
        })
        .filter(|row| {
            query
                .scope
                .as_ref()
                .is_none_or(|scope| row["scope"] == *scope)
        })
        .map(|row| match query.detail {
            Detail::Full => row.clone(),
            Detail::Compact => compact(row),
        })
        .collect();
    Ok(
        json!({"detail":match query.detail {Detail::Compact=>"compact",Detail::Full=>"full"},"properties":properties}),
    )
}

fn compact(row: &Value) -> Value {
    let mut row = row.clone();
    let schema = row["schema"].as_object_mut().expect("property schema");
    schema.remove("drag_step");
    if schema.remove("input_schema").is_some() {
        schema.insert("requires_full".into(), json!(true));
    }
    if let Some(variants) = schema.get_mut("variants").and_then(Value::as_array_mut) {
        for variant in variants {
            *variant = variant["id"].clone();
        }
    }
    row
}

#[cfg(test)]
#[path = "property_catalog_tests.rs"]
mod tests;
