use resvg::usvg::fontdb::Database;

/// Load platform fonts and make SVG's generic `sans-serif` family resolve on
/// systems that do not install fontdb's default Arial family (notably Linux).
pub(super) fn load_system_fonts(database: &mut Database) {
    database.load_system_fonts();

    let family = preferred_sans_family(database.faces().flat_map(|face| {
        face.families
            .iter()
            .map(move |(family, _)| (family.as_str(), face.monospaced))
    }))
    .map(str::to_owned);
    if let Some(family) = family {
        database.set_sans_serif_family(family);
    }
}

// svg2pdf still uses fontdb 0.23. Keep its font fallback policy identical to
// the maintained font stack used by raster export and visible-content bounds.
pub(super) fn load_pdf_system_fonts(database: &mut svg2pdf::usvg::fontdb::Database) {
    database.load_system_fonts();
    let family = preferred_sans_family(database.faces().flat_map(|face| {
        face.families
            .iter()
            .map(move |(family, _)| (family.as_str(), face.monospaced))
    }))
    .map(str::to_owned);
    if let Some(family) = family {
        database.set_sans_serif_family(family);
    }
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

#[cfg(test)]
mod tests {
    use super::preferred_sans_family;

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
