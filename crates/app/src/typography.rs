//! Semantic typography for the application chrome.

use egui::{FontFamily, FontId, RichText, TextStyle};

pub(crate) const LARGE_TITLE_PT: f32 = 26.0;
pub(crate) const TITLE_1_PT: f32 = 22.0;
pub(crate) const TITLE_2_PT: f32 = 17.0;
pub(crate) const TITLE_3_PT: f32 = 15.0;
pub(crate) const HEADLINE_PT: f32 = 13.0;
pub(crate) const BODY_PT: f32 = 13.0;
pub(crate) const CALLOUT_PT: f32 = 12.0;
pub(crate) const SUBHEADLINE_PT: f32 = 12.0;
pub(crate) const CAPTION_PT: f32 = 11.0;
pub(crate) const MONOSPACE_PT: f32 = 13.0;
/// Count badges sit in the corner of a 30 pt icon button; anything larger
/// covers the glyph, so they stay below the caption size.
pub(crate) const BADGE_PT: f32 = 9.0;

pub(crate) const EMPHASIZED_FAMILY_NAME: &str = "plotium-emphasized";

const LARGE_TITLE_STYLE: &str = "plotium.large-title";
const TITLE_1_STYLE: &str = "plotium.title-1";
const TITLE_3_STYLE: &str = "plotium.title-3";
const HEADLINE_STYLE: &str = "plotium.headline";
const CALLOUT_STYLE: &str = "plotium.callout";
const SUBHEADLINE_STYLE: &str = "plotium.subheadline";
const CAPTION_STYLE: &str = "plotium.caption";

fn named(name: &'static str) -> TextStyle {
    TextStyle::Name(name.into())
}

fn emphasized() -> FontFamily {
    FontFamily::Name(EMPHASIZED_FAMILY_NAME.into())
}

pub(crate) fn apply(ctx: &egui::Context) {
    ctx.all_styles_mut(|style| {
        style.text_styles.insert(
            TextStyle::Small,
            FontId::new(CAPTION_PT, FontFamily::Proportional),
        );
        style.text_styles.insert(
            TextStyle::Body,
            FontId::new(BODY_PT, FontFamily::Proportional),
        );
        style.text_styles.insert(
            TextStyle::Button,
            FontId::new(BODY_PT, FontFamily::Proportional),
        );
        style
            .text_styles
            .insert(TextStyle::Heading, FontId::new(TITLE_2_PT, emphasized()));
        style.text_styles.insert(
            TextStyle::Monospace,
            FontId::new(MONOSPACE_PT, FontFamily::Monospace),
        );
        for (name, size, family) in [
            (LARGE_TITLE_STYLE, LARGE_TITLE_PT, FontFamily::Proportional),
            (TITLE_1_STYLE, TITLE_1_PT, FontFamily::Proportional),
            (TITLE_3_STYLE, TITLE_3_PT, emphasized()),
            (HEADLINE_STYLE, HEADLINE_PT, emphasized()),
            (CALLOUT_STYLE, CALLOUT_PT, FontFamily::Proportional),
            (SUBHEADLINE_STYLE, SUBHEADLINE_PT, FontFamily::Proportional),
            (CAPTION_STYLE, CAPTION_PT, FontFamily::Proportional),
        ] {
            style
                .text_styles
                .insert(named(name), FontId::new(size, family));
        }
    });
}

pub(crate) fn large_title(text: impl Into<String>) -> RichText {
    RichText::new(text).text_style(named(LARGE_TITLE_STYLE))
}

pub(crate) fn title_3(text: impl Into<String>) -> RichText {
    RichText::new(text).text_style(named(TITLE_3_STYLE))
}

pub(crate) fn headline(text: impl Into<String>) -> RichText {
    RichText::new(text).text_style(named(HEADLINE_STYLE))
}

pub(crate) fn headline_label(ui: &mut egui::Ui, text: impl Into<String>) -> egui::Response {
    ui.label(headline(text))
}

pub(crate) fn callout(text: impl Into<String>) -> RichText {
    RichText::new(text).text_style(named(CALLOUT_STYLE))
}

pub(crate) fn subheadline_font() -> FontId {
    FontId::new(SUBHEADLINE_PT, FontFamily::Proportional)
}

pub(crate) fn headline_font() -> FontId {
    FontId::new(HEADLINE_PT, emphasized())
}

pub(crate) fn callout_font() -> FontId {
    FontId::new(CALLOUT_PT, FontFamily::Proportional)
}

pub(crate) fn caption(text: impl Into<String>) -> RichText {
    RichText::new(text).text_style(named(CAPTION_STYLE))
}

pub(crate) fn caption_font() -> FontId {
    FontId::new(CAPTION_PT, FontFamily::Proportional)
}

pub(crate) fn badge_font() -> FontId {
    FontId::new(BADGE_PT, FontFamily::Proportional)
}

#[cfg(test)]
pub(crate) fn test_context() -> egui::Context {
    let ctx = egui::Context::default();
    let mut fonts = egui::FontDefinitions::default();
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    let emphasized = fonts
        .families
        .get(&FontFamily::Proportional)
        .expect("egui test fonts include a proportional family")
        .clone();
    fonts
        .families
        .insert(FontFamily::Name(EMPHASIZED_FAMILY_NAME.into()), emphasized);
    ctx.set_fonts(fonts);
    apply(&ctx);
    ctx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_scale_keeps_the_chrome_labels_readable() {
        assert_eq!(
            [
                LARGE_TITLE_PT,
                TITLE_1_PT,
                TITLE_2_PT,
                TITLE_3_PT,
                BODY_PT,
                CALLOUT_PT,
                SUBHEADLINE_PT,
                CAPTION_PT,
            ],
            [26.0, 22.0, 17.0, 15.0, 13.0, 12.0, 12.0, 11.0]
        );
    }

    #[test]
    fn default_application_styles_never_drop_below_eleven_points() {
        let ctx = egui::Context::default();
        apply(&ctx);
        let style = ctx.global_style();
        for text_style in [
            TextStyle::Small,
            TextStyle::Body,
            TextStyle::Button,
            TextStyle::Heading,
            TextStyle::Monospace,
        ] {
            assert!(style.text_styles[&text_style].size >= 11.0);
        }
    }
}
