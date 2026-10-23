use super::{DatasetId, TableDataset};
use plotium_data::{ColumnId, LiteralValue, RevisionId, RowId, TableId};

/// Uncommitted cell text belongs to its original immutable table revision.
/// Keeping it in session state lets background writers respect the draft.
#[derive(Clone, Debug)]
pub struct TableCellEditDraft {
    pub dataset: DatasetId,
    pub table: TableId,
    pub revision: RevisionId,
    pub row: RowId,
    pub column: ColumnId,
    pub original: LiteralValue,
    pub original_text: String,
    pub text: String,
    pub error: Option<String>,
    pub take_focus: bool,
}

impl TableCellEditDraft {
    pub fn validate_target(&self, table: &TableDataset) -> Result<(), String> {
        let revision = &table.typed_state.envelope.revision;
        if table.resource_id != self.dataset || revision.table_id != self.table {
            return Err("The original table is unavailable. Copy your draft or discard it.".into());
        }
        if table.provenance.is_some() {
            return Err("This table is now read-only. Copy your draft or discard it.".into());
        }
        if revision.id != self.revision {
            return Err("The table changed while you were editing. Copy your draft, then discard it and reopen the cell.".into());
        }
        if revision.snapshot.schema.column(self.column).is_none() {
            return Err(
                "The original column is unavailable. Copy your draft or discard it.".into(),
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Dataset, PlotiumApp, ProjectTransition, ProjectTransitionPhase};

    fn app_with_draft() -> PlotiumApp {
        let mut app = PlotiumApp::new();
        let table = crate::state::materialized_float_series_table(
            ("x".into(), "".into(), vec![Some(1.0)]),
            vec![],
            "plotium.test.draft-transition.v1",
        )
        .unwrap();
        let revision = &table.typed_state.envelope.revision;
        app.session.ui.table_cell_edit = Some(TableCellEditDraft {
            dataset: table.resource_id,
            table: revision.table_id,
            revision: revision.id,
            row: table.typed_rows(1, &[]).unwrap().row_ids[0],
            column: revision.snapshot.schema.columns[0].id,
            original: LiteralValue::Null,
            original_text: "1".into(),
            text: "unfinished".into(),
            error: None,
            take_focus: false,
        });
        app.doc.datasets.push(Dataset::Table(Box::new(table)));
        app
    }

    #[test]
    fn project_transitions_keep_uncommitted_cell_input_visible() {
        for target in [
            ProjectTransition::New,
            ProjectTransition::Close,
            ProjectTransition::Quit,
            ProjectTransition::Open("another.plotium".into()),
        ] {
            let mut app = app_with_draft();
            let context = app.session.project_context;
            app.request_project_transition(target.clone());
            assert!(app.session.ui.project_transition.is_none());
            assert_eq!(app.session.project_context, context);
            assert_eq!(app.session.ui.sheet_open, Some(0));
            let draft = app.session.ui.table_cell_edit.as_ref().unwrap();
            assert_eq!(draft.text, "unfinished");
            assert!(draft.error.as_ref().unwrap().contains("Apply or discard"));
            app.session.ui.table_cell_edit = None;
            app.request_project_transition(target.clone());
            let transition = app.session.ui.project_transition.as_ref().unwrap();
            assert_eq!(transition.target, target);
            assert_eq!(
                transition.phase,
                if target == ProjectTransition::Quit {
                    ProjectTransitionPhase::CheckUnsaved
                } else {
                    ProjectTransitionPhase::Ready
                }
            );
        }
    }

    #[test]
    fn installing_a_project_does_not_leave_an_old_cell_edit_lock() {
        let mut app = app_with_draft();
        app.install_loaded_project(PlotiumApp::new());
        assert!(app.session.ui.table_cell_edit.is_none());
    }
}
