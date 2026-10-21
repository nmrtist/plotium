//! Single-operation headless adapter for the shared tool registry.
use super::{Command, ParseError, ParseOutcome, Status};
use plotium_core::automation::*;
use plotium_core::state::PlotiumApp;
use serde::Deserialize;
use std::collections::VecDeque;
use std::ffi::OsString;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq)]
pub enum ToolCommand {
    List,
    Describe(String),
    Run {
        input: PathBuf,
        request: PathBuf,
        output: Option<PathBuf>,
    },
}
pub fn parse(mut args: VecDeque<OsString>) -> Result<ParseOutcome, ParseError> {
    let action = args
        .pop_front()
        .and_then(|s| s.into_string().ok())
        .ok_or_else(|| ParseError::new("tool requires list, describe or run"))?;
    let command = match action.as_str() {
        "list" if args.is_empty() => ToolCommand::List,
        "describe" if args.len() == 1 => ToolCommand::Describe(
            args.pop_front()
                .and_then(|s| s.into_string().ok())
                .ok_or_else(|| ParseError::new("tool ID must be Unicode"))?,
        ),
        "run" => {
            let input = args
                .pop_front()
                .filter(|s| !s.to_string_lossy().starts_with('-'))
                .map(PathBuf::from)
                .ok_or_else(|| ParseError::new("tool run requires an input dataset or project"))?;
            let mut request = None;
            let mut output = None;
            while let Some(flag) = args.pop_front() {
                let value = args
                    .pop_front()
                    .filter(|s| !s.to_string_lossy().starts_with('-'))
                    .ok_or_else(|| ParseError::new("tool run option requires a path"))?;
                match flag.to_str() {
                    Some("--request") if request.is_none() => request = Some(PathBuf::from(value)),
                    Some("--output") if output.is_none() => output = Some(PathBuf::from(value)),
                    _ => {
                        return Err(ParseError::new(
                            "tool run accepts --request and optional --output once each",
                        ));
                    }
                }
            }
            ToolCommand::Run {
                input,
                request: request.ok_or_else(|| {
                    ParseError::new("tool run requires --request <operation.json>")
                })?,
                output,
            }
        }
        _ => {
            return Err(ParseError::new(
                "usage: tool list | tool describe <id> | tool run <input> --request <operation.json> [--output <project.plotium>]",
            ));
        }
    };
    Ok(ParseOutcome::Command(Command::Tool(command)))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Operation {
    tool_id: String,
    #[serde(default = "empty_parameters")]
    parameters: serde_json::Value,
    /// Projects require explicit stable targets. A single native input defaults
    /// to its newly loaded DatasetId, or resolves a field_key once at loading.
    targets: Option<TargetSelector>,
    field_key: Option<String>,
}
fn empty_parameters() -> serde_json::Value {
    serde_json::json!({})
}
pub fn run(command: ToolCommand) -> Status {
    let result = match command {
        ToolCommand::List => Ok(serde_json::json!(
            ToolRegistry::built_in().descriptors().collect::<Vec<_>>()
        )),
        ToolCommand::Describe(id) => ToolRegistry::built_in()
            .get(&id)
            .cloned()
            .ok_or_else(|| format!("Unknown tool: {id}"))
            .and_then(|tool| serde_json::to_value(tool).map_err(|e| e.to_string())),
        ToolCommand::Run {
            input,
            request,
            output,
        } => run_operation(input, request, output),
    };
    match result {
        Ok(value) => {
            println!("{value}");
            if value.get("error").is_some()
                || matches!(
                    value
                        .pointer("/value/task/status")
                        .and_then(serde_json::Value::as_str),
                    Some("failed" | "stale" | "cancelled")
                )
            {
                Status::BatchFailed
            } else {
                Status::Success
            }
        }
        Err(message) => {
            eprintln!("plotium-cli: {message}");
            println!(
                "{}",
                serde_json::json!({"error":{"code":"operation_failed","message":message}})
            );
            Status::BatchFailed
        }
    }
}
fn run_operation(
    input: PathBuf,
    request: PathBuf,
    output: Option<PathBuf>,
) -> Result<serde_json::Value, String> {
    let operation: Operation =
        serde_json::from_slice(&std::fs::read(request).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let project = input.extension().is_some_and(|e| e == "plotium");
    eprintln!("plotium-cli: loading {}", input.display());
    let mut app = if project {
        plotium_core::project::load_project(&input).map_err(|e| e.to_string())?
    } else {
        let loaded = plotium_core::workflow::load_dataset(&input).map_err(|e| e.to_string())?;
        let mut app = PlotiumApp::new_with_settings(Default::default());
        app.doc.datasets.push(loaded.dataset);
        app
    };
    if operation.targets.is_some() && operation.field_key.is_some() {
        return Err("Supply targets or field_key, never both.".into());
    }
    let targets = if let Some(targets) = operation.targets {
        targets
    } else if !project {
        let dataset = &app.doc.datasets[0];
        let resource = if let Some(key) = operation.field_key {
            let field = dataset
                .field_descriptors()
                .into_iter()
                .find(|f| f.local_id == key)
                .ok_or_else(|| format!("Unknown input field key: {key}"))?;
            ProjectResourceProvider::new(&app)
                .descriptors()
                .into_iter()
                .find(|d| {
                    d.resource.kind.0 == KIND_FIELD
                        && d.resource.parent_id.as_deref()
                            == Some(&dataset.resource_id().to_string())
                        && d.resource.local_id.as_deref() == Some(field.local_id.as_str())
                })
                .map(|d| d.resource)
                .ok_or_else(|| "Input field is not exposed as a resource.".to_owned())?
        } else {
            ResourceRef::from(dataset.resource_id())
        };
        TargetSelector::Explicit {
            ids: vec![resource.id],
        }
    } else {
        return Err("Project operations require explicit stable targets.".into());
    };
    let plan = match plan_tool(
        &app,
        ToolRequest {
            tool_id: operation.tool_id,
            tool_version: 1,
            parameters: operation.parameters,
            targets,
            expected_revision: DocumentRevision(app.doc.automation_revision),
            caller: CallerType::Workflow,
        },
    ) {
        Ok(plan) => plan,
        Err(AutomationError::DiffusionInput { report }) => {
            return Ok(
                serde_json::json!({"error":{"code":"invalid_diffusion_input","report":report}}),
            );
        }
        Err(error) => {
            return Ok(
                serde_json::json!({"error":{"code":"preflight_failed","message":error.to_string()}}),
            );
        }
    };
    eprintln!("plotium-cli: executing {}", plan.request.tool_id);
    let mut result = execute_tool(&mut app, plan, ExecutionAuthority::ReversibleModify)
        .map_err(|e| e.to_string())?;
    if let Some(task_id) = result
        .value
        .get("task")
        .and_then(|task| task.get("task_id"))
        .and_then(serde_json::Value::as_str)
        .map(|id| TaskId(id.into()))
    {
        let mut previous = None;
        loop {
            app.poll_compute();
            let receipt = app
                .session
                .scientific_tasks
                .get(&task_id)
                .ok_or_else(|| "task_not_found; do not automatically resubmit".to_owned())?;
            let progress = (
                receipt.status,
                receipt.phase.clone(),
                receipt.completed,
                receipt.total,
            );
            if previous.as_ref() != Some(&progress) {
                if let (Some(completed), Some(total)) = (receipt.completed, receipt.total) {
                    eprintln!(
                        "plotium-cli: task {} {:?} {} {completed}/{total}",
                        task_id.0, receipt.status, receipt.phase
                    );
                } else {
                    eprintln!(
                        "plotium-cli: task {} {:?} {} (total unknown)",
                        task_id.0, receipt.status, receipt.phase
                    );
                }
                previous = Some(progress);
            }
            if receipt.status.terminal() {
                result.produced = receipt.produced.clone();
                result.modified = receipt.modified.clone();
                result.value = serde_json::json!({"task":receipt});
                result.after_revision = DocumentRevision(app.doc.automation_revision);
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
    if let Some(path) = output {
        plotium_core::project::save_project(&app, &path, false).map_err(|e| e.to_string())?;
    }
    serde_json::to_value(result).map_err(|e| e.to_string())
}
