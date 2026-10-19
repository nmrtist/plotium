use super::*;
use objc2_foundation::{NSString, NSURL};
use plotium_core::state::ProjectTransition;

#[test]
fn apple_document_list_preserves_unicode_spaces_and_rejects_recipes() {
    let list = NSAppleEventDescriptor::listDescriptor();
    for (index, path) in [
        "/tmp/\u{4E2D}\u{6587} space.plotium",
        "/tmp/recipe.plotiumproc",
    ]
    .into_iter()
    .enumerate()
    {
        let url = NSURL::fileURLWithPath(&NSString::from_str(path));
        list.insertDescriptor_atIndex(
            &NSAppleEventDescriptor::descriptorWithFileURL(&url),
            index as isize + 1,
        );
    }
    let paths = decode_documents(&list);
    assert_eq!(
        paths[0],
        Ok(PathBuf::from("/tmp/\u{4E2D}\u{6587} space.plotium"))
    );
    assert!(paths[1].is_err());
}

#[test]
fn cold_queue_waits_for_existing_project_confirmation_and_reports_bad_paths() {
    let context = egui::Context::default();
    let mut app = PlotiumApp::new_with_settings(Default::default());
    app.request_project_transition(ProjectTransition::New);
    let mut queue = VecDeque::from([Ok(PathBuf::from("/missing/project.plotium"))]);
    poll_pending(&mut queue, &mut app, &context);
    assert_eq!(queue.len(), 1);
    app.session.ui.project_transition = None;
    poll_pending(&mut queue, &mut app, &context);
    assert!(queue.is_empty());
    assert!(app.session.ui.project_transition.is_none());
    assert!(app.session.status.contains("could not be opened"));
}
