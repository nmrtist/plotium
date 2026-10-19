//! Queue Finder open-document events before the GUI event loop starts.
use std::{
    cell::RefCell, collections::VecDeque, ffi::CStr, os::unix::ffi::OsStrExt, path::PathBuf,
};

use objc2::{
    DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained, sel,
};
use objc2_app_kit::NSApplicationWillFinishLaunchingNotification;
use objc2_foundation::{
    NSAppleEventDescriptor, NSAppleEventManager, NSNotification, NSNotificationCenter, NSObject,
    NSObjectProtocol,
};
use plotium_core::state::PlotiumApp;

const CORE_EVENT_CLASS: u32 = u32::from_be_bytes(*b"aevt");
const OPEN_DOCUMENTS: u32 = u32::from_be_bytes(*b"odoc");

#[derive(Default)]
struct DocumentState {
    context: RefCell<Option<egui::Context>>,
    pending: RefCell<VecDeque<Result<PathBuf, String>>>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements. Apple events and polling
    // run on the main thread, and the handler only queues owned filesystem paths.
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = DocumentState]
    struct DocumentHandler;

    unsafe impl NSObjectProtocol for DocumentHandler {}

    impl DocumentHandler {
        #[unsafe(method(applicationWillFinishLaunching:))]
        fn will_finish_launching(&self, _notification: &NSNotification) {
            let manager = NSAppleEventManager::sharedAppleEventManager();
            // SAFETY: Two descriptor arguments and no return value, as required
            // by NSAppleEventManager. This object is retained until unregistration.
            unsafe {
                let _: () = msg_send![&manager,
                    setEventHandler: self,
                    andSelector: sel!(handleDocuments:withReplyEvent:),
                    forEventClass: CORE_EVENT_CLASS,
                    andEventID: OPEN_DOCUMENTS
                ];
            }
            log::debug!("macOS document event handler registered before launch completion");
        }

        #[unsafe(method(handleDocuments:withReplyEvent:))]
        fn documents(&self, event: &NSAppleEventDescriptor, _reply: &NSAppleEventDescriptor) {
            // SAFETY: This selector takes one 32-bit AEKeyword and returns an
            // optional autoreleased descriptor. The direct object is an AEList.
            let direct: Option<Retained<NSAppleEventDescriptor>> = unsafe {
                msg_send![event, paramDescriptorForKeyword: u32::from_be_bytes(*b"----")]
            };
            let requests = direct.map_or_else(
                || vec![Err("Finder did not provide a document list.".into())],
                |list| decode_documents(&list),
            );
            self.ivars().pending.borrow_mut().extend(requests);
            log::debug!("macOS document event queued");
            if let Some(context) = self.ivars().context.borrow().as_ref() {
                super::native_lifecycle::show_window(context);
            }
        }
    }
);

fn decode_documents(list: &NSAppleEventDescriptor) -> Vec<Result<PathBuf, String>> {
    (1..=list.numberOfItems())
        .map(|index| {
            let descriptor = list.descriptorAtIndex(index).ok_or("Missing Finder file descriptor")?;
            // Finder may send an alias or a file URL. Ask Apple Event Manager
            // for its documented file-URL coercion rather than parsing URLs.
            // SAFETY: One 32-bit DescType argument, optional retained descriptor.
            let file: Option<Retained<NSAppleEventDescriptor>> = unsafe {
                msg_send![&*descriptor, coerceToDescriptorType: u32::from_be_bytes(*b"furl")]
            };
            let url = file.and_then(|file| file.fileURLValue()).ok_or("Finder file URL could not be decoded")?;
            if !url.isFileURL() {
                return Err("Only local .plotium project files can be opened from Finder.".into());
            }
            // SAFETY: NSURL owns the NUL-terminated filesystem representation;
            // copy it while url is retained. Preserve non-UTF-8 path bytes too.
            let bytes = unsafe { CStr::from_ptr(url.fileSystemRepresentation().as_ptr()) };
            let path = PathBuf::from(std::ffi::OsStr::from_bytes(bytes.to_bytes()));
            if !path.extension().is_some_and(|extension| extension.eq_ignore_ascii_case("plotium")) {
                return Err("Open a .plotium project from Finder. Apply processing recipes from Plotium's processing controls.".into());
            }
            Ok(path)
        })
        .collect()
}

pub(crate) struct OpenDocuments {
    handler: Retained<DocumentHandler>,
}

impl OpenDocuments {
    /// Install before run_native: cold-launch events can precede window creation.
    pub(crate) fn new() -> Self {
        let mtm = MainThreadMarker::new().expect("the GUI starts on the main thread");
        let handler = DocumentHandler::alloc(mtm).set_ivars(DocumentState::default());
        // SAFETY: NSObject init initializes the superclass with this signature.
        let handler: Retained<DocumentHandler> = unsafe { msg_send![super(handler), init] };
        // NSApplication installs its default Apple event handlers during startup.
        // Register at WillFinishLaunching, after that setup but before the first
        // cold-launch open-document event. Do not replace winit's app delegate.
        // SAFETY: This observer selector takes one NSNotification. The retained
        // handler outlives the registration, which is removed in Drop.
        unsafe {
            NSNotificationCenter::defaultCenter().addObserver_selector_name_object(
                &handler,
                sel!(applicationWillFinishLaunching:),
                Some(NSApplicationWillFinishLaunchingNotification),
                None,
            );
        }
        Self { handler }
    }

    pub(crate) fn set_context(&self, context: &egui::Context) {
        *self.handler.ivars().context.borrow_mut() = Some(context.clone());
    }

    pub(crate) fn poll(&self, app: &mut PlotiumApp, context: &egui::Context) {
        poll_pending(&mut self.handler.ivars().pending.borrow_mut(), app, context);
    }
}

fn poll_pending(
    pending: &mut VecDeque<Result<PathBuf, String>>,
    app: &mut PlotiumApp,
    context: &egui::Context,
) {
    if pending.is_empty() {
        return;
    }
    // Never replace an unsaved-changes prompt or a project/save already queued
    // by another entry point. The existing transition owns Save/Discard/Cancel.
    if app.session.ui.project_transition.is_some()
        || app.session.ui.project_save_in_progress
        || app.session.ui.pending_project_save.is_some()
        || app.compute_busy()
    {
        context.request_repaint_after(std::time::Duration::from_millis(100));
        return;
    }
    super::native_lifecycle::show_window(context);
    match pending.pop_front().expect("nonempty document queue") {
        Ok(path) => super::file_dialogs::open_recent_path(app, &path),
        Err(error) => app.session.status = format!("Could not open Finder document: {error}"),
    }
}

impl Drop for OpenDocuments {
    fn drop(&mut self) {
        let manager = NSAppleEventManager::sharedAppleEventManager();
        // SAFETY: Unregister before the retained callback target is released.
        unsafe {
            NSNotificationCenter::defaultCenter().removeObserver(&self.handler);
            let _: () = msg_send![&manager,
                removeEventHandlerForEventClass: CORE_EVENT_CLASS,
                andEventID: OPEN_DOCUMENTS
            ];
        }
    }
}

#[cfg(test)]
#[path = "native_documents_tests.rs"]
mod tests;
