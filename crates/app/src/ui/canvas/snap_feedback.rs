//! Trackpad feedback when a drag snaps to an alignment guide.
//!
//! The guide lines are the feedback on every platform; macOS adds a haptic tap
//! through `NSHapticFeedbackManager`, which does nothing without a Force Touch
//! trackpad.

use egui::{Context, Id};
use plotium_core::layout::SnapGuide;

const SHOWN: &str = "plotium_snap_feedback_guides";

/// Compares the guides left after this frame's interaction handling with those
/// of the previous frame. Interaction code may set and clear guides within one
/// frame, so only the settled result is compared.
pub(super) fn update(ctx: &Context, guides: &[SnapGuide]) {
    let previous = ctx.data_mut(|data| {
        let previous = data.get_temp::<Vec<SnapGuide>>(Id::new(SHOWN));
        if previous.as_deref() != Some(guides) {
            data.insert_temp(Id::new(SHOWN), guides.to_vec());
        }
        previous.unwrap_or_default()
    });
    if newly_engaged(&previous, guides) {
        platform::alignment();
    }
}

/// A guide that was not showing has appeared. Holding a snap keeps the same
/// guide, so one engagement gives one tap.
fn newly_engaged(previous: &[SnapGuide], current: &[SnapGuide]) -> bool {
    current.iter().any(|guide| !previous.contains(guide))
}

#[cfg(target_os = "macos")]
mod platform {
    use objc2_app_kit::{
        NSHapticFeedbackManager, NSHapticFeedbackPattern, NSHapticFeedbackPerformanceTime,
        NSHapticFeedbackPerformer,
    };

    pub(super) fn alignment() {
        NSHapticFeedbackManager::defaultPerformer().performFeedbackPattern_performanceTime(
            NSHapticFeedbackPattern::Alignment,
            NSHapticFeedbackPerformanceTime::DrawCompleted,
        );
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    pub(super) fn alignment() {}
}

#[cfg(test)]
mod tests {
    use super::*;

    fn guide(vertical: bool, pos: f32) -> SnapGuide {
        SnapGuide { vertical, pos }
    }

    #[test]
    fn engaging_a_guide_fires_once_while_it_is_held() {
        let x = guide(true, 72.0);
        assert!(newly_engaged(&[], &[x]));
        assert!(!newly_engaged(&[x], &[x]));
        assert!(!newly_engaged(&[x], &[]));
    }

    #[test]
    fn a_second_axis_or_a_different_line_fires_again() {
        let x = guide(true, 72.0);
        assert!(newly_engaged(&[x], &[x, guide(false, 36.0)]));
        assert!(newly_engaged(&[x], &[guide(true, 144.0)]));
    }
}
