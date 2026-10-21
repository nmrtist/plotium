use super::*;
use crate::theme::MarkerPaint;
use plotium_figure::{Color, PositiveFiniteF32};

fn temp_styles_dir(name: &str) -> PathBuf {
    let base = std::env::var_os("CARGO_TARGET_TMPDIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let dir = base.join(format!("plotium-styles-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn save_list_load_delete_roundtrip() {
    let dir = temp_styles_dir("roundtrip");
    let mut style = Theme::by_id("nature").unwrap();
    style.marker_palette = vec![
        Some(MarkerPaint {
            fill: Some(Color::rgb(0xf2, 0xd7, 0x97)),
            edge_color: Some(Color::rgb(0x40, 0x40, 0x40)),
            edge_width_pt: PositiveFiniteF32::new(0.5),
            hollow: true,
        }),
        None,
    ];

    assert!(list_user_styles(&dir).is_empty());
    let path = save_user_style(&dir, "  Lab paper  ", &style).unwrap();
    assert_eq!(path.extension().unwrap(), STYLE_EXTENSION);
    assert!(user_style_exists(&dir, "Lab paper"));

    let listed = list_user_styles(&dir);
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].name, "Lab paper");

    let loaded = load_user_style(&dir, "Lab paper").unwrap();
    assert_eq!(
        (loaded.id.as_str(), loaded.name.as_str()),
        ("Lab paper", "Lab paper")
    );
    assert_eq!(
        Theme {
            id: style.id.clone(),
            name: style.name.clone(),
            ..loaded
        },
        style
    );

    delete_user_style(&dir, "Lab paper").unwrap();
    assert!(list_user_styles(&dir).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn builtin_ids_and_invalid_names_are_rejected_before_writing() {
    let dir = temp_styles_dir("reserved");
    let theme = Theme::by_id("publication").unwrap();
    for bad in ["nature", "Publication", "", "a/b", "con"] {
        assert!(
            matches!(
                save_user_style(&dir, bad, &theme),
                Err(StyleError::Invalid(_))
            ),
            "accepted {bad:?}"
        );
    }
    assert!(!dir.exists());
}

#[test]
fn file_name_is_the_style_identity() {
    let dir = temp_styles_dir("identity");
    let theme = Theme::by_id("vibrant").unwrap();
    let saved = save_user_style(&dir, "Old", &theme).unwrap();
    std::fs::rename(&saved, dir.join(format!("New.{STYLE_EXTENSION}"))).unwrap();

    let loaded = load_user_style(&dir, "New").unwrap();
    assert_eq!((loaded.id.as_str(), loaded.name.as_str()), ("New", "New"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn malformed_or_unusable_files_are_reported() {
    let dir = temp_styles_dir("malformed");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(format!("Broken.{STYLE_EXTENSION}")), b"{}").unwrap();
    assert!(matches!(
        load_user_style(&dir, "Broken"),
        Err(StyleError::Format(_))
    ));

    let path = save_user_style(&dir, "Tiny", &Theme::by_id("nature").unwrap()).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        &path,
        text.replace("\"base_font_pt\": 7.0", "\"base_font_pt\": 0.0"),
    )
    .unwrap();
    assert!(matches!(
        load_user_style(&dir, "Tiny"),
        Err(StyleError::Invalid(_))
    ));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn files_that_cannot_be_managed_by_name_are_not_listed() {
    let dir = temp_styles_dir("unlisted");
    let theme = Theme::by_id("nature").unwrap();
    let saved = save_user_style(&dir, "Kept", &theme).unwrap();
    for name in ["nature", " padded ", "a?b"] {
        std::fs::copy(&saved, dir.join(format!("{name}.{STYLE_EXTENSION}"))).unwrap();
    }
    let names: Vec<_> = list_user_styles(&dir).into_iter().map(|s| s.name).collect();
    assert_eq!(names, ["Kept"]);
    let _ = std::fs::remove_dir_all(&dir);
}
