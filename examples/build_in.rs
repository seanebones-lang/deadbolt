//! In-process loop. No LLM. No socket.
use deadbolt::{AdmitDecision, Deadbolt};

fn main() {
    let dir = std::env::temp_dir().join(format!("deadbolt-build-in-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    let db = Deadbolt::open_at(&dir, true, 60);
    db.ensure_agent("shop-bot").expect("ensure");
    match db.admit("shop-bot", "shell") {
        AdmitDecision::Allow => println!("allow"),
        AdmitDecision::Deny { code } => panic!("first admit {}", code.as_str()),
    }
    db.kill("shop-bot").expect("kill");
    match db.admit("shop-bot", "shell") {
        AdmitDecision::Deny { code } => {
            println!("{code}", code = code.as_str());
            assert_eq!(code.as_str(), "killed");
        }
        AdmitDecision::Allow => panic!("expected killed"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}
