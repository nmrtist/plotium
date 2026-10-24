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
    prefix: Option<String>,
    #[serde(default)]
    detail: Detail,
}

pub(crate) fn schema() -> Value {
    let catalog = property_catalog();
    let rows = catalog["properties"].as_array().expect("catalog rows");
    json!({"keys":{"type":"array","minItems":1,"maxItems":crate::session::PROPERTIES.len(),"uniqueItems":true,"items":{"type":"string"}},
        "scope":{"type":"string","enum":scopes(rows)},"prefix":{"type":"string","minLength":1,"description":"Property id prefix, such as series.line or object.axes"},"detail":{"type":"string","enum":["compact","full"],"default":"compact"}})
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
        (
            "prefix",
            arguments.get("prefix").is_none_or(Value::is_string),
        ),
    ] {
        if !valid {
            return Err(
                Failure::new("invalid_parameters", format!("Invalid {field} type")).at_field(field),
            );
        }
    }
    let query: Query = serde_json::from_value(arguments)
        .map_err(|e| Failure::new("invalid_parameters", e.to_string()))?;
    if query.prefix.as_deref() == Some("") {
        return Err(
            Failure::new("invalid_parameters", "prefix must not be empty").at_field("prefix"),
        );
    }
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
        .filter(|row| {
            query.prefix.as_ref().is_none_or(|prefix| {
                row["id"]
                    .as_str()
                    .is_some_and(|id| id.starts_with(prefix.as_str()))
            })
        })
        .map(|row| match query.detail {
            Detail::Full => row.clone(),
            Detail::Compact => compact(row),
        })
        .collect();
    if properties.is_empty() && query.prefix.is_some() {
        let mut e = Failure::new("invalid_parameters", "No property id starts with prefix")
            .at_field("prefix");
        e.details.as_mut().expect("field details")["prefixes"] = json!(groups(rows));
        return Err(e);
    }
    Ok(
        json!({"detail":match query.detail {Detail::Compact=>"compact",Detail::Full=>"full"},"properties":properties}),
    )
}

/// Property id prefixes up to their second segment, such as `series.line`.
fn groups(rows: &[Value]) -> BTreeSet<String> {
    rows.iter()
        .filter_map(|row| row["id"].as_str())
        .map(|id| id.splitn(3, '.').take(2).collect::<Vec<_>>().join("."))
        .collect()
}

/// One row per property: what a value write needs. The scope and series
/// component follow from the id prefix; nested schemas and unit codes are
/// left to full detail.
fn compact(row: &Value) -> Value {
    let schema = &row["schema"];
    let mut out = json!({"id":row["id"],"label":row["label"],"type":schema["type"]});
    for key in ["min", "max", "format"] {
        if let Some(value) = schema.get(key) {
            out[key] = value.clone();
        }
    }
    if schema["exclusive_min"] == true {
        out["exclusive_min"] = json!(true);
    }
    if let Some(variants) = schema["variants"].as_array() {
        out["values"] = variants.iter().map(|v| v["id"].clone()).collect();
    }
    if schema.get("input_schema").is_some() || schema.get("codes").is_some() {
        out["requires_full"] = json!(true);
    }
    if let Some(unit) = row["unit"].as_str().filter(|unit| !unit.is_empty()) {
        out["unit"] = json!(unit);
    }
    if row["access"] != "read_write" {
        out["access"] = row["access"].clone();
    }
    let applicability = &row["applicability"];
    if !applicability["encoding"].is_null() {
        out["encoding"] = applicability["encoding"].clone();
    }
    if applicability["required_field_capabilities"]
        .as_array()
        .is_some_and(|c| !c.is_empty())
    {
        out["requires"] = applicability["required_field_capabilities"].clone();
    }
    out
}

#[cfg(test)]
#[path = "property_catalog_tests.rs"]
mod tests;
