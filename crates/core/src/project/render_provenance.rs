use plotium_figure::{Figure, Polygon, Series};

/// View snapshots intentionally serialize no runtime IDs. Rebuild already runs
/// during loading to recover derived axes. Its binding order is authoritative:
/// reuse the origins only when the complete ordered run sequence matches, not
/// by names, matching individual points, or an unvalidated collection position.
/// A diverged snapshot remains viewable but has explicitly unverified provenance.
pub(super) fn restore(snapshot: &mut Figure, rebuilt: &Figure) {
    if snapshot.polygons.len() == rebuilt.polygons.len()
        && snapshot
            .polygons
            .iter()
            .zip(&rebuilt.polygons)
            .all(|(a, b)| same_mark(a, b))
    {
        for (saved, current) in snapshot.polygons.iter_mut().zip(&rebuilt.polygons) {
            saved.origin = current.origin;
        }
    }
    if snapshot.series.len() != rebuilt.series.len()
        || !snapshot
            .series
            .iter()
            .zip(&rebuilt.series)
            .all(|(a, b)| same_run(a, b))
    {
        return;
    }
    for (saved, current) in snapshot.series.iter_mut().zip(&rebuilt.series) {
        saved.origin = current.origin;
    }
}
fn same_mark(a: &Polygon, b: &Polygon) -> bool {
    a.points == b.points && a.name == b.name && a.fill == b.fill
}
fn same_run(a: &Series, b: &Series) -> bool {
    a.points == b.points
        && a.name == b.name
        && a.color == b.color
        && a.width == b.width
        && a.kind == b.kind
        && a.appearance == b.appearance
}
#[cfg(test)]
mod tests {
    use super::*;
    use plotium_figure::{Axis, SeriesOrigin};
    #[test]
    fn restores_identical_named_runs_only_after_full_sequence_validation() {
        let mut native = Figure::new("", Axis::new("", 0., 1.), Axis::new("", 0., 1.));
        for id in [7, 9] {
            let mut s = Series::line("same", vec![[0., 0.], [1., 1.]]);
            s.origin = Some(SeriesOrigin(id));
            native.series.push(s);
        }
        let mut saved: Figure =
            serde_json::from_value(serde_json::to_value(&native).unwrap()).unwrap();
        assert!(saved.series.iter().all(|s| s.origin.is_none()));
        restore(&mut saved, &native);
        assert_eq!(
            saved.series.iter().map(|s| s.origin).collect::<Vec<_>>(),
            native.series.iter().map(|s| s.origin).collect::<Vec<_>>()
        );
        for s in &mut saved.series {
            s.origin = None;
        }
        saved.series[1].width += 1.;
        restore(&mut saved, &native);
        assert!(saved.series.iter().all(|s| s.origin.is_none()));
    }
}
