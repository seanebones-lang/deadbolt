//! `deadbolt` operator CLI. Not a model tool. Local transports only.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{ArgAction, Parser, Subcommand};
use deadbolt::{
    bind_refused, default_bind_path, format_export, mcp_proxy, serve, ActionRequest, Deadbolt,
    DeadboltConfig, DeadboltError, PolicyPatch,
};

#[derive(Parser)]
#[command(
    name = "deadbolt",
    version,
    about = "Lease gate for tool, MCP, and spawn. Does not shut down frontier models."
)]
struct Cli {
    /// Idle lease window in seconds. Use consistently for setup and the sidecar.
    #[arg(long, global = true, default_value_t = 60,
          value_parser = clap::value_parser!(u64).range(1..=86400))]
    lease_ttl_secs: u64,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Register a child under an existing parent. Operator-only; inherits ceilings.
    RegisterChild {
        #[arg(long)]
        parent: String,
        #[arg(long)]
        agent: String,
        #[arg(long)]
        swarm_task_id: Option<String>,
    },
    /// Explicit operator-only idle-lease setup. Does not renew existing runs.
    Ensure {
        #[arg(long)]
        agent: String,
    },
    /// Issue a fresh supervised run. Only its separate supervisor credential renews TTL.
    Supervise {
        #[arg(long)]
        agent: String,
        /// Optional immutable absolute Unix deadline, never extended by renewal.
        #[arg(long)]
        hard_deadline_at: Option<i64>,
    },
    /// Manage renewal-only supervisor credentials, separate from workload keys.
    Supervisor {
        #[command(subcommand)]
        command: SupervisorCommand,
    },
    /// Issue a fresh run with a fixed Unix-seconds deadline. Operator-only.
    Issue {
        #[arg(long)]
        agent: String,
        /// Absolute Unix seconds; never extended. Existing agent IDs are refused.
        #[arg(long)]
        hard_deadline_at: i64,
    },
    /// Review, approve and revoke exact actions. Operator-only.
    Action {
        #[command(subcommand)]
        command: ActionCommand,
    },
    /// Manage single-agent admission credentials. Operator-only; secrets go to files.
    Credential {
        #[command(subcommand)]
        command: CredentialCommand,
    },
    /// List leases. Read-only.
    Status {
        /// One agent id. Omit to list every lease.
        #[arg(long)]
        agent: Option<String>,
    },
    /// Pause one agent.
    Pause {
        #[arg(long)]
        agent: String,
    },
    /// Clip one tool on one agent.
    Clip {
        tool: String,
        #[arg(long)]
        agent: String,
    },
    /// Clear pause and clips. Does not resurrect a kill.
    Resume {
        #[arg(long)]
        agent: String,
        /// One-shot approve after resume. Does not clear policy.
        #[arg(long)]
        approve: Option<String>,
    },
    /// Revoke one agent and its children. No `--all`.
    Kill {
        #[arg(long)]
        agent: String,
    },
    /// In-process self-check. No API keys.
    Drill,
    /// Operator-token-required Unix or loopback TCP sidecar. Refuses public binds.
    Serve {
        /// Socket path or `127.0.0.1:PORT`. Requires `DEADBOLT_TOKEN` by default.
        #[arg(long)]
        bind: Option<PathBuf>,
        /// Legacy Unix socket-possession authority. Never enables tokenless TCP.
        #[arg(long)]
        legacy_unauthenticated_unix: bool,
    },
    /// Export one agent's evidence as JSONL.
    Export {
        #[arg(long)]
        agent: String,
        #[arg(long)]
        out: Option<PathBuf>,
        /// JSONL when true. Token lines when false.
        #[arg(long, default_value_t = true, action = ArgAction::Set)]
        json: bool,
        /// Include child leases.
        #[arg(long, default_value_t = false)]
        children: bool,
    },
    /// Set lease blast radius. Omitted fields remain as stored.
    Policy {
        #[arg(long)]
        agent: String,
        /// Comma-separated tool allow-list.
        #[arg(long)]
        tools: Option<String>,
        /// Comma-separated host allow-list.
        #[arg(long)]
        dest: Option<String>,
        /// Comma-separated trusted offline tools that may omit dest. Empty clears the list.
        #[arg(long)]
        offline_tools: Option<String>,
        /// USD cap. Crossing it pauses the lease.
        #[arg(long)]
        spend_cap: Option<f64>,
        /// Comma-separated tools that need one approve.
        #[arg(long)]
        irreversible: Option<String>,
    },
    /// One shot for an irreversible tool. Not a model tool.
    Approve {
        #[arg(long)]
        agent: String,
        #[arg(long)]
        tool: String,
    },
    /// JSON incident. Tokens only.
    Incident {
        #[arg(long)]
        agent: String,
        #[arg(long)]
        out: Option<PathBuf>,
        /// JSON when true. Bare `--json` is true. `--json true` still works.
        #[arg(
            long,
            action = ArgAction::Set,
            num_args = 0..=1,
            default_missing_value = "true",
            default_value = "true"
        )]
        json: bool,
        /// Accepted. The file always lists children.
        #[arg(
            long,
            action = ArgAction::Set,
            num_args = 0..=1,
            default_missing_value = "true",
            default_value = "true"
        )]
        children: bool,
    },
    /// Stdio MCP proxy. Admits `tools/call` before the child runs it.
    McpProxy {
        #[arg(long)]
        agent: String,
        /// Bolt-on admit. Unix socket or `127.0.0.1:PORT`. Default is in-process.
        #[arg(long)]
        serve_sock: Option<PathBuf>,
        /// Child command. Everything after `--`.
        #[arg(last = true, required = true, allow_hyphen_values = true)]
        command: Vec<String>,
    },
}

#[derive(Subcommand)]
enum CredentialCommand {
    /// Issue to a new file. Never prints the secret or replaces an existing file.
    Issue {
        #[arg(long)]
        agent: String,
        #[arg(long)]
        id: String,
        #[arg(long, default_value_t = 3600)]
        ttl_secs: u64,
        #[arg(long)]
        out: PathBuf,
    },
    /// Revoke one credential, leaving the lease and other credentials intact.
    Revoke {
        #[arg(long)]
        id: String,
    },
    /// List metadata only, without changing leases.
    List,
}

#[derive(Subcommand)]
enum SupervisorCommand {
    /// Issue a new root-bound renewal key to a fresh private file. Never prints it.
    Issue {
        #[arg(long)]
        agent: String,
        #[arg(long)]
        id: String,
        #[arg(long, default_value_t = 3600)]
        ttl_secs: u64,
        #[arg(long)]
        out: PathBuf,
    },
    /// Revoke one renewal key. No other credential or lease is changed.
    Revoke {
        #[arg(long)]
        id: String,
    },
    /// List renewal metadata only.
    List,
}

#[derive(Subcommand)]
enum ActionCommand {
    /// Print the exact envelope and fingerprint for human review. No permission granted.
    Inspect {
        #[arg(long)]
        file: PathBuf,
    },
    /// Approve the reviewed envelope once; also require exact admission for its tool.
    Approve {
        #[arg(long)]
        file: PathBuf,
        /// Fingerprint shown by inspect; refuses a request changed since review.
        #[arg(long)]
        fingerprint: String,
    },
    /// Revoke a pending action. Keeps exact-only policy in place.
    Revoke {
        #[arg(long)]
        agent: String,
        #[arg(long)]
        nonce: String,
    },
    /// Require exact approval before any grant exists. Explicitly opt out with --allow-legacy.
    Require {
        #[arg(long)]
        agent: String,
        #[arg(long)]
        tool: String,
        #[arg(long)]
        allow_legacy: bool,
    },
}

fn read_action(path: &std::path::Path) -> Result<ActionRequest, DeadboltError> {
    use std::io::Read;
    let mut raw = String::new();
    std::fs::File::open(path)
        .map_err(|_| DeadboltError::BadRequest)?
        .take(32769)
        .read_to_string(&mut raw)
        .map_err(|_| DeadboltError::BadRequest)?;
    ActionRequest::from_json(&raw)
}

fn cfg(lease_ttl_secs: u64) -> DeadboltConfig {
    let mut cfg = DeadboltConfig {
        token_env: Some("DEADBOLT_TOKEN".into()),
        lease_ttl_secs,
        ..DeadboltConfig::default()
    };
    if let Ok(path) = std::env::var("DEADBOLT_DB") {
        if !path.is_empty() {
            cfg.db_path = Some(path);
        }
    }
    if let Ok(path) = std::env::var("DEADBOLT_EVENTS") {
        if !path.is_empty() {
            cfg.events_path = Some(path);
        }
    }
    cfg
}

fn run() -> Result<(), DeadboltError> {
    let cli = Cli::parse();
    if matches!(cli.command, Command::Drill) {
        Deadbolt::drill()?;
        println!("deadbolt drill ok");
        return Ok(());
    }
    let db = Deadbolt::open(&cfg(cli.lease_ttl_secs));
    match cli.command {
        Command::RegisterChild {
            parent,
            agent,
            swarm_task_id,
        } => {
            let live = db.register_child(&parent, &agent, swarm_task_id.as_deref())?;
            println!("agent={agent} parent={parent} live={live}");
            if !live {
                return Err(DeadboltError::ChildInactive);
            }
        }
        Command::Ensure { agent } => {
            db.ensure_agent(&agent)?;
            println!("deadbolt ensured {agent}");
        }
        Command::Supervise {
            agent,
            hard_deadline_at,
        } => {
            db.issue_supervised_agent(&agent, hard_deadline_at)?;
            println!(
                "agent={agent} supervision_root={agent} hard_deadline_at={}",
                hard_deadline_at
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| "-".into())
            );
        }
        Command::Supervisor { command } => match command {
            SupervisorCommand::Issue {
                agent,
                id,
                ttl_secs,
                out,
            } => {
                let status = db.issue_supervisor_credential(&agent, &id, ttl_secs, &out)?;
                println!(
                    "supervisor={} agent={} expires_at={}",
                    status.credential_id, status.agent_id, status.expires_at
                );
            }
            SupervisorCommand::Revoke { id } => {
                db.revoke_supervisor_credential(&id)?;
                println!("deadbolt supervisor revoked {id}");
            }
            SupervisorCommand::List => {
                for row in db.supervisor_credentials()? {
                    println!(
                        "supervisor={} agent={} expires_at={} revoked_at={}",
                        row.credential_id,
                        row.agent_id,
                        row.expires_at,
                        row.revoked_at
                            .map(|n| n.to_string())
                            .unwrap_or_else(|| "-".into())
                    );
                }
            }
        },
        Command::Action { command } => match command {
            ActionCommand::Inspect { file } => {
                let action = read_action(&file)?;
                println!("fingerprint={}", action.fingerprint()?);
                println!(
                    "{}",
                    serde_json::to_string_pretty(&action).map_err(|_| DeadboltError::BadRequest)?
                );
            }
            ActionCommand::Approve { file, fingerprint } => {
                let action = read_action(&file)?;
                if action.fingerprint()? != fingerprint {
                    return Err(DeadboltError::FingerprintMismatch);
                }
                db.approve_action(&action)?;
                println!(
                    "deadbolt action approved agent={} tool={} nonce={} fingerprint={}",
                    action.agent_id,
                    action.tool,
                    action.nonce,
                    action.fingerprint()?
                );
            }
            ActionCommand::Revoke { agent, nonce } => {
                db.revoke_action(&agent, &nonce)?;
                println!("deadbolt action revoked agent={agent} nonce={nonce}");
            }
            ActionCommand::Require {
                agent,
                tool,
                allow_legacy,
            } => {
                db.require_exact_action(&agent, &tool, !allow_legacy)?;
                println!(
                    "deadbolt exact requirement agent={agent} tool={tool} required={}",
                    !allow_legacy
                );
            }
        },
        Command::Credential { command } => match command {
            CredentialCommand::Issue {
                agent,
                id,
                ttl_secs,
                out,
            } => {
                let status = db.issue_credential(&agent, &id, ttl_secs, &out)?;
                println!(
                    "credential={} agent={} expires_at={}",
                    status.credential_id, status.agent_id, status.expires_at
                );
            }
            CredentialCommand::Revoke { id } => {
                db.revoke_credential(&id)?;
                println!("deadbolt credential revoked {id}");
            }
            CredentialCommand::List => {
                for row in db.credentials()? {
                    println!(
                        "credential={} agent={} expires_at={} revoked_at={}",
                        row.credential_id,
                        row.agent_id,
                        row.expires_at,
                        row.revoked_at
                            .map(|v| v.to_string())
                            .unwrap_or_else(|| "-".into())
                    );
                }
            }
        },
        Command::Status { agent } => {
            let rows = db.status(agent.as_deref())?;
            if rows.is_empty() {
                println!("deadbolt status none");
                if agent.is_some() {
                    return Err(DeadboltError::NotFound);
                }
            }
            for row in rows {
                let clips = if row.clips.is_empty() {
                    "-".to_string()
                } else {
                    row.clips.join(",")
                };
                println!(
                    "agent={} state={} expires_at={} parent={} clips={} swarm_task={} hard_deadline_at={} supervision_root={}",
                    row.agent_id,
                    row.state,
                    row.expires_at,
                    row.parent_id.as_deref().unwrap_or("-"),
                    clips,
                    row.swarm_task_id.as_deref().unwrap_or("-"),
                    row.hard_deadline_at.map(|v| v.to_string()).unwrap_or_else(|| "-".into()),
                    row.supervision_root.as_deref().unwrap_or("-")
                );
            }
        }
        Command::Issue {
            agent,
            hard_deadline_at,
        } => {
            db.issue_agent(&agent, hard_deadline_at)?;
            println!("agent={agent} hard_deadline_at={hard_deadline_at}");
        }
        Command::Pause { agent } => {
            db.pause(&agent)?;
            println!("deadbolt paused {agent}");
        }
        Command::Clip { tool, agent } => {
            db.clip(&agent, &tool)?;
            println!("deadbolt clipped {agent} {tool}");
        }
        Command::Resume { agent, approve } => {
            db.resume(&agent)?;
            if let Some(tool) = approve {
                db.approve(&agent, &tool)?;
                println!("deadbolt resumed {agent} approve {tool}");
            } else {
                println!("deadbolt resumed {agent}");
            }
        }
        Command::Kill { agent } => {
            let report = db.kill(&agent)?;
            println!(
                "deadbolt killed {agent} revoked={} swarm={}",
                report.revoked.len(),
                report.swarm_task_ids.len()
            );
        }
        Command::Serve {
            bind,
            legacy_unauthenticated_unix,
        } => {
            let path = bind.unwrap_or_else(default_bind_path);
            if bind_refused(&path) {
                return Err(DeadboltError::BindRefused);
            }
            println!("deadbolt serve {}", path.display());
            let mut config = cfg(cli.lease_ttl_secs);
            config.allow_legacy_unauthenticated_unix = legacy_unauthenticated_unix;
            serve(&config, &path)?;
        }
        Command::Export {
            agent,
            out,
            json,
            children,
        } => {
            let rows = db.export(&agent, children)?;
            let text = format_export(&rows, json);
            if let Some(path) = out {
                std::fs::write(path, text).map_err(|_| DeadboltError::StoreUnavailable)?;
            } else {
                print!("{text}");
            }
        }
        Command::Policy {
            agent,
            tools,
            dest,
            offline_tools,
            spend_cap,
            irreversible,
        } => {
            db.set_policy(
                &agent,
                PolicyPatch {
                    tools_allow: split_list(tools),
                    dest_allow: split_list(dest),
                    offline_tools: split_list(offline_tools),
                    spend_cap_usd: spend_cap,
                    irreversible: split_list(irreversible),
                },
            )?;
            println!("deadbolt policy {agent}");
        }
        Command::Approve { agent, tool } => {
            db.approve(&agent, &tool)?;
            println!("deadbolt approve {agent} {tool}");
        }
        Command::Incident {
            agent,
            out,
            json: _,
            children: _,
        } => {
            let text = db.incident(&agent)?;
            if let Some(path) = out {
                std::fs::write(path, text).map_err(|_| DeadboltError::StoreUnavailable)?;
            } else {
                println!("{text}");
            }
        }
        Command::McpProxy {
            agent,
            serve_sock,
            command,
        } => {
            mcp_proxy(&agent, &db, serve_sock.as_deref(), &command)?;
        }
        Command::Drill => unreachable!("drill handled above"),
    }
    Ok(())
}

fn split_list(raw: Option<String>) -> Option<Vec<String>> {
    raw.map(|s| {
        s.split(',')
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .map(str::to_string)
            .collect()
    })
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{err}");
            ExitCode::from(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_cli_accepts_explicit_offline_clear() {
        let cli =
            Cli::try_parse_from(["deadbolt", "policy", "--agent", "A", "--offline-tools", ""])
                .unwrap();
        let Command::Policy { offline_tools, .. } = cli.command else {
            panic!("wrong command")
        };
        assert_eq!(split_list(offline_tools), Some(Vec::new()));
    }
}
