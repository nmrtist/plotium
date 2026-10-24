use plotium_figure::{Figure, Polygon, Series};

/// View snapshots intentionally serialize no runtime IDs. Rebuild already runs
/// during loading to recover derived axes. Its binding order is authoritative:
/// reuse the origins only when the complete ordered run sequence matches, not
/// by names, matching individual points, or an unvalidated collection position.
/// A diverged snapshot remains viewable but has explicitly unverified provenance.
pub(super) fn restore(snapshot: &mut Figure, rebuilt: &Figure) {
    let amplified;
    let rebuilt = if snapshot.display_gain != rebuilt.display_gain {
        amplified = {
            let mut figure = rebuilt.clone();
            if let Err(error) = figure.set_display_gain(snapshot.display_gain) {
                eprintln!("Could not restore amplified series provenance: {error}");
                return;
            }
            figure
        };
        &amplified
    } else {
        rebuilt
    };
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
    snapshot.inherit_display_gain_sources(rebuilt);
    for (saved, current) in snapshot.series.iter_mut().zip(&rebuilt.series) {
        saved.origin = current.origin;
    }
}
fn same_mark(a: &Polygon, b: &Polygon) -> bool {
    a.points == b.points && a.name == b.name && a.fill == b.fill
}
fn same_run(a: &Series, b: &Series) -> bool {
    a.points == b.points
        && a.display_baseline == b.display_baseline
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

    #[test]
    fn restores_origins_after_repeated_display_gain_steps() {
        let mut native = Figure::new("", Axis::new("", 0., 1.), Axis::new("", 0., 20.));
        let mut series = Series::line("trace", vec![[0., 10.], [1., 10.123456789]]);
        series.display_baseline = 10.;
        series.origin = Some(SeriesOrigin(7));
        native.series.push(series);
        let mut displayed = native.clone();
        for gain in [1.13, 2.63, 0.00001, 3.27] {
            displayed.set_display_gain(gain).unwrap();
        }
        let mut saved: Figure =
            serde_json::from_value(serde_json::to_value(&displayed).unwrap()).unwrap();
        restore(&mut saved, &native);
        assert_eq!(saved.series[0].origin, Some(SeriesOrigin(7)));
        saved.set_display_gain(1.).unwrap();
        assert_eq!(saved.series[0].points, native.series[0].points);
    }
}
