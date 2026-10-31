use super::*;
use crate::automation::*;
use serde_json::json;
fn fixture() -> PlotiumApp {
    let mut a = super::curve_tests::fixture(false);
    let p = &mut a.doc.canvases[0];
    let id = p.allocate_object_id();
    let mut o = p.objects[1].clone();
    o.id = id;
    o.name = "c".into();
    p.objects.push(o);
    p.create_panel_for_plot(id).unwrap();
    let ids = p.panels.iter().map(|p| p.id).collect::<Vec<_>>();
    for (id, x) in ids.into_iter().zip([5., 65., 135.]) {
        let child = p.panel(id).unwrap().item_order[0];
        p.set_layout_frame(
            child,
            ObjectFrame::new(x * MM_TO_PT, 4. * MM_TO_PT, 40. * MM_TO_PT, 71. * MM_TO_PT),
        );
    }
    a.clear_history();
    a
}
fn targets(a: &PlotiumApp) -> Vec<TargetRef> {
    let p = &a.doc.canvases[0];
    p.panels
        .iter()
        .map(|x| TargetRef::resource(panel_resource(p, x)))
        .collect()
}
fn plan(a: &PlotiumApp, edits: serde_json::Value) -> FigureEditPlan {
    a.plan_figure_edit(DocumentRevision(a.doc.automation_revision),&serde_json::from_value(json!({"canvas":TargetRef::resource(a.doc.canvases[0].resource_id.into()),"edits":edits})).unwrap()).unwrap()
}
#[test]
fn figure_subset_grid_scales_children_preserves_other_panel_and_is_one_undo() {
    let mut a = fixture();
    let t = targets(&a);
    let before = a.doc.canvases[0].clone();
    let p = plan(
        &a,
        json!([{"op":"page","margin_mm":[4,4,15,4],"gutter_mm":5},{"op":"grid","panels":[t[1],t[2]],"rows":1,"cols":2}]),
    );
    a.commit_figure_edit(p).unwrap();
    let c = &a.doc.canvases[0];
    assert_eq!(c.panels[0], before.panels[0]);
    assert_eq!(c.objects[0].frame, before.objects[0].frame);
    for (i, x) in [(1, 4.), (2, 92.5)] {
        let f = c.panels[i].frame;
        assert!((f.x / MM_TO_PT - x).abs() < 1e-4);
        assert!((f.width / MM_TO_PT - 83.5).abs() < 1e-4);
        assert!((c.objects[i].frame.width - f.width).abs() < 1e-3);
        assert_eq!(
            c.objects[i].plot().unwrap().binding,
            before.objects[i].plot().unwrap().binding
        );
    }
    let after = c.panels.clone();
    a.undo();
    assert_eq!(a.doc.canvases[0].panels, before.panels);
    assert_eq!(a.doc.canvases[0].layout, before.layout);
    assert!(!a.can_undo());
    a.redo();
    assert_eq!(a.doc.canvases[0].panels, after);
}
#[test]
fn figure_distribution_and_alignment_reuse_native_centres_and_keep_sizes() {
    let mut a = fixture();
    let t = targets(&a);
    let p = plan(
        &a,
        json!([{"op":"distribute","panels":t,"axis":"horizontal"}]),
    );
    a.commit_figure_edit(p).unwrap();
    let c = &a.doc.canvases[0];
    for (p, x) in c.panels.iter().zip([5., 70., 135.]) {
        assert!((p.frame.x / MM_TO_PT - x).abs() < 1e-4);
        assert!((p.frame.width / MM_TO_PT - 40.).abs() < 1e-4);
    }
    a.undo();
    assert!((a.doc.canvases[0].panels[1].frame.x / MM_TO_PT - 65.).abs() < 1e-4);
    a.redo();
    let p = plan(
        &a,
        json!([{"op":"panel_frame","target":t[1],"frame_pt":[100,30,100,100]},{"op":"align","panels":[t[0],t[1]],"edge":"top"}]),
    );
    a.commit_figure_edit(p).unwrap();
    assert_eq!(
        a.doc.canvases[0].panels[0].frame.y,
        a.doc.canvases[0].panels[1].frame.y
    );
}
#[test]
fn figure_bad_batch_forgery_duplicates_locks_invalid_dimensions_and_missing_assets_are_atomic() {
    let mut a = fixture();
    let t = targets(&a);
    let canvas = TargetRef::resource(a.doc.canvases[0].resource_id.into());
    let cases = vec![
        json!({"op":"panel_frame","target":t[0],"frame_pt":[0,0,0,1]}),
        json!({"op":"page","size_mm":[0,90]}),
        json!({"op":"grid","panels":[t[0],t[0]],"rows":1,"cols":2}),
        json!({"op":"grid","panels":t,"rows":1,"cols":2}),
        json!({"op":"image_create","parent":canvas,"name":"missing","frame_pt":[0,0,10,10],"path":"/tmp/plotium-r7-definitely-missing.png"}),
    ];
    for bad in cases {
        let p: FigureEditParams = serde_json::from_value(
            json!({"canvas":canvas,"edits":[{"op":"page","size_mm":[190,90]},bad]}),
        )
        .unwrap();
        assert!(
            a.plan_figure_edit(DocumentRevision(a.doc.automation_revision), &p)
                .is_err()
        );
        assert_eq!(a.doc.canvases[0].size_mm, [180., 90.]);
        assert!(!a.can_undo());
    }
    a.doc.canvases[0].panels[0].locked = true;
    let p: FigureEditParams = serde_json::from_value(
        json!({"canvas":canvas,"edits":[{"op":"align","panels":[t[0],t[1]],"edge":"left"}]}),
    )
    .unwrap();
    assert!(
        a.plan_figure_edit(DocumentRevision(a.doc.automation_revision), &p)
            .is_err()
    );
    let mut forged = t[0].clone();
    forged.resource.parent_id = Some("forged".into());
    let p:FigureEditParams=serde_json::from_value(json!({"canvas":canvas,"edits":[{"op":"panel_frame","target":forged,"frame_pt":[0,0,1,1]}]})).unwrap();
    assert!(
        a.plan_figure_edit(DocumentRevision(a.doc.automation_revision), &p)
            .is_err()
    );
}
#[test]
fn figure_native_text_and_labels_keep_ids_across_undo_and_reject_wire_reconstruction() {
    let mut a = fixture();
    let t = targets(&a);
    let canvas = TargetRef::resource(a.doc.canvases[0].resource_id.into());
    let p = plan(
        &a,
        json!([{"op":"panel_label","target":t[0],"text":"a","visible":true},{"op":"text_create","parent":canvas,"name":"caption","frame_pt":[10,220,200,20],"text":"Native caption","font_pt":12}]),
    );
    let wire = serde_json::to_value(&p).unwrap();
    let untrusted: FigureEditPlan = serde_json::from_value(wire).unwrap();
    assert!(a.commit_figure_edit(untrusted).is_err());
    let created = p.created[0].clone();
    a.commit_figure_edit(p).unwrap();
    let count = a.doc.canvases[0].objects.len();
    a.undo();
    assert_eq!(a.doc.canvases[0].objects.len(), count - 1);
    a.redo();
    let o = a.doc.canvases[0].objects.last().unwrap();
    assert_eq!(
        created,
        TargetRef::resource(canvas_object_ref(a.doc.canvases[0].resource_id, o.id))
    );
    let p = plan(
        &a,
        json!([{"op":"text_set","target":created,"text":"Edited","font_pt":14}]),
    );
    a.commit_figure_edit(p).unwrap();
    let ContentKind::Text(text) = &a.doc.canvases[0].objects.last().unwrap().kind else {
        panic!()
    };
    assert_eq!(text.text, "Edited");
}
#[test]
fn figure_svg_assets_are_normalized_frozen_and_survive_save() {
    let mut a = fixture();
    let canvas = TargetRef::resource(a.doc.canvases[0].resource_id.into());
    let dir = std::env::temp_dir().join(format!("plotium-svg-images-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&dir).unwrap();
    let svg = dir.join("structure.svg");
    std::fs::write(
        &svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="40pt" height="20pt"><!-- source --><path d="M1 1 L39 19" stroke="black"/></svg>"#,
    )
    .unwrap();
    let p = plan(
        &a,
        json!([{"op":"image_create","parent":canvas,"name":"structure","frame_pt":[10,220,40,20],"path":svg}]),
    );
    std::fs::remove_file(&svg).unwrap();
    a.commit_figure_edit(p).unwrap();
    let asset = a.doc.assets.values().next().unwrap();
    assert!(asset.is_vector());
    assert_eq!(asset.pixel_size, [54, 27]);
    assert!(!String::from_utf8_lossy(&asset.bytes).contains("source"));
    let project = dir.join("structure.plotium");
    crate::project::save_project(&a, &project, false).unwrap();
    let loaded = crate::project::load_project(&project).unwrap();
    assert_eq!(loaded.doc.assets, a.doc.assets);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn figure_png_jpeg_assets_are_frozen_deduplicated_and_undo_redo_retains_ids() {
    let mut a = fixture();
    let canvas = TargetRef::resource(a.doc.canvases[0].resource_id.into());
    let dir = std::env::temp_dir().join(format!("plotium-r7-images-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&dir).unwrap();
    let png = dir.join("source.png");
    let jpg = dir.join("source.jpg");
    let image = image::RgbImage::from_pixel(8, 4, image::Rgb([20, 100, 200]));
    image.save(&png).unwrap();
    image.save(&jpg).unwrap();
    let p = plan(
        &a,
        json!([{"op":"image_create","parent":canvas,"name":"png","frame_pt":[10,220,20,10],"path":png},{"op":"image_create","parent":canvas,"name":"same png","frame_pt":[40,220,20,10],"path":png},{"op":"image_create","parent":canvas,"name":"jpeg","frame_pt":[70,220,20,10],"path":jpg}]),
    );
    let created = p.created.clone();
    std::fs::write(&png, b"changed after preflight").unwrap();
    std::fs::remove_file(&jpg).unwrap();
    a.commit_figure_edit(p).unwrap();
    assert_eq!(a.doc.assets.len(), 2);
    assert!(
        a.doc
            .assets
            .values()
            .all(|a| matches!(a.format.as_str(), "png" | "jpeg"))
    );
    let project = dir.join("images.plotium");
    crate::project::save_project(&a, &project, false).unwrap();
    let loaded = crate::project::load_project(&project).unwrap();
    assert_eq!(loaded.doc.assets, a.doc.assets);
    assert_eq!(
        loaded.doc.canvases[0].objects.len(),
        a.doc.canvases[0].objects.len()
    );
    let ids = a.doc.assets.keys().copied().collect::<Vec<_>>();
    let high = a.doc.canvases[0].next_object_id;
    a.undo();
    assert!(a.doc.assets.is_empty());
    assert_eq!(a.doc.canvases[0].next_object_id, high);
    a.redo();
    assert_eq!(a.doc.assets.keys().copied().collect::<Vec<_>>(), ids);
    for t in created {
        assert!(
            a.doc.canvases[0].objects.iter().any(|o| t
                == TargetRef::resource(canvas_object_ref(a.doc.canvases[0].resource_id, o.id)))
        );
    }
    a.undo();
    let p = plan(
        &a,
        json!([{"op":"text_create","parent":canvas,"name":"later","frame_pt":[10,220,20,10],"text":"later","font_pt":12}]),
    );
    assert_eq!(p.created[0].resource.local_id, Some(high.to_string()));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn figure_exact_geometry_rejects_nonfinite_auto_height_and_native_grid_clamping() {
    let mut a = fixture();
    let t = targets(&a);
    let canvas = TargetRef::resource(a.doc.canvases[0].resource_id.into());
    for n in [f32::NAN, f32::INFINITY, -1., 0.] {
        let p = FigureEditParams {
            canvas: canvas.clone(),
            edits: vec![FigureEdit::PanelFrame {
                target: t[0].clone(),
                frame_pt: [0., 0., n, 100.],
            }],
        };
        assert!(
            a.plan_figure_edit(DocumentRevision(a.doc.automation_revision), &p)
                .is_err()
        );
    }
    let p = FigureEditParams {
        canvas: canvas.clone(),
        edits: vec![
            FigureEdit::Page {
                size_mm: Some([180., 90.]),
                size_preset: None,
                margin_mm: Some([4., 89.99, 15., 89.99]),
                gutter_mm: Some(0.),
            },
            FigureEdit::Grid {
                panels: vec![t[0].clone()],
                rows: 1,
                cols: 1,
            },
        ],
    };
    assert!(
        a.plan_figure_edit(DocumentRevision(a.doc.automation_revision), &p)
            .unwrap_err()
            .contains("1 pt")
    );
    a.doc.canvases[0].auto_height = true;
    let p = FigureEditParams {
        canvas,
        edits: vec![FigureEdit::PanelFrame {
            target: t[0].clone(),
            frame_pt: [0., 0., 100., 100.],
        }],
    };
    assert!(
        a.plan_figure_edit(DocumentRevision(a.doc.automation_revision), &p)
            .unwrap_err()
            .contains("Auto height")
    );
}

#[test]
fn page_size_preset_sets_journal_width_identity_and_margins_as_one_undo() {
    let mut a = fixture();
    a.doc.canvases[0].layout.margin_mm = [3.; 4];
    let before = a.doc.canvases[0].clone();
    let p = plan(&a, json!([{"op":"page","size_preset":"science-3col"}]));
    assert_eq!(p.result["size_mm"], json!([183., before.size_mm[1]]));
    assert_eq!(p.result["size_preset"], json!("science-3col"));
    a.commit_figure_edit(p).unwrap();
    let c = &a.doc.canvases[0];
    assert_eq!(c.size_mm, [183., before.size_mm[1]]);
    assert_eq!(c.size_preset_id.as_deref(), Some("science-3col"));
    assert_eq!(c.layout.margin_mm, [0.; 4]);
    a.undo();
    let c = &a.doc.canvases[0];
    assert_eq!((c.size_mm, c.layout), (before.size_mm, before.layout));
    assert_eq!(c.size_preset_id, before.size_preset_id);
    let p = plan(
        &a,
        json!([{"op":"page","size_preset":"paper-a4","margin_mm":[5,5,5,5]}]),
    );
    assert_eq!(p.result["size_mm"], json!([210., 297.]));
    assert_eq!(p.result["margin_mm"], json!([5., 5., 5., 5.]));
    let canvas = TargetRef::resource(a.doc.canvases[0].resource_id.into());
    for (edit, message) in [
        (
            json!({"op":"page","size_preset":"nature-9col"}),
            "Unknown size preset",
        ),
        (
            json!({"op":"page","size_preset":"nature-1col","size_mm":[89,60]}),
            "not both",
        ),
    ] {
        let p = serde_json::from_value(json!({"canvas":canvas,"edits":[edit]})).unwrap();
        let error = a
            .plan_figure_edit(DocumentRevision(a.doc.automation_revision), &p)
            .unwrap_err();
        assert!(error.contains(message), "{error}");
    }
}

fn materialize_default(a: &mut PlotiumApp, object: ObjectId) -> TargetRef {
    let series = a.doc.canvases[0]
        .object(object)
        .unwrap()
        .plot()
        .unwrap()
        .binding
        .series[0]
        .id;
    let table = a.doc.datasets[0].as_table().unwrap();
    let edit = MaterializeCurveEdit {
        target: a.series_target(0, object, series).unwrap(),
        retain_y_column_id: table.series_bindings[0].value_column,
        table_revision: table.typed_state.envelope.revision.id,
    };
    let p = a
        .plan_materialize_curves(DocumentRevision(a.doc.automation_revision), &[edit])
        .unwrap();
    a.commit_curve_bindings(p).unwrap();
    let created = a.doc.canvases[0]
        .object(object)
        .unwrap()
        .plot()
        .unwrap()
        .binding
        .series[1]
        .id;
    a.series_target(0, object, created).unwrap()
}

fn figure_history_keeps_retired_series_invalid(redo_figure: bool) {
    let mut a = super::curve_tests::fixture(true);
    let object = a.doc.canvases[0].objects[0].id;
    let other = a.doc.canvases[0].objects[1].id;
    let other_before = a.doc.canvases[0]
        .object(other)
        .unwrap()
        .plot()
        .unwrap()
        .next_series_id;
    let p = plan(&a, json!([{"op":"page","margin_mm":[4,4,15,4]}]));
    a.commit_figure_edit(p).unwrap();
    let retired = materialize_default(&mut a, object);
    assert!(a.curve_target(&retired).is_ok());
    let high = a.doc.canvases[0]
        .object(object)
        .unwrap()
        .plot()
        .unwrap()
        .next_series_id;
    a.undo(); // Retire the newly materialized curve, preserving its allocator.
    assert!(a.curve_target(&retired).is_err());
    a.undo(); // Restore the older figure snapshot, without reviving retired identities.
    if redo_figure {
        a.redo();
    }
    assert_eq!(
        a.doc.canvases[0]
            .object(object)
            .unwrap()
            .plot()
            .unwrap()
            .next_series_id,
        high
    );
    let created = materialize_default(&mut a, object);
    assert_ne!(created, retired);
    assert!(a.curve_target(&created).is_ok());
    assert!(a.curve_target(&retired).is_err());
    a.undo();
    a.redo();
    assert!(a.curve_target(&created).is_ok());
    assert!(a.curve_target(&retired).is_err());
    assert_eq!(
        a.doc.canvases[0]
            .object(other)
            .unwrap()
            .plot()
            .unwrap()
            .next_series_id,
        other_before
    );
}

#[test]
fn figure_undo_preserves_series_high_water_and_rejects_retired_target() {
    figure_history_keeps_retired_series_invalid(false);
}

#[test]
fn figure_redo_preserves_series_high_water_and_rejects_retired_target() {
    figure_history_keeps_retired_series_invalid(true);
}

#[test]
fn panel_z_order_restore_keeps_series_allocators_with_content_ids_not_positions() {
    let mut a = super::curve_tests::fixture(true);
    let object = a.doc.canvases[0].objects[0].id;
    let other = a.doc.canvases[0].objects[1].id;
    let panel = a.doc.canvases[0].parent_panel(object).unwrap();
    let other_before = a.doc.canvases[0]
        .object(other)
        .unwrap()
        .plot()
        .unwrap()
        .next_series_id;
    a.select_panel(0, panel);
    a.z_order_selected(crate::actions::ZOrder::Front);
    assert_eq!(a.doc.canvases[0].objects[1].id, object);
    let retired = materialize_default(&mut a, object);
    let high = a.doc.canvases[0]
        .object(object)
        .unwrap()
        .plot()
        .unwrap()
        .next_series_id;
    a.undo();
    a.undo();
    assert_eq!(a.doc.canvases[0].objects[0].id, object);
    assert_eq!(
        a.doc.canvases[0]
            .object(object)
            .unwrap()
            .plot()
            .unwrap()
            .next_series_id,
        high
    );
    assert_eq!(
        a.doc.canvases[0]
            .object(other)
            .unwrap()
            .plot()
            .unwrap()
            .next_series_id,
        other_before
    );
    a.redo();
    assert_eq!(a.doc.canvases[0].objects[1].id, object);
    let created = materialize_default(&mut a, object);
    assert_ne!(created, retired);
    assert!(a.curve_target(&retired).is_err());
    assert_eq!(
        a.doc.canvases[0]
            .object(other)
            .unwrap()
            .plot()
            .unwrap()
            .next_series_id,
        other_before
    );
}

fn grid_preference_fixture() -> PlotiumApp {
    let mut a = fixture();
    for object in &mut a.doc.canvases[0].objects {
        let plot = object.plot_mut().unwrap();
        plot.axis_overrides.x_range = Some(AxisRange { min: 0., max: 3. });
        plot.axis_overrides.y_range = Some(AxisRange { min: 0., max: 6. });
    }
    a.rebuild_canvas(0);
    a
}

fn assert_original_figure_content(a: &PlotiumApp, before: &CanvasDocument) {
    let page = &a.doc.canvases[0];
    assert_eq!(page.panels, before.panels);
    assert_eq!(page.size_mm, before.size_mm);
    for saved in &before.objects {
        let live = page.object(saved.id).unwrap();
        assert_eq!(live.frame, saved.frame);
        assert_eq!(live.name, saved.name);
        assert_eq!((live.visible, live.locked), (saved.visible, saved.locked));
        let (live, saved) = (live.plot().unwrap(), saved.plot().unwrap());
        assert_eq!(live.binding, saved.binding);
        assert_eq!(live.viewport, saved.viewport);
        assert_eq!(live.axis_overrides, saved.axis_overrides);
    }
}

fn caption_edit(a: &PlotiumApp) -> serde_json::Value {
    json!({"op":"text_create","parent":TargetRef::resource(a.doc.canvases[0].resource_id.into()),"name":"caption","frame_pt":[10,220,200,20],"text":"Keep the native grid preference","font_pt":12})
}

#[test]
fn figure_grid_preflight_preserves_a_later_nonundoable_toggle() {
    let mut a = grid_preference_fixture();
    let before = a.doc.canvases[0].clone();
    let p = plan(&a, json!([caption_edit(&a)]));
    let revision = a.doc.automation_revision;
    a.set_show_grid(0, true);
    assert_eq!(a.doc.automation_revision, revision);
    assert!(!a.can_undo());
    a.commit_figure_edit(p).unwrap();
    assert!(a.doc.canvases[0].layout.show_grid);
    assert_eq!(a.doc.canvases[0].objects.len(), before.objects.len() + 1);
    assert_original_figure_content(&a, &before);
    a.undo();
    assert!(a.doc.canvases[0].layout.show_grid);
    assert_eq!(a.doc.canvases[0].objects.len(), before.objects.len());
    assert!(!a.can_undo());
    assert_original_figure_content(&a, &before);
}

#[test]
fn figure_grid_text_history_preserves_the_latest_nonundoable_toggle() {
    let mut a = grid_preference_fixture();
    let before = a.doc.canvases[0].clone();
    let p = plan(&a, json!([caption_edit(&a)]));
    a.commit_figure_edit(p).unwrap();
    a.set_show_grid(0, true);
    a.undo();
    assert!(a.doc.canvases[0].layout.show_grid);
    assert_eq!(a.doc.canvases[0].objects.len(), before.objects.len());
    assert!(!a.can_undo());
    a.set_show_grid(0, false);
    a.redo();
    assert!(!a.doc.canvases[0].layout.show_grid);
    assert_eq!(a.doc.canvases[0].objects.len(), before.objects.len() + 1);
    a.set_show_grid(0, true);
    a.undo();
    a.redo();
    assert!(a.doc.canvases[0].layout.show_grid);
    assert_original_figure_content(&a, &before);
}

#[test]
fn figure_grid_page_history_restores_geometry_but_keeps_the_latest_toggle() {
    let mut a = grid_preference_fixture();
    let before = a.doc.canvases[0].clone();
    let p = plan(
        &a,
        json!([{"op":"page","margin_mm":[4,4,15,4],"gutter_mm":7}]),
    );
    let revision = a.doc.automation_revision;
    a.set_show_grid(0, true);
    assert_eq!(a.doc.automation_revision, revision);
    a.commit_figure_edit(p).unwrap();
    let mut after_layout = before.layout;
    after_layout.margin_mm = [4., 4., 15., 4.];
    after_layout.gutter_mm = 7.;
    after_layout.show_grid = true;
    assert_eq!(a.doc.canvases[0].layout, after_layout);
    a.set_show_grid(0, false);
    a.undo();
    assert_eq!(a.doc.canvases[0].layout, before.layout);
    assert!(!a.can_undo());
    a.set_show_grid(0, true);
    a.redo();
    assert_eq!(a.doc.canvases[0].layout, after_layout);
    a.set_show_grid(0, false);
    a.undo();
    a.redo();
    after_layout.show_grid = false;
    assert_eq!(a.doc.canvases[0].layout, after_layout);
    assert_original_figure_content(&a, &before);
}
