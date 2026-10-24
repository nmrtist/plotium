use super::super::{CanvasViewport, ProjectError, Result, ViewportDto};
use crate::state::AxisOverrides;
use plotium_figure::Figure;

pub(super) fn restore(
    dto: Option<&ViewportDto>,
    figure: &mut Figure,
    axis_overrides: &AxisOverrides,
    snapshot_backed: bool,
) -> Result<CanvasViewport> {
    let mut viewport = dto
        .map(ViewportDto::to_viewport)
        .unwrap_or_else(|| CanvasViewport::from_figure(figure));
    viewport.reset_unsupported_gain(figure);
    figure
        .check_display_gain(viewport.display_gain)
        .map_err(|error| ProjectError::Invalid(format!("Invalid plot display gain: {error}")))?;
    if snapshot_backed {
        figure
            .set_display_gain(viewport.display_gain)
            .map_err(|error| {
                ProjectError::Invalid(format!("Invalid plot display gain: {error}"))
            })?;
    } else {
        if axis_overrides.y_range.is_some() && figure.y.categories.is_none() {
            viewport.auto_y = false;
        }
        viewport.sync_full_from(figure);
        viewport.apply_to(figure);
    }
    Ok(viewport)
}
