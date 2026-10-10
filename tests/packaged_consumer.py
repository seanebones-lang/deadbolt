"""Verify an embedded consumer outside the checkout using a Cargo .crate (Python 3.12+)."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile
import tomllib


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--crate", type=Path, help="defaults to this source version's Cargo package")
    args = parser.parse_args()
    source = Path(__file__).resolve().parents[1]
    metadata = tomllib.loads((source / "Cargo.toml").read_text(encoding="utf-8"))["package"]
    crate = args.crate or source / "target/package" / f"{metadata['name']}-{metadata['version']}.crate"
    with tempfile.TemporaryDirectory(prefix="deadbolt-consumer-") as temporary:
        root = Path(temporary)
        with tarfile.open(crate.resolve()) as archive:
            archive.extractall(root / "package", filter="data")
        packages = list((root / "package").iterdir())
        if len(packages) != 1 or not (packages[0] / "Cargo.toml").is_file():
            raise RuntimeError("expected one packaged Cargo root")
        package = packages[0]
        consumer = root / "consumer"
        (consumer / "src").mkdir(parents=True)
        (consumer / "Cargo.toml").write_text(
            '[package]\nname="outside-consumer"\nversion="0.0.0"\nedition="2021"\n'
            '[dependencies]\ndeadbolt={package="n11-deadbolt",path='
            + json.dumps(str(package)) + '}\n', encoding="utf-8")
        shutil.copy2(package / "Cargo.lock", consumer / "Cargo.lock")
        (consumer / "src/main.rs").write_text('''
use deadbolt::{Deadbolt, DenyCode, PolicyPatch};
fn main() {
    let root = std::path::PathBuf::from(std::env::args_os().nth(1).unwrap());
    let gate = Deadbolt::open_at(&root.join("state"), true, 60);
    gate.ensure_agent("consumer-run").unwrap();
    gate.set_policy("consumer-run", PolicyPatch {
        tools_allow: Some(vec!["write_file".into()]), ..Default::default()
    }).unwrap();
    let effect = root.join("effect.txt");
    let result = gate.dispatch("consumer-run", "write_file", None, || {
        std::fs::write(&effect, "allowed").unwrap(); 42
    });
    assert_eq!(result, Ok(42));
    let blocked: Result<(), DenyCode> = gate.dispatch("consumer-run", "other_tool", None,
        || panic!("off-policy body ran"));
    assert_eq!(blocked, Err(DenyCode::PurposeExceeded));
    gate.kill("consumer-run").unwrap();
    let killed: Result<(), DenyCode> = gate.dispatch("consumer-run", "write_file", None,
        || std::fs::write(&effect, "wrong").unwrap());
    assert_eq!(killed, Err(DenyCode::Killed));
    assert_eq!(std::fs::read_to_string(effect).unwrap(), "allowed");
    println!("outside-checkout embedded consumer passed");
}
''', encoding="utf-8")
        # Isolate both the project and output; preserve the package's locked
        # dependency versions and resolve only the added consumer offline.
        env = {**os.environ, "CARGO_TARGET_DIR": str(root / "build")}
        subprocess.run(["cargo", "metadata", "--offline", "--format-version", "1"],
                       cwd=consumer, env=env, check=True, stdout=subprocess.DEVNULL)
        subprocess.run(["cargo", "run", "--locked", "--offline", "--", str(root)],
                       cwd=consumer, env=env, check=True)


if __name__ == "__main__":
    main()
