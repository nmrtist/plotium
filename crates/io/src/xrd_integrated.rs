//! Integrated powder patterns stored as text: pyFAI and Dioptas `.xy` and `.dat`,
//! Fit2D `.chi`, and GSAS `.xye`.

use crate::format::{DataFormat, XrdFormat};
use crate::{
    Acquisition, AcquisitionIdentity, IoError, LoadResult, LoadWarning, LoadWarningCode,
    Provenance, XrdData,
};
use std::path::Path;

const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;
const MIN_POINTS: usize = 2;
/// Wavelengths in pyFAI headers are in metres when they are this small.
const METRES_BELOW: f64 = 1e-6;
const METRES_TO_ANGSTROM: f64 = 1e10;
const NM_TO_ANGSTROM: f64 = 10.0;
const PER_NM_TO_PER_ANGSTROM: f64 = 0.1;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Ruler {
    TwoThetaDeg,
    TwoThetaRad,
    /// Q in Å⁻¹.
    Q,
    /// Q in nm⁻¹.
    QPerNm,
}

#[derive(Debug, Clone, PartialEq)]
enum HeaderAxis {
    /// No header text at all, as in bare two-column exports.
    Unstated,
    Ruler(Ruler),
    /// A diffraction axis this reader does not convert.
    Unsupported(&'static str),
    /// Header text that names something else, or no axis.
    Other,
}

#[derive(Debug, PartialEq)]
struct Header {
    axis: HeaderAxis,
    wavelength_angstrom: Option<f64>,
}

/// pyFAI and Dioptas write the axis as the first word of the column line, and
/// may quote file paths or sample names above it, so only whole words count.
fn axis_word(word: &str) -> HeaderAxis {
    match word.to_ascii_lowercase().as_str() {
        "2th_deg" | "2theta" | "2-theta" | "two_theta" | "twotheta" => {
            HeaderAxis::Ruler(Ruler::TwoThetaDeg)
        }
        "2th_rad" => HeaderAxis::Ruler(Ruler::TwoThetaRad),
        "q_nm^-1" => HeaderAxis::Ruler(Ruler::QPerNm),
        "q_a^-1" | "q_å^-1" => HeaderAxis::Ruler(Ruler::Q),
        "r_mm" => HeaderAxis::Unsupported("radial distance in mm"),
        "d_a" | "d_nm" | "d*2_a^-2" | "d*2_nm^-2" => HeaderAxis::Unsupported("d spacing"),
        _ => HeaderAxis::Other,
    }
}

fn fit2d_axis(label: &str) -> HeaderAxis {
    let label = label.to_ascii_lowercase();
    if label.starts_with("2-theta") {
        HeaderAxis::Ruler(Ruler::TwoThetaDeg)
    } else if label.starts_with("radial") {
        HeaderAxis::Unsupported("radial distance")
    } else {
        HeaderAxis::Other
    }
}

fn header_axis(lines: &[&str]) -> HeaderAxis {
    let body: Vec<&str> = lines
        .iter()
        .copied()
        .filter(|l| !l.starts_with("/*"))
        .collect();
    let Some(last) = body.last() else {
        return HeaderAxis::Unstated;
    };
    // Fit2D: title, x label, y label, point count.
    if body.len() >= 3 && last.parse::<usize>().is_ok() {
        return fit2d_axis(body[body.len() - 3]);
    }
    let columns = last.trim_start_matches('#').trim();
    columns
        .split_whitespace()
        .next()
        .map_or(HeaderAxis::Unstated, axis_word)
}

fn wavelength_in(line: &str) -> Option<f64> {
    let lower = line.to_ascii_lowercase();
    let start = lower.find("wavelength")? + "wavelength".len();
    let is_separator = |c: char| matches!(c, ':' | '=' | ' ' | '\t' | '(' | ')' | '[' | ']');
    let mut words = lower[start..]
        .split(is_separator)
        .filter(|word| !word.is_empty());
    let mut word = words.next()?;
    let mut unit = None;
    if word.parse::<f64>().is_err() {
        unit = Some(word);
        word = words.next()?;
    }
    let value: f64 = word.parse().ok()?;
    let scale = match unit.or_else(|| words.next()) {
        Some("m") => METRES_TO_ANGSTROM,
        Some("nm") => NM_TO_ANGSTROM,
        Some("a" | "å" | "ang" | "angstrom" | "angstroms") => 1.0,
        _ if value < METRES_BELOW => METRES_TO_ANGSTROM,
        _ => 1.0,
    };
    (value.is_finite() && value > 0.0).then_some(value * scale)
}

enum Row {
    Point(f64, f64),
    /// Numbers, but a masked bin: pyFAI writes these as nan.
    Masked,
    Text,
}

fn row(line: &str) -> Row {
    let mut values = line.split_whitespace().map(str::parse::<f64>);
    match (values.next(), values.next()) {
        (Some(Ok(x)), Some(Ok(y))) if x.is_finite() && y.is_finite() => Row::Point(x, y),
        (Some(Ok(_)), Some(Ok(_))) => Row::Masked,
        _ => Row::Text,
    }
}

struct Parsed {
    header: Header,
    rows: Vec<(f64, f64)>,
    masked: usize,
}

fn parse(text: &str) -> Result<Parsed, IoError> {
    let mut lines_before = Vec::new();
    let mut wavelength_angstrom = None;
    let mut rows = Vec::new();
    let mut masked = 0;
    for (index, line) in text.lines().enumerate() {
        let line = line.trim_start_matches('\u{feff}').trim();
        if line.is_empty() {
            continue;
        }
        wavelength_angstrom = wavelength_angstrom.or_else(|| wavelength_in(line));
        match row(line) {
            Row::Point(x, y) if !line.starts_with('#') => rows.push((x, y)),
            Row::Masked if !rows.is_empty() => masked += 1,
            _ if rows.is_empty() => lines_before.push(line),
            _ if line.starts_with('#') => {}
            _ => {
                return Err(IoError::InvalidXrd(format!(
                    "malformed pattern row on line {}",
                    index + 1
                )));
            }
        }
    }
    if rows.len() < MIN_POINTS {
        return Err(IoError::InvalidXrd("no pattern rows found".to_owned()));
    }
    let header = Header {
        axis: header_axis(&lines_before),
        wavelength_angstrom,
    };
    Ok(Parsed {
        header,
        rows,
        masked,
    })
}

fn read(path: &Path) -> Result<String, IoError> {
    if std::fs::metadata(path)?.len() > MAX_FILE_BYTES {
        return Err(IoError::InvalidXrd(
            "file is too large for a pattern".into(),
        ));
    }
    Ok(String::from_utf8_lossy(&std::fs::read(path)?).into_owned())
}

fn increasing(rows: &[(f64, f64)]) -> bool {
    rows.windows(2).all(|pair| pair[1].0 > pair[0].0)
}

fn extension(path: &Path) -> String {
    path.extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

/// Whether `path` is a pattern. A header that names another axis keeps the file
/// away from XRD: `.chi` is also Larch's chi(k), and `.xy` holds any two columns.
/// `.dat` is shared with XAS, so it must state its axis.
pub fn is_integrated_pattern(path: &Path) -> bool {
    let extension = extension(path);
    if !path.is_file() || !matches!(extension.as_str(), "xy" | "xye" | "chi" | "dat") {
        return false;
    }
    let Ok(text) = read(path) else {
        return false;
    };
    parse(&text).is_ok_and(|parsed| {
        let mut rows = parsed.rows;
        if rows
            .first()
            .zip(rows.last())
            .is_some_and(|(a, b)| a.0 > b.0)
        {
            rows.reverse();
        }
        let accepted = match parsed.header.axis {
            HeaderAxis::Ruler(_) | HeaderAxis::Unsupported(_) => true,
            HeaderAxis::Unstated => extension != "dat",
            // GSAS `.xye` opens with a free-text title.
            HeaderAxis::Other => extension == "xye",
        };
        accepted && increasing(&rows)
    })
}

pub fn load(path: &Path) -> Result<LoadResult, IoError> {
    let Parsed {
        header,
        mut rows,
        masked,
    } = parse(&read(path)?)?;
    if rows
        .first()
        .zip(rows.last())
        .is_some_and(|(a, b)| a.0 > b.0)
    {
        rows.reverse();
    }
    if !increasing(&rows) {
        return Err(IoError::InvalidXrd(
            "the pattern's x values must increase strictly".to_owned(),
        ));
    }
    let wavelength = header.wavelength_angstrom;
    let mut notes = Vec::new();
    let ruler = match header.axis {
        HeaderAxis::Ruler(ruler) => ruler,
        HeaderAxis::Unsupported(axis) => {
            return Err(IoError::InvalidXrd(format!(
                "this pattern is on {axis}; export it on 2theta or Q to open it"
            )));
        }
        HeaderAxis::Unstated | HeaderAxis::Other => {
            notes.push("No axis in the header; read as 2theta in degrees.".to_owned());
            Ruler::TwoThetaDeg
        }
    };
    let q_scale = match ruler {
        Ruler::TwoThetaDeg | Ruler::TwoThetaRad => None,
        Ruler::Q => Some(1.0),
        Ruler::QPerNm => Some(PER_NM_TO_PER_ANGSTROM),
    };
    let two_theta_deg: Vec<f64> = match q_scale {
        None if ruler == Ruler::TwoThetaRad => rows.iter().map(|row| row.0.to_degrees()).collect(),
        None => rows.iter().map(|row| row.0).collect(),
        Some(scale) => {
            let wavelength = wavelength.ok_or_else(|| {
                IoError::InvalidXrd(
                    "this pattern is on Q and its header has no wavelength; add a \
                     `# Wavelength: <value> A` line or export it on 2theta"
                        .to_owned(),
                )
            })?;
            rows.iter()
                .map(|row| {
                    let sine = row.0 * scale * wavelength / (4.0 * std::f64::consts::PI);
                    if (0.0..=1.0).contains(&sine) {
                        Ok(2.0 * sine.asin().to_degrees())
                    } else {
                        Err(IoError::InvalidXrd(format!(
                            "Q = {} is out of reach at this wavelength",
                            row.0
                        )))
                    }
                })
                .collect::<Result<_, _>>()?
        }
    };
    if masked > 0 {
        let word = if masked == 1 { "row" } else { "rows" };
        notes.push(format!("{masked} masked {word} without a value skipped."));
    }
    // Background-subtracted exports go below zero; XRD intensity cannot.
    let clipped = rows.iter().filter(|row| row.1 < 0.0).count();
    let data = XrdData {
        two_theta_deg,
        intensity: rows.iter().map(|row| row.1.max(0.0)).collect(),
        attenuation: None,
        source: path.display().to_string(),
        instrument: None,
        target: None,
        wavelength_angstrom: wavelength,
        voltage_kv: None,
        current_ma: None,
        scan_step_deg: None,
        scan_speed_deg_min: None,
    };
    data.validate()
        .map_err(|error| IoError::InvalidXrd(error.into()))?;
    if clipped > 0 {
        notes.push(format!("{clipped} negative intensities were set to zero."));
    }
    let warnings = notes
        .into_iter()
        .map(|message| LoadWarning {
            code: LoadWarningCode::InvalidMetadata,
            message,
            path: Some(path.to_path_buf()),
        })
        .collect();
    Ok(LoadResult::new(
        Acquisition::Xrd(Box::new(data)),
        AcquisitionIdentity::from_path(path),
        DataFormat::Xrd(XrdFormat::IntegratedPattern),
        Provenance {
            selected_path: path.to_path_buf(),
            data_path: path.to_path_buf(),
            parameter_paths: Vec::new(),
            companion_paths: Vec::new(),
        },
        warnings,
    ))
}

#[cfg(test)]
#[path = "xrd_integrated_tests.rs"]
mod tests;
