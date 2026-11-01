//! Legend box geometry, identical across screen, SVG, EMF and diagnostics.
//! Sizes are proportions of the legend text so small type stays compact.
use crate::legend_content::LegendMark;
use crate::rich_text;
use plotium_figure::{Color, Figure, GuideLayout, GuidePlacement};

/// Most columns automatic layout tries; a wider legend reads as a sentence.
const MAX_AUTO_COLUMNS: usize = 4;

#[derive(Clone, Copy)]
pub(crate) struct LegendLayout {
    pub row: f32,
    pub swatch: f32,
    pub rect_height: f32,
    pub dot_radius: f32,
    pub text_gap: f32,
    pub column_gap: f32,
    pub padding: f32,
    pub entry_width: f32,
    pub columns: usize,
    pub title_height: f32,
    pub width: f32,
    pub height: f32,
}

/// Layout that the figure's settings fix without looking at the plot, or a
/// single column when the count is left to automatic placement.
pub(crate) fn legend_layout(fig: &Figure, entries: &[(&str, Color, LegendMark)]) -> LegendLayout {
    with_columns(fig, entries, fixed_columns(fig, entries.len()).unwrap_or(1))
}

/// Layouts automatic placement may use, fewest columns first. The first always
/// stays, even when it is too wide, so the list is never empty; extra columns
/// must fit within `free_width` points.
pub(crate) fn column_options(
    fig: &Figure,
    entries: &[(&str, Color, LegendMark)],
    free_width: f32,
) -> Vec<LegendLayout> {
    let single = legend_layout(fig, entries);
    let mut options = vec![single];
    if fixed_columns(fig, entries.len()).is_none() {
        options.extend(
            (2..=MAX_AUTO_COLUMNS.min(entries.len()))
                .map(|columns| with_columns(fig, entries, columns))
                .take_while(|layout| layout.width <= free_width),
        );
    }
    options
}

/// The column count the figure dictates, or none when automatic placement
/// chooses it.
fn fixed_columns(fig: &Figure, count: usize) -> Option<usize> {
    match (fig.guide_columns, fig.guide_layout) {
        (Some(columns), _) => Some(columns as usize),
        (None, GuideLayout::Horizontal) => Some(count),
        (None, GuideLayout::Vertical) => Some(1),
        (None, GuideLayout::Auto) => (fig.guide_placement != GuidePlacement::Auto
            || fig.legend_position.is_some())
        .then_some(1),
    }
}

fn with_columns(
    fig: &Figure,
    entries: &[(&str, Color, LegendMark)],
    columns: usize,
) -> LegendLayout {
    let font = fig.typography.legend_pt;
    let row = font * 1.25;
    let swatch = font * 1.5;
    let text_gap = font * 0.4;
    let column_gap = font * 0.8;
    let padding = font * 0.4;
    let chars = entries
        .iter()
        .map(|(name, _, _)| rich_text::width_in_chars(name))
        .fold(0.0, f32::max);
    let entry_width = swatch + text_gap + chars * font * 0.6;
    let columns = columns.clamp(1, entries.len().max(1));
    let title_height = if fig.guide_title.trim().is_empty() {
        0.0
    } else {
        row
    };
    let title_width = rich_text::width_in_chars(&fig.guide_title) * font * 0.6;
    let entries_width = entry_width * columns as f32 + column_gap * (columns - 1) as f32;
    let entry_rows = entries.len().div_ceil(columns).max(1);
    LegendLayout {
        row,
        swatch,
        rect_height: font * 0.9,
        dot_radius: font * 0.4,
        text_gap,
        column_gap,
        padding,
        entry_width,
        columns,
        title_height,
        width: entries_width.max(title_width) + padding * 2.0,
        height: title_height + entry_rows as f32 * row + padding * 2.0,
    }
}

/// Whether automatic placement chooses the column count.
pub(crate) fn has_automatic_columns(fig: &Figure, count: usize) -> bool {
    fixed_columns(fig, count).is_none()
}

pub(crate) fn legend_entry_origin(layout: &LegendLayout, index: usize) -> (f32, f32) {
    let column = index % layout.columns;
    let row = index / layout.columns;
    (
        layout.padding + (layout.entry_width + layout.column_gap) * column as f32,
        layout.padding + layout.title_height + layout.row * (row as f32 + 0.5),
    )
}
