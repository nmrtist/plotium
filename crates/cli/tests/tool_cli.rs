use serde_json::Value;
use std::path::PathBuf;
use std::process::Command;

fn temp_dir(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("plotium-cli-tool-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    root
}

#[test]
fn tool_run_without_input_imports_a_csv_into_a_new_project() {
    let root = temp_dir("csv");
    let csv = root.join("curve.csv");
    std::fs::write(&csv, "x,y\n1,2\n2,4\n3,9\n").unwrap();
    let request = root.join("import.json");
    std::fs::write(
        &request,
        serde_json::json!({
            "tool_id": "data.import",
            "parameters": {"mode": "table_data_only", "paths": [csv]}
        })
        .to_string(),
    )
    .unwrap();
    let project = root.join("new.plotium");

    let output = Command::new(env!("CARGO_BIN_EXE_plotium-cli"))
        .args(["tool", "run", "--request"])
        .arg(&request)
        .arg("--output")
        .arg(&project)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["produced"][0]["kind"], "plotium.dataset");
    let app = plotium_core::project::load_project(&project).unwrap();
    assert_eq!(app.doc.datasets.len(), 1);
}

#[test]
fn tool_run_with_a_csv_input_points_to_data_import() {
    let root = temp_dir("hint");
    let csv = root.join("curve.csv");
    std::fs::write(&csv, "x,y\n1,2\n").unwrap();
    let request = root.join("op.json");
    std::fs::write(&request, r#"{"tool_id":"data.import","parameters":{}}"#).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_plotium-cli"))
        .args(["tool", "run"])
        .arg(&csv)
        .arg("--request")
        .arg(&request)
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("omit the input"));
}

#[test]
fn tool_run_without_input_requires_targets_for_dataset_tools() {
    let root = temp_dir("targets");
    let request = root.join("op.json");
    std::fs::write(&request, r#"{"tool_id":"processing.inspect"}"#).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_plotium-cli"))
        .args(["tool", "run", "--request"])
        .arg(&request)
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("explicit stable targets"));
}
