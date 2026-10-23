use super::{ActivityId, ProjectContextId};
use crate::automation::ResourceRef;

/// Identity of one live AI connection, never stored in project data.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ActivitySourceId(uuid::Uuid);

impl Default for ActivitySourceId {
    fn default() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResultFollowMode {
    Off,
    Following,
    Paused,
}

/// Following owns navigation only until a user interaction takes it back.
pub struct ResultFollow {
    context: Option<ProjectContextId>,
    source: Option<ActivitySourceId>,
    mode: ResultFollowMode,
    cursor: ActivityId,
    acknowledged: Vec<(ResourceRef, ActivityId)>,
    pause_reason: Option<String>,
    bound_once: bool,
}

impl Default for ResultFollow {
    fn default() -> Self {
        Self {
            context: None,
            source: None,
            mode: ResultFollowMode::Following,
            cursor: ActivityId(0),
            acknowledged: Vec::new(),
            pause_reason: None,
            bound_once: false,
        }
    }
}

impl ResultFollow {
    pub fn synchronize(&mut self, context: ProjectContextId, source: Option<ActivitySourceId>) {
        if self.context != Some(context) || self.source != source {
            self.acknowledged.clear();
        }
        if self.bound_once && (self.context != Some(context) || self.source != source) {
            self.stop();
        }
        if self.context != Some(context) {
            self.cursor = ActivityId(0);
        }
        self.context = Some(context);
        self.source = source;
        self.bound_once |= source.is_some();
    }

    pub fn source(&self) -> Option<ActivitySourceId> {
        self.source
    }

    pub fn mode(&self) -> ResultFollowMode {
        self.mode
    }

    pub fn cursor(&self) -> ActivityId {
        self.cursor
    }

    pub fn advance(&mut self, cursor: ActivityId) {
        self.cursor.0 = self.cursor.0.max(cursor.0);
        self.acknowledged
            .retain(|(_, operation)| operation.0 > self.cursor.0);
    }

    pub fn is_unseen(&self, resource: &ResourceRef, operation: ActivityId) -> bool {
        operation.0 > self.cursor.0
            && !self
                .acknowledged
                .iter()
                .any(|(seen, through)| seen == resource && through.0 >= operation.0)
    }

    /// Viewing one result does not acknowledge other operations in the history.
    pub fn acknowledge(&mut self, resource: ResourceRef, operation: ActivityId) {
        if !self.is_unseen(&resource, operation) {
            return;
        }
        if let Some((_, through)) = self
            .acknowledged
            .iter_mut()
            .find(|(seen, _)| *seen == resource)
        {
            *through = operation;
        } else {
            self.acknowledged.push((resource, operation));
        }
        if self.acknowledged.len() > super::BACKGROUND_ACTIVITY_CAPACITY {
            self.acknowledged.remove(0);
        }
    }

    pub fn pause_reason(&self) -> Option<&str> {
        self.pause_reason.as_deref()
    }

    pub fn pause(&mut self, reason: &str) {
        if self.mode == ResultFollowMode::Following {
            self.mode = ResultFollowMode::Paused;
            self.pause_reason = Some(reason.to_owned());
        }
    }

    pub fn resume(&mut self) {
        if self.source.is_some() {
            self.mode = ResultFollowMode::Following;
            self.pause_reason = None;
        }
    }

    pub fn stop(&mut self) {
        self.mode = ResultFollowMode::Off;
        self.pause_reason = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connection_defaults_to_following_but_user_pause_is_sticky() {
        let context = ProjectContextId::default();
        let source = ActivitySourceId::default();
        let mut follow = ResultFollow::default();
        follow.synchronize(context, None);
        follow.synchronize(context, Some(source));
        assert_eq!(follow.mode(), ResultFollowMode::Following);
        follow.pause("Browsing data");
        for _ in 0..3 {
            follow.synchronize(context, Some(source));
        }
        assert_eq!(follow.mode(), ResultFollowMode::Paused);
        assert_eq!(follow.pause_reason(), Some("Browsing data"));
        follow.resume();
        assert_eq!(follow.mode(), ResultFollowMode::Following);
        follow.advance(ActivityId(8));
        follow.advance(ActivityId(3));
        assert_eq!(follow.cursor(), ActivityId(8));
        follow.synchronize(ProjectContextId::default(), Some(source));
        assert_eq!(follow.cursor(), ActivityId(0));
        assert_eq!(follow.mode(), ResultFollowMode::Off);
    }

    #[test]
    fn connection_loss_replacement_and_project_change_require_explicit_resume() {
        let context = ProjectContextId::default();
        let source = ActivitySourceId::default();
        for (next_context, next_source) in [
            (context, None),
            (context, Some(ActivitySourceId::default())),
            (ProjectContextId::default(), Some(source)),
        ] {
            let mut follow = ResultFollow::default();
            follow.synchronize(context, Some(source));
            follow.synchronize(next_context, next_source);
            assert_eq!(follow.mode(), ResultFollowMode::Off);
            follow.synchronize(next_context, Some(source));
            assert_eq!(follow.mode(), ResultFollowMode::Off);
            follow.resume();
            assert_eq!(follow.mode(), ResultFollowMode::Following);
        }
    }

    #[test]
    fn explicit_stop_before_connection_is_respected() {
        let mut follow = ResultFollow::default();
        follow.stop();
        follow.synchronize(
            ProjectContextId::default(),
            Some(ActivitySourceId::default()),
        );
        assert_eq!(follow.mode(), ResultFollowMode::Off);
    }

    #[test]
    fn pause_before_connection_remains_sticky_when_connection_arrives() {
        let mut follow = ResultFollow::default();
        let context = ProjectContextId::default();
        follow.synchronize(context, None);
        follow.pause("Browsing data");
        follow.synchronize(context, Some(ActivitySourceId::default()));
        assert_eq!(follow.mode(), ResultFollowMode::Paused);
        assert_eq!(follow.pause_reason(), Some("Browsing data"));
        follow.resume();
        assert_eq!(follow.mode(), ResultFollowMode::Following);
    }

    #[test]
    fn installing_a_project_does_not_restore_default_following() {
        let mut app = super::super::PlotiumApp::new();
        let source = ActivitySourceId::default();
        app.session
            .result_follow
            .synchronize(app.session.project_context, Some(source));
        app.session.result_follow.advance(ActivityId(5));
        app.start_new_project();
        app.session
            .result_follow
            .synchronize(app.session.project_context, Some(source));
        assert_eq!(app.session.result_follow.mode(), ResultFollowMode::Off);
        assert_eq!(app.session.result_follow.cursor(), ActivityId(0));
        app.session.result_follow.resume();
        assert_eq!(
            app.session.result_follow.mode(),
            ResultFollowMode::Following
        );
    }
}
