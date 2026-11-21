pub(super) fn normalize_afm_unit(unit: &str) -> String {
    match unit.trim() {
        "~m" | "um" => "µm".to_owned(),
        other => other.to_owned(),
    }
}

pub(super) fn first_number(value: &str) -> Option<f64> {
    value
        .split(|character: char| {
            !(character.is_ascii_digit() || matches!(character, '.' | '-' | '+' | 'e' | 'E'))
        })
        .find_map(|token| (!token.is_empty()).then(|| token.parse().ok()).flatten())
}

pub(super) fn physical_unit(value: &str) -> Option<&str> {
    value
        .split_whitespace()
        .find(|token| token.chars().any(char::is_alphabetic))
}

pub(super) fn sensitivity_scale(value: &str) -> Option<(f64, String)> {
    let multiplier = first_number(parenthesized(value).unwrap_or(value))?;
    let unit = value
        .split_whitespace()
        .find(|token| token.to_ascii_lowercase().contains("/v"))?;
    let numerator = unit.split('/').next()?.trim_matches(|character: char| {
        !character.is_alphanumeric() && character != 'µ' && character != '~'
    });
    (!numerator.is_empty() && multiplier.is_finite())
        .then(|| (multiplier, normalize_afm_unit(numerator)))
}

pub(super) fn calibrated_si_value(value: &str, target: &str) -> Option<f64> {
    let number = first_number(parenthesized(value).unwrap_or(value))?;
    let lower = value.to_ascii_lowercase().replace('µ', "u");
    let factor = match target {
        "m/v" if lower.contains("nm/v") => 1e-9,
        "m/v" if lower.contains("um/v") => 1e-6,
        "m/v" if lower.contains("mm/v") => 1e-3,
        _ => 1.0,
    };
    Some(number * factor)
}

pub(super) fn is_physical_deflection_unit(unit: &str) -> bool {
    let lower = unit.to_ascii_lowercase();
    ["m", "nm", "um", "µm", "pm"]
        .iter()
        .any(|physical| lower == *physical || lower.starts_with(&format!("{physical}/")))
}

/// The quantity after the hard scale is the display value, not the units/LSB.
pub(super) fn display_quantity(value: &str) -> Option<(f64, &str)> {
    let mut quantity = value
        .rsplit_once(')')
        .map_or(value, |(_, tail)| tail)
        .split_whitespace();
    Some((quantity.next()?.parse().ok()?, quantity.next()?))
}

pub(super) fn sample_period_seconds(value: &str) -> Option<f64> {
    let (magnitude, unit) = display_quantity(value).or_else(|| Some((value.parse().ok()?, "s")))?;
    let factor = match unit.to_ascii_lowercase().replace(['µ', 'μ'], "u").as_str() {
        "s" => 1.0,
        "ms" => 1e-3,
        "us" => 1e-6,
        "ns" => 1e-9,
        _ => return None,
    };
    let period = magnitude * factor;
    (period.is_finite() && period > 0.0).then_some(period)
}

pub(super) fn length_factor_nm(unit: &str) -> Option<f64> {
    match unit
        .to_ascii_lowercase()
        .replace(['µ', 'μ', '~'], "u")
        .as_str()
    {
        "m" => Some(1e9),
        "mm" => Some(1e6),
        "um" => Some(1e3),
        "nm" => Some(1.0),
        "pm" => Some(1e-3),
        _ => None,
    }
}

fn parenthesized(value: &str) -> Option<&str> {
    let start = value.rfind('(')? + 1;
    let end = value[start..].find(')')? + start;
    Some(&value[start..end])
}
