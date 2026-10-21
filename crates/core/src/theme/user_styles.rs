//! User styles: themes captured from a canvas and saved as `.plotiumstyle`
//! JSON files in the configuration directory, next to the built-in themes.

use super::Theme;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const STYLE_EXTENSION: &str = "plotiumstyle";
const STYLE_VERSION: u32 = 1;

#[derive(Debug, thiserror::Error)]
pub enum StyleError {
    #[error("{0}")]
    Invalid(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("style file is not valid: {0}")]
    Format(#[from] serde_json::Error),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserStyleInfo {
    pub name: String,
    pub path: PathBuf,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StyleFile {
    schema_version: u32,
    style: Theme,
}

pub fn styles_dir() -> Option<PathBuf> {
    crate::settings::config_dir().map(|dir| dir.join("styles"))
}

/// Trim and check a style name: a valid file name that does not shadow a
/// built-in theme id.
pub fn validate_style_name(name: &str) -> Result<&str, StyleError> {
    // Styles follow the processing-template file name rules.
    let name = crate::project::validate_template_name(name)
        .map_err(|error| StyleError::Invalid(error.to_string().replace("template", "style")))?;
    if Theme::BUILTIN_IDS
        .iter()
        .any(|id| id.eq_ignore_ascii_case(name))
    {
        return Err(StyleError::Invalid(format!(
            "\"{name}\" is the name of a built-in theme"
        )));
    }
    Ok(name)
}

fn style_path(dir: &Path, name: &str) -> Result<PathBuf, StyleError> {
    let name = validate_style_name(name)?;
    Ok(dir.join(format!("{name}.{STYLE_EXTENSION}")))
}

pub fn user_style_exists(dir: &Path, name: &str) -> bool {
    style_path(dir, name).is_ok_and(|path| path.exists())
}

pub fn list_user_styles(dir: &Path) -> Vec<UserStyleInfo> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut styles: Vec<UserStyleInfo> = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let is_style = path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case(STYLE_EXTENSION));
            if !is_style || !path.is_file() {
                return None;
            }
            let name = path.file_stem()?.to_str()?.to_owned();
            // Only list files that load and delete by name resolve back to,
            // so every listed row can be managed from the style browser.
            (style_path(dir, &name).ok()? == path).then_some(UserStyleInfo { name, path })
        })
        .collect();
    styles.sort_by_key(|style| style.name.to_lowercase());
    styles
}

/// Save `theme` under `name`, overwriting a style of the same name.
pub fn save_user_style(dir: &Path, name: &str, theme: &Theme) -> Result<PathBuf, StyleError> {
    let path = style_path(dir, name)?;
    let file = StyleFile {
        schema_version: STYLE_VERSION,
        style: theme.clone(),
    };
    let json = serde_json::to_vec_pretty(&file)?;
    std::fs::create_dir_all(dir)?;
    std::fs::write(&path, json)?;
    Ok(path)
}

/// Load the style saved under `name`. The file name is the style's identity,
/// so a renamed file answers to its new name.
pub fn load_user_style(dir: &Path, name: &str) -> Result<Theme, StyleError> {
    let path = style_path(dir, name)?;
    let file: StyleFile = serde_json::from_slice(&std::fs::read(&path)?)?;
    if file.schema_version != STYLE_VERSION {
        return Err(StyleError::Invalid(format!(
            "unsupported style version {}",
            file.schema_version
        )));
    }
    let mut theme = file.style;
    check_sizes(&theme)?;
    let name = name.trim().to_owned();
    theme.id.clone_from(&name);
    theme.name = name;
    Ok(theme)
}

/// Largest size, in points, a style file may set. Far beyond any figure, it
/// only keeps a hand-edited value from reaching layout as an absurd size.
const MAX_SIZE_PT: f32 = 1000.0;

/// Style files can be edited by hand; reject sizes the renderers cannot draw.
fn check_sizes(theme: &Theme) -> Result<(), StyleError> {
    let t = &theme.figure_typography;
    let positive = [
        ("base font size", Some(theme.base_font_pt)),
        ("tick label size", Some(t.tick_pt)),
        ("axis title size", Some(t.label_pt)),
        ("figure title size", Some(t.title_pt)),
        ("legend size", Some(t.legend_pt)),
        ("line width", theme.line_width_pt),
        ("marker size", theme.marker_size_pt),
    ];
    let non_negative = [
        ("axis line width", t.axis_line_pt),
        ("tick length", t.tick_length_pt),
    ];
    for (what, value) in positive {
        if let Some(value) = value
            && !(value > 0.0 && value <= MAX_SIZE_PT)
        {
            return Err(StyleError::Invalid(format!(
                "{what} must be above 0 and at most {MAX_SIZE_PT} pt"
            )));
        }
    }
    for (what, value) in non_negative {
        if !(0.0..=MAX_SIZE_PT).contains(&value) {
            return Err(StyleError::Invalid(format!(
                "{what} must be between 0 and {MAX_SIZE_PT} pt"
            )));
        }
    }
    Ok(())
}

pub fn delete_user_style(dir: &Path, name: &str) -> Result<(), StyleError> {
    std::fs::remove_file(style_path(dir, name)?)?;
    Ok(())
}

#[cfg(test)]
#[path = "user_styles_tests.rs"]
mod tests;
