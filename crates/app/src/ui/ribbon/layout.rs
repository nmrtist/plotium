//! Width mathematics for the Ribbon: per-group scale selection, group
//! measurement, ordering, and the last-resort overflow partition. All text is
//! sized through an injected [`Measure`] so the same arithmetic runs under the
//! live font system and in headless tests.

use egui::FontId;
use plotium_core::state::WorkflowTab;

use super::super::commands::{self, CommandDescriptor};
use super::RibbonDensity;
use super::buttons::short_label;

/// Every group is one tile high at every scale: Large tiles, and two stacked
/// rows of [`STACK_ROW_HEIGHT`] with a [`STACK_GAP`] between them, share the
/// same height, so scaling a group never moves its neighbours vertically.
pub(super) const TILE_HEIGHT: f32 = 46.0;
pub(super) const STACK_ROW_HEIGHT: f32 = 22.0;
pub(super) const STACK_GAP: f32 = 2.0;
/// The command row's gap on either side of a group separator, and the
/// separator's own width; together they are what one group costs beyond
/// its measured width.
pub(super) const GROUP_GAP: f32 = 4.0;
pub(super) const SEPARATOR_WIDTH: f32 = 6.0;
pub(super) const GROUP_SLOT_EXTRA: f32 = GROUP_GAP * 2.0 + SEPARATOR_WIDTH;

/// Returns the width of `text` in `font`. Layout decisions and painting must
/// agree on glyph widths (a per-character estimate drifts on CJK and long
/// labels), so production injects the live font system via [`text_measure`]
/// and tests inject a deterministic stand-in.
pub(super) type Measure<'a> = &'a dyn Fn(&str, FontId) -> f32;

/// The production [`Measure`]: galley layout through the context's fonts.
/// Owning a context clone keeps the closure free of `Ui` borrows, so callers
/// can keep mutating the `Ui` they are laying out.
pub(super) fn text_measure(ctx: egui::Context) -> impl Fn(&str, FontId) -> f32 {
    move |text, font| {
        ctx.fonts_mut(|fonts| {
            fonts
                .layout_no_wrap(text.to_owned(), font, egui::Color32::PLACEHOLDER)
                .size()
                .x
        })
    }
}

/// How much room one group takes, from the richest rendering down. Groups
/// step down this ladder one at a time as the window narrows, so a narrow
/// Ribbon keeps every group in its place instead of moving commands into a
/// menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum GroupScale {
    /// Icon-over-label tiles in one row.
    Large,
    /// Icon-beside-label rows, two per column.
    Medium,
    /// Icon-only squares, two per column; commands without an icon keep
    /// their Medium row so they never become unlabelled.
    Small,
    /// One tile carrying the group's icon and title; the group's Large
    /// layout opens from it.
    Collapsed,
}

impl GroupScale {
    pub(super) const ALL: [Self; 4] = [Self::Large, Self::Medium, Self::Small, Self::Collapsed];

    fn index(self) -> usize {
        self as usize
    }
}

/// The task row's density summary, derived from the plan: `Full` when every
/// group sits at Large, `Scaled` once any group has stepped down or overflowed.
pub(super) fn density(expanded: bool, plan: &TabPlan) -> RibbonDensity {
    if !expanded {
        RibbonDensity::Collapsed
    } else if plan.is_full() {
        RibbonDensity::Full
    } else {
        RibbonDensity::Scaled
    }
}

/// The scale of every group of a tab, plus which groups still fit on the
/// Ribbon at that scale. `shown` only carries `false` once every group is
/// Collapsed and the row still does not fit: the More menu is the floor,
/// never the first resort.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct TabPlan {
    pub(super) scales: Vec<GroupScale>,
    pub(super) shown: Vec<bool>,
}

impl TabPlan {
    fn is_full(&self) -> bool {
        self.scales.iter().all(|&scale| scale == GroupScale::Large)
            && self.shown.iter().all(|&shown| shown)
    }
}

/// Chooses each group's scale so the tab fits `budget`. Groups step down
/// one level at a time, lowest priority first (rightmost first among equals),
/// and the whole row is retried after every step, so the visible set is
/// always the richest one that fits. The invariant: a higher-priority group
/// is never asked to shrink further than a lower-priority one. A level that
/// would not make a group narrower is skipped for that group (a one-command
/// group's Medium row can be wider than its Large tile), which keeps every
/// step a genuine reduction. When even all-Collapsed overflows, the More
/// menu absorbs whole groups behind a reserved `more_width`, via the same
/// priority rule as before.
pub(super) fn plan(
    priorities: &[u8],
    widths: &[[f32; 4]],
    budget: f32,
    more_width: f32,
) -> TabPlan {
    debug_assert_eq!(priorities.len(), widths.len());
    let count = priorities.len();
    let mut scales = vec![GroupScale::Large; count];
    let total = |scales: &[GroupScale]| -> f32 {
        scales
            .iter()
            .enumerate()
            .map(|(index, scale)| widths[index][scale.index()])
            .sum()
    };
    if total(&scales) <= budget {
        return TabPlan {
            scales,
            shown: vec![true; count],
        };
    }
    let mut order: Vec<usize> = (0..count).collect();
    order.sort_by_key(|&index| {
        (
            std::cmp::Reverse(priorities[index]),
            std::cmp::Reverse(index),
        )
    });
    for level in [GroupScale::Medium, GroupScale::Small, GroupScale::Collapsed] {
        for &index in &order {
            scales[index] = narrowest_up_to(&widths[index], level);
            if total(&scales) <= budget {
                return TabPlan {
                    scales,
                    shown: vec![true; count],
                };
            }
        }
    }
    let collapsed: Vec<f32> = widths
        .iter()
        .map(|width| width[GroupScale::Collapsed.index()])
        .collect();
    let shown = shown_groups(priorities, &collapsed, (budget - more_width).max(0.0));
    TabPlan { scales, shown }
}

/// The scale at or above `level` with the smallest width, preferring the
/// richer scale on ties so a step that saves nothing is not taken.
fn narrowest_up_to(widths: &[f32; 4], level: GroupScale) -> GroupScale {
    let mut best = GroupScale::Large;
    for scale in GroupScale::ALL {
        if scale > level {
            break;
        }
        if widths[scale.index()] < widths[best.index()] {
            best = scale;
        }
    }
    best
}

/// Which groups stay on the Ribbon within `budget`. Admission runs in
/// priority order; when a group does not fit, its peers at the same priority
/// may still take the remaining space, but nothing of lower priority can
/// leapfrog it. The invariant: a group in the More menu never outranks a
/// group that stayed visible (equal rank may split, so one oversized group
/// cannot dam every smaller peer behind it).
pub(super) fn shown_groups(priorities: &[u8], widths: &[f32], budget: f32) -> Vec<bool> {
    debug_assert_eq!(priorities.len(), widths.len());
    let mut ranked: Vec<usize> = (0..priorities.len()).collect();
    ranked.sort_by_key(|&index| priorities[index]);
    let mut shown = vec![false; priorities.len()];
    let mut used = 0.0;
    let mut blocked_at: Option<u8> = None;
    for index in ranked {
        if blocked_at.is_some_and(|blocked| priorities[index] > blocked) {
            break;
        }
        if used + widths[index] > budget {
            blocked_at.get_or_insert(priorities[index]);
            continue;
        }
        shown[index] = true;
        used += widths[index];
    }
    shown
}

/// Width of one group at `scale`, including the caption below it.
pub(super) fn group_width(
    title: &str,
    entries: &[&CommandDescriptor],
    scale: GroupScale,
    measure: Measure,
) -> f32 {
    let commands = if scale == GroupScale::Collapsed {
        collapsed_width(title, measure)
    } else {
        let columns = columns(entries, scale, measure);
        columns.iter().map(|column| column.width).sum::<f32>()
            + column_spacing(scale) * columns.len().saturating_sub(1) as f32
    };
    commands.max(measure(title, crate::typography::caption_font()) + 8.0)
}

pub(super) fn column_spacing(scale: GroupScale) -> f32 {
    if scale == GroupScale::Large { 4.0 } else { 2.0 }
}

/// A Collapsed group is one tile wide enough for its title and the caret.
pub(super) fn collapsed_width(title: &str, measure: Measure) -> f32 {
    (measure(
        &collapsed_label(title),
        crate::typography::subheadline_font(),
    ) + 18.0)
        .clamp(58.0, 112.0)
}

pub(super) fn collapsed_label(title: &str) -> String {
    format!("{title} {}", egui_phosphor::regular::CARET_DOWN)
}

/// One column of a group's render plan: a single command at Large, or up
/// to two stacked commands at Medium and Small, all painted `width` wide so
/// a column reads as one even stack.
pub(super) struct Column<'a> {
    pub(super) cells: Vec<Cell<'a>>,
    pub(super) width: f32,
}

/// One command with the scale it is actually painted at. This is the
/// group's scale except at Small, where a command that an icon alone cannot
/// name (no icon, or an icon its group-mates share, as parameterized
/// families do) keeps its Medium row.
pub(super) struct Cell<'a> {
    pub(super) command: &'a CommandDescriptor,
    pub(super) scale: GroupScale,
}

/// The column plan of a group at a non-Collapsed `scale`. Measurement and
/// painting both read it, so the width budget and the pixels cannot drift.
pub(super) fn columns<'a>(
    entries: &[&'a CommandDescriptor],
    scale: GroupScale,
    measure: Measure,
) -> Vec<Column<'a>> {
    let tile = tile_width(entries, measure);
    let per_column = if scale == GroupScale::Large { 1 } else { 2 };
    let mut columns: Vec<Column<'a>> = Vec::new();
    for command in stacking_order(entries, per_column) {
        let cell_scale = if scale == GroupScale::Small && !icon_identifies(command, entries) {
            GroupScale::Medium
        } else {
            scale
        };
        let width = match cell_scale {
            GroupScale::Large => tile,
            GroupScale::Medium => medium_width(command, measure),
            GroupScale::Small | GroupScale::Collapsed => STACK_ROW_HEIGHT,
        };
        let cell = Cell {
            command,
            scale: cell_scale,
        };
        match columns.last_mut() {
            Some(column) if column.cells.len() < per_column => {
                column.cells.push(cell);
                column.width = column.width.max(width);
            }
            _ => columns.push(Column {
                cells: vec![cell],
                width,
            }),
        }
    }
    columns
}

/// The order cells are dealt into columns, top to bottom then left to
/// right. When every command of a stacked group declares a stack row
/// ([`commands::stack_row`]), the two rows are interleaved so each column
/// pairs the matching members of the two families; otherwise, and at Large,
/// the catalog order stands.
fn stacking_order<'a>(
    entries: &[&'a CommandDescriptor],
    per_column: usize,
) -> Vec<&'a CommandDescriptor> {
    let rows: Option<Vec<u8>> = entries
        .iter()
        .map(|command| commands::stack_row(command.id))
        .collect();
    let Some(rows) = rows.filter(|_| per_column == 2) else {
        return entries.to_vec();
    };
    let family = |row: u8| -> Vec<&'a CommandDescriptor> {
        entries
            .iter()
            .zip(&rows)
            .filter(|&(_, &r)| r == row)
            .map(|(&command, _)| command)
            .collect()
    };
    let (top, bottom) = (family(0), family(1));
    let mut order = Vec::with_capacity(entries.len());
    for index in 0..top.len().max(bottom.len()) {
        order.extend(top.get(index));
        order.extend(bottom.get(index));
    }
    order
}

/// Whether `command`'s icon alone tells it apart within its group: it has
/// one, and no group-mate shows the same glyph.
pub(super) fn icon_identifies(command: &CommandDescriptor, entries: &[&CommandDescriptor]) -> bool {
    command.icon.is_some_and(|icon| {
        entries
            .iter()
            .filter(|other| other.icon == Some(icon))
            .count()
            == 1
    })
}

/// All tiles in a group share the width of the widest short label, so a group
/// reads as one row of even targets instead of a ragged strip.
pub(super) fn tile_width(entries: &[&CommandDescriptor], measure: Measure) -> f32 {
    entries
        .iter()
        .map(|command| measure(&short_label(command), crate::typography::subheadline_font()) + 18.0)
        .fold(58.0, f32::max)
        .min(112.0)
}

/// A Medium row: the glyph, a two-space gap, and the short label.
pub(super) fn medium_width(command: &CommandDescriptor, measure: Measure) -> f32 {
    let label = measure(&short_label(command), crate::typography::callout_font());
    let content = if command.icon.is_some() {
        label + 24.0
    } else {
        label
    };
    (content + 16.0).clamp(40.0, 180.0)
}

pub(super) fn groups_for_tab(
    catalog: &[CommandDescriptor],
    tab: WorkflowTab,
) -> Vec<(&'static str, u8, Vec<&CommandDescriptor>)> {
    let mut groups: Vec<(&'static str, u8, Vec<&CommandDescriptor>)> = Vec::new();
    for command in catalog {
        let Some(placement) = command.ribbon.filter(|placement| placement.tab == tab) else {
            continue;
        };
        if let Some((_, priority, entries)) = groups
            .iter_mut()
            .find(|(group, _, _)| *group == placement.group)
        {
            *priority = (*priority).min(placement.priority);
            entries.push(command);
        } else {
            groups.push((placement.group, placement.priority, vec![command]));
        }
    }
    groups.sort_by_key(|(group, _, _)| group_order(tab, group));
    groups
}

/// Left-to-right order of every Ribbon group, tab by tab, following each
/// tab's workflow reading. Every (tab, group) pair the placement tables can
/// produce must appear here: an unlisted pair would fall back to catalog
/// iteration order, which is accidental. Guarded by
/// `every_ribbon_group_has_an_explicit_order`.
pub(super) fn group_order(tab: WorkflowTab, group: &str) -> u8 {
    let order: &[&str] = match tab {
        WorkflowTab::Data => &["Import", "Build", "Export"],
        WorkflowTab::Process => &["Processing", "Correct", "Transform", "XAS", "Recipes"],
        WorkflowTab::Analyze => &[
            "Range",
            "XPS",
            "Extract",
            "Regions",
            "Diffusion",
            "Peaks",
            "Review",
            "Overlay",
            "Peak Fit",
            "Curve Fit",
            "Statistics",
            "Interpret",
        ],
        WorkflowTab::Figure => &["Create", "Chart", "Data", "Style", "Canvas", "Output"],
        WorkflowTab::Arrange => &[
            "Layout",
            "Align",
            "Distribute",
            "Order",
            "Guides",
            "Annotate",
            "Object",
            "Canvas",
        ],
        WorkflowTab::View => &["Navigate", "Display"],
    };
    order
        .iter()
        .position(|&name| name == group)
        .map_or(u8::MAX, |index| index as u8)
}

#[cfg(test)]
#[path = "layout_tests.rs"]
mod tests;
