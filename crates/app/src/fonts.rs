use std::sync::Arc;

use egui::{FontData, FontDefinitions, FontFamily};
use fontdb::{Database, Family, Query, Weight};

use crate::typography::EMPHASIZED_FAMILY_NAME;

const SYSTEM_UI_REGULAR_KEY: &str = "plotium-system-ui-regular";
const SYSTEM_UI_SEMIBOLD_KEY: &str = "plotium-system-ui-semibold";
const SYSTEM_CJK_REGULAR_KEY: &str = "plotium-system-cjk-regular";
const SYSTEM_CJK_SEMIBOLD_KEY: &str = "plotium-system-cjk-semibold";
const FIGURE_KEY: &str = "plotium-figure-regular";

#[derive(Clone)]
struct LoadedFont {
    bytes: Vec<u8>,
    index: u32,
}

#[derive(Default)]
struct PlatformFonts {
    system_ui_regular: Option<LoadedFont>,
    system_ui_semibold: Option<LoadedFont>,
    system_cjk_regular: Option<LoadedFont>,
    system_cjk_semibold: Option<LoadedFont>,
    /// Arial or Helvetica, so figures on the canvas match their exports.
    figure: Option<LoadedFont>,
}

pub(crate) fn definitions() -> FontDefinitions {
    build_definitions(load_platform_fonts())
}

fn build_definitions(platform: PlatformFonts) -> FontDefinitions {
    let mut fonts = FontDefinitions::default();

    if let Some(font) = platform.system_cjk_regular.clone() {
        insert_font(&mut fonts, SYSTEM_CJK_REGULAR_KEY, font);
    }
    if let Some(font) = platform.system_cjk_semibold.clone() {
        insert_font(&mut fonts, SYSTEM_CJK_SEMIBOLD_KEY, font);
    }
    if let Some(font) = platform.system_ui_regular.clone() {
        insert_font(&mut fonts, SYSTEM_UI_REGULAR_KEY, font);
    }
    if let Some(font) = platform.system_ui_semibold.clone() {
        insert_font(&mut fonts, SYSTEM_UI_SEMIBOLD_KEY, font);
    }
    if let Some(font) = platform.figure.clone() {
        insert_font(&mut fonts, FIGURE_KEY, font);
    }

    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    prioritize_proportional(&mut fonts, &platform);
    build_emphasized_family(&mut fonts, &platform);
    build_figure_family(&mut fonts, &platform);
    fonts
}

/// Figure text uses the export typeface first and the UI stack for glyphs it
/// lacks, such as CJK.
fn build_figure_family(fonts: &mut FontDefinitions, platform: &PlatformFonts) {
    let mut family = Vec::new();
    if platform.figure.is_some() {
        family.push(FIGURE_KEY.to_owned());
    }
    family.extend(
        fonts
            .families
            .get(&FontFamily::Proportional)
            .expect("egui default fonts include a proportional family")
            .iter()
            .cloned(),
    );
    fonts.families.insert(
        FontFamily::Name(plotium_render::rich_text::FIGURE_FONT_FAMILY.into()),
        family,
    );
}

fn insert_font(fonts: &mut FontDefinitions, key: &str, font: LoadedFont) {
    let mut data = FontData::from_owned(font.bytes);
    data.index = font.index;
    fonts.font_data.insert(key.to_owned(), Arc::new(data));
}

fn prioritize_proportional(fonts: &mut FontDefinitions, platform: &PlatformFonts) {
    let proportional = fonts
        .families
        .get_mut(&FontFamily::Proportional)
        .expect("egui default fonts include a proportional family");
    let mut preferred = Vec::new();
    if platform.system_ui_regular.is_some() {
        preferred.push(SYSTEM_UI_REGULAR_KEY.to_owned());
    }
    preferred.push("phosphor".to_owned());
    if platform.system_cjk_regular.is_some() {
        preferred.push(SYSTEM_CJK_REGULAR_KEY.to_owned());
    }
    for name in preferred.iter().rev() {
        if let Some(index) = proportional.iter().position(|candidate| candidate == name) {
            proportional.remove(index);
        }
        proportional.insert(0, name.clone());
    }
}

fn build_emphasized_family(fonts: &mut FontDefinitions, platform: &PlatformFonts) {
    let mut family = Vec::new();
    if platform.system_ui_semibold.is_some() {
        family.push(SYSTEM_UI_SEMIBOLD_KEY.to_owned());
    } else if platform.system_ui_regular.is_some() {
        family.push(SYSTEM_UI_REGULAR_KEY.to_owned());
    }
    family.push("phosphor".to_owned());
    if platform.system_cjk_semibold.is_some() {
        family.push(SYSTEM_CJK_SEMIBOLD_KEY.to_owned());
    } else if platform.system_cjk_regular.is_some() {
        family.push(SYSTEM_CJK_REGULAR_KEY.to_owned());
    }
    for fallback in fonts
        .families
        .get(&FontFamily::Proportional)
        .expect("egui default fonts include a proportional family")
    {
        if !family.contains(fallback) {
            family.push(fallback.clone());
        }
    }
    fonts
        .families
        .insert(FontFamily::Name(EMPHASIZED_FAMILY_NAME.into()), family);
}

fn load_platform_fonts() -> PlatformFonts {
    let mut db = Database::new();
    db.load_system_fonts();

    let (ui_families, cjk_families) = platform_family_queries();
    PlatformFonts {
        system_ui_regular: load(
            &db,
            "the system UI regular font",
            &ui_families,
            Weight::NORMAL,
        ),
        system_ui_semibold: load(
            &db,
            "the system UI semibold font",
            &ui_families,
            Weight::SEMIBOLD,
        ),
        system_cjk_regular: load(
            &db,
            "the system CJK regular font",
            &cjk_families,
            Weight::NORMAL,
        ),
        system_cjk_semibold: load(
            &db,
            "the system CJK semibold font",
            &cjk_families,
            Weight::SEMIBOLD,
        ),
        figure: load(
            &db,
            "Arial or Helvetica for figure text",
            &[
                Family::Name("Arial"),
                Family::Name("Helvetica"),
                Family::Name("Liberation Sans"),
            ],
            Weight::NORMAL,
        ),
    }
}

fn load(db: &Database, label: &str, families: &[Family<'_>], weight: Weight) -> Option<LoadedFont> {
    let result = db
        .query(&Query {
            families,
            weight,
            ..Query::default()
        })
        .ok_or_else(|| format!("could not find {label}"))
        .and_then(|id| {
            db.with_face_data(id, |bytes, index| LoadedFont {
                bytes: bytes.to_vec(),
                index,
            })
            .ok_or_else(|| format!("could not read {label}"))
        });

    match result {
        Ok(font) => Some(font),
        Err(error) => {
            eprintln!("Plotium font fallback: {error}; using the remaining font stack");
            None
        }
    }
}

#[cfg(target_os = "macos")]
fn platform_family_queries() -> (Vec<Family<'static>>, Vec<Family<'static>>) {
    (
        vec![
            Family::Name("System Font"),
            Family::Name(".SF NS Text"),
            Family::Name("SF Pro Text"),
        ],
        vec![
            Family::Name("PingFang SC"),
            Family::Name("Hiragino Sans GB"),
        ],
    )
}

#[cfg(windows)]
fn platform_family_queries() -> (Vec<Family<'static>>, Vec<Family<'static>>) {
    (
        vec![Family::Name("Segoe UI Variable"), Family::Name("Segoe UI")],
        vec![
            Family::Name("Microsoft YaHei UI"),
            Family::Name("Microsoft YaHei"),
        ],
    )
}

#[cfg(all(unix, not(target_os = "macos")))]
fn platform_family_queries() -> (Vec<Family<'static>>, Vec<Family<'static>>) {
    (
        vec![Family::SansSerif],
        vec![
            Family::Name("Noto Sans CJK SC"),
            Family::Name("Noto Sans SC"),
            Family::Name("Source Han Sans SC"),
            Family::Name("WenQuanYi Zen Hei"),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use skrifa::{FontRef, MetadataProvider};

    fn fake_font(index: u32) -> LoadedFont {
        LoadedFont {
            bytes: vec![0],
            index,
        }
    }

    #[test]
    fn system_fonts_have_the_intended_family_priority() {
        let fonts = build_definitions(PlatformFonts {
            system_ui_regular: Some(fake_font(1)),
            system_ui_semibold: Some(fake_font(2)),
            system_cjk_regular: Some(fake_font(3)),
            system_cjk_semibold: Some(fake_font(4)),
            figure: Some(fake_font(5)),
        });

        assert_eq!(
            &fonts.families[&FontFamily::Proportional][..3],
            [SYSTEM_UI_REGULAR_KEY, "phosphor", SYSTEM_CJK_REGULAR_KEY]
        );
        let emphasized = FontFamily::Name(EMPHASIZED_FAMILY_NAME.into());
        assert_eq!(
            &fonts.families[&emphasized][..3],
            [SYSTEM_UI_SEMIBOLD_KEY, "phosphor", SYSTEM_CJK_SEMIBOLD_KEY]
        );
        assert_eq!(fonts.font_data[SYSTEM_UI_REGULAR_KEY].index, 1);
        assert_eq!(fonts.font_data[SYSTEM_UI_SEMIBOLD_KEY].index, 2);
        assert_eq!(fonts.font_data[SYSTEM_CJK_REGULAR_KEY].index, 3);
        assert_eq!(fonts.font_data[SYSTEM_CJK_SEMIBOLD_KEY].index, 4);
        let figure = FontFamily::Name(plotium_render::rich_text::FIGURE_FONT_FAMILY.into());
        assert_eq!(
            &fonts.families[&figure][..2],
            [FIGURE_KEY, SYSTEM_UI_REGULAR_KEY]
        );
    }

    #[test]
    fn regular_face_substitutes_when_semibold_is_missing() {
        let fonts = build_definitions(PlatformFonts {
            system_ui_regular: Some(fake_font(1)),
            system_ui_semibold: None,
            system_cjk_regular: None,
            system_cjk_semibold: None,
            figure: None,
        });
        let emphasized = FontFamily::Name(EMPHASIZED_FAMILY_NAME.into());
        assert_eq!(fonts.families[&emphasized][0], SYSTEM_UI_REGULAR_KEY);
    }

    #[test]
    fn missing_system_fonts_preserve_portable_fallbacks() {
        let defaults = FontDefinitions::default();
        let fonts = build_definitions(PlatformFonts::default());

        assert_eq!(fonts.families[&FontFamily::Proportional][0], "phosphor");
        assert!(
            fonts.families[&FontFamily::Proportional]
                .iter()
                .any(|font| defaults.families[&FontFamily::Proportional].contains(font))
        );
        assert_eq!(
            fonts.families[&FontFamily::Monospace],
            defaults.families[&FontFamily::Monospace]
        );
    }

    #[test]
    fn cjk_fallback_never_preempts_phosphor() {
        let fonts = build_definitions(PlatformFonts {
            system_ui_regular: None,
            system_ui_semibold: None,
            system_cjk_regular: Some(fake_font(3)),
            system_cjk_semibold: None,
            figure: None,
        });
        let proportional = &fonts.families[&FontFamily::Proportional];
        let phosphor = proportional
            .iter()
            .position(|name| name == "phosphor")
            .unwrap();
        let cjk = proportional
            .iter()
            .position(|name| name == SYSTEM_CJK_REGULAR_KEY)
            .unwrap();
        assert!(phosphor < cjk);
    }

    #[test]
    fn loaded_platform_faces_have_expected_glyph_coverage() {
        let platform = load_platform_fonts();
        for face in [
            platform.system_ui_regular.as_ref(),
            platform.system_ui_semibold.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            let face = FontRef::from_index(&face.bytes, face.index).expect("valid system UI face");
            assert!(face.charmap().map('A').is_some());
        }
        if let Some(cjk) = platform.system_cjk_regular.as_ref() {
            let cjk = FontRef::from_index(&cjk.bytes, cjk.index).expect("valid system CJK face");
            for ch in ['中', '文', '图'] {
                assert!(cjk.charmap().map(ch).is_some(), "missing glyph {ch}");
            }
        }
    }
}
