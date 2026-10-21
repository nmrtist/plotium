//! Super- and subscripts in figure text. `^{…}` raises a run and `_{…}`
//! lowers one; Unicode superscript and subscript characters (`cm⁻²`, `H₂O`)
//! are drawn the same way, so every label gets true scripts set in the
//! figure's own font instead of whatever glyphs a fallback font supplies.

/// Distance from a line's visual middle down to its alphabetic baseline, in
/// ems. SVG text is placed on the baseline instead of using
/// `dominant-baseline`, which rescales script runs and is ignored by many
/// vector editors.
pub const MIDDLE_TO_BASELINE: f32 = 0.35;
/// Distance from a line's top down to its baseline, in ems.
pub const TOP_TO_BASELINE: f32 = 0.75;

/// SVG font stack. Nature-portfolio journals ask for Arial or Helvetica; a
/// bare `sans-serif` lets vector editors substitute their own default.
pub const SVG_FONT_FAMILY: &str = "Arial, Helvetica, sans-serif";
/// egui family the app registers with Arial (or Helvetica) first, so figure
/// text on the canvas matches the exported typeface.
pub const FIGURE_FONT_FAMILY: &str = "plotium-figure";

/// Script size relative to the surrounding text.
pub const SCRIPT_SCALE: f32 = 0.7;
/// Baseline offsets in ems of the surrounding text.
const SUPER_RISE: f32 = 0.33;
const SUB_DROP: f32 = 0.15;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Script {
    Normal,
    Super,
    Sub,
}

impl Script {
    pub fn scale(self) -> f32 {
        match self {
            Script::Normal => 1.0,
            Script::Super | Script::Sub => SCRIPT_SCALE,
        }
    }

    /// Baseline offset for text of `font_size`, positive downward.
    pub fn baseline_offset(self, font_size: f32) -> f32 {
        match self {
            Script::Normal => 0.0,
            Script::Super => -SUPER_RISE * font_size,
            Script::Sub => SUB_DROP * font_size,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Run {
    pub text: String,
    pub script: Script,
}

/// Whether `text` needs script handling at all.
pub fn has_scripts(text: &str) -> bool {
    let candidate = text.contains("^{")
        || text.contains("_{")
        || text.chars().any(|c| unicode_script(c).is_some());
    candidate && runs(text).iter().any(|run| run.script != Script::Normal)
}

/// Split `text` into runs. A `^` or `_` not followed by a closed brace group
/// stays literal, so names like `sample_1` are untouched.
pub fn runs(text: &str) -> Vec<Run> {
    let mut out: Vec<Run> = Vec::new();
    let mut push = |ch: char, script: Script| {
        let ch = if script != Script::Normal && ch == '-' {
            '−'
        } else {
            ch
        };
        match out.last_mut() {
            Some(run) if run.script == script => run.text.push(ch),
            _ => out.push(Run {
                text: ch.to_string(),
                script,
            }),
        }
    };
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if matches!(ch, '^' | '_')
            && chars.get(i + 1) == Some(&'{')
            && let Some(len) = chars[i + 2..].iter().position(|&c| c == '}')
        {
            let script = if ch == '^' {
                Script::Super
            } else {
                Script::Sub
            };
            for &inner in &chars[i + 2..i + 2 + len] {
                push(inner, script);
            }
            i += len + 3;
            continue;
        }
        match unicode_script(ch) {
            Some((plain, script)) => push(plain, script),
            None => push(ch, Script::Normal),
        }
        i += 1;
    }
    out
}

/// `text` with markup removed and scripts flattened, for plain-text sinks.
pub fn plain(text: &str) -> String {
    runs(text).into_iter().map(|run| run.text).collect()
}

/// Length of `text` in full-size characters, counting script characters at
/// their reduced size.
pub fn width_in_chars(text: &str) -> f32 {
    runs(text)
        .iter()
        .map(|run| run.text.chars().count() as f32 * run.script.scale())
        .sum()
}

/// `text` with scripts as Unicode superscript and subscript characters
/// where they exist, for sinks that draw a single run (EMF).
pub fn unicode(text: &str) -> String {
    if !has_scripts(text) {
        return text.to_owned();
    }
    let mut out = String::new();
    for run in runs(text) {
        for ch in run.text.chars() {
            let scripted = (run.script != Script::Normal)
                .then(|| {
                    SCRIPT_CHARS
                        .iter()
                        .find(|&&(_, plain, script)| plain == ch && script == run.script)
                })
                .flatten();
            out.push(scripted.map_or(ch, |&(c, _, _)| c));
        }
    }
    out
}

const SCRIPT_CHARS: &[(char, char, Script)] = &[
    ('⁰', '0', Script::Super),
    ('¹', '1', Script::Super),
    ('²', '2', Script::Super),
    ('³', '3', Script::Super),
    ('⁴', '4', Script::Super),
    ('⁵', '5', Script::Super),
    ('⁶', '6', Script::Super),
    ('⁷', '7', Script::Super),
    ('⁸', '8', Script::Super),
    ('⁹', '9', Script::Super),
    ('⁺', '+', Script::Super),
    ('⁻', '−', Script::Super),
    ('⁼', '=', Script::Super),
    ('⁽', '(', Script::Super),
    ('⁾', ')', Script::Super),
    ('ⁿ', 'n', Script::Super),
    ('ⁱ', 'i', Script::Super),
    ('₀', '0', Script::Sub),
    ('₁', '1', Script::Sub),
    ('₂', '2', Script::Sub),
    ('₃', '3', Script::Sub),
    ('₄', '4', Script::Sub),
    ('₅', '5', Script::Sub),
    ('₆', '6', Script::Sub),
    ('₇', '7', Script::Sub),
    ('₈', '8', Script::Sub),
    ('₉', '9', Script::Sub),
    ('₊', '+', Script::Sub),
    ('₋', '−', Script::Sub),
    ('₌', '=', Script::Sub),
    ('₍', '(', Script::Sub),
    ('₎', ')', Script::Sub),
    ('ₐ', 'a', Script::Sub),
    ('ₑ', 'e', Script::Sub),
    ('ₒ', 'o', Script::Sub),
    ('ₓ', 'x', Script::Sub),
];

fn unicode_script(ch: char) -> Option<(char, Script)> {
    SCRIPT_CHARS
        .iter()
        .find(|&&(c, _, _)| c == ch)
        .map(|&(_, plain, script)| (plain, script))
}

/// SVG `<text>` content for `text` at `font_size`: escaped, with scripts as
/// `<tspan>`s shifted by relative `dy`, which every SVG consumer honours
/// (unlike `baseline-shift`). The text may end off the baseline, so each
/// call must start a fresh `<text>` or set an absolute `y` afterwards.
pub(crate) fn svg_content(text: &str, font_size: f32) -> String {
    if !has_scripts(text) {
        return escape(text);
    }
    let mut out = String::new();
    let mut offset = 0.0;
    for run in runs(text) {
        let target = run.script.baseline_offset(font_size);
        let dy = target - offset;
        offset = target;
        let body = escape(&run.text);
        if run.script == Script::Normal && dy == 0.0 {
            out.push_str(&body);
        } else if run.script == Script::Normal {
            out.push_str(&format!(r#"<tspan dy="{dy:.2}">{body}</tspan>"#));
        } else {
            let size = font_size * SCRIPT_SCALE;
            out.push_str(&format!(
                r#"<tspan dy="{dy:.2}" font-size="{size:.2}">{body}</tspan>"#
            ));
        }
    }
    out
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// An egui galley for `text`, with scripts set smaller and aligned to the
/// top or bottom of the line.
#[cfg(feature = "screen")]
pub(crate) fn galley(
    painter: &egui::Painter,
    text: &str,
    font: egui::FontId,
    color: egui::Color32,
) -> std::sync::Arc<egui::Galley> {
    let font = figure_font(painter, font);
    if !has_scripts(text) {
        return painter.layout_no_wrap(text.to_owned(), font, color);
    }
    let mut job = egui::text::LayoutJob::default();
    for run in runs(text) {
        let mut format = egui::TextFormat::simple(
            egui::FontId::new(font.size * run.script.scale(), font.family.clone()),
            color,
        );
        format.valign = match run.script {
            Script::Normal => egui::Align::Center,
            Script::Super => egui::Align::TOP,
            Script::Sub => egui::Align::BOTTOM,
        };
        job.append(&run.text, 0.0, format);
    }
    painter.layout_job(job)
}

/// Swap the default proportional family for the figure family when the host
/// registered one; headless and test contexts keep the egui default.
#[cfg(feature = "screen")]
fn figure_font(painter: &egui::Painter, font: egui::FontId) -> egui::FontId {
    if font.family != egui::FontFamily::Proportional {
        return font;
    }
    let family = egui::FontFamily::Name(FIGURE_FONT_FAMILY.into());
    if painter.fonts(|fonts| fonts.definitions().families.contains_key(&family)) {
        egui::FontId::new(font.size, family)
    } else {
        font
    }
}

/// Like `Painter::text`, with super- and subscripts.
#[cfg(feature = "screen")]
pub(crate) fn paint(
    painter: &egui::Painter,
    pos: egui::Pos2,
    anchor: egui::Align2,
    text: impl AsRef<str>,
    font: egui::FontId,
    color: egui::Color32,
) -> egui::Rect {
    let galley = galley(painter, text.as_ref(), font, color);
    let rect = anchor.anchor_size(pos, galley.size());
    painter.galley(rect.min, galley, color);
    rect
}

#[cfg(test)]
#[path = "rich_text_tests.rs"]
mod tests;
