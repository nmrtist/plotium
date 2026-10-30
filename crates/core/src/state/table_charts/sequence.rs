//! The sequence variable of an in-situ series: one numeric position per trace,
//! read from column names such as `300 K`, `t=10 min` or `scan 12`.

/// Trace positions in column order, strictly monotonic.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SequenceAxis {
    pub values: Vec<f64>,
    pub label: String,
}

/// Read a sequence from column names that share everything but a number. Names
/// without such a number, or numbers that repeat or turn back, are not a
/// sequence.
pub(crate) fn sequence_axis<'a>(names: impl IntoIterator<Item = &'a str>) -> Option<SequenceAxis> {
    let names: Vec<&str> = names.into_iter().map(str::trim).collect();
    if names.len() < 2 {
        return None;
    }
    let prefix = number_prefix(&names);
    let suffix = number_suffix(&names, prefix.len());
    let numbers: Vec<&str> = names
        .iter()
        .map(|name| &name[prefix.len()..name.len() - suffix.len()])
        .collect();
    // `E-0.2` reads as a sign and `scan-1` as a separator; a shared hyphen
    // before a decimal could be either.
    if prefix.ends_with('-') && numbers.iter().any(|number| number.contains('.')) {
        return None;
    }
    let values = numbers
        .iter()
        .map(|number| {
            let number = number.replace('\u{2212}', "-");
            number.parse::<f64>().ok().filter(|value| value.is_finite())
        })
        .collect::<Option<Vec<f64>>>()?;
    is_strictly_monotonic(&values).then(|| SequenceAxis {
        values,
        label: sequence_label(prefix, suffix),
    })
}

pub(crate) fn is_strictly_monotonic(values: &[f64]) -> bool {
    values.iter().all(|value| value.is_finite())
        && (values.windows(2).all(|pair| pair[0] < pair[1])
            || values.windows(2).all(|pair| pair[0] > pair[1]))
}

/// Longest shared lead-in that stops before the number. A trailing sign or
/// decimal point belongs to the number unless it follows a name, as in `scan-1`
/// or `Fig.2`.
fn number_prefix<'a>(names: &[&'a str]) -> &'a str {
    let first = names[0];
    let mut end = 0;
    for (index, ch) in first.char_indices() {
        if ch.is_ascii_digit() || names.iter().any(|name| !name[index..].starts_with(ch)) {
            break;
        }
        end = index + ch.len_utf8();
    }
    let mut prefix = &first[..end];
    for marks in [&['.'][..], &['+', '-', '\u{2212}'][..]] {
        if let Some(head) = prefix.strip_suffix(marks)
            && !head.ends_with(char::is_alphanumeric)
        {
            prefix = head;
        }
    }
    prefix
}

/// Longest shared tail after `start` that holds no digit.
fn number_suffix<'a>(names: &[&'a str], start: usize) -> &'a str {
    let first = names[0];
    let mut begin = first.len();
    for (index, ch) in first.char_indices().rev() {
        if index < start
            || ch.is_ascii_digit()
            || names.iter().any(|name| {
                let Some(position) = name.len().checked_sub(first.len() - index) else {
                    return true;
                };
                position < start
                    || !name
                        .get(position..)
                        .is_some_and(|tail| tail.starts_with(ch))
            })
        {
            break;
        }
        begin = index;
    }
    &first[begin..]
}

fn sequence_label(prefix: &str, suffix: &str) -> String {
    let unit = suffix.trim();
    let name = prefix
        .trim()
        .trim_end_matches(['=', ':', '_', '-', '.'])
        .trim();
    let name = if name.is_empty() {
        match unit {
            "ms" | "s" | "sec" | "min" | "h" | "hr" => "Time",
            "K" | "°C" | "℃" => "Temperature",
            "V" | "mV" => "Potential",
            _ => "",
        }
    } else {
        name
    };
    match (name.is_empty(), unit.is_empty()) {
        (false, false) => format!("{name} ({unit})"),
        (false, true) => name.to_owned(),
        (true, _) => unit.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::sequence_axis;

    fn read(names: &[&str]) -> Option<(Vec<f64>, String)> {
        sequence_axis(names.iter().copied()).map(|axis| (axis.values, axis.label))
    }

    #[test]
    fn reads_numbers_and_units_from_column_names() {
        assert_eq!(
            read(&["300 K", "350 K", "400 K"]),
            Some((vec![300.0, 350.0, 400.0], "Temperature (K)".to_owned()))
        );
        assert_eq!(
            read(&["t=0.5 min", "t=1.5 min", "t=10 min"]),
            Some((vec![0.5, 1.5, 10.0], "t (min)".to_owned()))
        );
        assert_eq!(
            read(&["10", "11", "12"]),
            Some((vec![10.0, 11.0, 12.0], String::new()))
        );
        assert_eq!(
            read(&["scan 1", "scan 2", "scan 10"]),
            Some((vec![1.0, 2.0, 10.0], "scan".to_owned()))
        );
    }

    #[test]
    fn keeps_signs_and_decimal_points_with_the_number() {
        assert_eq!(
            read(&["-0.1 V", "-0.2 V"]),
            Some((vec![-0.1, -0.2], "Potential (V)".to_owned()))
        );
        assert_eq!(
            read(&["E=-0.2 V", "E=0.1 V"]),
            Some((vec![-0.2, 0.1], "E (V)".to_owned()))
        );
        assert_eq!(
            read(&[".5 h", ".6 h"]),
            Some((vec![0.5, 0.6], "Time (h)".to_owned()))
        );
        assert_eq!(
            read(&["scan-1", "scan-2"]),
            Some((vec![1.0, 2.0], "scan".to_owned()))
        );
        assert_eq!(
            read(&["\u{2212}0.2 V", "\u{2212}0.1 V"]),
            Some((vec![-0.2, -0.1], "Potential (V)".to_owned()))
        );
        assert_eq!(
            read(&["Fig.1", "Fig.2", "Fig.10"]),
            Some((vec![1.0, 2.0, 10.0], "Fig".to_owned()))
        );
        assert_eq!(read(&["E-0.2 V", "E-0.1 V"]), None);
    }

    #[test]
    fn rejects_names_that_are_not_a_sequence() {
        assert_eq!(read(&["a", "b"]), None);
        assert_eq!(read(&["300 K"]), None);
        assert_eq!(read(&["1 K", "2 K", "2 K"]), None);
        assert_eq!(read(&["1 K", "3 K", "2 K"]), None);
        assert_eq!(read(&["Fe 1", "Co 2"]), None);
        assert_eq!(read(&["10 K", "20 min"]), None);
    }
}
