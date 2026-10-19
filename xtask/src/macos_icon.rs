//! Render macOS bundle icons and installer artwork directly from vectors.
use std::{fs, path::Path, process::Command};

fn render(source: &[u8], pixels: u32) -> Result<resvg::tiny_skia::Pixmap, String> {
    let tree = resvg::usvg::Tree::from_data(source, &resvg::usvg::Options::default())
        .map_err(|error| error.to_string())?;
    let mut image = resvg::tiny_skia::Pixmap::new(pixels, pixels).ok_or("Invalid icon size")?;
    // Match assets/icon-256-macos.png: 210 px artwork inside a 256 px canvas.
    let padding = pixels as f32 * 23.0 / 256.0;
    let scale = pixels as f32 * 210.0 / 256.0 / tree.size().width();
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_row(scale, 0.0, 0.0, scale, padding, padding),
        &mut image.as_mut(),
    );
    Ok(image)
}

pub(super) fn create(root: &Path, output: &Path) -> Result<(), String> {
    let regular = fs::read(root.join("assets/icon.svg")).map_err(|e| e.to_string())?;
    let small = fs::read(root.join("assets/icon-small.svg")).map_err(|e| e.to_string())?;
    let iconset = output.with_extension("iconset");
    fs::create_dir_all(&iconset).map_err(|e| e.to_string())?;
    for size in [16, 32, 128, 256, 512] {
        for retina in [false, true] {
            let pixels = if retina { size * 2 } else { size };
            let image = render(if pixels <= 32 { &small } else { &regular }, pixels)?;
            let name = format!("icon_{size}x{size}{}.png", if retina { "@2x" } else { "" });
            image
                .save_png(iconset.join(name))
                .map_err(|e| e.to_string())?;
        }
    }
    let status = Command::new("iconutil")
        .args(["-c", "icns", "-o"])
        .arg(output)
        .arg(&iconset)
        .status()
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err(format!("iconutil failed: {status}"));
    }
    fs::remove_dir_all(iconset).map_err(|e| e.to_string())
}

fn render_background(source: &[u8], scale: u32) -> Result<resvg::tiny_skia::Pixmap, String> {
    let tree = resvg::usvg::Tree::from_data(source, &resvg::usvg::Options::default())
        .map_err(|error| error.to_string())?;
    let mut image =
        resvg::tiny_skia::Pixmap::new(640 * scale, 360 * scale).ok_or("Invalid background size")?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale as f32, scale as f32),
        &mut image.as_mut(),
    );
    Ok(image)
}

pub(super) fn create_background(root: &Path, output: &Path) -> Result<(), String> {
    let source =
        fs::read(root.join("assets/macos-dmg-background.svg")).map_err(|e| e.to_string())?;
    fs::create_dir_all(output).map_err(|e| e.to_string())?;
    for scale in [1, 2] {
        let filename = if scale == 1 {
            "background.png"
        } else {
            "background@2x.png"
        };
        render_background(&source, scale)?
            .save_png(output.join(filename))
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installer_arrow_renders_at_both_resolutions_between_the_icons() {
        let source = include_bytes!("../../assets/macos-dmg-background.svg");
        for scale in [1, 2] {
            let image = render_background(source, scale).unwrap();
            assert_eq!((image.width(), image.height()), (640 * scale, 360 * scale));
            let background = image.pixel(0, 0).unwrap();
            assert_eq!(background.alpha(), 255);
            assert_ne!(image.pixel(320 * scale, 160 * scale).unwrap(), background);
            for x in [160, 480] {
                assert_eq!(image.pixel(x * scale, 160 * scale).unwrap(), background);
            }
        }
    }

    #[test]
    fn vectors_render_at_high_resolution_without_losing_dock_padding() {
        let source = include_bytes!("../../assets/icon.svg");
        for size in [256, 1024] {
            let image = render(source, size).unwrap();
            assert_eq!(image.width(), size);
            assert_eq!(image.height(), size);
            let padding = size * 23 / 256;
            for y in 0..size {
                for x in 0..size {
                    if x < padding || y < padding || x >= size - padding || y >= size - padding {
                        assert_eq!(image.pixel(x, y).unwrap().alpha(), 0);
                    }
                }
            }
            assert_ne!(image.pixel(size / 2, size / 2).unwrap().alpha(), 0);
        }
    }
}
