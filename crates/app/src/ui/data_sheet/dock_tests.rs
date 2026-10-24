use super::*;
use egui::{Event, Key, Modifiers, Pos2, Rect};
use plotium_core::data::{ColumnChunk, ColumnSchema, LogicalType, ScalarValue};
use plotium_core::state::{CanvasDocument, Dataset, TableDataset, Tool};

/// Full application frames, so the dock, the board and the global shortcut
/// handlers run in their real order.
struct Workspace {
    ctx: egui::Context,
    app: PlotiumApp,
    clipboard: super::super::clipboard_table::ClipboardTablePaste,
    workflow: super::super::batch_workflow::AutomationUi,
    title: Option<String>,
    /// Scrolling animates over time, so every frame advances the clock.
    time: f64,
}

impl Workspace {
    fn new(app: PlotiumApp) -> Self {
        let mut workspace = Self {
            ctx: crate::typography::test_context(),
            app,
            clipboard: Default::default(),
            workflow: Default::default(),
            title: None,
            time: 0.0,
        };
        workspace.settle();
        workspace
    }

    fn frame(&mut self, events: Vec<Event>) -> egui::FullOutput {
        let Self {
            ctx,
            app,
            clipboard,
            workflow,
            title,
            time,
        } = self;
        *time += 0.25;
        ctx.run_ui(
            egui::RawInput {
                time: Some(*time),
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1400.0, 900.0))),
                events,
                ..Default::default()
            },
            |ui| {
                crate::ui::render(
                    app,
                    clipboard,
                    workflow,
                    &mut Default::default(),
                    title,
                    ui,
                    false,
                    crate::ui::RibbonChrome::default(),
                );
            },
        )
    }

    fn settle(&mut self) -> egui::FullOutput {
        for _ in 0..3 {
            self.frame(vec![]);
        }
        self.frame(vec![])
    }

    fn key(&mut self, key: Key) {
        self.frame(vec![Event::Key {
            key,
            physical_key: Some(key),
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }]);
    }

    fn click(&mut self, position: Pos2) {
        let pointer = |pressed| Event::PointerButton {
            pos: position,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        self.frame(vec![Event::PointerMoved(position), pointer(true)]);
        self.frame(vec![pointer(false)]);
        // The repaint a focus claim requests, which arms the grid's key lock.
        self.frame(vec![]);
    }

    fn dock(&self) -> Rect {
        dock_rect(&self.ctx).expect("the data sheet is docked")
    }

    /// Center of the first galley with `text` inside the dock.
    fn text_in_dock(&mut self, text: &str) -> Option<Rect> {
        let dock = self.dock();
        let output = self.frame(vec![]);
        output.shapes.iter().find_map(|shape| match &shape.shape {
            egui::Shape::Text(galley) if galley.galley.job.text == text => {
                let rect = Rect::from_min_size(galley.pos, galley.galley.size());
                // Right-aligned numbers sit flush against their cell's clip edge.
                let anchor = rect.left_center() + egui::vec2(1.0, 0.0);
                (dock.contains(anchor) && shape.clip_rect.contains(anchor)).then_some(rect)
            }
            _ => None,
        })
    }

    /// Click inside the value cell to the right of a row number.
    fn click_row(&mut self, row: usize) {
        let number = self
            .text_in_dock(&row.to_string())
            .expect("the row number is visible");
        self.click(number.center() + egui::vec2(60.0, 0.0));
    }

    fn board(&self) -> Rect {
        self.ctx
            .data(|data| {
                data.get_temp::<super::super::workspace_geometry::WorkspaceGeometry>(egui::Id::new(
                    "plotium.workspace_geometry",
                ))
            })
            .expect("the board was laid out")
            .board_rect
    }
}

fn app_with_pages(pages: usize) -> PlotiumApp {
    let mut app = PlotiumApp::new_with_settings(plotium_core::settings::Settings::default());
    for index in 0..pages {
        app.doc.canvases.push(CanvasDocument::new(
            format!("Page {}", index + 1),
            [180.0, 120.0],
        ));
    }
    app.session.active_canvas = (pages > 0).then_some(0);
    app
}

fn table(name: &str, columns: usize, rows: usize) -> TableDataset {
    let columns = (0..columns)
        .map(|column| {
            (
                ColumnSchema::new(format!("c{column}"), LogicalType::Float64),
                ColumnChunk::optional_f64(
                    (0..rows).map(|row| Some((row * 1000 + column) as f64 + 0.25)),
                ),
            )
        })
        .collect();
    let mut table =
        TableDataset::from_materialized(columns, vec![], None, vec![], "plotium.test.dock.v1")
            .unwrap();
    table.name = Some(name.to_owned());
    table
}

fn values(app: &PlotiumApp, dataset: usize) -> Vec<ScalarValue> {
    let table = app.doc.datasets[dataset].as_table().unwrap();
    let rows = table
        .typed_rows(
            table.typed_state.envelope.revision.snapshot.row_count as usize,
            &[],
        )
        .unwrap();
    rows.columns
        .iter()
        .flat_map(|column| column.values.iter().cloned())
        .collect()
}

#[test]
fn new_table_docks_below_the_board_empty_ready_to_type_and_undoable() {
    let mut workspace = Workspace::new(app_with_pages(3));
    let ctx = workspace.ctx.clone();
    commands::execute(
        commands::CommandId::NewTable,
        &mut workspace.app,
        &mut workspace.clipboard,
        &ctx,
    );
    workspace.settle();

    assert!(
        ctx.memory(|memory| memory.area_rect(egui::Id::new("plotium_data_sheet")))
            .is_none()
    );
    assert!(!workspace.dock().intersects(workspace.board()));
    assert!(workspace.dock().top() >= workspace.board().bottom());
    assert_eq!(workspace.app.doc.canvases.len(), 3);
    assert!(
        values(&workspace.app, 0)
            .iter()
            .all(|value| matches!(value, ScalarValue::Null))
    );

    workspace.frame(vec![Event::Text("5".to_owned())]);
    assert_eq!(
        workspace
            .app
            .session
            .ui
            .table_cell_edit
            .as_ref()
            .map(|edit| edit.text.as_str()),
        Some("5")
    );

    workspace.key(Key::Escape);
    assert!(workspace.app.session.ui.table_cell_edit.is_none());
    workspace.app.undo();
    workspace.settle();
    assert!(workspace.app.doc.datasets.is_empty());
    assert!(dock_rect(&ctx).is_none());
}

#[test]
fn escape_in_the_cell_editor_keeps_the_canvas_selection_and_tool() {
    let mut app = app_with_pages(1);
    let panel = app.doc.canvases[0].create_panel(
        "Panel".to_owned(),
        plotium_core::state::ObjectFrame::new(10.0, 10.0, 100.0, 80.0),
    );
    app.select_panel(0, panel);
    app.set_tool(Tool::Text);
    app.doc
        .datasets
        .push(Dataset::Table(Box::new(table("Values", 2, 3))));
    app.session.ui.sheet_open = Some(0);
    let mut workspace = Workspace::new(app);
    workspace.click_row(2);
    workspace.frame(vec![Event::Text("9".to_owned())]);
    workspace.frame(vec![]);
    assert!(workspace.app.session.ui.table_cell_edit.is_some());

    workspace.key(Key::Escape);

    assert!(workspace.app.session.ui.table_cell_edit.is_none());
    assert_eq!(workspace.app.session.tool, Tool::Text);
    assert_eq!(
        workspace
            .app
            .session
            .ui
            .hierarchical_selection
            .lead()
            .and_then(|path| path.panel),
        Some(panel)
    );
}

#[test]
fn sheet_transforms_stay_in_the_dock_and_link_back_to_their_source() {
    let mut app = app_with_pages(1);
    app.doc
        .datasets
        .push(Dataset::Table(Box::new(table("Source", 2, 3))));
    app.session.ui.sheet_open = Some(0);
    let revision = &app.doc.datasets[0]
        .as_table()
        .unwrap()
        .typed_state
        .envelope
        .revision;
    let column = revision.snapshot.schema.columns[1].id;
    let plan = plotium_core::data::RelPlanV1::new(plotium_core::data::Relation::Project {
        input: Box::new(plotium_core::data::Relation::SnapshotRead(
            plotium_core::data::SnapshotRead {
                table: revision.table_id,
                revision: revision.id,
                fingerprint: revision.snapshot.fingerprint,
            },
        )),
        columns: vec![column],
    });
    app.start_table_transform(plan, vec![0], "Projected".into(), 16 * 1024 * 1024)
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while app.session.table_transform_job.is_some() && std::time::Instant::now() < deadline {
        app.poll_table_transform();
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert_eq!(app.doc.canvases.len(), 1);
    assert_eq!(app.session.ui.sheet_open, Some(1));
    let mut workspace = Workspace::new(app);

    let link = workspace
        .text_in_dock("Derived from Source")
        .expect("the derived table links to its source");
    workspace.click(link.center());
    workspace.frame(vec![]);

    assert_eq!(workspace.app.session.ui.sheet_open, Some(0));
    assert_eq!(workspace.app.doc.canvases.len(), 1);
}

#[test]
fn dock_height_survives_close_reopen_and_rename() {
    let mut app = app_with_pages(1);
    app.settings.window.data_sheet.height = 333.0;
    app.doc
        .datasets
        .push(Dataset::Table(Box::new(table("Heights", 2, 3))));
    app.session.ui.sheet_open = Some(0);
    let mut workspace = Workspace::new(app);
    assert_eq!(workspace.dock().height(), 333.0);

    workspace.app.session.ui.sheet_open = None;
    workspace.settle();
    assert!(dock_rect(&workspace.ctx).is_none());
    workspace.app.doc.datasets[0].as_table_mut().unwrap().name = Some("Renamed".into());
    workspace.app.session.ui.sheet_open = Some(0);
    workspace.settle();

    assert_eq!(workspace.dock().height(), 333.0);
    assert!(workspace.text_in_dock("Renamed").is_some());
}

// Settings persistence runs in the Unix-only isolated-HOME harness.
#[cfg(unix)]
#[test]
fn dragged_dock_height_is_saved_for_the_next_launch() {
    crate::test_settings::isolated(
        "ui::data_sheet::dock_tests::dragged_dock_height_is_saved_for_the_next_launch",
        || {
            let mut app = app_with_pages(1);
            app.doc
                .datasets
                .push(Dataset::Table(Box::new(table("Heights", 2, 3))));
            app.session.ui.sheet_open = Some(0);
            let mut workspace = Workspace::new(app);
            let before = workspace.dock().height();
            let edge = egui::pos2(workspace.dock().center().x, workspace.dock().top());
            let target = edge - egui::vec2(0.0, 60.0);
            let button = |pressed| Event::PointerButton {
                pos: edge,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            };
            workspace.frame(vec![Event::PointerMoved(edge)]);
            workspace.frame(vec![button(true)]);
            workspace.frame(vec![Event::PointerMoved(target)]);
            workspace.frame(vec![Event::PointerMoved(target)]);
            workspace.frame(vec![Event::PointerButton {
                pos: target,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            }]);
            let after = workspace.app.settings.window.data_sheet.height;
            assert!((after - before - 60.0).abs() < 1.0, "{before} -> {after}");

            let mut relaunched = PlotiumApp::new_with_settings(plotium_core::settings::load());
            relaunched
                .doc
                .canvases
                .push(CanvasDocument::new("Page".into(), [180.0, 120.0]));
            relaunched.session.active_canvas = Some(0);
            relaunched
                .doc
                .datasets
                .push(Dataset::Table(Box::new(table("Heights", 2, 3))));
            relaunched.session.ui.sheet_open = Some(0);
            let relaunched = Workspace::new(relaunched);
            assert!((relaunched.dock().height() - after).abs() < 1.0);
        },
    );
}

/// The smallest row number drawn in the dock, i.e. the first visible row.
fn first_visible_row(workspace: &mut Workspace) -> usize {
    let dock = workspace.dock();
    let output = workspace.frame(vec![]);
    output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Text(galley)
                if dock.contains(galley.pos)
                    && shape
                        .clip_rect
                        .contains(galley.pos + egui::vec2(1.0, galley.galley.size().y / 2.0)) =>
            {
                galley.galley.job.text.parse::<usize>().ok()
            }
            _ => None,
        })
        .min()
        .expect("row numbers are visible")
}

#[test]
fn each_table_keeps_its_scroll_position_and_selection() {
    let mut app = app_with_pages(1);
    for name in ["A", "B"] {
        app.doc
            .datasets
            .push(Dataset::Table(Box::new(table(name, 2, 300))));
    }
    app.session.ui.sheet_open = Some(0);
    let mut workspace = Workspace::new(app);
    workspace.click_row(3);
    for _ in 0..40 {
        workspace.key(Key::ArrowDown);
    }
    workspace.settle();
    let scrolled = first_visible_row(&mut workspace);
    assert!(scrolled > 1, "the table scrolled");

    workspace.app.session.ui.sheet_open = Some(1);
    workspace.settle();
    assert_eq!(first_visible_row(&mut workspace), 1);
    assert!(workspace.text_in_dock("Row 43, c0").is_none());

    workspace.app.session.ui.sheet_open = Some(0);
    workspace.settle();
    assert_eq!(first_visible_row(&mut workspace), scrolled);
    assert!(workspace.text_in_dock("Row 43, c0").is_some());
}

#[test]
fn the_last_column_of_a_wide_table_is_reachable() {
    let mut app = app_with_pages(1);
    app.doc
        .datasets
        .push(Dataset::Table(Box::new(table("Wide", 60, 3))));
    app.session.ui.sheet_open = Some(0);
    let mut workspace = Workspace::new(app);
    assert!(workspace.text_in_dock("c59").is_none());

    workspace.click_row(1);
    for _ in 0..59 {
        workspace.key(Key::ArrowRight);
    }
    workspace.settle();

    assert!(workspace.text_in_dock("c59").is_some());
    assert!(workspace.text_in_dock("Row 1, c59").is_some());
}

// Settings persistence runs in the Unix-only isolated-HOME harness.
#[cfg(unix)]
#[test]
fn a_collapsed_sheet_expands_for_a_new_table() {
    crate::test_settings::isolated(
        "ui::data_sheet::dock_tests::a_collapsed_sheet_expands_for_a_new_table",
        || {
            let mut app = app_with_pages(1);
            app.settings.window.data_sheet.collapsed = true;
            let mut workspace = Workspace::new(app);
            let ctx = workspace.ctx.clone();
            commands::execute(
                commands::CommandId::NewTable,
                &mut workspace.app,
                &mut workspace.clipboard,
                &ctx,
            );
            workspace.settle();

            assert!(workspace.dock().height() >= 140.0);
            workspace.frame(vec![Event::Text("7".to_owned())]);
            assert!(workspace.app.session.ui.table_cell_edit.is_some());
            assert!(!plotium_core::settings::load().window.data_sheet.collapsed);
        },
    );
}
