//! Origin worksheet exports: leading label rows (`Short Name`, `Long Name`,
//! `Units`, `Comments`, …) keyed by a first column that then carries row
//! numbers for the data rows.

use super::SourceRows;

pub(super) struct OriginLabels {
    pub headers: Vec<String>,
    pub comments: Vec<String>,
    /// The row-number column is kept only when it is the sole x candidate.
    pub kept_row_numbers: bool,
}

/// Strips Origin label rows and the row-number column from `rows`, returning
/// the derived column labels. Returns `None` and leaves `rows` untouched unless
/// the table starts with an Origin label row and every data row is numbered
/// consecutively in the first column.
pub(super) fn take_labels(rows: &mut SourceRows) -> Option<OriginLabels> {
    let first = rows.first()?.1.first()?.trim();
    if !matches!(first, "Short Name" | "Long Name") {
        return None;
    }
    let label_rows = rows
        .iter()
        .take_while(|(_, cells)| {
            let key = cells[0].trim();
            !key.is_empty() && key.parse::<f64>().is_err()
        })
        .count();
    let width = rows[0].1.len() - 1;
    if label_rows == rows.len() || width == 0 || !numbered_consecutively(&rows[label_rows..]) {
        return None;
    }
    let label = |key: &str| {
        rows[..label_rows]
            .iter()
            .find(|(_, cells)| cells[0].trim() == key)
            .map(|(_, cells)| cells[1..].iter().map(|c| plain_text(c.trim())).collect())
            .unwrap_or_else(|| vec![String::new(); width])
    };
    let (short, long, units, mut comments): (Vec<String>, Vec<String>, Vec<String>, Vec<String>) = (
        label("Short Name"),
        label("Long Name"),
        label("Units"),
        label("Comments"),
    );
    let mut headers = (0..width)
        .map(|i| {
            let base = [&long[i], &comments[i], &short[i]]
                .into_iter()
                .find(|s| !s.is_empty())
                .cloned()
                .unwrap_or_else(|| format!("Column {}", i + 1));
            if units[i].is_empty() {
                base
            } else {
                format!("{base} ({})", units[i])
            }
        })
        .collect::<Vec<_>>();
    let duplicated = (0..width)
        .map(|i| headers.iter().filter(|h| **h == headers[i]).count() > 1)
        .collect::<Vec<_>>();
    for (i, header) in headers.iter_mut().enumerate() {
        if duplicated[i] {
            let suffix = if short[i].is_empty() {
                format!("{}", i + 1)
            } else {
                short[i].clone()
            };
            *header = format!("{header} [{suffix}]");
        }
    }
    // Equal Short Names can still collide; the column number always differs.
    for i in 0..width {
        if headers[..i].contains(&headers[i]) {
            headers[i] = format!("{} ({})", headers[i], i + 1);
        }
    }
    rows.drain(..label_rows);
    let kept_row_numbers = width == 1;
    if kept_row_numbers {
        headers.insert(0, "Row".to_owned());
    } else {
        for (_, cells) in rows.iter_mut() {
            cells.remove(0);
        }
    }
    if kept_row_numbers {
        comments.insert(0, String::new());
    }
    Some(OriginLabels {
        headers,
        comments,
        kept_row_numbers,
    })
}

fn numbered_consecutively(rows: &[(usize, Vec<String>)]) -> bool {
    let numbers = rows
        .iter()
        .map(|(_, cells)| cells[0].trim().parse::<u64>().ok())
        .collect::<Option<Vec<_>>>();
    numbers.is_some_and(|numbers| numbers.windows(2).all(|pair| pair[1] == pair[0] + 1))
}

/// Nested escapes deeper than this are kept verbatim, bounding recursion on
/// untrusted header text.
const MAX_ESCAPE_DEPTH: usize = 8;

/// Origin rich-text escapes as plain text: `\+(…)` and `\-(…)` become Unicode
/// scripts where every character has one, else Plotium `^{…}`/`_{…}` markup;
/// `\g(…)` maps Symbol-font letters to Greek; other escapes keep their text.
pub(super) fn plain_text(text: &str) -> String {
    plain_text_at(text, 0)
}

fn plain_text_at(text: &str, depth: usize) -> String {
    if depth > MAX_ESCAPE_DEPTH {
        return text.to_owned();
    }
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find('\\') {
        out.push_str(&rest[..start]);
        let escape = &rest[start + 1..];
        let mut chars = escape.chars();
        let (Some(code), Some('(')) = (chars.next(), chars.next()) else {
            out.push('\\');
            rest = escape;
            continue;
        };
        let body_start = code.len_utf8() + 1;
        let Some(body_len) = balanced_len(&escape[body_start..]) else {
            out.push('\\');
            rest = escape;
            continue;
        };
        let inner = plain_text_at(&escape[body_start..body_start + body_len], depth + 1);
        match code {
            '+' => out.push_str(&script(&inner, SUPERSCRIPTS, "^")),
            '-' => out.push_str(&script(&inner, SUBSCRIPTS, "_")),
            'g' => out.extend(inner.chars().map(greek)),
            _ => out.push_str(&inner),
        }
        rest = &escape[body_start + body_len + 1..];
    }
    out.push_str(rest);
    out
}

/// Byte length of the text before the `)` that closes an already-open `(`.
fn balanced_len(text: &str) -> Option<usize> {
    let mut depth = 0usize;
    for (index, ch) in text.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' if depth == 0 => return Some(index),
            ')' => depth -= 1,
            _ => {}
        }
    }
    None
}

fn script(text: &str, table: &[(char, char)], marker: &str) -> String {
    text.chars()
        .map(|ch| {
            table
                .iter()
                .find(|(plain, _)| *plain == ch)
                .map(|&(_, s)| s)
        })
        .collect::<Option<String>>()
        .unwrap_or_else(|| format!("{marker}{{{text}}}"))
}

const SUPERSCRIPTS: &[(char, char)] = &[
    ('0', '⁰'),
    ('1', '¹'),
    ('2', '²'),
    ('3', '³'),
    ('4', '⁴'),
    ('5', '⁵'),
    ('6', '⁶'),
    ('7', '⁷'),
    ('8', '⁸'),
    ('9', '⁹'),
    ('+', '⁺'),
    ('-', '⁻'),
    ('−', '⁻'),
    ('=', '⁼'),
    ('(', '⁽'),
    (')', '⁾'),
    ('n', 'ⁿ'),
    ('i', 'ⁱ'),
];

const SUBSCRIPTS: &[(char, char)] = &[
    ('0', '₀'),
    ('1', '₁'),
    ('2', '₂'),
    ('3', '₃'),
    ('4', '₄'),
    ('5', '₅'),
    ('6', '₆'),
    ('7', '₇'),
    ('8', '₈'),
    ('9', '₉'),
    ('+', '₊'),
    ('-', '₋'),
    ('−', '₋'),
    ('=', '₌'),
    ('(', '₍'),
    (')', '₎'),
];

/// Origin's `\g()` renders its text in the Symbol font, whose Latin letters
/// stand for Greek ones.
fn greek(ch: char) -> char {
    const LOWER: &str = "αβχδεφγηιϕκλμνοπθρστυϖωξψζ";
    if ch.is_ascii_lowercase() {
        LOWER
            .chars()
            .nth(usize::from(ch as u8 - b'a'))
            .unwrap_or(ch)
    } else if ch == 'J' {
        'ϑ'
    } else if ch == 'V' {
        'ς'
    } else if ch.is_ascii_uppercase() {
        let lower = greek(ch.to_ascii_lowercase());
        lower.to_uppercase().next().unwrap_or(ch)
    } else {
        ch
    }
}
