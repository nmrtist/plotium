use super::*;

/// macOS accent choices (light and dark variants), Windows' default, and
/// extremes a custom Windows accent can reach.
const ACCENTS: [[u8; 3]; 14] = [
    [0, 122, 255],
    [10, 132, 255],
    [175, 82, 222],
    [255, 45, 85],
    [255, 59, 48],
    [255, 149, 0],
    [255, 204, 0],
    [40, 205, 65],
    [142, 142, 147],
    [0, 120, 212],
    [255, 255, 255],
    [0, 0, 0],
    [128, 128, 128],
    [0, 255, 255],
];

fn themes() -> [Visuals; 2] {
    [Visuals::light(), Visuals::dark()]
}

#[test]
fn selected_text_stays_readable_for_every_accent() {
    for [r, g, b] in ACCENTS {
        for mut visuals in themes() {
            apply(&mut visuals, Some(Color32::from_rgb(r, g, b)));
            let ratio = contrast_ratio(visuals.selection.stroke.color, visuals.selection.bg_fill);
            assert!(
                ratio >= SELECTED_TEXT_CONTRAST,
                "accent {r},{g},{b} dark={} gives {ratio:.2}",
                visuals.dark_mode
            );
        }
    }
}

#[test]
fn links_keep_the_stock_contrast_for_every_accent() {
    for [r, g, b] in ACCENTS {
        for mut visuals in themes() {
            apply(&mut visuals, Some(Color32::from_rgb(r, g, b)));
            let ratio = contrast_ratio(visuals.hyperlink_color, visuals.panel_fill);
            assert!(
                ratio >= LINK_CONTRAST,
                "accent {r},{g},{b} dark={} gives {ratio:.2}",
                visuals.dark_mode
            );
        }
    }
}

#[test]
fn the_selection_fill_stands_out_from_the_panel_for_every_accent() {
    for [r, g, b] in ACCENTS {
        for mut visuals in themes() {
            apply(&mut visuals, Some(Color32::from_rgb(r, g, b)));
            let ratio = contrast_ratio(visuals.selection.bg_fill, visuals.panel_fill);
            assert!(
                ratio >= FILL_CONTRAST,
                "accent {r},{g},{b} dark={} gives {ratio:.2}",
                visuals.dark_mode
            );
        }
    }
}

#[test]
fn the_selection_fill_carries_the_accent_hue() {
    let blue = Color32::from_rgb(0, 122, 255);
    for mut visuals in themes() {
        apply(&mut visuals, Some(blue));
        let fill = visuals.selection.bg_fill;
        assert!(fill.b() > fill.r() && fill.b() > fill.g(), "{fill:?}");
    }
}

#[test]
fn no_accent_restores_the_stock_colours() {
    for stock in themes() {
        let mut visuals = stock.clone();
        apply(&mut visuals, Some(Color32::from_rgb(255, 59, 48)));
        apply(&mut visuals, None);
        assert_eq!(visuals.selection, stock.selection);
        assert_eq!(visuals.hyperlink_color, stock.hyperlink_color);
    }
}

#[test]
fn reapplying_the_same_accent_is_stable() {
    let accent = Some(Color32::from_rgb(175, 82, 222));
    for mut visuals in themes() {
        apply(&mut visuals, accent);
        let first = visuals.clone();
        apply(&mut visuals, accent);
        assert_eq!(visuals, first);
    }
}

#[test]
fn focus_regain_fires_once_per_transition() {
    let ctx = Context::default();
    let frame = |focused: bool| {
        let input = egui::RawInput {
            focused,
            ..Default::default()
        };
        let mut fired = false;
        let _ = ctx.run_ui(input, |ui| fired = regained_focus(ui.ctx()));
        fired
    };
    assert!(!frame(true), "the first frame has no previous focus state");
    assert!(!frame(false));
    assert!(frame(true));
    assert!(!frame(true));
}
