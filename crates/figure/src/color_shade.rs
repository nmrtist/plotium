//! Perceptual shading of colors in OKLab (Björn Ottosson, 2020).
use crate::Color;

/// OKLab lightness of an ink shade: near black, so thin rules read clearly on
/// white paper, yet light enough to keep a trace of the hue.
const INK_LIGHTNESS: f64 = 0.31;
/// Share of the source chroma an ink shade keeps, and its absolute cap, so a
/// vivid fill tints the ink without making it colorful.
const INK_CHROMA_SHARE: f64 = 0.32;
const INK_CHROMA_MAX: f64 = 0.032;

impl Color {
    /// A near-black ink tinted toward this color's hue, for outlines and
    /// whiskers drawn around a fill. Every source maps to the same OKLab
    /// lightness, so inks around different fills carry equal visual weight;
    /// a neutral source gives a neutral dark grey. A fill already darker than
    /// that keeps its own lightness, so the ink never reads lighter than it.
    pub fn ink_shade(self) -> Color {
        let [lightness, a, b] = to_oklab(self);
        let chroma = a.hypot(b);
        let keep = if chroma > 1e-9 {
            (INK_CHROMA_SHARE * chroma).min(INK_CHROMA_MAX) / chroma
        } else {
            0.0
        };
        from_oklab([INK_LIGHTNESS.min(lightness), a * keep, b * keep])
    }
}

fn to_linear(channel: u8) -> f64 {
    let c = f64::from(channel) / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn to_srgb(linear: f64) -> u8 {
    let c = linear.clamp(0.0, 1.0);
    let v = if c <= 0.003_130_8 {
        12.92 * c
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn to_oklab(color: Color) -> [f64; 3] {
    let [r, g, b] = [color.r, color.g, color.b].map(to_linear);
    let l = (0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b).cbrt();
    let m = (0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b).cbrt();
    let s = (0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b).cbrt();
    [
        0.210_454_255_3 * l + 0.793_617_785_0 * m - 0.004_072_046_8 * s,
        1.977_998_495_1 * l - 2.428_592_205_0 * m + 0.450_593_709_9 * s,
        0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766_0 * s,
    ]
}

fn from_oklab([lightness, a, b]: [f64; 3]) -> Color {
    let l = (lightness + 0.396_337_777_4 * a + 0.215_803_757_3 * b).powi(3);
    let m = (lightness - 0.105_561_345_8 * a - 0.063_854_172_8 * b).powi(3);
    let s = (lightness - 0.089_484_177_5 * a - 1.291_485_548_0 * b).powi(3);
    Color::rgb(
        to_srgb(4.076_741_662_1 * l - 3.307_711_591_3 * m + 0.230_969_929_2 * s),
        to_srgb(-1.268_438_004_6 * l + 2.609_757_401_1 * m - 0.341_319_396_5 * s),
        to_srgb(-0.004_196_086_3 * l - 0.703_418_614_7 * m + 1.707_614_701_0 * s),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(color: Color) -> String {
        format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b)
    }

    /// WCAG contrast ratio against white.
    fn contrast_on_white(color: Color) -> f64 {
        let [r, g, b] = [color.r, color.g, color.b].map(to_linear);
        1.05 / (0.2126 * r + 0.7152 * g + 0.0722 * b + 0.05)
    }

    #[test]
    fn oklab_round_trips_srgb() {
        for color in [
            Color::BLACK,
            Color::rgb(255, 255, 255),
            Color::rgb(0xf5, 0xd7, 0x8e),
            Color::rgb(0x0f, 0x4d, 0x92),
        ] {
            assert_eq!(hex(from_oklab(to_oklab(color))), hex(color));
        }
    }

    #[test]
    fn ink_shades_are_near_black_and_keep_the_hue() {
        assert_eq!(hex(Color::rgb(0xf5, 0xd7, 0x8e).ink_shade()), "#37301e");
        assert_eq!(hex(Color::rgb(0x93, 0xb5, 0xdc).ink_shade()), "#29313b");
        // A neutral fill gives a neutral grey.
        let grey = Color::rgb(0xd9, 0xd9, 0xd9).ink_shade();
        assert!(grey.r == grey.g && grey.g == grey.b, "{}", hex(grey));
        // Pale and dark, warm and cool fills all get an equally strong ink.
        for fill in [
            Color::rgb(0xf5, 0xd7, 0x8e),
            Color::rgb(0x3f, 0xa9, 0xf5),
            Color::rgb(0x0f, 0x4d, 0x92),
            Color::rgb(0xd1, 0x24, 0x2a),
            Color::rgb(255, 255, 255),
        ] {
            let ink = fill.ink_shade();
            let ratio = contrast_on_white(ink);
            assert!((12.0..14.5).contains(&ratio), "{} -> {ratio:.1}", hex(ink));
        }
        // Darker fills keep their own lightness: black stays black.
        assert_eq!(Color::BLACK.ink_shade(), Color::BLACK);
        let navy = Color::rgb(0x0b, 0x1f, 0x3a);
        // Rounding back to 8-bit channels may nudge lightness by a hair.
        assert!(to_oklab(navy.ink_shade())[0] <= to_oklab(navy)[0] + 2e-3);
    }
}
