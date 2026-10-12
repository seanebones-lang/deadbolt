"""Compile v1 source consumer, prove required edits, compile migrated v2 consumer."""
import io
import json
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile

ROOT=Path(__file__).resolve().parents[1]
OLD='3a857f8438dcda4bdaadb414d37f128508dc44f1'
ORIGINAL='''use deadbolt::{AgentStatus, DenyCode, ExportRow};
fn classify(code: DenyCode) -> &'static str {
    match code {
        DenyCode::Killed => "killed", DenyCode::Paused => "paused",
        DenyCode::PurposeExceeded => "purpose_exceeded", DenyCode::LeaseExpired => "lease_expired",
        DenyCode::StoreUnavailable => "store_unavailable", DenyCode::NoLease => "no_lease",
        DenyCode::SpendCap => "spend_cap", DenyCode::NeedsHuman => "needs_human",
    }
}
fn main() {
    let _status = AgentStatus { agent_id:"test".into(),state:"active".into(),expires_at:0,parent_id:None,clips:vec![],swarm_task_id:None };
    let _row = ExportRow { class:"observed".into(),kind:"test".into(),cid:"test".into(),payload_sha256:"test".into(),premises:vec![],agent_id:"test".into(),tool:None,decision:None,code:None,ts:0 };
    println!("{}",classify(DenyCode::Killed));
}
'''


def main():
    if subprocess.run(['git','cat-file','-e',OLD+'^{commit}'],cwd=ROOT,capture_output=True).returncode:
        subprocess.run(['git','fetch','origin',OLD],cwd=ROOT,check=True,timeout=90)
    content=subprocess.check_output(['git','archive',OLD,'Cargo.toml','Cargo.lock','src','examples/build_in.rs','README.md','LICENSE','NOTICE'],cwd=ROOT)
    migrated=ORIGINAL.replace('DenyCode::StoreUnavailable =>','DenyCode::DeadlineExceeded => "deadline_exceeded", DenyCode::StoreUnavailable =>')
    migrated=migrated.replace('DenyCode::NeedsHuman => "needs_human",', 'DenyCode::NeedsHuman => "needs_human", DenyCode::PolicyRequired => "policy_required",')
    migrated=migrated.replace('swarm_task_id:None }','swarm_task_id:None,hard_deadline_at:None,supervision_root:None }')
    migrated=migrated.replace('code:None,ts:0 }','code:None,ts:0,offline_tools:None,hard_deadline_at:None,supervision_root:None,expires_at:None,renewed:None }')
    reports=[]
    with tempfile.TemporaryDirectory(prefix='deadbolt-rust-migration-') as directory:
        root=Path(directory)
        with tarfile.open(fileobj=io.BytesIO(content)) as tar:tar.extractall(root/'old',filter='data')
        for name,dep,program,expected in [('v1-original',root/'old',ORIGINAL,True),('v2-unmigrated',ROOT,ORIGINAL,False),('v2-migrated',ROOT,migrated,True)]:
            consumer=root/name;(consumer/'src').mkdir(parents=True)
            (consumer/'Cargo.toml').write_text('[package]\nname="migration-probe"\nversion="0.0.0"\nedition="2021"\n[dependencies]\ndeadbolt={package="n11-deadbolt",path='+json.dumps(str(dep))+'}\n')
            (consumer/'src/main.rs').write_text(program)
            # The old consumer needs its own pinned dependency graph even when
            # the current crate no longer uses those versions. Seed its lock
            # from the reviewed source, then hydrate it before offline checks.
            # Fetch may add the consumer root; compilation must not change it.
            (consumer/'Cargo.lock').write_bytes((dep/'Cargo.lock').read_bytes())
            fetched=subprocess.run(['cargo','fetch','--manifest-path',str(consumer/'Cargo.toml')],
                                   cwd=consumer,capture_output=True,text=True,timeout=180)
            if fetched.returncode:raise RuntimeError(name+' dependency preparation failed:\n'+fetched.stderr[-3000:])
            result=subprocess.run(['cargo','check','--locked','--offline'],cwd=consumer,env={**os.environ,'CARGO_TARGET_DIR':str(root/'target')},capture_output=True,text=True,timeout=180)
            if (result.returncode==0)!=expected:raise RuntimeError(name+' unexpected result:\n'+result.stderr[-3000:])
            if not expected:assert 'E0004' in result.stderr and 'E0063' in result.stderr
            reports.append({'case':name,'compiles':result.returncode==0,'expected':expected})
    print(json.dumps({'passed':True,'old_source_commit':OLD,'checks':reports}))


if __name__=='__main__':main()
