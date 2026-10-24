//! Deleting a whole dataset. Plots, derived tables and reports resolve their
//! sources by `DatasetId`, so a dataset anything still draws from is never removed.
use super::*;
use crate::actions::Action;

impl PlotiumApp {
    /// Everything that still needs dataset `id`, one readable place each.
    pub fn dataset_users(&self, id: DatasetId) -> Vec<String> {
        let mut users = Vec::new();
        for canvas in &self.doc.canvases {
            for item in &canvas.objects {
                let Some(plot) = item.plot() else {
                    continue;
                };
                if plot.binding.contains_dataset(id)
                    || plot.display_owner == Some(id)
                    || plot.projections.attached_datasets().contains(&id)
                {
                    let panel = canvas
                        .parent_panel(item.id)
                        .and_then(|panel| canvas.panel(panel))
                        .map(|panel| canvas.panel_label_text(panel))
                        .filter(|label| !label.is_empty())
                        .map_or_else(String::new, |label| format!(" panel {label}"));
                    users.push(format!(
                        "page \"{}\"{panel} (plot \"{}\")",
                        canvas.name, item.name
                    ));
                }
            }
        }
        for dataset in &self.doc.datasets {
            if dataset
                .lineage()
                .is_some_and(|lineage| lineage.sources.contains(&id))
            {
                users.push(format!("derived dataset \"{}\"", dataset.display_name()));
            }
        }
        for report in &self.doc.reports {
            if report.source.dataset == id {
                users.push(format!("analysis report \"{}\"", report.name));
            }
        }
        users
    }

    pub fn check_dataset_deletable(&self, id: DatasetId) -> Result<(), String> {
        let users = self.dataset_users(id);
        if users.is_empty() {
            return Ok(());
        }
        let name = self
            .doc
            .dataset_by_id(id)
            .map_or_else(|| id.to_string(), Dataset::display_name);
        Err(format!(
            "Dataset \"{name}\" is still used by {}. Delete those curves, panels, derived datasets or reports first.",
            users.join("; ")
        ))
    }

    pub fn delete_dataset_action(&self, index: usize) -> Result<Action, String> {
        let dataset = self
            .doc
            .datasets
            .get(index)
            .ok_or_else(|| "The dataset no longer exists.".to_owned())?;
        self.check_dataset_deletable(dataset.resource_id())?;
        Ok(Action::DeleteDataset {
            index,
            dataset: Box::new(dataset.clone()),
        })
    }

    /// Delete one dataset as an undoable step, or say in the status line what
    /// still uses it.
    pub fn delete_dataset(&mut self, index: usize) {
        let result = self.delete_dataset_action(index).and_then(|action| {
            let name = self.doc.datasets[index].display_name();
            self.try_execute_action(action)
                .map(|()| name)
                .map_err(|error| error.to_string())
        });
        self.session.status = match result {
            Ok(name) => format!("Deleted dataset \"{name}\"."),
            Err(message) => message,
        };
    }

    /// Repair index-addressed session state after `datasets.remove(index)`.
    pub(crate) fn forget_removed_dataset(&mut self, index: usize, id: DatasetId) {
        self.session.compute.invalidate_dataset(id);
        if matches!(
            self.session.ui.craft_feedback.get(&id),
            Some(CraftRunFeedback::Running(_))
        ) {
            self.session
                .ui
                .craft_feedback
                .insert(id, CraftRunFeedback::Cancelled);
        }
        self.remap_dataset_indices(|stored| match stored.cmp(&index) {
            std::cmp::Ordering::Less => Some(stored),
            std::cmp::Ordering::Equal => None,
            std::cmp::Ordering::Greater => Some(stored - 1),
        });
        self.session
            .ui
            .frame_selection
            .retain(|frame| *frame != BoardFrameId::Sheet(id));
    }

    /// Repair index-addressed session state after `datasets.insert(index, _)`.
    pub(crate) fn shift_for_inserted_dataset(&mut self, index: usize) {
        self.remap_dataset_indices(|stored| Some(stored + usize::from(stored >= index)));
    }

    /// Move every session field that stores a dataset position; state for a
    /// position that no longer exists is closed.
    fn remap_dataset_indices(&mut self, map: impl Fn(usize) -> Option<usize>) {
        self.session.dataset_epoch += 1;
        let ui = &mut self.session.ui;
        ui.data_selection = ui.data_selection.iter().filter_map(|&i| map(i)).collect();
        for set in [
            &mut ui.data_browser_collapsed_datasets,
            &mut ui.data_browser_collapsed_derived,
            &mut ui.data_browser_expanded_analysis,
        ] {
            *set = set.iter().filter_map(|&i| map(i)).collect();
        }
        for slot in [
            &mut ui.sheet_open,
            &mut ui.data_browser_last_active,
            &mut ui.curve_fit_task_dataset,
            &mut ui.stat_task_dataset,
            &mut ui.fit_dataset,
        ] {
            *slot = slot.and_then(&map);
        }
        // Node keys embed positions; the browser re-derives its highlight.
        ui.data_browser_selected_node = None;
        if let Some(RenameState {
            target: RenameTarget::Data(dataset),
            ..
        }) = &mut ui.rename
        {
            match map(*dataset) {
                Some(moved) => *dataset = moved,
                None => ui.rename = None,
            }
        }
        if let Some(export) = &mut ui.data_export {
            match map(export.dataset) {
                Some(moved) => export.dataset = moved,
                None => ui.data_export = None,
            }
        }
        if let Some(draft) = &mut ui.stat_draft {
            match map(draft.dataset) {
                Some(moved) => draft.dataset = moved,
                None => ui.stat_draft = None,
            }
        }
    }
}

#[cfg(test)]
#[path = "dataset_delete_tests.rs"]
mod tests;
