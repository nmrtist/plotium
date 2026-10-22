use resvg::usvg::fontdb::Database;

/// Load platform fonts and make SVG's generic families resolve on systems that
/// do not install fontdb's defaults, such as Arial and Times New Roman
/// (notably Linux).
pub(super) fn load_system_fonts(database: &mut Database) {
    database.load_system_fonts();
    let generic = GenericFamilies::of(database.faces().flat_map(|face| {
        face.families
            .iter()
            .map(move |(family, _)| (family.as_str(), face.monospaced))
    }));
    if let Some(family) = generic.sans_serif {
        database.set_sans_serif_family(family);
    }
    if let Some(family) = generic.serif {
        database.set_serif_family(family);
    }
}

// svg2pdf still uses fontdb 0.23. Keep its font fallback policy identical to
// the maintained font stack used by raster export and visible-content bounds.
pub(super) fn load_pdf_system_fonts(database: &mut svg2pdf::usvg::fontdb::Database) {
    database.load_system_fonts();
    let generic = GenericFamilies::of(database.faces().flat_map(|face| {
        face.families
            .iter()
            .map(move |(family, _)| (family.as_str(), face.monospaced))
    }));
    if let Some(family) = generic.sans_serif {
        database.set_sans_serif_family(family);
    }
    if let Some(family) = generic.serif {
        database.set_serif_family(family);
    }
}

/// Installed families for SVG's generic names; none keeps fontdb's default.
struct GenericFamilies {
    sans_serif: Option<String>,
    serif: Option<String>,
}

impl GenericFamilies {
    fn of<'a>(faces: impl Iterator<Item = (&'a str, bool)>) -> Self {
        let faces: Vec<_> = faces.collect();
        Self {
            sans_serif: preferred_sans_family(faces.iter().copied()).map(str::to_owned),
            serif: preferred_family(faces.iter().map(|(family, _)| *family), PREFERRED_SERIF)
                .map(str::to_owned),
        }
    }
}

/// The serif plot face, then its metric-compatible substitute.
const PREFERRED_SERIF: &[&str] = &["Times New Roman", "Liberation Serif", "DejaVu Serif"];

/// The installed family earliest in `preferred`, if any.
fn preferred_family<'a>(
    families: impl Iterator<Item = &'a str>,
    preferred: &[&str],
) -> Option<&'a str> {
    families
        .filter_map(|family| {
            preferred
                .iter()
                .position(|candidate| family.eq_ignore_ascii_case(candidate))
                .map(|rank| (rank, family))
        })
        .min_by_key(|(rank, _)| *rank)
        .map(|(_, family)| family)
}

fn preferred_sans_family<'a>(families: impl Iterator<Item = (&'a str, bool)>) -> Option<&'a str> {
    const PREFERRED_SANS: &[&str] = &["Arial", "Liberation Sans", "DejaVu Sans", "Noto Sans"];
    let mut preferred = None;
    let mut proportional = None;
    let mut first = None;
    for (family, monospaced) in families {
        first.get_or_insert(family);
        if !monospaced {
            proportional.get_or_insert(family);
        }
        if let Some(rank) = PREFERRED_SANS
            .iter()
            .position(|candidate| family.eq_ignore_ascii_case(candidate))
            && preferred.is_none_or(|(best, _)| rank < best)
        {
            preferred = Some((rank, family));
        }
    }
    preferred
        .map(|(_, family)| family)
        .or(proportional)
        .or(first)
}

/// Diagnostics use the same export font discovery, while retaining estimated
/// text bounds rather than claiming that every glyph was shaped successfully.
pub(crate) fn diagnostic_font_status() -> &'static str {
    let mut db = Database::new();
    load_system_fonts(&mut db);
    if db.faces().next().is_none() {
        "missing"
    } else {
        "available_glyph_coverage_unverified"
    }
}

#[cfg(test)]
mod tests {
    use super::{PREFERRED_SERIF, preferred_family, preferred_sans_family};

    #[test]
    fn serif_preference_picks_the_closest_substitute() {
        assert_eq!(
            preferred_family(
                ["DejaVu Serif", "Liberation Serif", "Noto Sans"].into_iter(),
                PREFERRED_SERIF
            ),
            Some("Liberation Serif")
        );
        assert_eq!(
            preferred_family(["Noto Sans"].into_iter(), PREFERRED_SERIF),
            None
        );
    }

    #[test]
    fn sans_preference_is_independent_of_font_discovery_order() {
        assert_eq!(
            preferred_sans_family(
                [
                    ("DejaVu Sans", false),
                    ("ARIAL", false),
                    ("Noto Sans", false)
                ]
                .into_iter()
            ),
            Some("ARIAL")
        );
    }

    #[test]
    fn sans_fallback_prefers_proportional_and_handles_empty_font_database() {
        assert_eq!(
            preferred_sans_family([("Mono", true), ("Proportional", false)].into_iter()),
            Some("Proportional")
        );
        assert_eq!(
            preferred_sans_family([("Mono", true)].into_iter()),
            Some("Mono")
        );
        assert_eq!(preferred_sans_family(std::iter::empty()), None);
    }
}
