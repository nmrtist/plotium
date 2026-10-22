//! Reproducible synthetic native project for exact-deletion MCP/GUI acceptance.
//! Usage: cargo run -p plotium-core --example deletion_refinement_fixture -- /new/output/dir
use plotium_core::{automation::*, data::*, state::*};
use serde_json::json;
use std::path::PathBuf;

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let out = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("Supply a new output directory")?,
    );
    std::fs::create_dir(&out)?;
    let out = out.canonicalize()?;
    let mut app = PlotiumApp::new();
    let mut columns = Vec::new();
    for (name, unit, values) in [
        ("time", "s", vec![0., 0.5, 1., 1.5, 2., 2.5, 3.]),
        ("signal_1", "mA", vec![1., 1.5, 1.3, 2., 2.4, 2.1, 2.7]),
        ("signal_2", "mA", vec![1.5, 2., 2.4, 2.1, 2.8, 3.1, 3.4]),
        ("signal_3", "mA", vec![2., 2.5, 2.3, 3., 3.4, 3.1, 3.7]),
        ("signal_4", "mA", vec![2.5, 3., 3.4, 3.1, 3.8, 4.1, 4.4]),
        ("signal_5", "mA", vec![3., 3.5, 3.3, 4., 4.4, 4.1, 4.7]),
        ("signal_6", "mA", vec![3.5, 4., 4.4, 4.1, 4.8, 5.1, 5.4]),
        ("error", "mA", vec![0.15; 7]),
    ] {
        let mut schema = ColumnSchema::new(name, LogicalType::Float64);
        schema.unit = Some(UnitRegistry::plotium_v1().resolve(unit)?);
        columns.push((
            schema,
            ColumnChunk::optional_f64(values.into_iter().map(Some)),
        ));
    }
    let mut table = TableDataset::from_materialized(
        columns,
        vec![],
        None,
        vec![],
        "plotium.test.content_refinement",
    )?;
    table.name = Some("Synthetic signals".into());
    table.board_sheet_visible = false;
    app.doc.datasets.push(Dataset::Table(Box::new(table)));
    app.doc.canvases.push(CanvasDocument::new(
        "Content refinement".into(),
        [180., 90.],
    ));
    let canvas = TargetRef::resource(app.doc.canvases[0].resource_id.into());
    for (i, frame) in [[15., 15., 225., 220.], [270., 15., 225., 220.]]
        .into_iter()
        .enumerate()
    {
        let table = app.doc.datasets[0].as_table().ok_or("Missing table")?;
        let r = &table.typed_state.envelope.revision;
        let columns = &r.snapshot.schema.columns;
        let series = (1..=if i == 0 { 6 } else { 2 })
            .map(|j| NewCurve {
                y_axis: Default::default(),
                kind: if j % 2 == 0 {
                    CurveKind::Scatter
                } else {
                    CurveKind::Line
                },
                source: TableCurveSource {
                    dataset_id: table.resource_id,
                    field_id: app.doc.datasets[0].default_field_id().expect("table field"),
                    table_id: r.table_id,
                    table_revision: r.id,
                    x_column_id: columns[0].id,
                    y_column_id: columns[j].id,
                    y_error_column_id: (j == 1).then_some(columns[7].id),
                },
            })
            .collect();
        let p = app.plan_plot_creation(
            DocumentRevision(app.doc.automation_revision),
            &PlotCreateParams {
                parent: canvas.clone(),
                name: "Signal".into(),
                canvas: None,
                panel_name: Some(format!("Panel {}", i + 1)),
                panel_frame_pt: Some(frame),
                series,
            },
        )?;
        app.commit_curve_bindings(p)?;
    }
    for i in 0..2 {
        let page = &mut app.doc.canvases[0];
        let id = page.objects[i].id;
        page.set_content_frame(id, ObjectFrame::new(0., 0., 225., 170.))?;
        let plot = page.objects[i].plot_mut().ok_or("Missing plot")?;
        plot.axis_overrides.x_range = Some(AxisRange::new(0., 3.));
        plot.axis_overrides.y_range = Some(AxisRange::new(0., if i == 0 { 6. } else { 4. }));
        for (j, series) in plot.binding.series.iter_mut().enumerate() {
            if let plotium_figure::SeriesEncoding::Line(line) = &mut series.encoding {
                let palette = [
                    (0, 114, 178),
                    (230, 159, 0),
                    (0, 158, 115),
                    (204, 121, 167),
                    (213, 94, 0),
                    (86, 180, 233),
                ];
                let (r, g, b) = palette[j];
                line.color =
                    plotium_figure::ColorSource::Explicit(plotium_figure::Color::rgb(r, g, b));
                line.width =
                    plotium_figure::PositiveFiniteF32::new(0.8).expect("positive fixture width");
                line.appearance.marker_size = plotium_figure::PositiveFiniteF32::new(3.5);
            }
            series.label = Some(if j < 2 {
                "Repeated label".into()
            } else {
                format!("Signal {}", j + 1)
            });
        }
    }
    let source = out.join("original.png");
    let replacement = out.join("replacement.jpg");
    image::RgbImage::from_fn(160, 80, |x, y| {
        image::Rgb(if (x / 40 + y / 40) % 2 == 0 {
            [20, 100, 180]
        } else {
            [160, 210, 240]
        })
    })
    .save(&source)?;
    image::RgbImage::from_fn(80, 160, |x, y| {
        image::Rgb(if (x / 20 + y / 20) % 2 == 0 {
            [220, 90, 20]
        } else {
            [255, 210, 130]
        })
    })
    .save(&replacement)?;
    let panel = TargetRef::resource(panel_resource(
        &app.doc.canvases[0],
        &app.doc.canvases[0].panels[0],
    ));
    let params: FigureEditParams = serde_json::from_value(json!({"canvas":canvas,"edits":[
        {"op":"text_create","parent":panel,"name":"Duplicate note to delete","frame_pt":[5,178,100,18],"text":"Duplicate annotation: remove only this note.","font_pt":9},
        {"op":"image_create","parent":panel,"name":"Shared image to delete","frame_pt":[5,200,50,25],"path":source},
        {"op":"image_create","parent":canvas,"name":"Shared image to retain","frame_pt":[275,215,50,25],"path":source},
        {"op":"text_create","parent":canvas,"name":"Loose note","frame_pt":[340,215,150,20],"text":"Keep this note and the blue image.","font_pt":9}
    ]}))?;
    let p = app.plan_figure_edit(DocumentRevision(app.doc.automation_revision), &params)?;
    app.commit_figure_edit(p)?;
    let page = &mut app.doc.canvases[0];
    let id = page.allocate_object_id();
    page.objects.push(ContentItem {
        id,
        name: "Intentional overlap marker".into(),
        frame: ObjectFrame::new(225., 155., 25., 15.),
        locked: false,
        visible: true,
        kind: ContentKind::Shape(ShapeObject::new(ShapeKind::Ellipse)),
    });
    let page = &mut app.doc.canvases[0];
    page.create_panel(
        "Empty panel to delete".into(),
        ObjectFrame::new(460., 240., 30., 10.),
    );
    page.groups.push(LayoutGroup {
        id: 1,
        members: page.panels[..2]
            .iter()
            .map(|p| GroupMember::Panel(p.id))
            .collect(),
    });
    page.next_group_id = 2;
    page.x_viewport_links.push(XViewportLinkGroup {
        id: XViewportLinkId::new(),
        members: page
            .objects
            .iter()
            .filter(|o| o.plot().is_some())
            .map(|o| o.id)
            .collect(),
    });
    app.rebuild_canvas(0);
    app.session.active_canvas = Some(0);
    app.clear_history();
    let project = out.join("before.plotium");
    plotium_core::project::save_project(&app, &project, false)?;
    for format in [
        plotium_core::export::ExportFormat::Svg,
        plotium_core::export::ExportFormat::Pdf,
        plotium_core::export::ExportFormat::Png,
    ] {
        let settings = plotium_core::export::ExportSettings {
            format,
            scope: plotium_core::export::ExportPageScope::All,
            dpi: 144,
            target_width_mm: None,
            trim_to_visible_content: false,
            allow_missing_images: false,
        };
        plotium_core::export::export_canvases_with_assets(
            &app.doc.canvases,
            &app.doc.assets,
            Some(0),
            &settings,
            &out.join("before"),
        )?;
    }
    println!(
        "Open {}. Delete the duplicate note, the first shared image, the first of the left plot's two same-name Series, the right plot, and the originally empty panel. Keep both plot parent panels, the other shared image, remaining curves, manual ranges, source data and intentional marker.",
        project.display()
    );
    Ok(())
}
