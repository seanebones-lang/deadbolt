//! `deadbolt` operator CLI. Not a model tool. Local socket only.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{ArgAction, Parser, Subcommand};
use deadbolt::{
    bind_refused, default_bind_path, format_export, mcp_proxy, serve, Deadbolt, DeadboltConfig,
    DeadboltError,
};

#[derive(Parser)]
#[command(
    name = "deadbolt",
    about = "Lease gate for tool, MCP, and spawn. Does not shut down frontier models."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
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
    },
    /// Revoke one agent and its children. No `--all`.
    Kill {
        #[arg(long)]
        agent: String,
    },
    /// In-process self-check. No API keys.
    Drill,
    /// Local Unix sidecar. Refuses `0.0.0.0`.
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

fn cfg() -> DeadboltConfig {
    DeadboltConfig {
        token_env: Some("DEADBOLT_TOKEN".into()),
        ..DeadboltConfig::default()
    }
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
        Command::Resume { agent } => {
            db.resume(&agent)?;
            println!("deadbolt resumed {agent}");
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

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{err}");
            ExitCode::from(1)
        }
    }
}
