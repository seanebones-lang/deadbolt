//! In-process loop. No LLM. No socket.
use deadbolt::{Deadbolt, DenyCode};

fn main() {
    let mut workspace = tempfile::Builder::new();
    workspace.prefix("deadbolt-build-in-");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        workspace.permissions(std::fs::Permissions::from_mode(0o700));
    }
    let dir = workspace.tempdir().expect("private temp dir");
    let db = Deadbolt::open_at(dir.path(), true, 60);
    db.ensure_agent("shop-bot").expect("ensure");
    db.dispatch("shop-bot", "shell", None, || println!("allow"))
        .expect("first dispatch");
    db.kill("shop-bot").expect("kill");
    let denied: Result<(), DenyCode> =
        db.dispatch("shop-bot", "shell", None, || panic!("denied body ran"));
    assert_eq!(denied, Err(DenyCode::Killed));
    println!("killed");
}
