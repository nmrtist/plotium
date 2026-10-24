//! What the hints read about a plot's native curves and stated intent.
use super::*;
use crate::state::PlotIntent;

/// What diagnostics need to know about a plot's native curves.
#[derive(Default)]
pub(in super::super) struct PlotFacts {
    pub(super) transposed: bool,
    /// Built from explicit columns, so every mark carries its Series origin.
    pub(super) explicit: bool,
    /// Kinds of the visible native Series.
    pub(super) kinds: Vec<(SeriesId, SeriesKind)>,
    /// Table rows at each x of visible bar, box and violin Series.
    pub(super) rows_per_x: Vec<(SeriesId, Vec<usize>)>,
    /// Series whose rows could not be read, with the reason.
    pub(super) unreadable: Vec<(SeriesId, String)>,
    /// Series a narrowed query asked about; `None` reports every Series.
    pub(super) selected: Option<BTreeSet<SeriesId>>,
    pub(super) intent: PlotIntent,
    /// Replicate counts the user stated for summary-only curves.
    pub(super) stated_replicates: Vec<(SeriesId, u32)>,
}

impl PlotFacts {
    /// `rows` reads the table rows behind bars, boxes and violins.
    pub(in super::super) fn of(
        app: &PlotiumApp,
        binding: &DataBinding,
        intent: &PlotIntent,
        rows: bool,
    ) -> Self {
        let mut facts = Self {
            transposed: binding.coordinates.as_ref().is_some_and(|c| c.transposed),
            explicit: binding.coordinates.is_some(),
            intent: intent.clone(),
            ..Self::default()
        };
        for series in binding.series.iter().filter(|s| s.visible) {
            let SeriesEncoding::Line(line) = &series.encoding else {
                continue;
            };
            facts.kinds.push((series.id, line.kind));
            if let Some(n) = series.replicates {
                facts.stated_replicates.push((series.id, n.get()));
            }
            if rows && is_mark(line.kind) {
                match app.curve_rows_per_x(binding, series) {
                    Ok(sizes) => facts.rows_per_x.push((series.id, sizes)),
                    Err(reason) => facts.unreadable.push((series.id, reason)),
                }
            }
        }
        facts
    }

    pub(in super::super) fn select(&mut self, selected: Option<BTreeSet<SeriesId>>) {
        self.selected = selected;
    }

    pub(super) fn reports(&self, id: SeriesId) -> bool {
        self.selected.as_ref().is_none_or(|s| s.contains(&id))
    }

    pub(super) fn kind(&self, origin: Option<SeriesOrigin>) -> Option<SeriesKind> {
        let id = SeriesId::new(origin?.0);
        self.kinds.iter().find(|(s, _)| *s == id).map(|(_, k)| *k)
    }

    pub(super) fn has(&self, kind: SeriesKind) -> bool {
        self.kinds.iter().any(|(_, k)| *k == kind)
    }
}
