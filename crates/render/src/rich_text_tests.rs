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
