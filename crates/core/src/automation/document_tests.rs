use super::*;
use crate::PseudoDisplay;
use crate::state::*;
use plotium_data::{ColumnChunk, ColumnSchema, ColumnValues, LogicalType};

pub(crate) fn fixture() -> PlotiumApp {
    let mut app = PlotiumApp::new();
    for offset in [0.0, 5.0] {
        let columns: Vec<_> = ["x", "y_left", "error", "y_right"]
            .iter()
            .map(|name| ColumnSchema::new(*name, LogicalType::Float64))
            .collect();
        let x = columns[0].id;
        let bindings = vec![
            TableSeriesBinding {
                value_column: columns[1].id,
                uncertainty_column: Some(columns[2].id),
                fit: None,
            },
            TableSeriesBinding {
                value_column: columns[3].id,
                uncertainty_column: None,
                fit: None,
            },
        ];
        let data = [
            vec![0.0, 1.0, 2.0],
            vec![1.0 + offset, 2.0 + offset, 4.0 + offset],
            vec![0.1, 0.2, 0.3],
            vec![2.0 + offset, 3.0 + offset, 5.0 + offset],
        ];
        let mut table = TableDataset::from_materialized(
            columns
                .into_iter()
                .zip(data)
                .map(|(c, v)| (c, ColumnChunk::all_valid(ColumnValues::Float64(v))))
                .collect(),
            vec![],
            Some(x),
            bindings,
            "plotium.test.r1",
        )
        .unwrap();
        table.name = Some("Synthetic table".into());
        table.board_sheet_visible = false;
        app.doc.datasets.push(Dataset::Table(Box::new(table)));
    }
    let mut canvas = CanvasDocument::new("R1 synthetic acceptance".into(), [180.0, 90.0]);
    for left in [0.0, 250.0] {
        let id = canvas.allocate_object_id();
        let mut object = app.build_plot_object(
            0,
            ObjectFrame::new(left, 0.0, 235.0, 210.0),
            id,
            "Same plot".into(),
        );
        let plot = object.plot_mut().unwrap();
        let mut second = SeriesBinding::from_dataset(&app.doc.datasets[1]).unwrap();
        second.id = plot.allocate_series_id();
        plot.binding.series.push(second);
        for series in &mut plot.binding.series {
            series.label = Some("Same curve".into());
        }
        canvas.objects.push(object);
        canvas.create_panel_for_plot(id).unwrap();
    }
    let id = canvas.allocate_object_id();
    canvas.objects.push(ContentItem {
        id,
        name: "Loose caption".into(),
        frame: ObjectFrame::new(10.0, 220.0, 200.0, 20.0),
        locked: false,
        visible: true,
        kind: ContentKind::Text(TextBox::label("Synthetic R1 fixture".into())),
    });
    app.doc.canvases.push(canvas);
    app.session.active_canvas = Some(0);
    app.set_active_dataset(Some(0));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while app.compute_busy() && std::time::Instant::now() < deadline {
        app.poll_compute();
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(!app.compute_busy());
    app
}
fn query(
    app: &PlotiumApp,
    target: TargetRef,
    view: DocumentView,
    offset: usize,
    limit: usize,
) -> ToolRequest {
    ToolRequest {
        tool_id: DOCUMENT_INSPECT.into(),
        tool_version: 1,
        parameters: json!(DocumentInspectParams {
            target: target.clone(),
            view,
            offset,
            limit
        }),
        targets: if target.component.is_some() {
            TargetSelector::Components {
                targets: vec![target],
            }
        } else {
            TargetSelector::Explicit {
                ids: vec![target.resource.id],
            }
        },
        expected_revision: DocumentRevision(app.doc.automation_revision),
        caller: CallerType::Agent,
    }
}
fn read(
    app: &mut PlotiumApp,
    target: TargetRef,
    view: DocumentView,
    offset: usize,
    limit: usize,
) -> ToolResult {
    let p = plan_tool(app, query(app, target, view, offset, limit)).unwrap();
    execute_tool(app, p, ExecutionAuthority::Read).unwrap()
}
fn targets(app: &PlotiumApp) -> Vec<TargetRef> {
    app.doc.canvases[0]
        .objects
        .iter()
        .flat_map(|o| app.series_targets(0, o.id))
        .collect()
}
#[test]
fn traverse_same_named_panels_objects_series_and_actual_source_columns() {
    let mut app = fixture();
    let root = read(
        &mut app,
        document_root_target(),
        DocumentView::Children,
        0,
        50,
    );
    let canvas: TargetRef =
        serde_json::from_value(root.value["items"][0]["target"].clone()).unwrap();
    let list = read(&mut app, canvas, DocumentView::Children, 0, 50);
    assert_eq!(list.value["total"], 3);
    assert_eq!(list.value["items"][2]["kind"], "text");
    let mut found = vec![];
    for panel in list.value["items"].as_array().unwrap().iter().take(2) {
        let p: TargetRef = serde_json::from_value(panel["target"].clone()).unwrap();
        let object = read(&mut app, p.clone(), DocumentView::Children, 0, 50);
        assert_eq!(object.value["items"][0]["name"], "Same plot");
        let o: TargetRef =
            serde_json::from_value(object.value["items"][0]["target"].clone()).unwrap();
        let curves = read(&mut app, o.clone(), DocumentView::Children, 0, 50);
        assert_eq!(curves.value["total"], 2);
        for (index, curve) in curves.value["items"].as_array().unwrap().iter().enumerate() {
            let t: TargetRef = serde_json::from_value(curve["target"].clone()).unwrap();
            let detail = read(&mut app, t.clone(), DocumentView::SelfOnly, 0, 50);
            let n = &detail.value["node"];
            assert_eq!(n["ancestors"][2], json!(p));
            assert_eq!(n["ancestors"][3], json!(o));
            assert_eq!(
                n["source"]["dataset"],
                json!(ResourceRef::from(app.doc.datasets[index].resource_id()))
            );
            assert_eq!(
                n["source"]["field"]["local_id"],
                app.doc.datasets[index]
                    .field_descriptor(app.doc.datasets[index].default_field_id().unwrap())
                    .unwrap()
                    .local_id
            );
            assert_eq!(n["source"]["status"], "resolved");
            assert_eq!(n["source"]["binding_mode"], "table_default");
            assert_eq!(n["source"]["per_plot_explicit_binding_supported"], true);
            assert_eq!(n["source"]["series_columns"].as_array().unwrap().len(), 2);
            assert!(!n["source"]["series_columns"][0]["error_column"].is_null());
            let source_id = n["source"]["field"]["id"].as_str().unwrap();
            assert!(
                ProjectResourceProvider::new(&app)
                    .inspect(source_id)
                    .is_some()
            );
            assert!(app.doc.automation_runs.is_empty());
            found.push(t);
        }
    }
    assert_eq!(found, targets(&app));
    app.doc.canvases[0].objects.swap(0, 1);
    let result = read(&mut app, found[0].clone(), DocumentView::SelfOnly, 0, 50);
    assert_eq!(result.value["node"]["target"], json!(found[0]));
}
#[test]
fn invalid_identity_and_stale_revision_never_fall_back() {
    let mut app = fixture();
    let target = targets(&app)[0].clone();
    let mut cases = vec![];
    for key in ["id", "kind", "parent_id", "local_id"] {
        let mut t = target.clone();
        let mut r = json!(t.resource);
        r[key] = json!("forged");
        t.resource = serde_json::from_value(r).unwrap();
        cases.push(t);
    }
    let mut missing = target.clone();
    missing.component = Some(ComponentRef::Series(SeriesId::new(999)));
    cases.push(missing);
    let mut root = document_root_target();
    root.resource.parent_id = Some("forged".into());
    cases.push(root);
    let mut panel = TargetRef::resource(panel_resource(
        &app.doc.canvases[0],
        &app.doc.canvases[0].panels[0],
    ));
    panel.resource.id.push('x');
    cases.push(panel);
    for t in cases {
        assert!(matches!(
            plan_tool(&app, query(&app, t, DocumentView::SelfOnly, 0, 50)),
            Err(AutomationError::InvalidSelector(_))
        ));
    }
    let req = query(&app, target.clone(), DocumentView::SelfOnly, 0, 50);
    let plan = plan_tool(&app, req.clone()).unwrap();
    app.doc.automation_revision += 1;
    assert!(matches!(
        plan_tool(&app, req),
        Err(AutomationError::StaleRevision { .. })
    ));
    assert!(matches!(
        execute_tool(&mut app, plan, ExecutionAuthority::Read),
        Err(AutomationError::StaleRevision { .. })
    ));
    app.doc.canvases[0].objects.remove(0);
    assert!(plan_tool(&app, query(&app, target, DocumentView::SelfOnly, 0, 50)).is_err());
}
#[test]
fn pagination_budgets_and_single_detail_failure() {
    let mut app = fixture();
    let root = document_root_target();
    let first = read(&mut app, root.clone(), DocumentView::Children, 0, 1);
    assert_eq!(first.value["next_offset"], Value::Null);
    let canvas = canvas_target(&app.doc.canvases[0]);
    let first = read(&mut app, canvas.clone(), DocumentView::Children, 0, 1);
    let next = read(&mut app, canvas.clone(), DocumentView::Children, 1, 1);
    assert_ne!(
        first.value["items"][0]["target"],
        next.value["items"][0]["target"]
    );
    assert_eq!(first.value["next_offset"], 1);
    assert!(
        read(&mut app, canvas.clone(), DocumentView::Children, 99, 1).value["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    for limit in [0, 201] {
        assert!(
            plan_tool(
                &app,
                query(&app, root.clone(), DocumentView::Children, 0, limit)
            )
            .is_err()
        );
    }
    let leaf = targets(&app)[0].clone();
    let empty = read(&mut app, leaf, DocumentView::Children, 0, 50);
    assert_eq!(empty.value["total"], 0);
}
#[test]
fn response_budget_shortens_pages_without_skipping_and_rejects_oversized_nodes() {
    let mut app = fixture();
    app.doc.canvases[0].name = "a".repeat(130_000);
    let mut c = app.doc.canvases[0].clone();
    c.resource_id = CanvasId::new();
    app.doc.canvases.push(c);
    let mut c = app.doc.canvases[0].clone();
    c.resource_id = CanvasId::new();
    app.doc.canvases.push(c);
    let first = read(
        &mut app,
        document_root_target(),
        DocumentView::Children,
        0,
        50,
    );
    assert!(serde_json::to_vec(&first).unwrap().len() <= DOCUMENT_RESPONSE_BYTES);
    assert_eq!(first.value["items"].as_array().unwrap().len(), 2);
    assert_eq!(first.value["next_offset"], 2);
    let last = read(
        &mut app,
        document_root_target(),
        DocumentView::Children,
        2,
        50,
    );
    assert_eq!(last.value["items"].as_array().unwrap().len(), 1);
    let target = canvas_target(&app.doc.canvases[0]);
    app.doc.canvases[0].name = "x".repeat(DOCUMENT_RESPONSE_BYTES);
    let p = plan_tool(&app, query(&app, target, DocumentView::SelfOnly, 0, 50)).unwrap();
    assert!(matches!(
        execute_tool(&mut app, p, ExecutionAuthority::Read),
        Err(AutomationError::ResponseTooLarge)
    ));
}
#[test]
fn damaged_sources_are_observable_and_never_retargeted() {
    let mut app = fixture();
    let target = targets(&app)[0].clone();
    app.doc.datasets[0].as_table_mut().unwrap().x_binding = None;
    assert_eq!(
        read(&mut app, target.clone(), DocumentView::SelfOnly, 0, 50).value["node"]["source"]["status"],
        "unbound"
    );
    app.doc.datasets[0].as_table_mut().unwrap().x_binding = Some(plotium_data::ColumnId::new());
    assert_eq!(
        read(&mut app, target.clone(), DocumentView::SelfOnly, 0, 50).value["node"]["source"]["status"],
        "missing_column"
    );
    app.doc.canvases[0].objects[0]
        .plot_mut()
        .unwrap()
        .binding
        .series[0]
        .source
        .field = FieldId::new(999999);
    assert_eq!(
        read(&mut app, target.clone(), DocumentView::SelfOnly, 0, 50).value["node"]["source"]["status"],
        "missing_field"
    );
    app.doc.datasets.remove(0);
    assert_eq!(
        read(&mut app, target, DocumentView::SelfOnly, 0, 50).value["node"]["source"]["status"],
        "missing_dataset"
    );
}
#[test]
#[ignore = "Generate an isolated synthetic GUI acceptance project at PLOTIUM_R1_FIXTURE"]
fn write_r1_acceptance_project() {
    let path = std::env::var_os("PLOTIUM_R1_FIXTURE").expect("explicit output path required");
    let app = fixture();
    crate::project::save_project(&app, std::path::Path::new(&path), false).unwrap();
}

#[test]
fn content_details_and_damaged_item_are_explicit() {
    let mut app = fixture();
    let caption = object_target(&app.doc.canvases[0], &app.doc.canvases[0].objects[2]);
    let text = read(&mut app, caption, DocumentView::SelfOnly, 0, 50);
    assert_eq!(text.value["node"]["text"], "Synthetic R1 fixture");
    assert_eq!(text.value["node"]["frame_space"], "page_pt");
    let target = targets(&app)[0].clone();
    app.doc.canvases[0].objects[0]
        .plot_mut()
        .unwrap()
        .binding
        .series[0]
        .source
        .selection = crate::state::SeriesSelection::TraceItem(plotium_data::TraceItemId::new());
    assert_eq!(
        read(&mut app, target.clone(), DocumentView::SelfOnly, 0, 50).value["node"]["source"]["status"],
        "missing_item"
    );
    let req = query(&app, target, DocumentView::SelfOnly, 0, 50);
    let mut implicit = req;
    implicit.targets = TargetSelector::CurrentSelection;
    assert!(matches!(
        plan_tool(&app, implicit),
        Err(AutomationError::InvalidSelector(_))
    ));
}

fn app_with_live_plot(dataset: Dataset) -> PlotiumApp {
    let mut app = PlotiumApp::new_with_settings(crate::settings::Settings::default());
    app.doc.datasets.push(dataset);
    let canvas = crate::workflow::build_default_canvas(&app.doc.datasets[0], "Live discovery");
    app.doc.canvases.push(canvas);
    app.session.active_canvas = Some(0);
    app
}

fn assert_discovery_matches_property_targets(app: &mut PlotiumApp) -> Vec<TargetRef> {
    let expected = app.series_targets(0, app.doc.canvases[0].objects[0].id);
    assert!(!expected.is_empty());
    let owner = object_target(&app.doc.canvases[0], &app.doc.canvases[0].objects[0]);
    let detail = read(app, owner.clone(), DocumentView::SelfOnly, 0, 50);
    assert_eq!(detail.value["node"]["child_count"], expected.len());
    let mut found = Vec::new();
    for offset in 0..expected.len() {
        let page = read(app, owner.clone(), DocumentView::Children, offset, 1);
        assert_eq!(page.value["total"], expected.len());
        let target: TargetRef =
            serde_json::from_value(page.value["items"][0]["target"].clone()).unwrap();
        assert!(ProjectResourceProvider::new(app).component_exists(&target));
        let detail = read(app, target.clone(), DocumentView::SelfOnly, 0, 50);
        assert_eq!(detail.value["node"]["target"], json!(target));
        let key = if detail.value["node"]["encoding"] == "contour" {
            crate::properties::contour::LINE_WIDTH.as_str()
        } else {
            crate::properties::line::STROKE_WIDTH.as_str()
        };
        for tool in [TOOL_INSPECT, TOOL_SET] {
            let parameters = if tool == TOOL_SET {
                json!({"key":key,"value":1.5})
            } else {
                json!({"key":key})
            };
            let request = ToolRequest {
                tool_id: tool.into(),
                tool_version: 1,
                parameters,
                targets: TargetSelector::Components {
                    targets: vec![target.clone()],
                },
                expected_revision: DocumentRevision(app.doc.automation_revision),
                caller: CallerType::Agent,
            };
            let plan = plan_tool(app, request).unwrap();
            assert_eq!(plan.targets.len(), 1);
            assert_eq!(plan.targets[0].status, TargetCompatibility::Compatible);
            let authority = plan.required_authority;
            execute_tool(app, plan, authority).unwrap();
        }
        found.push(target);
    }
    assert_eq!(found, expected);
    found
}

fn assert_dormant_series_is_not_addressable(app: &PlotiumApp, target: TargetRef) {
    assert!(!ProjectResourceProvider::new(app).component_exists(&target));
    assert!(matches!(
        plan_tool(app, query(app, target, DocumentView::SelfOnly, 0, 50)),
        Err(AutomationError::InvalidSelector(_))
    ));
}

#[test]
fn live_discovery_follows_recording_channel_without_exposing_dormant_series() {
    let recording = ElectrophysiologyDataset::load(plotium_io::ElectrophysiologyData {
        abf_version: "2.9".into(),
        sample_rate_hz: 10_000.,
        channels: ["pA", "mV"]
            .into_iter()
            .map(|unit| plotium_io::RecordedChannel {
                name: unit.into(),
                unit: plotium_io::ElectricalUnit::from_symbol(unit),
            })
            .collect(),
        sweeps: (0..2)
            .map(|i| plotium_io::Sweep {
                start_time_s: i as f64,
                channels: vec![vec![1., 2.], vec![10., 20.]],
                commands: vec![],
            })
            .collect(),
        protocol: None,
        source: "live-discovery.abf".into(),
        import_warnings: vec![],
    });
    let mut app = app_with_live_plot(Dataset::Electrophysiology(Box::new(recording)));
    assert_eq!(
        app.doc.canvases[0].objects[0]
            .plot()
            .unwrap()
            .binding
            .series
            .len(),
        4
    );
    // An authored hidden Series is still active/addressable, unlike a dormant channel.
    app.doc.canvases[0].objects[0]
        .plot_mut()
        .unwrap()
        .binding
        .series[0]
        .visible = false;
    let first = assert_discovery_matches_property_targets(&mut app);
    assert_eq!(first.len(), 2);
    assert_eq!(
        read(&mut app, first[0].clone(), DocumentView::SelfOnly, 0, 50).value["node"]["visible"],
        false
    );
    app.doc.datasets[0]
        .as_electrophysiology_mut()
        .unwrap()
        .selected_channel = 1;
    let second = assert_discovery_matches_property_targets(&mut app);
    assert_eq!(second.len(), 2);
    assert!(second.iter().all(|target| !first.contains(target)));
    for target in &first {
        assert_dormant_series_is_not_addressable(&app, target.clone());
    }
    app.doc.datasets[0]
        .as_electrophysiology_mut()
        .unwrap()
        .selected_channel = 0;
    assert_eq!(assert_discovery_matches_property_targets(&mut app), first);
}

#[test]
fn live_discovery_follows_pseudo_display_and_preserves_external_series() {
    let dim = plotium_io::Dim {
        spectral_width_hz: 4000.,
        observe_freq_mhz: 400.,
        carrier_ppm: 0.,
        nucleus: "1H".into(),
        group_delay: 0.,
    };
    let diffusion = plotium_io::DiffusionMeta {
        gamma: 2.675_222e8,
        delta: 2e-3,
        big_delta: 0.1,
        tau: 0.,
        shape_factor: 1. / 3.,
    };
    let gradients = (0..16).map(|i| 0.02 + i as f64 * 0.02).collect::<Vec<_>>();
    let values = gradients
        .iter()
        .flat_map(|g| {
            let attenuation = (-1.2e-9 * diffusion.b_factor(*g)).exp();
            (0..64).map(move |j| {
                let time = j as f64 / 4000.;
                num_complex::Complex64::from_polar(
                    attenuation * (-time / 0.2).exp(),
                    std::f64::consts::TAU * 400. * time,
                )
            })
        })
        .collect();
    let mut data = crate::nmr_test_support::load_2d(plotium_io::NmrData2D {
        data: values,
        rows: 16,
        cols: 64,
        domain: plotium_io::Domain::Time,
        direct: dim.clone(),
        indirect: dim,
        quad: plotium_io::QuadMode::Complex,
        indirect_conjugate: false,
        experiment: Some("diffusion".into()),
        pseudo_axis: Some(plotium_io::PseudoAxis {
            name: "g".into(),
            kind: plotium_io::PseudoKind::Gradient,
            values: gradients,
            unit: "T/m".into(),
            source: plotium_io::AxisSource::EmbeddedList,
        }),
        diffusion: Some(diffusion),
        nus: None,
        source: "live-discovery-dosy".into(),
    })
    .unwrap();
    assert!(data.build_dosy_map());
    data.display = PseudoDisplay::Stack;
    let mut app = app_with_live_plot(Dataset::Nmr2D(Box::new(data)));
    let mut table_app = fixture();
    let table = table_app.doc.datasets.remove(0);
    let mut external = SeriesBinding::from_dataset(&table).unwrap();
    let plot = app.doc.canvases[0].objects[0].plot_mut().unwrap();
    external.id = plot.allocate_series_id();
    let external_id = external.id;
    plot.binding.series.push(external);
    app.doc.datasets.push(table);
    let stack = assert_discovery_matches_property_targets(&mut app);
    assert!(
        stack
            .iter()
            .any(|t| t.component == Some(ComponentRef::Series(external_id)))
    );
    app.set_pseudo_display(0, PseudoDisplay::DosyMap);
    let map = assert_discovery_matches_property_targets(&mut app);
    assert_eq!(map.len(), 1);
    assert!(map.iter().all(|target| !stack.contains(target)));
    for target in &stack {
        assert_dormant_series_is_not_addressable(&app, target.clone());
    }
    app.set_pseudo_display(0, PseudoDisplay::Stack);
    assert_eq!(assert_discovery_matches_property_targets(&mut app), stack);
}
