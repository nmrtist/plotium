use plotium_core::state::{ActivitySourceId, PlotiumApp};

pub(super) struct Capture {
    source: ActivitySourceId,
    paused: bool,
}

impl Capture {
    pub(super) fn new(app: &mut PlotiumApp, paused: bool) -> Self {
        let capture = Self {
            source: ActivitySourceId::default(),
            paused,
        };
        capture.refresh(app);
        capture
    }

    pub(super) fn refresh(&self, app: &mut PlotiumApp) {
        // Synthetic scenes have no live AI connection to keep the source bound.
        app.session
            .result_follow
            .synchronize(app.session.project_context, Some(self.source));
        app.session.result_follow.resume();
        if self.paused {
            app.session
                .result_follow
                .pause("You navigated or edited the workspace.");
        }
    }
}
