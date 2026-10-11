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
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
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
    /// Local Unix or token-required loopback TCP sidecar. Refuses public binds.
    Serve {
        /// Socket path or `127.0.0.1:PORT`. TCP requires `DEADBOLT_TOKEN`.
        #[arg(long)]
        bind: Option<PathBuf>,
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

fn cfg() -> DeadboltConfig {
    let mut cfg = DeadboltConfig {
        token_env: Some("DEADBOLT_TOKEN".into()),
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
    let db = Deadbolt::open(&cfg());
    match cli.command {
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
                    return Err(DeadboltError::BadRequest);
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
            }
            for row in rows {
                let clips = if row.clips.is_empty() {
                    "-".to_string()
                } else {
                    row.clips.join(",")
                };
                println!(
                    "agent={} state={} expires_at={} parent={} clips={} swarm_task={}",
                    row.agent_id,
                    row.state,
                    row.expires_at,
                    row.parent_id.as_deref().unwrap_or("-"),
                    clips,
                    row.swarm_task_id.as_deref().unwrap_or("-")
                );
            }
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
        Command::Serve { bind } => {
            let path = bind.unwrap_or_else(default_bind_path);
            if bind_refused(&path) {
                return Err(DeadboltError::BindRefused);
            }
            println!("deadbolt serve {}", path.display());
            serve(&cfg(), &path)?;
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
            spend_cap,
            irreversible,
        } => {
            db.set_policy(
                &agent,
                PolicyPatch {
                    tools_allow: split_list(tools),
                    dest_allow: split_list(dest),
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
