use super::*;
use crate::settings::Settings;
use crate::state::{
    CanvasDocument, ContentItem, ContentKind, ObjectFrame, Selection, SelectionPath, TextBox,
};

fn app() -> PlotiumApp {
    let mut app = PlotiumApp::new_with_settings(Settings::default());
    let mut page = CanvasDocument::new("Page".into(), [100.0, 100.0]);
    page.board_pos = [500.0, 600.0];
    let id = page.allocate_object_id();
    page.objects.push(ContentItem {
        id,
        name: "Label".into(),
        frame: ObjectFrame::new(10.0, 20.0, 30.0, 40.0),
        locked: true,
        visible: true,
        kind: ContentKind::Text(TextBox::label("Test".into())),
    });
    app.doc.canvases.push(page);
    app.session.active_canvas = Some(0);
    app
}

#[test]
fn selection_scope_ignores_stale_selections_in_other_scopes() {
    let mut app = app();
    app.session.ui.frame_selection = vec![BoardFrameId::Page(app.doc.canvases[0].resource_id)];
    assert!(selected_board_fit(&app).is_err());
    app.session.ui.selection = Selection::single(app.doc.canvases[0].objects[0].id);
    for scope in [SelectionScope::CanvasObjects, SelectionScope::Layers] {
        app.session.ui.selection_scope = scope;
        let target = selected_board_fit(&app).unwrap();
        assert_eq!(
            board_fit_bounds(&app, &target),
            Some(([510.0, 620.0, 540.0, 660.0], 1))
        );
    }
    app.session.ui.selection_scope = SelectionScope::Board;
    assert_eq!(
        board_fit_bounds(&app, &selected_board_fit(&app).unwrap())
            .unwrap()
            .0[0],
        500.0
    );
    app.session.ui.selection_scope = SelectionScope::DataList;
    assert!(selected_board_fit(&app).is_err());
}

#[test]
fn panels_and_children_use_live_page_coordinates_and_clipping() {
    let mut app = app();
    let page = &mut app.doc.canvases[0];
    let content = page.objects[0].id;
    let panel = page.create_panel("Panel".into(), ObjectFrame::new(100.0, 200.0, 25.0, 35.0));
    page.panel_mut(panel).unwrap().item_order.push(content);
    page.panel_mut(panel).unwrap().clip_children = true;
    let canvas = page.resource_id;
    app.session
        .ui
        .hierarchical_selection
        .replace(SelectionPath::content(canvas, Some(panel), content));
    let target = selected_board_fit(&app).unwrap();
    assert_eq!(
        board_fit_bounds(&app, &target),
        Some(([610.0, 820.0, 625.0, 835.0], 1))
    );
    app.session
        .ui
        .hierarchical_selection
        .replace(SelectionPath::panel(canvas, panel));
    let panel_target = selected_board_fit(&app).unwrap();
    assert_eq!(
        board_fit_bounds(&app, &panel_target),
        Some(([600.0, 800.0, 625.0, 835.0], 1))
    );
    app.doc.canvases[0].panel_mut(panel).unwrap().frame.x += 50.0;
    assert_eq!(board_fit_bounds(&app, &target).unwrap().0[0], 660.0);
    app.doc.canvases[0].panel_mut(panel).unwrap().visible = false;
    assert!(board_fit_bounds(&app, &target).is_none());
    assert!(board_fit_bounds(&app, &panel_target).is_none());
}

#[test]
fn targets_survive_selection_changes_and_partial_deletion_but_not_identity_reuse() {
    let mut app = app();
    let page = app.doc.canvases[0].resource_id;
    app.new_table_dataset();
    let sheet = BoardFrameId::Sheet(app.doc.datasets[0].resource_id());
    let target = captured_board_fit(
        &app,
        [
            BoardFitItem::Frame(BoardFrameId::Page(page)),
            BoardFitItem::Frame(sheet),
            BoardFitItem::Frame(sheet),
        ],
    )
    .unwrap();
    assert_eq!(board_fit_bounds(&app, &target).unwrap().1, 2);
    app.session.ui.frame_selection.clear();
    app.doc.canvases.clear();
    assert_eq!(board_fit_bounds(&app, &target).unwrap().1, 1);
    app.doc.datasets.clear();
    app.doc
        .canvases
        .push(CanvasDocument::new("Replacement".into(), [100.0, 100.0]));
    assert!(board_fit_bounds(&app, &target).is_none());
    assert_eq!(
        board_fit_bounds(&app, &BoardFitTarget::AllFrames)
            .unwrap()
            .1,
        1
    );
}

#[test]
fn data_list_resolves_tables_without_an_active_page_and_discards_invalid_indices() {
    let mut app = PlotiumApp::new_with_settings(Settings::default());
    app.new_table_dataset();
    app.new_table_dataset();
    app.session.ui.selection_scope = SelectionScope::DataList;
    app.session.ui.data_selection = vec![0, 1, 0, usize::MAX];
    let target = selected_board_fit(&app).unwrap();
    assert_eq!(board_fit_bounds(&app, &target).unwrap().1, 2);
    assert_eq!(app.session.active_canvas, None);
    app.session.ui.selection_scope = SelectionScope::CanvasList;
    assert!(selected_board_fit(&app).is_err());
}

#[test]
fn invisible_and_degenerate_content_are_not_fit_targets() {
    let mut app = app();
    app.session.ui.selection = Selection::single(app.doc.canvases[0].objects[0].id);
    app.doc.canvases[0].objects[0].visible = false;
    assert!(selected_board_fit(&app).is_err());
    app.doc.canvases[0].objects[0].visible = true;
    for width in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        app.doc.canvases[0].objects[0].frame.width = width;
        assert!(selected_board_fit(&app).is_err());
    }
}

#[test]
fn whole_object_multiselection_includes_members_beyond_the_hierarchical_lead() {
    let mut app = app();
    let page = &mut app.doc.canvases[0];
    let first = page.objects[0].id;
    let second = page.allocate_object_id();
    let mut object = page.objects[0].clone();
    object.id = second;
    object.frame.x = 110.0;
    page.objects.push(object);
    app.set_selection(Selection::Objects(vec![first, second]));
    assert_eq!(app.session.ui.hierarchical_selection.paths().len(), 1);
    let target = selected_board_fit(&app).unwrap();
    assert_eq!(
        board_fit_bounds(&app, &target),
        Some(([510.0, 620.0, 640.0, 660.0], 2))
    );
}

#[test]
fn a_table_without_a_sheet_maps_to_one_result_page_and_missing_results_disable_fit() {
    let mut app = crate::state::curve_tests::fixture(false);
    app.session.ui.selection_scope = SelectionScope::DataList;
    app.session.ui.data_selection = vec![0];
    let page = app.doc.canvases[0].resource_id;
    let target = selected_board_fit(&app).unwrap();
    assert_eq!(
        target,
        BoardFitTarget::Items(vec![BoardFitItem::Frame(BoardFrameId::Page(page))])
    );
    app.doc.canvases.clear();
    assert!(selected_board_fit(&app).is_err());
}
