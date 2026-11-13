use super::{Dataset, DatasetId, PlotiumApp};
use crate::actions::Action;
use serde::{Deserialize, Serialize};

/// User labels only. Acquisition facts and processing never read these values.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DatasetOrganization {
    experiment_override: Option<String>,
    tags: Vec<String>,
}

impl DatasetOrganization {
    pub fn is_empty(&self) -> bool {
        self.experiment_override.is_none() && self.tags.is_empty()
    }

    pub fn experiment_override(&self) -> Option<&str> {
        self.experiment_override.as_deref()
    }

    pub fn tags(&self) -> &[String] {
        &self.tags
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        let key = tag.to_lowercase();
        self.tags
            .iter()
            .any(|existing| existing.to_lowercase() == key)
    }
}

pub enum OrganizationEdit<'a> {
    Experiment(Option<&'a str>),
    AddTag(&'a str),
    RemoveTag(&'a str),
}

impl Dataset {
    pub fn organization(&self) -> &DatasetOrganization {
        match self {
            Self::Nmr(d) => &d.organization,
            Self::Nmr2D(d) => &d.organization,
            Self::Table(d) => &d.organization,
            Self::Electrophysiology(d) => &d.organization,
            Self::Afm(d) => &d.organization,
            Self::MassSpec(d) => &d.organization,
            Self::Xrd(d) => &d.organization,
            Self::Xps(d) => &d.organization,
            Self::Xas(d) => &d.organization,
        }
    }

    pub fn organization_mut(&mut self) -> &mut DatasetOrganization {
        match self {
            Self::Nmr(d) => &mut d.organization,
            Self::Nmr2D(d) => &mut d.organization,
            Self::Table(d) => &mut d.organization,
            Self::Electrophysiology(d) => &mut d.organization,
            Self::Afm(d) => &mut d.organization,
            Self::MassSpec(d) => &mut d.organization,
            Self::Xrd(d) => &mut d.organization,
            Self::Xps(d) => &mut d.organization,
            Self::Xas(d) => &mut d.organization,
        }
    }

    pub fn experiment_label(&self) -> Option<&str> {
        self.organization().experiment_override().or_else(|| {
            self.acquisition_identity()
                .acquisition
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
        })
    }

    pub fn browser_name(&self) -> String {
        self.name().unwrap_or_else(|| {
            let identity = self.acquisition_identity();
            identity
                .subject
                .clone()
                .unwrap_or_else(|| identity.source_label.clone())
        })
    }

    pub fn compact_category(&self) -> &'static str {
        match self {
            Self::Nmr(_) | Self::Nmr2D(_) => "NMR",
            Self::Table(_) => "Table",
            _ => self.kind_label(),
        }
    }

    pub fn source_identifier(&self) -> &str {
        match self {
            Self::Nmr(d) => d.data.source(),
            Self::Nmr2D(d) => &d.data.source,
            Self::Electrophysiology(d) => &d.data.source,
            Self::Afm(d) => &d.data.source,
            Self::MassSpec(d) => &d.run.source,
            Self::Xrd(d) => &d.data.source,
            Self::Xps(d) => &d.experiment.source,
            Self::Xas(d) => &d.scan.source,
            Self::Table(_) => &self.acquisition_identity().source_label,
        }
    }

    pub fn dimension_label(&self) -> Option<&'static str> {
        match self {
            Self::Nmr(_) => Some("1D"),
            Self::Nmr2D(data) if data.volume.is_some() => Some("3D"),
            Self::Nmr2D(data) if data.is_pseudo() => Some("Pseudo-2D"),
            Self::Nmr2D(_) => Some("2D"),
            _ => None,
        }
    }
}

impl PlotiumApp {
    pub fn edit_dataset_organization(&mut self, targets: &[DatasetId], edit: OrganizationEdit<'_>) {
        let mut seen = std::collections::HashSet::new();
        let actions = targets
            .iter()
            .copied()
            .filter(|id| seen.insert(*id))
            .filter_map(|dataset| {
                let before = self
                    .doc
                    .datasets
                    .get(self.doc.dataset_index(dataset)?)?
                    .organization()
                    .clone();
                let mut after = before.clone();
                match edit {
                    OrganizationEdit::Experiment(value) => {
                        after.experiment_override = value
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                            .map(str::to_owned);
                    }
                    OrganizationEdit::AddTag(tag) => {
                        let tag = tag.trim();
                        if !tag.is_empty() && !after.has_tag(tag) {
                            after.tags.push(tag.to_owned());
                        }
                    }
                    OrganizationEdit::RemoveTag(tag) => {
                        let tag = tag.trim().to_lowercase();
                        after.tags.retain(|value| value.to_lowercase() != tag);
                    }
                }
                (before != after).then_some(Action::SetDatasetOrganization {
                    dataset,
                    before,
                    after,
                })
            })
            .collect::<Vec<_>>();
        if !actions.is_empty() {
            self.execute_action(Action::Composite(actions));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batch_metadata_round_trips_and_does_not_replace_acquisition_identity() {
        let mut app = PlotiumApp::new_with_settings(Default::default());
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../io/tests/fixtures/nmr/jcamp-ppm.dx");
        app.load_from(&fixture);
        app.load_from(&fixture);
        let ids: Vec<_> = app.doc.datasets.iter().map(Dataset::resource_id).collect();
        let mut imported = app.doc.datasets[0].acquisition_identity().clone();
        imported.acquisition = Some("Imported protocol".into());
        app.doc.datasets[0].set_acquisition_identity(imported.clone());
        app.edit_dataset_organization(&ids[..1], OrganizationEdit::AddTag("Sample"));
        app.edit_dataset_organization(&ids[1..], OrganizationEdit::AddTag("Control"));
        app.edit_dataset_organization(&ids, OrganizationEdit::AddTag(" Batch "));
        app.edit_dataset_organization(&ids, OrganizationEdit::AddTag("batch"));
        app.edit_dataset_organization(&ids, OrganizationEdit::AddTag(" "));
        app.undo();
        assert_eq!(app.doc.datasets[0].organization().tags(), ["Sample"]);
        assert_eq!(app.doc.datasets[1].organization().tags(), ["Control"]);
        app.redo();
        app.edit_dataset_organization(&ids, OrganizationEdit::RemoveTag("sample"));
        assert_eq!(app.doc.datasets[0].organization().tags(), ["Batch"]);
        assert_eq!(
            app.doc.datasets[1].organization().tags(),
            ["Control", "Batch"]
        );
        app.undo();
        app.edit_dataset_organization(&ids, OrganizationEdit::Experiment(Some("HNCO")));
        assert_eq!(app.doc.datasets[0].dimension_label(), Some("1D"));
        assert_eq!(app.doc.datasets[0].acquisition_identity(), &imported);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("metadata.plotium");
        crate::project::save_project(&app, &path, false).unwrap();
        let mut loaded = crate::project::load_project(&path).unwrap();
        assert_eq!(loaded.doc.datasets[0].experiment_label(), Some("HNCO"));
        assert_eq!(
            loaded.doc.datasets[0].organization().tags(),
            ["Sample", "Batch"]
        );
        assert_eq!(loaded.doc.datasets[0].acquisition_identity(), &imported);
        loaded.edit_dataset_organization(&ids, OrganizationEdit::Experiment(None));
        assert_eq!(
            loaded.doc.datasets[0].experiment_label(),
            Some("Imported protocol")
        );
    }
}
