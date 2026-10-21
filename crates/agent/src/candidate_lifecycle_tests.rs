use super::*;
use crate::Session;
use crate::candidate_tests::*;
use plotium_core::state::*;
use serde_json::{Value, json};
use std::time::{Duration, Instant};
fn plan(a: &mut PlotiumApp, s: &mut Session) -> Value {
    let q = edit_args(
        a,
        json!([{"op":"text_set","target":target(a,2),"text":"Candidate"}]),
    );
    s.call(a, "plotium_plan", q).unwrap()
}
#[test]
fn candidate_entry_errors_do_not_consume_plan_or_modify_content() {
    let (mut a, mut s, _) = fixture();
    let p = plan(&mut a, &mut s);
    let before = content(&a);
    for (key, v, code) in [
        ("max_edge", json!(2049), "preview_budget_exceeded"),
        ("max_edge", json!(0), "invalid_parameters"),
        ("unexpected", json!(true), "invalid_parameters"),
        ("expected_revision", json!(99), "stale_revision"),
        (
            "expected_project_context",
            json!(ProjectContextId::default()),
            "project_changed",
        ),
        ("plan_id", json!("unknown"), "expired_plan"),
    ] {
        let mut q = preview_args(&a, &p);
        q[key] = v;
        assert_eq!(
            s.call(&mut a, "plotium_preview_plan", q).unwrap_err().code,
            code,
            "{key}"
        );
        assert_eq!(content(&a), before);
    }
    s.set_allow_modify(false);
    let q = preview_args(&a, &p);
    assert_eq!(
        s.call(&mut a, "plotium_preview_plan", q).unwrap_err().code,
        "permission_denied"
    );
    s.set_allow_modify(true);
    let q = preview_args(&a, &p);
    assert_eq!(
        s.call_when_available(&mut a, "plotium_preview_plan", q, true)
            .unwrap_err()
            .code,
        "busy"
    );
    let q = preview_args(&a, &p);
    assert!(s.call(&mut a, "plotium_preview_plan", q).is_ok());
    s.call(&mut a, "plotium_execute", json!({"plan_id":p["plan_id"]}))
        .unwrap();
    let q = preview_args(&a, &p);
    assert_eq!(
        s.call(&mut a, "plotium_preview_plan", q).unwrap_err().code,
        "expired_plan"
    );
}
#[test]
fn candidate_unsupported_is_preview_only_and_existing_plan_still_executes() {
    let (mut a, mut s, _) = fixture();
    let q = edit_args(&a, json!([{"op":"page","size_mm":[180,100]}]));
    let p = s.call(&mut a, "plotium_plan", q).unwrap();
    let q = preview_args(&a, &p);
    assert_eq!(
        s.call(&mut a, "plotium_preview_plan", q).unwrap_err().code,
        "unsupported_operation"
    );
    s.call(&mut a, "plotium_execute", json!({"plan_id":p["plan_id"]}))
        .unwrap();
    assert_eq!(a.doc.canvases[0].size_mm, [180., 100.]);
}
fn wait(c: &mut Connection, a: &mut PlotiumApp, blocked: bool) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while c.preview.is_some() || c.retiring_preview.is_some() {
        let _result = c.poll_preview_when_available(a, blocked);
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
}
#[test]
fn candidate_background_delivery_rechecks_revision_context_permission_plan_and_cancel() {
    for fault in [
        "revision", "context", "revoke", "regrant", "consume", "cancel", "busy", "stop",
    ] {
        let (mut a, s, _) = fixture();
        let mut c = Connection::start(|| {}).unwrap();
        c.session = s;
        c.synchronize_project(&a);
        let p = plan(&mut a, &mut c.session);
        let q = preview_args(&a, &p);
        let (tx, rx) = oneshot::channel();
        let cancellation = CancellationToken::new();
        c.dispatch_request(
            &mut a,
            PendingRequest {
                name: "plotium_preview_plan".into(),
                arguments: q,
                response: tx,
                cancellation: cancellation.clone(),
                transport: None,
            },
            false,
        )
        .unwrap();
        let expected = match fault {
            "revision" => {
                a.execute_action(plotium_core::actions::Action::rename_canvas(
                    0,
                    a.doc.canvases[0].name.clone(),
                    "Human edit".into(),
                ));
                "stale_revision"
            }
            "context" => {
                a.session.project_context = ProjectContextId::default();
                "project_changed"
            }
            "revoke" => {
                c.session.set_allow_modify(false);
                "permission_denied"
            }
            "regrant" => {
                c.session.set_allow_modify(false);
                c.session.set_allow_modify(true);
                "permission_denied"
            }
            "consume" => {
                c.session
                    .call(&mut a, "plotium_execute", json!({"plan_id":p["plan_id"]}))
                    .unwrap();
                "stale_revision"
            }
            "cancel" => {
                cancellation.cancel();
                "request_cancelled"
            }
            "stop" => {
                c.stop();
                ""
            }
            _ => "busy",
        };
        let before = content(&a);
        let revision = a.doc.automation_revision;
        let history = a.session.undo_stack.len();
        wait(&mut c, &mut a, fault == "busy");
        if fault == "stop" {
            assert!(rx.blocking_recv().is_err());
        } else {
            assert_eq!(
                rx.blocking_recv().unwrap().unwrap_err().code,
                expected,
                "{fault}"
            );
        }
        assert_eq!(content(&a), before);
        assert_eq!(a.doc.automation_revision, revision);
        assert_eq!(a.session.undo_stack.len(), history);
        // A delivered/withheld candidate grants no authority to the old execute call.
        if ["revision", "revoke", "consume", "context"].contains(&fault) {
            assert!(
                c.session
                    .call(&mut a, "plotium_execute", json!({"plan_id":p["plan_id"]}))
                    .is_err()
            );
        }
    }
}
#[test]
fn candidate_selection_changes_reject_deletion_without_touching_history() {
    let (mut a, mut s, _) = fixture();
    let t = target(&a, 2);
    let q = edit_args(&a, json!([{"op":"content_delete","target":t}]));
    let p = s.call(&mut a, "plotium_plan", q).unwrap();
    a.doc.canvases[0].selected_object = Some(a.doc.canvases[0].objects[2].id);
    let q = preview_args(&a, &p);
    assert_eq!(
        s.call(&mut a, "plotium_preview_plan", q).unwrap_err().code,
        "invalid_parameters"
    );
    assert!(!a.can_undo());
}
#[test]
fn candidate_shares_busy_slot_with_committed_preview_and_cancel_does_not_consume() {
    let (mut a, s, _) = fixture();
    let mut c = Connection::start(|| {}).unwrap();
    c.session = s;
    c.synchronize_project(&a);
    let p = plan(&mut a, &mut c.session);
    let (tx, rx) = oneshot::channel();
    let token = CancellationToken::new();
    let arguments = preview_args(&a, &p);
    c.dispatch_request(
        &mut a,
        PendingRequest {
            name: "plotium_preview_plan".into(),
            arguments,
            response: tx,
            cancellation: token.clone(),
            transport: None,
        },
        false,
    )
    .unwrap();
    let (tx, rx2) = oneshot::channel();
    let arguments = image_args(&a);
    assert_eq!(
        c.dispatch_request(
            &mut a,
            PendingRequest {
                name: "plotium_render_preview".into(),
                arguments,
                response: tx,
                cancellation: CancellationToken::new(),
                transport: None
            },
            false
        )
        .unwrap_err()
        .code,
        "busy"
    );
    assert_eq!(rx2.blocking_recv().unwrap().unwrap_err().code, "busy");
    token.cancel();
    wait(&mut c, &mut a, false);
    assert_eq!(
        rx.blocking_recv().unwrap().unwrap_err().code,
        "request_cancelled"
    );
    let q = preview_args(&a, &p);
    assert!(c.session.call(&mut a, "plotium_preview_plan", q).is_ok());
    assert!(
        c.session
            .call(&mut a, "plotium_execute", json!({"plan_id":p["plan_id"]}))
            .is_ok()
    );
}

#[test]
fn candidate_discovery_advertises_full_schema_and_distinct_permission() {
    let (mut a, mut s, _) = fixture();
    for allow in [false, true] {
        s.set_allow_modify(allow);
        let state = s
            .call(&mut a, "plotium_capabilities_get", serde_json::json!({}))
            .unwrap();
        let tools = state["connection_capabilities"]["tools"]
            .as_array()
            .unwrap();
        let candidate = tools
            .iter()
            .find(|t| t["name"] == "plotium_preview_plan")
            .unwrap();
        assert!(candidate.get("currently_permitted").is_none());
        assert_eq!(candidate["required_permission"], "reversible_modify");
        let preview = tools
            .iter()
            .find(|t| t["name"] == "plotium_render_preview")
            .unwrap();
        assert!(preview.get("currently_permitted").is_none());
    }
    let tool = crate::protocol::definitions()
        .into_iter()
        .find(|t| t.name == "plotium_preview_plan")
        .unwrap();
    for key in [
        "plan_id",
        "expected_project_context",
        "expected_revision",
        "target",
    ] {
        assert!(
            tool.input_schema["required"]
                .as_array()
                .unwrap()
                .contains(&json!(key))
        );
    }
}
