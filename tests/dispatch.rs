use deadbolt::{Deadbolt, DenyCode, PolicyPatch};
use std::sync::{Arc, Barrier};

#[test]
fn async_dispatch_rechecks_when_polled_and_preserves_results() {
    use std::future::Future;
    use std::task::{Context, Poll, Waker};
    let state = tempfile::tempdir().unwrap();
    let gate = Deadbolt::open_at(state.path(), true, 60);
    gate.ensure_agent("worker").unwrap();
    let file = state.path().join("effect");
    let mut context = Context::from_waker(Waker::noop());
    let mut allowed = Box::pin(gate.dispatch_async("worker", "write_file", None, || async {
        std::fs::write(&file, "allowed").unwrap();
        42
    }));
    assert!(!file.exists());
    assert_eq!(allowed.as_mut().poll(&mut context), Poll::Ready(Ok(42)));
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "allowed");
    let mut denied = Box::pin(gate.dispatch_async("worker", "write_file", None, || async {
        std::fs::write(&file, "wrong").unwrap();
    }));
    gate.kill("worker").unwrap();
    assert_eq!(
        denied.as_mut().poll(&mut context),
        Poll::Ready(Err(DenyCode::Killed))
    );
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "allowed");
}

#[test]
fn dispatch_checks_each_effect_and_preserves_result() {
    let state = tempfile::tempdir().unwrap();
    let gate = Deadbolt::open_at(state.path(), true, 60);
    gate.ensure_agent("worker").unwrap();
    gate.set_policy(
        "worker",
        PolicyPatch {
            tools_allow: Some(vec!["write_file".into()]),
            ..Default::default()
        },
    )
    .unwrap();
    let file = state.path().join("allowed.txt");
    assert_eq!(
        gate.dispatch("worker", "write_file", None, || {
            std::fs::write(&file, "allowed").unwrap();
            42
        }),
        Ok(42)
    );
    assert_eq!(std::fs::read_to_string(file).unwrap(), "allowed");
    assert_eq!(
        gate.dispatch("worker", "delete_file", None, || panic!("off-list body")),
        Err::<(), _>(DenyCode::PurposeExceeded)
    );
    gate.kill("worker").unwrap();
    assert_eq!(
        gate.dispatch("worker", "write_file", None, || panic!("killed body")),
        Err::<(), _>(DenyCode::Killed)
    );
}

#[test]
fn independent_dispatchers_execute_at_most_one_approved_effect() {
    let state = tempfile::tempdir().unwrap();
    let gate = Deadbolt::open_at(state.path(), true, 60);
    gate.ensure_agent("worker").unwrap();
    gate.set_policy(
        "worker",
        PolicyPatch {
            irreversible: Some(vec!["write_file".into()]),
            ..Default::default()
        },
    )
    .unwrap();
    gate.approve("worker", "write_file").unwrap();
    let barrier = Arc::new(Barrier::new(8));
    let workers: Vec<_> = (0..8)
        .map(|i| {
            let gate = Deadbolt::open_at(state.path(), true, 60);
            let file = state.path().join(format!("effect-{i}"));
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                gate.dispatch("worker", "write_file", None, || {
                    std::fs::write(file, "once")
                })
            })
        })
        .collect();
    let mut allowed = 0;
    for worker in workers {
        match worker.join().unwrap() {
            Ok(result) => {
                result.unwrap();
                allowed += 1;
            }
            Err(code) => assert_eq!(code, DenyCode::NeedsHuman),
        }
    }
    assert_eq!(allowed, 1);
    let effects = std::fs::read_dir(state.path())
        .unwrap()
        .filter(|entry| {
            entry
                .as_ref()
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("effect-")
        })
        .count();
    assert_eq!(effects, 1);
}

#[test]
fn unavailable_expired_and_paused_gates_do_not_call_body() {
    let state = tempfile::tempdir().unwrap();
    let blocked = state.path().join("not-a-directory");
    std::fs::write(&blocked, "fixture").unwrap();
    let closed = Deadbolt::open_at(&blocked, true, 60);
    assert_eq!(
        closed.dispatch("worker", "write_file", None, || panic!("unavailable body")),
        Err::<(), _>(DenyCode::StoreUnavailable)
    );
    let gate = Deadbolt::open_at(state.path(), true, 60);
    gate.ensure_agent("expired").unwrap();
    gate.force_expire("expired").unwrap();
    assert_eq!(
        gate.dispatch("expired", "write_file", None, || panic!("expired body")),
        Err::<(), _>(DenyCode::LeaseExpired)
    );
    gate.ensure_agent("paused").unwrap();
    gate.pause("paused").unwrap();
    assert_eq!(
        gate.dispatch("paused", "write_file", None, || panic!("paused body")),
        Err::<(), _>(DenyCode::Paused)
    );
}

#[test]
fn tool_failure_is_preserved_and_never_retried() {
    let state = tempfile::tempdir().unwrap();
    let gate = Deadbolt::open_at(state.path(), true, 60);
    gate.ensure_agent("worker").unwrap();
    gate.set_policy(
        "worker",
        PolicyPatch {
            irreversible: Some(vec!["write_file".into()]),
            ..Default::default()
        },
    )
    .unwrap();
    gate.approve("worker", "write_file").unwrap();
    assert_eq!(
        gate.dispatch("worker", "write_file", None, || Err::<(), _>("tool_error")),
        Ok(Err("tool_error"))
    );
    assert_eq!(
        gate.dispatch("worker", "write_file", None, || panic!("retried body")),
        Err::<(), _>(DenyCode::NeedsHuman)
    );
}
