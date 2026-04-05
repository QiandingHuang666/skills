use std::{
    env, fs,
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};
use clap::{Args, Parser, Subcommand};
use reqwest::blocking::Client;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use slurm_proto::{
    ConnectionAddRequest, ConnectionKind, ConnectionListData, ExecRunData, ExecRunRequest,
    RuntimeFile, ServerStatusData, SlurmJobsData, SlurmJobsRequest, SuccessResponse,
};

#[derive(Debug, Parser)]
#[command(name = "slurm-client", about = "Rust client scaffold for slurm-assistant")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Connection(ConnectionCommand),
    Exec(ExecCommand),
    Jobs(JobsCommand),
    Server(ServerCommand),
}

#[derive(Debug, Args)]
struct ServerCommand {
    #[command(subcommand)]
    command: ServerSubcommand,
}

#[derive(Debug, Subcommand)]
enum ServerSubcommand {
    Status {
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Args)]
struct ConnectionCommand {
    #[command(subcommand)]
    command: ConnectionSubcommand,
}

#[derive(Debug, Subcommand)]
enum ConnectionSubcommand {
    Add {
        #[arg(long)]
        label: String,
        #[arg(long)]
        host: Option<String>,
        #[arg(long)]
        port: Option<u16>,
        #[arg(long = "user")]
        username: Option<String>,
        #[arg(long)]
        kind: ConnectionKindArg,
        #[arg(long)]
        jump_host: Option<String>,
        #[arg(long)]
        json: bool,
    },
    List {
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Args)]
struct ExecCommand {
    #[arg(long = "connection")]
    connection_id: String,
    #[arg(short = 'c', long = "cmd")]
    command: String,
    #[arg(long, default_value_t = 30)]
    timeout_secs: u64,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct JobsCommand {
    #[arg(long = "connection")]
    connection_id: String,
    #[arg(long = "job-id")]
    job_id: Option<String>,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Clone, clap::ValueEnum)]
enum ConnectionKindArg {
    Local,
    Cluster,
    Instance,
    Server,
}

impl From<ConnectionKindArg> for ConnectionKind {
    fn from(value: ConnectionKindArg) -> Self {
        match value {
            ConnectionKindArg::Local => ConnectionKind::Local,
            ConnectionKindArg::Cluster => ConnectionKind::Cluster,
            ConnectionKindArg::Instance => ConnectionKind::Instance,
            ConnectionKindArg::Server => ConnectionKind::Server,
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Connection(cmd) => {
            let runtime = read_runtime_file(&runtime_file_path()?)?;
            match cmd.command {
                ConnectionSubcommand::Add {
                    label,
                    host,
                    port,
                    username,
                    kind,
                    jump_host,
                    json,
                } => {
                    let payload = add_connection(
                        &runtime,
                        &ConnectionAddRequest {
                            label,
                            host,
                            port,
                            username,
                            kind: kind.into(),
                            jump_host,
                        },
                    )?;
                    if json {
                        println!("{}", serde_json::to_string_pretty(&payload)?);
                    } else {
                        println!("Connection added");
                        println!("  id: {}", payload.data.connection_id);
                        println!("  created: {}", payload.data.created);
                    }
                }
                ConnectionSubcommand::List { json } => {
                    let payload = list_connections(&runtime)?;
                    if json {
                        println!("{}", serde_json::to_string_pretty(&payload)?);
                    } else if payload.data.connections.is_empty() {
                        println!("No connections configured");
                    } else {
                        println!("Connections");
                        for conn in payload.data.connections {
                            let endpoint = match (&conn.host, conn.port, &conn.username) {
                                (Some(host), Some(port), Some(user)) => format!("{user}@{host}:{port}"),
                                _ => "local".to_string(),
                            };
                            println!("  {} [{}] {}", conn.label, format!("{:?}", conn.kind).to_lowercase(), endpoint);
                        }
                    }
                }
            }
        }
        Command::Exec(cmd) => {
            let runtime = read_runtime_file(&runtime_file_path()?)?;
            let payload = exec_run(
                &runtime,
                &ExecRunRequest {
                    connection_id: cmd.connection_id,
                    command: cmd.command,
                    timeout_secs: cmd.timeout_secs,
                },
            )?;
            if cmd.json {
                println!("{}", serde_json::to_string_pretty(&payload)?);
            } else {
                if !payload.data.stdout.is_empty() {
                    print!("{}", payload.data.stdout);
                }
                if !payload.data.stderr.is_empty() {
                    eprint!("{}", payload.data.stderr);
                }
                eprintln!("exit_code: {}", payload.data.exit_code);
            }
        }
        Command::Jobs(cmd) => {
            let runtime = read_runtime_file(&runtime_file_path()?)?;
            let payload = jobs_query(
                &runtime,
                &SlurmJobsRequest {
                    connection_id: cmd.connection_id,
                    job_id: cmd.job_id,
                },
            )?;
            if cmd.json {
                println!("{}", serde_json::to_string_pretty(&payload)?);
            } else if payload.data.jobs.is_empty() {
                println!("No jobs found");
            } else {
                println!(
                    "{:<12} {:<12} {:<20} {:<12} {:<12} {:<12} {:<6} {}",
                    "JOBID", "PARTITION", "NAME", "USER", "STATE", "TIME", "NODES", "REASON"
                );
                for job in payload.data.jobs {
                    println!(
                        "{:<12} {:<12} {:<20} {:<12} {:<12} {:<12} {:<6} {}",
                        job.job_id,
                        truncate_for_table(&job.partition, 12),
                        truncate_for_table(&job.name, 20),
                        truncate_for_table(&job.user, 12),
                        truncate_for_table(&job.state, 12),
                        truncate_for_table(&job.time, 12),
                        job.nodes,
                        job.reason
                    );
                }
            }
        }
        Command::Server(cmd) => match cmd.command {
            ServerSubcommand::Status { json } => {
                let runtime = read_runtime_file(&runtime_file_path()?)?;
                let payload = fetch_server_status(&runtime)?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&payload)?);
                } else {
                    println!("Server status");
                    println!("  transport: {}", payload.data.transport);
                    println!("  endpoint: {}:{}", payload.data.host, payload.data.port);
                    println!("  runtime: {}", payload.data.runtime_path);
                    println!("  db: {}", payload.data.db_path);
                }
            }
        },
    }
    Ok(())
}

fn runtime_file_path() -> Result<PathBuf> {
    if let Ok(override_dir) = env::var("SLURM_ASSISTANT_DATA_DIR") {
        return Ok(PathBuf::from(override_dir).join("runtime.json"));
    }
    if cfg!(windows) {
        let base = env::var("APPDATA").context("APPDATA not set")?;
        return Ok(PathBuf::from(base).join("slurm-assistant").join("runtime.json"));
    }
    if let Ok(xdg_state) = env::var("XDG_STATE_HOME") {
        return Ok(PathBuf::from(xdg_state).join("slurm-assistant").join("runtime.json"));
    }
    let home = env::var("HOME").context("HOME not set")?;
    Ok(PathBuf::from(home)
        .join(".local")
        .join("share")
        .join("slurm-assistant")
        .join("runtime.json"))
}

fn read_runtime_file(path: &Path) -> Result<RuntimeFile> {
    let bytes = fs::read(path).with_context(|| format!("failed to read runtime file {}", path.display()))?;
    let runtime = serde_json::from_slice::<RuntimeFile>(&bytes)
        .with_context(|| format!("failed to parse runtime file {}", path.display()))?;
    Ok(runtime)
}

fn fetch_server_status(runtime: &RuntimeFile) -> Result<SuccessResponse<ServerStatusData>> {
    send_request_json(
        http_client()
            .get(format!("http://{}:{}/v1/server/status", runtime.host, runtime.port)),
        runtime,
        "failed to decode server status response",
    )
}

fn add_connection(
    runtime: &RuntimeFile,
    request: &ConnectionAddRequest,
) -> Result<SuccessResponse<slurm_proto::ConnectionAddData>> {
    send_request_json(
        http_client()
            .post(format!("http://{}:{}/v1/connections/add", runtime.host, runtime.port))
            .json(request),
        runtime,
        "failed to decode connection add response",
    )
}

fn list_connections(runtime: &RuntimeFile) -> Result<SuccessResponse<ConnectionListData>> {
    send_request_json(
        http_client()
            .get(format!("http://{}:{}/v1/connections/list", runtime.host, runtime.port)),
        runtime,
        "failed to decode connection list response",
    )
}

fn exec_run(runtime: &RuntimeFile, request: &ExecRunRequest) -> Result<SuccessResponse<ExecRunData>> {
    send_request_json(
        http_client()
            .post(format!("http://{}:{}/v1/exec/run", runtime.host, runtime.port))
            .json(request),
        runtime,
        "failed to decode exec response",
    )
}

fn jobs_query(
    runtime: &RuntimeFile,
    request: &SlurmJobsRequest,
) -> Result<SuccessResponse<SlurmJobsData>> {
    send_request_json(
        http_client()
            .post(format!("http://{}:{}/v1/slurm/jobs", runtime.host, runtime.port))
            .json(request),
        runtime,
        "failed to decode jobs response",
    )
}

fn http_client() -> Client {
    let client = Client::new();
    client
}

fn truncate_for_table(value: &str, width: usize) -> String {
    let count = value.chars().count();
    if count <= width {
        return value.to_string();
    }
    if width <= 1 {
        return "…".to_string();
    }
    let mut out = value.chars().take(width - 1).collect::<String>();
    out.push('+');
    out
}

fn send_request_json<T>(
    builder: reqwest::blocking::RequestBuilder,
    runtime: &RuntimeFile,
    decode_error_context: &str,
) -> Result<T>
where
    T: serde::de::DeserializeOwned,
{
    let response = builder
        .header(AUTHORIZATION, format!("Bearer {}", runtime.token))
        .header(CONTENT_TYPE, "application/json")
        .send()
        .context("failed to contact local server")?;

    let status = response.status();
    let body = response.text().context("failed to read server response body")?;
    if !status.is_success() {
        bail!("server returned {}: {}", status, body);
    }

    let payload = serde_json::from_str::<T>(&body)
        .with_context(|| decode_error_context.to_string())?;
    Ok(payload)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_status_payload_defaults_to_tcp() {
        let payload = SuccessResponse::new(ServerStatusData {
            pid: 0,
            started_at: "1970-01-01T00:00:00Z".to_string(),
            transport: "tcp".to_string(),
            host: "127.0.0.1".to_string(),
            port: 0,
            db_path: "state.db".to_string(),
            runtime_path: "runtime.json".to_string(),
        });
        assert_eq!(payload.data.transport, "tcp");
        assert_eq!(payload.data.host, "127.0.0.1");
    }

    #[test]
    fn runtime_file_roundtrip() {
        let temp = tempfile::tempdir().unwrap();
        let runtime_path = temp.path().join("runtime.json");
        let runtime = RuntimeFile {
            version: 1,
            transport: "tcp".to_string(),
            host: "127.0.0.1".to_string(),
            port: 47831,
            token: "token".to_string(),
            pid: 12345,
            started_at: "123Z".to_string(),
        };
        fs::write(&runtime_path, serde_json::to_vec_pretty(&runtime).unwrap()).unwrap();
        let read_back = read_runtime_file(&runtime_path).unwrap();
        assert_eq!(read_back, runtime);
    }

    #[test]
    fn truncate_for_table_adds_suffix_when_needed() {
        assert_eq!(truncate_for_table("short", 8), "short");
        assert_eq!(truncate_for_table("abcdefghijkl", 8), "abcdefg+");
    }
}
