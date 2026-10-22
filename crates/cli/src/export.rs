//! Export the canvases of a saved project through the shared `figure.export` tool.
use super::{Command, OutputFormat, ParseError, ParseOutcome, Status, fail_automation};
use plotium_core::automation::*;
use plotium_core::export::ExportFormat;
use plotium_core::state::PlotiumApp;
use std::collections::VecDeque;
use std::ffi::OsString;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq)]
pub struct ExportCommand {
    pub input: PathBuf,
    pub output_dir: PathBuf,
    pub format: OutputFormat,
    /// Canvas resource ids or exact names; empty exports every canvas.
    pub canvases: Vec<String>,
    pub dpi: Option<u16>,
    pub overwrite: bool,
}

pub fn parse(mut args: VecDeque<OsString>) -> Result<ParseOutcome, ParseError> {
    let mut input = None;
    let mut output_dir = None;
    let mut format = None;
    let mut canvases = Vec::new();
    let mut dpi = None;
    let mut overwrite = false;
    while let Some(token) = args.pop_front() {
        match token.to_str() {
            Some("--help" | "-h") => return Ok(ParseOutcome::Help),
            Some("--output-dir") if output_dir.is_none() => {
                output_dir = Some(PathBuf::from(take_value(&mut args, "--output-dir")?));
            }
            Some("--format") if format.is_none() => {
                format = Some(OutputFormat::explicit(&take_value(&mut args, "--format")?)?);
            }
            Some("--canvas") => {
                let value = take_value(&mut args, "--canvas")?
                    .into_string()
                    .map_err(|_| ParseError::new("--canvas must be valid Unicode"))?;
                canvases.push(value);
            }
            Some("--dpi") if dpi.is_none() => {
                let value = take_value(&mut args, "--dpi")?
                    .to_str()
                    .and_then(|value| value.parse::<u16>().ok())
                    .filter(|value| *value > 0)
                    .ok_or_else(|| ParseError::new("--dpi must be a positive integer"))?;
                dpi = Some(value);
            }
            Some("--overwrite") if !overwrite => overwrite = true,
            Some(value) if value.starts_with('-') => {
                return Err(ParseError::new(format!(
                    "{value} is invalid or was provided more than once for export"
                )));
            }
            _ if input.is_none() => input = Some(PathBuf::from(token)),
            _ => return Err(ParseError::new("export accepts exactly one project")),
        }
    }
    Ok(ParseOutcome::Command(Command::Export(ExportCommand {
        input: input.ok_or_else(|| ParseError::new("export requires <project.plotium>"))?,
        output_dir: output_dir
            .ok_or_else(|| ParseError::new("export requires --output-dir <dir>"))?,
        format: format.unwrap_or(OutputFormat(ExportFormat::Pdf)),
        canvases,
        dpi,
        overwrite,
    })))
}

fn take_value(args: &mut VecDeque<OsString>, flag: &str) -> Result<OsString, ParseError> {
    args.pop_front()
        .filter(|value| !value.to_string_lossy().starts_with("--"))
        .ok_or_else(|| ParseError::new(format!("{flag} requires a value")))
}

pub fn run(command: ExportCommand) -> Status {
    eprintln!("plotium-cli: loading {}", command.input.display());
    // The project is only read: nothing here saves it back to disk.
    let mut app = match plotium_core::project::load_project(&command.input) {
        Ok(app) => app,
        Err(error) => {
            eprintln!("plotium-cli: project load failed: {error}");
            return Status::Input;
        }
    };
    if app.doc.canvases.is_empty() {
        eprintln!("plotium-cli: the project has no canvases to export");
        return Status::Input;
    }
    let ids = match select_canvases(&app, &command.canvases) {
        Ok(ids) => ids,
        Err(message) => {
            eprintln!("plotium-cli: {message}");
            return Status::Usage;
        }
    };
    let mut parameters = serde_json::json!({
        "directory": command.output_dir,
        "format": command.format.name(),
        "overwrite": command.overwrite,
    });
    if let Some(dpi) = command.dpi {
        parameters["dpi"] = dpi.into();
    }
    let plan = match plan_tool(
        &app,
        ToolRequest {
            tool_id: "figure.export".into(),
            tool_version: 1,
            parameters,
            targets: TargetSelector::Explicit { ids },
            expected_revision: DocumentRevision(app.doc.automation_revision),
            caller: CallerType::Workflow,
        },
    ) {
        Ok(plan) => plan,
        Err(error) => return fail_automation(error),
    };
    eprintln!(
        "plotium-cli: exporting {} canvas(es) to {}",
        plan.targets.len(),
        command.output_dir.display()
    );
    // Writing files outside the project is the whole point of this command, so
    // it alone grants ExternalWrite; `tool run` stays at ReversibleModify.
    let result = match execute_tool(&mut app, plan, ExecutionAuthority::ExternalWrite) {
        Ok(result) => result,
        Err(error) => {
            eprintln!("plotium-cli: export failed: {error}");
            return Status::Export;
        }
    };
    match serde_json::to_string(&result) {
        Ok(output) => println!("{output}"),
        Err(error) => {
            eprintln!("plotium-cli: result serialization failed: {error}");
            return Status::Internal;
        }
    }
    let failures = result
        .targets
        .iter()
        .filter(|target| target.outcome != TargetOutcome::Succeeded)
        .inspect(|target| {
            eprintln!(
                "plotium-cli: canvas not exported ({:?}): {}",
                target.outcome, target.message
            )
        })
        .count();
    if failures == 0 {
        Status::Success
    } else {
        Status::Export
    }
}

/// Resolves `--canvas` selectors to stable canvas resource ids, in the order
/// given. An exact resource id wins over a name so an id is always usable.
fn select_canvases(app: &PlotiumApp, selectors: &[String]) -> Result<Vec<String>, String> {
    let canvases = &app.doc.canvases;
    let mut selected: Vec<usize> = Vec::new();
    if selectors.is_empty() {
        selected.extend(0..canvases.len());
    }
    for selector in selectors {
        let index = match canvases
            .iter()
            .position(|canvas| canvas.resource_id.to_string() == *selector)
        {
            Some(index) => index,
            None => {
                let matches: Vec<usize> = (0..canvases.len())
                    .filter(|&index| canvases[index].name == *selector)
                    .collect();
                match matches.as_slice() {
                    [index] => *index,
                    [] => return Err(format!("unknown canvas: {selector}")),
                    _ => {
                        return Err(format!(
                            "canvas name '{selector}' is ambiguous; select one by id: {}",
                            ids(app, &matches)
                        ));
                    }
                }
            }
        };
        if !selected.contains(&index) {
            selected.push(index);
        }
    }
    // Output files are named after the canvas, so two selected canvases with
    // one file stem would collide; with --overwrite the second would silently
    // replace the first. Stems are compared case-insensitively because the
    // default macOS and Windows file systems are.
    let stem = |index: usize| canvas_export_stem(&canvases[index].name).to_lowercase();
    for (position, &index) in selected.iter().enumerate() {
        let same: Vec<usize> = selected[position..]
            .iter()
            .copied()
            .filter(|&other| stem(other) == stem(index))
            .collect();
        if same.len() > 1 {
            return Err(format!(
                "canvases {} would export to the same file '{}'; rename one or select only one",
                ids(app, &same),
                canvas_export_stem(&canvases[index].name)
            ));
        }
    }
    Ok(selected
        .into_iter()
        .map(|index| canvases[index].resource_id.to_string())
        .collect())
}

fn ids(app: &PlotiumApp, indices: &[usize]) -> String {
    indices
        .iter()
        .map(|&index| app.doc.canvases[index].resource_id.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use plotium_core::state::CanvasDocument;

    #[test]
    fn canvas_selectors_resolve_ids_and_reject_ambiguous_names() {
        let mut app = PlotiumApp::new_with_settings(Default::default());
        for name in ["Overview", "Detail", "Detail", "A/B", "a_b"] {
            app.doc
                .canvases
                .push(CanvasDocument::new(name.to_owned(), [180.0, 120.0]));
        }
        let detail = app.doc.canvases[2].resource_id.to_string();
        assert_eq!(
            select_canvases(&app, &["Overview".into(), detail.clone()]),
            Ok(vec![app.doc.canvases[0].resource_id.to_string(), detail])
        );
        let ambiguous = select_canvases(&app, &["Detail".into()]).unwrap_err();
        assert!(ambiguous.contains("ambiguous"), "{ambiguous}");
        // Exporting everything would write both "Detail" canvases to one file.
        assert!(
            select_canvases(&app, &[])
                .unwrap_err()
                .contains("same file")
        );
        // Different names can still map to one case-insensitive file stem.
        let stems = select_canvases(&app, &["A/B".into(), "a_b".into()]).unwrap_err();
        assert!(stems.contains("same file 'A_B'"), "{stems}");
        assert!(select_canvases(&app, &["Missing".into()]).is_err());
    }
}
