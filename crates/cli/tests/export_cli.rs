use serde_json::Value;
use std::path::Path;
use std::process::{Command, Output};

fn project_with_one_canvas(root: &Path) -> std::path::PathBuf {
    let input = root.join("input.dx");
    std::fs::write(
        &input,
        "##TITLE=fixture\n\
         ##JCAMP-DX=5.01\n\
         ##DATA TYPE=NMR SPECTRUM\n\
         ##XUNITS=PPM\n\
         ##YUNITS=ARBITRARY UNITS\n\
         ##XFACTOR=1\n\
         ##YFACTOR=1\n\
         ##FIRSTX=0\n\
         ##LASTX=3\n\
         ##NPOINTS=4\n\
         ##.OBSERVE FREQUENCY=400\n\
         ##.OBSERVE NUCLEUS=^1H\n\
         ##XYDATA=(X++(Y..Y))\n\
         0 1 2 3 4\n\
         ##END=\n",
    )
    .unwrap();
    let loaded = plotium_core::workflow::load_dataset(&input).unwrap();
    let mut canvas = loaded.default_canvas();
    canvas.name = "Spectrum".to_owned();
    let mut app = plotium_core::state::PlotiumApp::new_with_settings(Default::default());
    app.doc.datasets.push(loaded.dataset);
    app.doc.canvases.push(canvas);
    let project = root.join("figures.plotium");
    plotium_core::project::save_project(&app, &project, false).unwrap();
    project
}

fn export(project: &Path, out: &Path, extra: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_plotium-cli"))
        .arg("export")
        .arg(project)
        .arg("--output-dir")
        .arg(out)
        .args(["--format", "svg"])
        .args(extra)
        .output()
        .unwrap()
}

#[test]
fn export_writes_each_canvas_and_refuses_to_overwrite_by_default() {
    let root = tempfile::tempdir().unwrap();
    let project = project_with_one_canvas(root.path());
    let saved = std::fs::read(&project).unwrap();
    let out = root.path().join("figures");

    let first = export(&project, &out, &[]);
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let result: Value = serde_json::from_slice(&first.stdout).unwrap();
    assert_eq!(result["tool_id"], "figure.export");
    assert_eq!(result["targets"][0]["outcome"], "succeeded");
    assert!(out.join("Spectrum.svg").is_file());

    let second = export(&project, &out, &[]);
    assert_eq!(second.status.code(), Some(6));
    let result: Value = serde_json::from_slice(&second.stdout).unwrap();
    assert_eq!(result["targets"][0]["outcome"], "failed");

    let replaced = export(&project, &out, &["--overwrite", "--canvas", "Spectrum"]);
    assert!(
        replaced.status.success(),
        "{}",
        String::from_utf8_lossy(&replaced.stderr)
    );

    let unknown = export(&project, &out, &["--canvas", "Missing"]);
    assert_eq!(unknown.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&unknown.stderr).contains("unknown canvas: Missing"));

    assert_eq!(std::fs::read(&project).unwrap(), saved);
}
