//! The host side of `serve`: a child process that is restarted after it dies or hangs.
use serde_json::json;
use std::{process::Command, time::Duration};
use unipeek::serve::{Worker, WorkerError};

fn worker() -> Worker {
    Worker::new(|| {
        let mut c = Command::new(env!("CARGO_BIN_EXE_unipeek"));
        c.arg("serve");
        c
    })
}

fn kill_from_outside(pid: u32) {
    let status = if cfg!(windows) {
        Command::new("taskkill").args(["/F", "/PID", &pid.to_string()]).output().unwrap().status
    } else {
        Command::new("kill").args(["-9", &pid.to_string()]).output().unwrap().status
    };
    assert!(status.success());
    std::thread::sleep(Duration::from_millis(200));
}

#[test]
fn a_killed_child_costs_one_request_and_is_restarted() {
    let mut w = worker();
    let reply = w.request(&json!({"id": 1, "op": "version"})).unwrap();
    assert_eq!(reply["id"], 1);
    assert_eq!(reply["status"], "ok");
    let first = w.pid().unwrap();

    kill_from_outside(first);
    let err = w.request(&json!({"id": 2, "op": "version"})).unwrap_err();
    assert!(matches!(err, WorkerError::Died(_)), "{err}");
    assert_eq!(w.restarts(), 1);
    assert_eq!(w.pid(), None, "the dead child is reaped");

    let reply = w.request(&json!({"id": 3, "op": "version"})).unwrap();
    assert_eq!(reply["id"], 3, "the next request runs in a fresh child");
    assert_ne!(w.pid(), Some(first));
    let reply = w.request(&json!({"id": 4, "op": "info", "source": "does-not-exist.unitypackage"})).unwrap();
    assert_eq!(reply["status"], "error", "a failing request is a reply, not a crash");
    assert_eq!(w.restarts(), 1);
}

#[test]
fn a_hung_child_times_out_and_a_missing_program_is_a_spawn_error() {
    // A child that reads nothing and never answers.
    let mut hung = Worker::new(|| {
        if cfg!(windows) {
            let mut c = Command::new("powershell");
            c.args(["-NoProfile", "-Command", "Start-Sleep -Seconds 30"]);
            c
        } else {
            let mut c = Command::new("sleep");
            c.arg("30");
            c
        }
    })
    .with_timeout(Duration::from_millis(300));
    let t = std::time::Instant::now();
    assert!(matches!(hung.request(&json!({"op": "version"})), Err(WorkerError::Timeout(_))));
    assert!(t.elapsed() < Duration::from_secs(10), "the hung child is killed, not waited for");
    assert_eq!((hung.restarts(), hung.pid()), (1, None));

    let mut missing = Worker::new(|| Command::new("unipeek-this-program-does-not-exist"));
    assert!(matches!(missing.request(&json!({"op": "version"})), Err(WorkerError::Spawn(_))));
}

#[test]
fn dropping_the_worker_ends_the_child() {
    let mut w = worker();
    w.request(&json!({"op": "version"})).unwrap();
    let pid = w.pid().unwrap();
    drop(w);
    std::thread::sleep(Duration::from_millis(200));
    let alive = if cfg!(windows) {
        let out = Command::new("tasklist").args(["/FI", &format!("PID eq {pid}"), "/NH"]).output().unwrap();
        String::from_utf8_lossy(&out.stdout).contains(&pid.to_string())
    } else {
        Command::new("kill").args(["-0", &pid.to_string()]).status().unwrap().success()
    };
    assert!(!alive, "child {pid} still runs");
}
