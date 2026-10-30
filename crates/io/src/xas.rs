//! X-ray absorption scans stored as text: XDI, NSLS XDAC, and `#`-commented
//! beamline column files such as the APS LabVIEW scans.

use crate::IoError;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;

const MAX_XAS_FILE_BYTES: u64 = 64 * 1024 * 1024;
const MIN_POINTS: usize = 4;
/// Thresholds from Larch `guess_energy_units`, so a file reads the same in both.
const KEV_MAX: f64 = 120.0;
const KEV_STEP: f64 = 0.005;
const ANGLE_MAX: f64 = 90.0;
const MOTOR_STEPS_MIN: f64 = 200_000.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XasFileKind {
    Xdi,
    Xdac,
    Columns,
}

#[derive(Debug, Clone, PartialEq)]
pub struct XasColumn {
    pub name: String,
    pub values: Vec<f64>,
}

/// Column positions guessed from their labels. The energy column is always
/// known; every other role may be absent and stays editable by the caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XasChannels {
    pub energy: usize,
    pub i0: Option<usize>,
    pub transmission: Option<usize>,
    pub reference: Option<usize>,
    /// Fluorescence candidates: named channels, then detector sums in file
    /// order. Multi-element detectors report one sum per emission line, so the
    /// caller picks the line of the absorbing element.
    pub fluorescence: Vec<usize>,
    /// An absorption coefficient the file already carries.
    pub mu: Option<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct XasScan {
    pub kind: XasFileKind,
    /// Energy is converted to eV; columns keep the file's row order.
    pub columns: Vec<XasColumn>,
    pub channels: XasChannels,
    pub beamline: Option<String>,
    pub sample: Option<String>,
    pub element: Option<String>,
    pub edge: Option<String>,
    /// Nominal edge energy recorded by the acquisition software, in eV.
    pub nominal_e0: Option<f64>,
    pub metadata: BTreeMap<String, String>,
    pub warnings: Vec<String>,
}

impl XasScan {
    pub fn energy(&self) -> &[f64] {
        &self.columns[self.channels.energy].values
    }

    pub fn column(&self, index: usize) -> Option<&[f64]> {
        self.columns
            .get(index)
            .map(|column| column.values.as_slice())
    }
}

pub fn load(path: &Path) -> Result<XasScan, IoError> {
    let file = File::open(path)?;
    if file.metadata()?.len() > MAX_XAS_FILE_BYTES {
        return Err(invalid("file is too large for a text scan"));
    }
    let mut bytes = Vec::new();
    file.take(MAX_XAS_FILE_BYTES).read_to_end(&mut bytes)?;
    parse(&String::from_utf8_lossy(&bytes))
}

pub fn parse(text: &str) -> Result<XasScan, IoError> {
    let text = text.trim_start_matches('\u{feff}');
    let lines: Vec<&str> = text.lines().collect();
    let first = lines
        .iter()
        .map(|line| line.trim())
        .find(|line| !line.is_empty())
        .ok_or_else(|| invalid("file is empty"))?;
    if first.starts_with("#") && first.trim_start_matches(['#', ' ']).starts_with("XDI/") {
        parse_xdi(&lines)
    } else if first.starts_with("XDAC") {
        parse_xdac(&lines)
    } else {
        parse_columns(&lines)
    }
}

fn invalid(message: impl Into<String>) -> IoError {
    IoError::InvalidXas(message.into())
}

fn comment_body(line: &str) -> Option<&str> {
    line.trim_start().strip_prefix('#').map(str::trim)
}

fn parse_xdi(lines: &[&str]) -> Result<XasScan, IoError> {
    let mut metadata = BTreeMap::new();
    let mut named_columns: BTreeMap<usize, String> = BTreeMap::new();
    let mut body = None;
    let mut in_user_comment = false;
    let mut user_comment = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let Some(comment) = comment_body(line) else {
            if !line.trim().is_empty() {
                body = Some(index);
                break;
            }
            continue;
        };
        if comment.starts_with("///") {
            in_user_comment = true;
        } else if comment.starts_with("---") {
            in_user_comment = false;
        } else if in_user_comment {
            user_comment.push(comment);
        } else if let Some((key, value)) = comment.split_once(':') {
            let (key, value) = (key.trim(), value.trim());
            match key
                .strip_prefix("Column.")
                .and_then(|number| number.parse().ok())
            {
                Some(number) => {
                    named_columns.insert(number, value.to_owned());
                }
                None => {
                    metadata.insert(key.to_owned(), value.to_owned());
                }
            }
        }
    }
    let body = body.ok_or_else(|| invalid("XDI file has no data rows"))?;
    let table = read_table(&lines[body..], body)?;
    let fields: Vec<(String, Option<String>)> = (1..=table.columns.len())
        .map(|number| {
            let Some(field) = named_columns.get(&number) else {
                return (format!("col{number}"), None);
            };
            let mut words = field.split_whitespace().map(str::to_owned);
            (words.next().unwrap_or_default(), words.next())
        })
        .collect();
    let (labels, units): (Vec<String>, Vec<Option<String>>) = fields.into_iter().unzip();
    if !labels.iter().any(|label| is_energy_label(label)) {
        return Err(invalid("XDI file has no energy column"));
    }
    let mut scan = assemble(XasFileKind::Xdi, labels, &units, table)?;
    scan.beamline = match (metadata.get("Facility.name"), metadata.get("Beamline.name")) {
        (Some(facility), Some(beamline)) => Some(format!("{facility} {beamline}")),
        (facility, beamline) => facility.or(beamline).cloned(),
    };
    scan.sample = metadata
        .get("Sample.name")
        .cloned()
        .or_else(|| (!user_comment.is_empty()).then(|| user_comment.join(" ")));
    scan.element = metadata.get("Element.symbol").cloned();
    scan.edge = metadata.get("Element.edge").cloned();
    scan.metadata = metadata;
    Ok(scan)
}

fn parse_xdac(lines: &[&str]) -> Result<XasScan, IoError> {
    let rule = lines
        .iter()
        .position(|line| {
            let line = line.trim();
            line.len() >= 8 && line.chars().all(|c| c == '-')
        })
        .ok_or_else(|| invalid("XDAC file has no column header"))?;
    let label_line = rule + 1;
    let labels: Vec<String> = lines
        .get(label_line)
        .ok_or_else(|| invalid("XDAC file has no column header"))?
        .split_whitespace()
        .map(str::to_owned)
        .collect();
    let table = read_table(&lines[label_line + 1..], label_line + 1)?;
    if labels.len() != table.columns.len() {
        return Err(invalid(format!(
            "XDAC header names {} columns but rows have {}",
            labels.len(),
            table.columns.len()
        )));
    }
    let mut scan = assemble(XasFileKind::Xdac, labels, &[], table)?;
    let mut comments = Vec::new();
    for line in lines[1..rule].iter().map(|line| line.trim()) {
        if line.is_empty() {
            continue;
        }
        if let Some(position) = line.rfind(" on ")
            && line.starts_with('"')
            && line.contains(" created on ")
        {
            scan.beamline = Some(line[position + 4..].trim().to_owned());
            continue;
        }
        match line.split_once('=') {
            Some((key, value)) => {
                let (key, value) = (key.trim(), value.trim());
                if key == "E0" {
                    scan.nominal_e0 = value
                        .split_whitespace()
                        .next()
                        .and_then(|number| number.parse().ok());
                    if scan.nominal_e0.is_none() {
                        scan.warnings.push(format!("unreadable E0 \"{value}\""));
                    }
                }
                scan.metadata.insert(key.to_owned(), value.to_owned());
            }
            None => comments.push(line),
        }
    }
    scan.sample = (!comments.is_empty()).then(|| comments.join(" "));
    Ok(scan)
}

fn parse_columns(lines: &[&str]) -> Result<XasScan, IoError> {
    let body = lines
        .iter()
        .position(|line| is_data_row(line))
        .ok_or_else(|| invalid("no numeric data rows"))?;
    let table = read_table(&lines[body..], body)?;
    let count = table.columns.len();
    if count < 2 {
        return Err(invalid("a scan needs at least energy and one signal"));
    }
    let header: Vec<&str> = lines[..body]
        .iter()
        .map(|line| line.trim_start_matches([' ', '\t', '#']).trim_end())
        .collect();
    let labels = numbered_labels(&header, count)
        .or_else(|| plain_labels(&header, count))
        .unwrap_or_else(|| (1..=count).map(|n| format!("col{n}")).collect());
    let mut scan = assemble(XasFileKind::Columns, labels, &[], table)?;
    for (index, line) in header.iter().enumerate() {
        if let Some(name) = line.strip_prefix("Beamline ") {
            scan.beamline = Some(name.trim().to_owned());
        } else if line.starts_with("User Comment") {
            scan.sample = header
                .get(index + 1)
                .filter(|comment| !comment.is_empty())
                .map(|comment| (*comment).to_owned());
        }
    }
    Ok(scan)
}

fn is_data_row(line: &str) -> bool {
    let mut tokens = line.split_whitespace().peekable();
    tokens.peek().is_some() && tokens.all(|token| token.parse::<f64>().is_ok())
}

/// APS scans list columns as `1) Mono Energy *   11) DXP12:0:Hg`. Their second
/// heading line is fixed-width with single spaces, so this list is the only
/// form that keeps multi-word names apart.
fn numbered_labels(header: &[&str], count: usize) -> Option<Vec<String>> {
    let mut labels: BTreeMap<usize, String> = BTreeMap::new();
    for line in header {
        let marks = number_marks(line);
        for (position, mark) in marks.iter().enumerate() {
            let end = marks
                .get(position + 1)
                .map_or(line.len(), |next| next.start);
            labels.insert(mark.number, clean_label(&line[mark.label..end]));
        }
    }
    (1..=count)
        .map(|number| labels.remove(&number))
        .collect::<Option<Vec<_>>>()
}

struct NumberMark {
    start: usize,
    label: usize,
    number: usize,
}

/// `N)` marks that start the line or follow whitespace.
fn number_marks(line: &str) -> Vec<NumberMark> {
    let bytes = line.as_bytes();
    let mut marks = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        let start = index;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            index += 1;
        }
        if index > start
            && bytes.get(index) == Some(&b')')
            && (start == 0 || bytes[start - 1].is_ascii_whitespace())
            && let Ok(number) = line[start..index].parse()
        {
            marks.push(NumberMark {
                start,
                label: index + 1,
                number,
            });
        }
        index = index.max(start) + 1;
    }
    marks
}

fn clean_label(label: &str) -> String {
    label.trim().trim_end_matches('*').trim_end().to_owned()
}

/// Closest comment line above the data with one label per column.
fn plain_labels(header: &[&str], count: usize) -> Option<Vec<String>> {
    header.iter().rev().find_map(|line| {
        let wide: Vec<String> = line
            .split('\t')
            .flat_map(|part| part.split("  "))
            .map(clean_label)
            .filter(|label| !label.is_empty())
            .collect();
        let usable = |labels: &[String]| {
            labels.len() == count
                && labels
                    .iter()
                    .all(|label| label.chars().any(char::is_alphabetic))
        };
        if usable(&wide) {
            return Some(wide);
        }
        let words: Vec<String> = line.split_whitespace().map(clean_label).collect();
        usable(&words).then_some(words)
    })
}

struct Table {
    columns: Vec<Vec<f64>>,
    warnings: Vec<String>,
}

fn read_table(lines: &[&str], first_line: usize) -> Result<Table, IoError> {
    let mut columns: Vec<Vec<f64>> = Vec::new();
    let mut warnings = Vec::new();
    for (offset, line) in lines.iter().enumerate() {
        if line.trim().is_empty() || comment_body(line).is_some() {
            continue;
        }
        let number = first_line + offset + 1;
        let row = line
            .split_whitespace()
            .map(str::parse)
            .collect::<Result<Vec<f64>, _>>()
            .map_err(|_| invalid(format!("line {number} is not numeric")))?;
        if columns.is_empty() {
            columns = vec![Vec::new(); row.len()];
        }
        if row.len() != columns.len() {
            // An interrupted scan leaves a partly written final row.
            if lines[offset + 1..]
                .iter()
                .all(|rest| rest.trim().is_empty())
            {
                warnings.push(format!("dropped incomplete final row at line {number}"));
                break;
            }
            return Err(invalid(format!(
                "line {number} has {} values, expected {}",
                row.len(),
                columns.len()
            )));
        }
        for (column, value) in columns.iter_mut().zip(row) {
            column.push(value);
        }
    }
    if columns
        .first()
        .is_none_or(|column| column.len() < MIN_POINTS)
    {
        return Err(invalid(format!(
            "a scan needs at least {MIN_POINTS} points"
        )));
    }
    Ok(Table { columns, warnings })
}

fn assemble(
    kind: XasFileKind,
    labels: Vec<String>,
    units: &[Option<String>],
    table: Table,
) -> Result<XasScan, IoError> {
    let Table {
        columns,
        mut warnings,
    } = table;
    if labels.len() != columns.len() {
        return Err(invalid(format!(
            "{} column labels for {} data columns",
            labels.len(),
            columns.len()
        )));
    }
    let mut columns: Vec<XasColumn> = labels
        .into_iter()
        .zip(columns)
        .map(|(name, values)| XasColumn { name, values })
        .collect();
    let channels = guess_channels(&columns);
    if channels.energy_guessed {
        warnings.push(format!(
            "no column is named as energy; using \"{}\"",
            columns[channels.roles.energy].name
        ));
    }
    let energy_index = channels.roles.energy;
    let declared = units.get(energy_index).and_then(Option::as_deref);
    let unit = match declared {
        Some(unit) if unit.eq_ignore_ascii_case("ev") => EnergyUnit::Ev,
        Some(unit) if unit.eq_ignore_ascii_case("kev") => EnergyUnit::Kev,
        _ if label_key(&columns[energy_index].name).contains("kev") => EnergyUnit::Kev,
        _ => EnergyUnit::guess(&columns[energy_index].values),
    };
    let energy = &mut columns[energy_index].values;
    match unit {
        EnergyUnit::Ev => {
            if energy.iter().all(|value| *value < KEV_MAX) {
                warnings.push("energy is below 120 eV; check the column unit".to_owned());
            }
        }
        EnergyUnit::Kev => {
            energy.iter_mut().for_each(|value| *value *= 1000.0);
            if declared.is_none() {
                warnings.push("energy looks like keV; converted to eV".to_owned());
            }
        }
        EnergyUnit::Angle => return Err(invalid("energy column looks like a mono angle")),
        EnergyUnit::Steps => return Err(invalid("energy column looks like motor steps")),
    }
    Ok(XasScan {
        kind,
        columns,
        channels: channels.roles,
        beamline: None,
        sample: None,
        element: None,
        edge: None,
        nominal_e0: None,
        metadata: BTreeMap::new(),
        warnings,
    })
}

enum EnergyUnit {
    Ev,
    Kev,
    Angle,
    Steps,
}

impl EnergyUnit {
    fn guess(energy: &[f64]) -> Self {
        let max = energy.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        if max > MOTOR_STEPS_MIN {
            return Self::Steps;
        }
        let steps = energy.windows(2).map(|pair| pair[1] - pair[0]);
        let smallest = steps.clone().map(f64::abs).fold(f64::INFINITY, f64::min);
        if max >= KEV_MAX || smallest >= KEV_STEP {
            return Self::Ev;
        }
        if max < ANGLE_MAX && steps.sum::<f64>() < 0.0 {
            Self::Angle
        } else {
            Self::Kev
        }
    }
}

struct GuessedChannels {
    roles: XasChannels,
    energy_guessed: bool,
}

/// Lower-case letters and digits only, so `Mono Energy` and `DXP12:Hg_Sum`
/// compare without punctuation.
fn label_key(label: &str) -> String {
    label
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

fn is_energy_label(label: &str) -> bool {
    let key = label_key(label);
    is_exact_energy_key(&key) || key.contains("energy")
}

fn is_exact_energy_key(key: &str) -> bool {
    matches!(
        key,
        "e" | "ev"
            | "ener"
            | "nergy"
            | "energy"
            | "energyev"
            | "energykev"
            | "monoe"
            | "monoenergy"
    )
}

fn guess_channels(columns: &[XasColumn]) -> GuessedChannels {
    let keys: Vec<String> = columns
        .iter()
        .map(|column| label_key(&column.name))
        .collect();
    let exact = |names: &[&str]| {
        names
            .iter()
            .find_map(|name| keys.iter().position(|key| key == name))
    };
    let energy = keys
        .iter()
        .position(|key| is_exact_energy_key(key))
        .or_else(|| {
            columns
                .iter()
                .position(|column| is_energy_label(&column.name))
        });
    let i0 = exact(&["i0", "io", "imon", "monitor"]).or_else(|| {
        columns.iter().position(|column| {
            let suffix = column.name.rsplit('_').next().unwrap_or_default();
            suffix.eq_ignore_ascii_case("i0")
        })
    });
    let named_fluorescence = |key: &str| matches!(key, "if" | "ifl" | "ifluor");
    let fluorescence = (0..keys.len())
        .filter(|&index| named_fluorescence(&keys[index]))
        .chain((0..keys.len()).filter(|&index| {
            let key = &keys[index];
            let derived = key.starts_with("mu") || key.starts_with("norm");
            let monitor = key.contains("total") || key.contains("i0");
            !named_fluorescence(key)
                && !derived
                && !monitor
                && (key.contains("fluo") || key.ends_with("sum"))
        }))
        .collect();
    GuessedChannels {
        roles: XasChannels {
            energy: energy.unwrap_or(0),
            i0,
            transmission: exact(&["it", "i1", "itrans", "itr", "itransmission"]),
            reference: exact(&["ir", "iref", "irefer", "ireference", "i2"]),
            fluorescence,
            mu: exact(&["mu", "xmu", "mutrans", "mufluor"]),
        },
        energy_guessed: energy.is_none(),
    }
}

#[cfg(test)]
#[path = "xas_tests.rs"]
mod tests;
