//! Host-data acceptance: cargo run -p plotium-core --example nmr_peak_acceptance -- /path/to/ubiquitin
use plotium_core::state::PlotiumApp;
fn main() -> Result<(), String> {
    let root = std::path::PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("Supply an ubiquitin experiment root")?,
    );
    for experiment in [2, 14, 16] {
        let loaded =
            plotium_io::load_path(root.join(experiment.to_string())).map_err(|e| e.to_string())?;
        let (dataset, _) = plotium_core::workflow::dataset_from_acquisition(loaded.acquisition)
            .map_err(|e| e.to_string())?;
        let mut app = PlotiumApp::new();
        let id = dataset.resource_id();
        app.doc.datasets.push(dataset);
        let v = app.doc.datasets[0]
            .as_nmr2d()
            .unwrap()
            .volume
            .as_ref()
            .unwrap();
        let indices = v.processed.shape().map(|n| n / 2);
        let ppm = [
            v.processed.axis_coordinate(0, indices[0])?,
            v.processed.axis_coordinate(1, indices[1])?,
            v.processed.axis_coordinate(2, indices[2])?,
        ];
        let expected = v.processed.signed_value(indices)?;
        let peak = app.add_peak_3d(id, ppm)?;
        app.edit_peak_3d(id, peak, ppm, "host acceptance observation".into())?;
        for axis in 0..3 {
            app.select_nmr_volume_plane(id, axis, 0)?;
            app.locate_peak_3d(id, peak)?;
        }
        app.delete_peak_3d(id, peak)?;
        app.undo();
        let path = std::env::temp_dir().join(format!(
            "plotium-host-peaks-{}-{experiment}.plotium",
            std::process::id()
        ));
        plotium_core::project::save_project(&app, &path, false).map_err(|e| e.to_string())?;
        let reopened = plotium_core::project::load_project(&path).map_err(|e| e.to_string())?;
        std::fs::remove_file(path).map_err(|e| e.to_string())?;
        let peak = &reopened.doc.datasets[0]
            .as_nmr2d()
            .unwrap()
            .volume
            .as_ref()
            .unwrap()
            .peaks
            .marks[0];
        if peak.ppm != ppm
            || peak.intensity != expected
            || peak.note != "host acceptance observation"
        {
            return Err("Peak roundtrip differs".into());
        }
        println!(
            "experiment={experiment} ppm={ppm:?} signed_intensity={expected} navigation=3axes undo=pass project_roundtrip=pass"
        );
    }
    Ok(())
}
