use super::*;

#[test]
fn project_roundtrip_preserves_overlay_binding() {
    let mut app = PlotiumApp::new();
    for _ in 0..2 {
        app.doc.datasets.push(Dataset::Nmr(Box::new(
            NmrDataset::load(synthetic_1d()).unwrap(),
        )));
    }
    app.doc.datasets[1].set_name(Some("treatment".to_owned()));
    let mut canvas = CanvasDocument::new("overlay".to_owned(), [120.0, 80.0]);
    let [w, h] = canvas.size_pt();
    let id = canvas.allocate_object_id();
    let mut object =
        app.build_plot_object(0, ObjectFrame::new(0.0, 0.0, w, h), id, "Plot 1".to_owned());
    object.plot_mut().unwrap().binding = crate::state::DataBinding {
        coordinates: None,
        series: vec![
            crate::state::SeriesBinding::from_dataset(&app.doc.datasets[0]).unwrap(),
            {
                let mut series =
                    crate::state::SeriesBinding::from_dataset(&app.doc.datasets[1]).unwrap();
                series.id = crate::state::SeriesId::new(1);
                series.set_primary_color(Color::rgb(10, 20, 30));
                series.label = Some("treated".to_owned());
                series
            },
        ],
    };
    canvas.objects.push(object);
    app.doc.canvases.push(canvas);
    app.focus_single(0);
    app.session.active_canvas = Some(0);

    let path = temp_project("overlay");
    let _ = std::fs::remove_file(&path);
    save_project(&app, &path, false).unwrap();
    let loaded = load_project(&path).unwrap();
    let _ = std::fs::remove_file(&path);

    let binding = &first_plot(&loaded).binding;
    assert_eq!(binding.series.len(), 2);
    assert_eq!(
        binding.series[0].source.resource,
        loaded.doc.datasets[0].resource_id()
    );
    assert_eq!(
        binding.series[1].source.resource,
        loaded.doc.datasets[1].resource_id()
    );
    assert_eq!(
        binding.series[1].primary_color(),
        Some(Color::rgb(10, 20, 30))
    );
    assert_eq!(binding.series[1].label.as_deref(), Some("treated"));
    assert_eq!(first_plot(&loaded).figure().series.len(), 2);
}

#[test]
fn project_roundtrip_preserves_stack_spec_and_series_fields() {
    let mut app = PlotiumApp::new();
    for _ in 0..2 {
        app.doc.datasets.push(Dataset::Nmr(Box::new(
            NmrDataset::load(synthetic_1d()).unwrap(),
        )));
    }
    let mut canvas = CanvasDocument::new("stack".to_owned(), [120.0, 80.0]);
    let [w, h] = canvas.size_pt();
    let id = canvas.allocate_object_id();
    let mut object =
        app.build_plot_object(0, ObjectFrame::new(0.0, 0.0, w, h), id, "Plot 1".to_owned());
    {
        let plot = object.plot_mut().unwrap();
        plot.binding = DataBinding {
            coordinates: None,
            series: vec![
                SeriesBinding::from_dataset(&app.doc.datasets[0]).unwrap(),
                {
                    let mut series = SeriesBinding::from_dataset(&app.doc.datasets[1]).unwrap();
                    series.id = crate::state::SeriesId::new(1);
                    if let plotium_figure::SeriesEncoding::Line(line) = &mut series.encoding {
                        line.scale = 2.5;
                    }
                    series.visible = false;
                    series
                },
            ],
        };
        plot.stack = StackSpec {
            mode: StackMode::Offset,
            spacing_y: 0.3,
            shear_x: 0.1,
            normalize: true,
            active: Some(1),
            occlude: true,
            gradient: Some(plotium_figure::ColormapId::Plasma),
            sequence_scale: true,
        };
    }
    canvas.objects.push(object);
    app.doc.canvases.push(canvas);
    app.focus_single(0);
    app.session.active_canvas = Some(0);

    let path = temp_project("stackspec");
    let _ = std::fs::remove_file(&path);
    save_project(&app, &path, false).unwrap();
    let loaded = load_project(&path).unwrap();
    let _ = std::fs::remove_file(&path);

    let plot = first_plot(&loaded);
    assert_eq!(plot.stack.mode, StackMode::Offset);
    assert_eq!(plot.stack.spacing_y, 0.3);
    assert_eq!(plot.stack.shear_x, 0.1);
    assert!(plot.stack.occlude && plot.stack.sequence_scale);
    assert_eq!(
        plot.stack.gradient,
        Some(plotium_figure::ColormapId::Plasma)
    );
    assert!(plot.stack.normalize);
    assert_eq!(plot.stack.active, Some(1));
    assert_eq!(plot.binding.series[1].line_scale(), 2.5);
    assert!(!plot.binding.series[1].visible);
}
