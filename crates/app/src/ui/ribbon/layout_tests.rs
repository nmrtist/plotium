use super::super::super::commands;
use super::super::super::commands::CommandId;
use super::*;
use plotium_core::state::PlotiumApp;

/// Deterministic stand-in for the live font system: proportional to the
/// font size like real glyphs, close to the Latin average width.
fn estimate(text: &str, font: FontId) -> f32 {
    text.chars().count() as f32 * font.size * 0.53
}

fn app() -> PlotiumApp {
    PlotiumApp::new_with_settings(plotium_core::settings::Settings::default())
}

/// Priorities and per-scale widths of a live tab, as `command_row` builds them.
fn tab_widths(app: &PlotiumApp, tab: WorkflowTab) -> (Vec<u8>, Vec<[f32; 4]>) {
    let catalog = commands::catalog(app);
    let groups = groups_for_tab(&catalog, tab);
    let priorities = groups.iter().map(|(_, priority, _)| *priority).collect();
    let widths = groups
        .iter()
        .map(|(title, _, entries)| {
            GroupScale::ALL
                .map(|scale| group_width(title, entries, scale, &estimate) + GROUP_SLOT_EXTRA)
        })
        .collect();
    (priorities, widths)
}

/// Four strictly narrowing scales, so every requested step is taken.
const LADDER: [f32; 4] = [100.0, 80.0, 60.0, 40.0];

#[test]
fn density_follows_the_active_tabs_measured_content() {
    let (priorities, widths) = tab_widths(&app(), WorkflowTab::View);
    let full_need: f32 = widths.iter().map(|width| width[0]).sum();

    // Full the moment the tab's content fits — no fixed window breakpoint.
    let fits = plan(&priorities, &widths, full_need + 1.0, 60.0);
    assert_eq!(density(true, &fits), RibbonDensity::Full);
    let scaled = plan(&priorities, &widths, full_need - 1.0, 60.0);
    assert_eq!(density(true, &scaled), RibbonDensity::Scaled);
    // Width never hides the command area: even an absurdly narrow window
    // stays on the Ribbon (Collapsed groups, then the More menu). Only the
    // user's own collapse produces Collapsed.
    let narrow = plan(&priorities, &widths, 100.0, 60.0);
    assert_eq!(density(true, &narrow), RibbonDensity::Scaled);
    assert_eq!(density(false, &fits), RibbonDensity::Collapsed);
}

#[test]
fn groups_step_down_lowest_priority_first() {
    let priorities = [0u8, 1, 2];
    let widths = [LADDER; 3];
    // 300 needed at Large; one Medium step on the lowest-priority group
    // (rightmost) brings it to 280, and nothing else moves.
    let plan = plan(&priorities, &widths, 280.0, 20.0);
    assert_eq!(
        plan.scales,
        vec![GroupScale::Large, GroupScale::Large, GroupScale::Medium]
    );
    assert_eq!(plan.shown, vec![true, true, true]);
}

#[test]
fn equal_priorities_step_down_from_the_right() {
    let priorities = [1u8, 1];
    let widths = [LADDER; 2];
    let plan = plan(&priorities, &widths, 180.0, 20.0);
    assert_eq!(plan.scales, vec![GroupScale::Large, GroupScale::Medium]);
}

#[test]
fn a_higher_priority_group_is_never_smaller_than_a_lower_one() {
    let priorities = [2u8, 0, 1, 3, 1];
    let widths = [LADDER; 5];
    let mut budget = 120.0;
    while budget <= 520.0 {
        let plan = plan(&priorities, &widths, budget, 20.0);
        for a in 0..priorities.len() {
            for b in 0..priorities.len() {
                if priorities[a] < priorities[b] {
                    assert!(
                        plan.scales[a] <= plan.scales[b],
                        "budget {budget}: group {a} (priority {}) is at {:?} while group {b} \
                         (priority {}) is at {:?}",
                        priorities[a],
                        plan.scales[a],
                        priorities[b],
                        plan.scales[b]
                    );
                }
            }
        }
        if plan.shown.iter().all(|&shown| shown) {
            let total: f32 = plan
                .scales
                .iter()
                .enumerate()
                .map(|(index, scale)| widths[index][*scale as usize])
                .sum();
            assert!(total <= budget, "budget {budget}: plan needs {total}");
        }
        budget += 7.0;
    }
}

#[test]
fn a_level_that_does_not_narrow_a_group_is_skipped() {
    // The second group's Medium row is wider than its Large tile (a
    // one-command group), so asking it for Medium must leave it Large; the
    // budget is then met by the first group's own Medium step.
    let priorities = [0u8, 1];
    let widths = [LADDER, [58.0, 70.0, 22.0, 58.0]];
    let plan = plan(&priorities, &widths, 150.0, 20.0);
    assert_eq!(plan.scales, vec![GroupScale::Medium, GroupScale::Large]);
}

#[test]
fn the_more_menu_opens_only_after_every_group_is_collapsed() {
    let priorities = [0u8, 1];
    let widths = [LADDER; 2];
    // 80 needed with both Collapsed; 70 does not fit, so More takes the
    // lower-priority group behind its 20 px reservation.
    let plan = plan(&priorities, &widths, 70.0, 20.0);
    assert_eq!(plan.scales, vec![GroupScale::Collapsed; 2]);
    assert_eq!(plan.shown, vec![true, false]);
}

#[test]
fn overflow_never_shows_a_group_outranked_by_a_hidden_one() {
    let priorities = [2u8, 0, 1, 3];
    let widths = [40.0, 50.0, 30.0, 20.0];
    // 50 + 30 fit; the priority-2 group does not, and it must also block
    // the smaller priority-3 group behind it — a lower-priority group must
    // never appear while a higher-priority one sits in the More menu.
    let shown = shown_groups(&priorities, &widths, 90.0);
    assert_eq!(shown, vec![false, true, true, false]);
}

#[test]
fn an_oversized_group_does_not_dam_its_equal_priority_peers() {
    let priorities = [1u8, 1, 2];
    let widths = [100.0, 30.0, 10.0];
    // The oversized first group overflows, its equal-priority peer still
    // takes the space, and everything of lower priority follows it into
    // More.
    let shown = shown_groups(&priorities, &widths, 40.0);
    assert_eq!(shown, vec![false, true, false]);
}

#[test]
fn stacked_scales_hold_two_cells_per_column() {
    let app = app();
    let catalog = commands::catalog(&app);
    for (_, _, entries) in groups_for_tab(&catalog, WorkflowTab::Arrange) {
        assert!(
            columns(&entries, GroupScale::Large, &estimate)
                .iter()
                .all(|column| column.cells.len() == 1)
        );
        for scale in [GroupScale::Medium, GroupScale::Small] {
            let columns = columns(&entries, scale, &estimate);
            assert!(
                columns
                    .iter()
                    .all(|column| (1..=2).contains(&column.cells.len()))
            );
            assert_eq!(columns.len(), entries.len().div_ceil(2));
        }
    }
}

/// The Align group's two families stack as rows: the horizontal alignments
/// across the top, each over its vertical counterpart, rather than pairing
/// catalog neighbours (which put Left over Center and Right over Top).
#[test]
fn align_stacks_the_horizontal_family_over_the_vertical_one() {
    use plotium_core::layout::Align;

    let app = app();
    let catalog = commands::catalog(&app);
    let groups = groups_for_tab(&catalog, WorkflowTab::Arrange);
    let (_, _, align) = groups
        .iter()
        .find(|(group, _, _)| *group == "Align")
        .expect("the Arrange tab has an Align group");
    let horizontal = [Align::Left, Align::HCenter, Align::Right].map(CommandId::Align);
    let vertical = [Align::Top, Align::VCenter, Align::Bottom].map(CommandId::Align);
    for scale in [GroupScale::Medium, GroupScale::Small] {
        let columns = columns(align, scale, &estimate);
        let row = |index: usize| -> Vec<CommandId> {
            columns
                .iter()
                .map(|column| column.cells[index].command.id)
                .collect()
        };
        assert_eq!(row(0), horizontal, "{scale:?} top row");
        assert_eq!(row(1), vertical, "{scale:?} bottom row");
    }
    // Large keeps the catalog order in one row.
    let large: Vec<CommandId> = columns(align, GroupScale::Large, &estimate)
        .iter()
        .map(|column| column.cells[0].command.id)
        .collect();
    assert_eq!(large, [horizontal, vertical].concat());
}

#[test]
fn small_keeps_the_label_of_a_command_its_icon_cannot_name() {
    let app = app();
    let catalog = commands::catalog(&app);
    let groups = groups_for_tab(&catalog, WorkflowTab::Figure);
    let (_, _, style) = groups
        .iter()
        .find(|(group, _, _)| *group == "Style")
        .expect("the Figure tab has a Style group");
    // The theme family shares one glyph, so at Small its members keep their
    // Medium rows; a command with a glyph of its own drops to an icon square.
    let mut saw_theme = false;
    let mut saw_square = false;
    for column in columns(style, GroupScale::Small, &estimate) {
        for cell in column.cells {
            let command = cell.command;
            if matches!(command.id, CommandId::ApplyTheme(_)) {
                assert_eq!(cell.scale, GroupScale::Medium);
                saw_theme = true;
            } else if icon_identifies(command, style) {
                assert_eq!(cell.scale, GroupScale::Small);
                saw_square = true;
            }
        }
    }
    assert!(
        saw_theme && saw_square,
        "test premise: Style mixes both kinds"
    );
    let iconless = commands::describe(&app, CommandId::CheckUpdates);
    assert!(!icon_identifies(&iconless, &[&iconless]));
}

#[test]
fn collapsed_groups_reserve_width_for_their_titles() {
    assert!(group_width("Guides", &[], GroupScale::Collapsed, &estimate) >= MIN_TILE_WIDTH);
    assert!(group_width("Distribute", &[], GroupScale::Collapsed, &estimate) <= MAX_TILE_WIDTH);
}

#[test]
fn every_ribbon_group_has_an_explicit_order() {
    for (tab, group) in commands::ribbon_group_pairs() {
        assert_ne!(
            group_order(tab, group),
            u8::MAX,
            "({tab:?}, {group:?}) has no explicit order; add it to group_order"
        );
    }
}
