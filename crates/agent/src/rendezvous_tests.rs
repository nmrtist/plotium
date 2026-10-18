use super::*;
use discovery::{Discovery, Lease};
#[test]
fn exclusive_os_lock_recovers_stale_discovery_and_only_owner_removes_it() {
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path().join("mcp");
    let mut first = Lease::acquire(dir.clone()).unwrap();
    assert!(Lease::acquire(dir.clone()).is_err());
    let old = Discovery {
        version: 1,
        instance_id: uuid::Uuid::new_v4(),
        url: "http://127.0.0.1:12345/mcp".into(),
        token: "a".repeat(64),
    };
    first.publish(&old).unwrap();
    assert_eq!(Discovery::read(&dir).unwrap().instance_id, old.instance_id);
    // Simulate a crash: OS releases lock, leaving a stale file behind.
    first.instance_for_test(None);
    drop(first);
    let mut next = Lease::acquire(dir.clone()).unwrap();
    let fresh = Discovery {
        instance_id: uuid::Uuid::new_v4(),
        ..old
    };
    next.publish(&fresh).unwrap();
    assert_eq!(
        Discovery::read(&dir).unwrap().instance_id,
        fresh.instance_id
    );
    drop(next);
    assert!(!dir.join("service.json").exists());
    assert!(Lease::acquire(dir).is_ok());
}
#[cfg(unix)]
#[test]
fn discovery_rejects_shared_permissions_symlinks_and_nonlocal_addresses() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path().join("mcp");
    let mut lease = Lease::acquire(dir.clone()).unwrap();
    let discovery = Discovery {
        version: 1,
        instance_id: uuid::Uuid::new_v4(),
        url: "http://127.0.0.1:12345/mcp".into(),
        token: "a".repeat(64),
    };
    lease.publish(&discovery).unwrap();
    assert_eq!(
        std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
        0o700
    );
    std::fs::set_permissions(
        dir.join("service.json"),
        std::fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    assert!(Discovery::read(&dir).is_err());
    lease
        .publish(&Discovery {
            url: "http://192.0.2.1:12345/mcp".into(),
            ..discovery.clone()
        })
        .unwrap();
    assert!(Discovery::read(&dir).is_err());
    lease.publish(&discovery).unwrap();
    let other = temp.path().join("other");
    std::fs::rename(dir.join("service.json"), &other).unwrap();
    symlink(other, dir.join("service.json")).unwrap();
    assert!(Discovery::read(&dir).is_err());
}
