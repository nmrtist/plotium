use super::*;

fn run(text: &str, script: Script) -> Run {
    Run {
        text: text.to_owned(),
        script,
    }
}

#[test]
fn brace_markup_and_unicode_scripts_split_into_runs() {
    let expected = vec![run("mA cm", Script::Normal), run("−2", Script::Super)];
    assert_eq!(runs("mA cm^{-2}"), expected);
    assert_eq!(runs("mA cm⁻²"), expected);
    assert_eq!(
        runs("H_{2}O"),
        vec![
            run("H", Script::Normal),
            run("2", Script::Sub),
            run("O", Script::Normal)
        ]
    );
    assert_eq!(runs("H₂O"), runs("H_{2}O"));
}

#[test]
fn bare_carets_and_underscores_stay_literal() {
    for text in ["sample_1", "x^2", "a_{open", "plain"] {
        assert_eq!(runs(text), vec![run(text, Script::Normal)]);
        assert!(!has_scripts(text));
    }
    assert!(has_scripts("cm⁻²"));
    assert_eq!(plain("×10^{5} s_{1}"), "×105 s1");
}

#[test]
fn single_run_fallback_uses_unicode_scripts() {
    assert_eq!(unicode("mA cm^{-2}"), "mA cm⁻²");
    assert_eq!(unicode("H_{2}O"), "H₂O");
    assert_eq!(unicode("x^{a}"), "xa");
}

#[test]
fn svg_scripts_are_smaller_shifted_tspans() {
    let svg = svg_content("cm^{-2} & s", 10.0);
    assert_eq!(
        svg,
        r#"cm<tspan dy="-3.30" font-size="7.00">−2</tspan><tspan dy="3.30"> &amp; s</tspan>"#
    );
    assert_eq!(svg_content("a<b", 10.0), "a&lt;b");
}

#[test]
fn script_characters_count_at_their_reduced_width() {
    assert_eq!(width_in_chars("sample_1"), 8.0);
    assert!((width_in_chars("cm^{-2}") - (2.0 + 2.0 * SCRIPT_SCALE)).abs() < 1e-6);
}

#[test]
fn svg_root_names_the_figure_typeface() {
    let fig = plotium_figure::Figure::new(
        "",
        plotium_figure::Axis::new("x", 0.0, 1.0),
        plotium_figure::Axis::new("y", 0.0, 1.0),
    );
    let svg = crate::svg::export(&fig);
    assert!(svg.contains(&format!(r#"font-family="{SVG_FONT_FAMILY}""#)));
}

#[test]
fn svg_plot_group_carries_the_chosen_face_and_weight() {
    let mut fig = plotium_figure::Figure::new(
        "",
        plotium_figure::Axis::new("x", 0.0, 1.0),
        plotium_figure::Axis::new("y", 0.0, 1.0),
    );
    let svg = crate::svg::export(&fig);
    assert!(svg.contains(&format!(r#"<g font-family="{SVG_FONT_FAMILY}">"#)));

    fig.typography.typeface = plotium_figure::Typeface::Helvetica;
    let svg = crate::svg::export(&fig);
    assert!(svg.contains(r#"<g font-family="Helvetica, Arial, sans-serif">"#));

    fig.typography.typeface = plotium_figure::Typeface::TimesNewRoman;
    fig.typography.font_weight = plotium_figure::FontWeight::Bold;
    let svg = crate::svg::export(&fig);
    assert!(
        svg.contains(r#"<g font-family="'Times New Roman', Times, serif" font-weight="bold">"#)
    );
}

#[cfg(feature = "screen")]
#[test]
fn figure_font_uses_the_registered_family_for_proportional_text() {
    let figure = egui::FontFamily::Name(FIGURE_FONT_FAMILY.into());
    let fonts_for = |register: bool| {
        let ctx = egui::Context::default();
        if register {
            let mut defs = egui::FontDefinitions::default();
            let stack = defs.families[&egui::FontFamily::Proportional].clone();
            defs.families.insert(figure.clone(), stack);
            ctx.set_fonts(defs);
        }
        let mut seen = Vec::new();
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            let painter = ui.painter();
            seen.push(figure_font(painter, egui::FontId::proportional(7.0)).family);
            seen.push(figure_font(painter, egui::FontId::monospace(7.0)).family);
            let times = plotium_figure::FigureTypography {
                typeface: plotium_figure::Typeface::TimesNewRoman,
                ..Default::default()
            };
            seen.push(figure_font(painter, plot_font(7.0, &times)).family);
        });
        seen
    };
    // An unregistered plot face falls back to the default figure family.
    assert_eq!(
        fonts_for(true),
        [figure.clone(), egui::FontFamily::Monospace, figure.clone()]
    );
    assert_eq!(
        fonts_for(false),
        [
            egui::FontFamily::Proportional,
            egui::FontFamily::Monospace,
            egui::FontFamily::Proportional
        ]
    );
}

#[cfg(feature = "screen")]
#[test]
fn navigation_font_sizes_collapse_a_zoom_gesture_onto_few_sizes() {
    let sizes: std::collections::BTreeSet<u32> = (0..1_000)
        .map(|step| 10.0 + step as f32 * 0.01)
        .map(|size| {
            let snapped = navigation_font_size(size);
            assert!((snapped / size - 1.0).abs() < 0.05, "{size} -> {snapped}");
            snapped.to_bits()
        })
        .collect();
    assert!(sizes.len() <= 9, "{}", sizes.len());
}

#[cfg(feature = "screen")]
#[test]
fn navigation_text_snaps_galleys_only_while_enabled() {
    let ctx = egui::Context::default();
    let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
        let width = |size| {
            let font = egui::FontId::proportional(size);
            galley(ui.painter(), "1234", font, egui::Color32::BLACK)
                .size()
                .x
        };
        assert_ne!(width(20.0), width(20.2));
        set_navigation_text(ui.ctx(), true);
        assert_eq!(width(20.0), width(20.2));
        set_navigation_text(ui.ctx(), false);
        assert_ne!(width(20.0), width(20.2));
    });
}
