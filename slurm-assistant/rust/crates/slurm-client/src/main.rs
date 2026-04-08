use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command as ProcessCommand, Stdio},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand};
use reqwest::blocking::Client;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde::Serialize;
use slurm_proto::{
    ConnectionAddRequest, ConnectionDeleteData, ConnectionKind, ConnectionListData,
    ConnectionRecord, ExecRunData, ExecRunRequest, FileDownloadRequest, FileTransferData,
    FileUploadRequest, RuntimeFile, ServerStatusData, SlurmCancelData, SlurmCancelRequest,
    SlurmFindGpuData, SlurmFindGpuRequest, SlurmGpuNode, SlurmJobsData, SlurmJobsRequest,
    SlurmLogData, SlurmLogRequest, SlurmStatusGpuData, SlurmStatusGpuRequest, SlurmSubmitData,
    SlurmSubmitRequest, SuccessResponse, SessionDeleteData, SessionListData, SessionNodeRole,
    SessionRecord, SessionState, SessionSummaryData, SessionUpsertRequest,
};

#[derive(Debug, Parser)]
#[command(
    name = "slurm-client",
    about = "Rust client scaffold for slurm-assistant"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Serialize)]
struct AllocPlanOutput {
    command: String,
    execute: bool,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct NodeInfoData {
    node: String,
    raw_output: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct NodeJobEntry {
    job_id: String,
    name: String,
    user: String,
    status: String,
    time: String,
    mem: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct NodeJobsData {
    node: String,
    running_jobs: Vec<NodeJobEntry>,
    pending_jobs: Vec<NodeJobEntry>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct PartitionNodeInfo {
    node: String,
    cpu_idle: u32,
    cpu_total: u32,
    jobs: u32,
    mem: String,
    gpu_idle: u32,
    gpu_total: u32,
    gpu_type: Option<String>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct PartitionInfoSection {
    partition: String,
    gpu_nodes: Vec<PartitionNodeInfo>,
    cpu_nodes: Vec<PartitionNodeInfo>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct PartitionInfoData {
    partitions: Vec<PartitionInfoSection>,
}

#[derive(Debug, Subcommand)]
enum Command {
    Alloc(AllocCommand),
    Cancel(CancelCommand),
    Connection(ConnectionCommand),
    Download(DownloadCommand),
    Exec(ExecCommand),
    FindGpu(FindGpuCommand),
    Jobs(JobsCommand),
    Log(LogCommand),
    NodeInfo(NodeInfoCommand),
    NodeJobs(NodeJobsCommand),
    PartitionInfo(PartitionInfoCommand),
    Release(ReleaseCommand),
    Run(RunCommand),
    Status(StatusCommand),
    Submit(SubmitCommand),
    Upload(UploadCommand),
    Server(ServerCommand),
    Session(SessionCommand),
}

#[derive(Debug, Args)]
struct CancelCommand {
    job_ids: Vec<String>,
    #[arg(long = "connection")]
    connection_id: String,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct AllocCommand {
    #[arg(short = 'p', long)]
    partition: String,
    #[arg(short = 'g', long = "gres")]
    gres: Option<String>,
    #[arg(short = 'c', long = "cpus")]
    cpus: Option<u32>,
    #[arg(long)]
    time: Option<String>,
    #[arg(long)]
    mem: Option<String>,
    #[arg(long)]
    nodelist: Option<String>,
    #[arg(long = "max-wait")]
    max_wait: Option<u32>,
    #[arg(long = "connection")]
    connection_id: String,
    #[arg(long)]
    execute: bool,
    #[arg(long)]
    json: bool,
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
    Ensure {
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
        #[arg(long = "default-keepalive-secs")]
        default_keepalive_secs: Option<u64>,
        #[arg(long)]
        json: bool,
    },
    List {
        #[arg(long)]
        json: bool,
    },
    Get {
        #[arg(long = "id")]
        connection_id: String,
        #[arg(long)]
        json: bool,
    },
    Remove {
        #[arg(long = "id")]
        connection_id: String,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Args)]
struct SessionCommand {
    #[command(subcommand)]
    command: SessionSubcommand,
}

#[derive(Debug, Subcommand)]
enum SessionSubcommand {
    Upsert {
        #[arg(long = "id")]
        session_id: String,
        #[arg(long = "connection")]
        connection_id: String,
        #[arg(long = "type")]
        session_type: String,
        #[arg(long)]
        description: Option<String>,
        #[arg(long, default_value = "active")]
        state: SessionStateArg,
        #[arg(long = "node-role", default_value = "unknown")]
        node_role: SessionNodeRoleArg,
        #[arg(long = "remote-host")]
        remote_host: Option<String>,
        #[arg(long = "compute-node")]
        compute_node: Option<String>,
        #[arg(long = "keepalive-secs")]
        keepalive_secs: Option<u64>,
        #[arg(long)]
        json: bool,
    },
    List {
        #[arg(long)]
        json: bool,
    },
    Get {
        #[arg(long = "id")]
        session_id: String,
        #[arg(long)]
        json: bool,
    },
    Remove {
        #[arg(long = "id")]
        session_id: String,
        #[arg(long)]
        json: bool,
    },
    Summary {
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

#[derive(Debug, Args)]
struct LogCommand {
    job_id: String,
    #[arg(long = "connection")]
    connection_id: String,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct NodeInfoCommand {
    node: String,
    #[arg(long = "connection")]
    connection_id: String,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct NodeJobsCommand {
    node: String,
    #[arg(long = "connection")]
    connection_id: String,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct PartitionInfoCommand {
    #[arg(short = 'p', long = "partition")]
    partition: Option<String>,
    #[arg(long = "connection")]
    connection_id: String,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct ReleaseCommand {
    job_id: String,
    #[arg(long = "connection")]
    connection_id: String,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct RunCommand {
    #[arg(long = "connection")]
    connection_id: String,
    #[arg(short = 'p', long)]
    partition: Option<String>,
    #[arg(short = 'g', long = "gres")]
    gres: Option<String>,
    #[arg(short = 'c', long = "cpus")]
    cpus: Option<u32>,
    #[arg(long)]
    time: Option<String>,
    #[arg(long)]
    mem: Option<String>,
    #[arg(long)]
    nodelist: Option<String>,
    #[arg(required = true, trailing_var_arg = true)]
    command: Vec<String>,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct StatusCommand {
    #[arg(long = "connection")]
    connection_id: String,
    #[arg(long)]
    gpu: bool,
    #[arg(long)]
    partition: Option<String>,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct FindGpuCommand {
    #[arg(long = "connection")]
    connection_id: String,
    gpu_type: Option<String>,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct SubmitCommand {
    script_path: String,
    #[arg(long = "connection")]
    connection_id: String,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct UploadCommand {
    local_path: String,
    remote_path: String,
    #[arg(long = "connection")]
    connection_id: String,
    #[arg(short = 'r', long)]
    recursive: bool,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct DownloadCommand {
    remote_path: String,
    local_path: String,
    #[arg(long = "connection")]
    connection_id: String,
    #[arg(short = 'r', long)]
    recursive: bool,
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

#[derive(Debug, Clone, clap::ValueEnum)]
enum SessionStateArg {
    Active,
    Idle,
    Closed,
}

impl From<SessionStateArg> for SessionState {
    fn from(value: SessionStateArg) -> Self {
        match value {
            SessionStateArg::Active => SessionState::Active,
            SessionStateArg::Idle => SessionState::Idle,
            SessionStateArg::Closed => SessionState::Closed,
        }
    }
}

#[derive(Debug, Clone, clap::ValueEnum)]
enum SessionNodeRoleArg {
    Login,
    Compute,
    Unknown,
}

impl From<SessionNodeRoleArg> for SessionNodeRole {
    fn from(value: SessionNodeRoleArg) -> Self {
        match value {
            SessionNodeRoleArg::Login => SessionNodeRole::Login,
            SessionNodeRoleArg::Compute => SessionNodeRole::Compute,
            SessionNodeRoleArg::Unknown => SessionNodeRole::Unknown,
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Alloc(cmd) => {
            let runtime = read_runtime_file(&runtime_file_path()?)?;
            let alloc_command = build_salloc_command(
                &cmd.partition,
                cmd.gres.as_deref(),
                cmd.cpus,
                cmd.time.as_deref(),
                cmd.mem.as_deref(),
                cmd.nodelist.as_deref(),
                cmd.max_wait,
            );
            if cmd.execute {
                let payload = exec_run(
                    &runtime,
                    &ExecRunRequest {
                        connection_id: cmd.connection_id,
                        command: alloc_command,
                        timeout_secs: 30,
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
            } else if cmd.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&AllocPlanOutput {
                        command: alloc_command,
                        execute: false,
                    })?
                );
            } else {
                println!("Interactive allocation command");
                println!("  {}", alloc_command);
            }
        }
        Command::Cancel(cmd) => {
            let runtime = read_runtime_file(&runtime_file_path()?)?;
            let payload = cancel_query(
                &runtime,
                &SlurmCancelRequest {
                    connection_id: cmd.connection_id,
                    job_ids: cmd.job_ids,
                },
            )?;
            if cmd.json {
                println!("{}", serde_json::to_string_pretty(&payload)?);
            } else {
                println!("Cancelled {} job(s)", payload.data.cancelled.len());
                for job_id in payload.data.cancelled {
                    println!("  {}", job_id);
                }
            }
        }
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
                    default_keepalive_secs,
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
                            default_keepalive_secs,
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
                                (Some(host), Some(port), Some(user)) => {
                                    format!("{user}@{host}:{port}")
                                }
                                _ => "local".to_string(),
                            };
                            println!(
                                "  {} [{}] {} keepalive:{}",
                                conn.label,
                                format!("{:?}", conn.kind).to_lowercase(),
                                endpoint,
                                conn.default_keepalive_secs
                                    .map(|v| v.to_string())
                                    .unwrap_or_else(|| "-".to_string())
                            );
                        }
                    }
                }
                ConnectionSubcommand::Get {
                    connection_id,
                    json,
                } => {
                    let payload = get_connection(&runtime, &connection_id)?;
                    if json {
                        println!("{}", serde_json::to_string_pretty(&payload)?);
                    } else {
                        print_connection_detail(&payload.data);
                    }
                }
                ConnectionSubcommand::Remove {
                    connection_id,
                    json,
                } => {
                    let payload = remove_connection(&runtime, &connection_id)?;
                    if json {
                        println!("{}", serde_json::to_string_pretty(&payload)?);
                    } else {
                        println!("Connection removed");
                        println!("  id: {}", connection_id);
                        println!("  deleted: {}", payload.data.deleted);
                    }
                }
            }
        }
        Command::Download(cmd) => {
            let runtime = read_runtime_file(&runtime_file_path()?)?;
            let payload = download_query(
                &runtime,
                &FileDownloadRequest {
                    connection_id: cmd.connection_id,
                    remote_path: cmd.remote_path,
                    local_path: cmd.local_path,
                    recursive: cmd.recursive,
                },
            )?;
            if cmd.json {
                println!("{}", serde_json::to_string_pretty(&payload)?);
            } else {
                print_transfer_text("Downloaded", &payload.data);
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
                    "{:<12} {:<12} {:<20} {:<12} {:<12} {:<12} {:<6} REASON",
                    "JOBID", "PARTITION", "NAME", "USER", "STATE", "TIME", "NODES"
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
        Command::Log(cmd) => {
            let runtime = read_runtime_file(&runtime_file_path()?)?;
            let payload = log_query(
                &runtime,
                &SlurmLogRequest {
                    connection_id: cmd.connection_id,
                    job_id: cmd.job_id,
                },
            )?;
            if cmd.json {
                println!("{}", serde_json::to_string_pretty(&payload)?);
            } else if !payload.data.found {
                println!("{}", payload.data.content);
            } else {
                print!("{}", payload.data.content);
                if !payload.data.content.ends_with('\n') {
                    println!();
                }
            }
        }
        Command::NodeInfo(cmd) => {
            let runtime = read_runtime_file(&runtime_file_path()?)?;
            let payload = node_info_query(&runtime, &cmd.connection_id, &cmd.node)?;
            if cmd.json {
                println!("{}", serde_json::to_string_pretty(&payload)?);
            } else {
                print!("{}", payload.raw_output);
                if !payload.raw_output.ends_with('\n') {
                    println!();
                }
            }
        }
        Command::NodeJobs(cmd) => {
            let runtime = read_runtime_file(&runtime_file_path()?)?;
            let payload = node_jobs_query(&runtime, &cmd.connection_id, &cmd.node)?;
            if cmd.json {
                println!("{}", serde_json::to_string_pretty(&payload)?);
            } else {
                print_node_jobs_text(&payload);
            }
        }
        Command::PartitionInfo(cmd) => {
            let runtime = read_runtime_file(&runtime_file_path()?)?;
            let payload =
                partition_info_query(&runtime, &cmd.connection_id, cmd.partition.as_deref())?;
            if cmd.json {
                println!("{}", serde_json::to_string_pretty(&payload)?);
            } else {
                print_partition_info_text(&payload);
            }
        }
        Command::Release(cmd) => {
            let runtime = read_runtime_file(&runtime_file_path()?)?;
            let payload = cancel_query(
                &runtime,
                &SlurmCancelRequest {
                    connection_id: cmd.connection_id,
                    job_ids: vec![cmd.job_id],
                },
            )?;
            if cmd.json {
                println!("{}", serde_json::to_string_pretty(&payload)?);
            } else {
                println!("Resources released");
                for job_id in payload.data.cancelled {
                    println!("  {}", job_id);
                }
            }
        }
        Command::Run(cmd) => {
            let runtime = read_runtime_file(&runtime_file_path()?)?;
            let srun_command = build_srun_command(
                &cmd.command,
                cmd.partition.as_deref(),
                cmd.gres.as_deref(),
                cmd.cpus,
                cmd.time.as_deref(),
                cmd.mem.as_deref(),
                cmd.nodelist.as_deref(),
            )?;
            let payload = exec_run(
                &runtime,
                &ExecRunRequest {
                    connection_id: cmd.connection_id,
                    command: srun_command,
                    timeout_secs: 30,
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
        Command::Status(cmd) => {
            if !cmd.gpu {
                bail!("only `status --gpu` is implemented right now");
            }
            let runtime = read_runtime_file(&runtime_file_path()?)?;
            let payload = status_gpu_query(
                &runtime,
                &SlurmStatusGpuRequest {
                    connection_id: cmd.connection_id,
                    partition: cmd.partition,
                },
            )?;
            if cmd.json {
                println!("{}", serde_json::to_string_pretty(&payload)?);
            } else {
                print_status_gpu_text(&payload.data);
            }
        }
        Command::FindGpu(cmd) => {
            let runtime = read_runtime_file(&runtime_file_path()?)?;
            let payload = find_gpu_query(
                &runtime,
                &SlurmFindGpuRequest {
                    connection_id: cmd.connection_id,
                    gpu_type: cmd.gpu_type,
                },
            )?;
            if cmd.json {
                println!("{}", serde_json::to_string_pretty(&payload)?);
            } else {
                print_find_gpu_text(&payload.data);
            }
        }
        Command::Submit(cmd) => {
            let runtime = read_runtime_file(&runtime_file_path()?)?;
            let payload = submit_query(
                &runtime,
                &SlurmSubmitRequest {
                    connection_id: cmd.connection_id,
                    script_path: cmd.script_path,
                },
            )?;
            if cmd.json {
                println!("{}", serde_json::to_string_pretty(&payload)?);
            } else {
                println!("{}", payload.data.raw_output);
                println!("job_id: {}", payload.data.job_id);
            }
        }
        Command::Upload(cmd) => {
            let runtime = read_runtime_file(&runtime_file_path()?)?;
            let payload = upload_query(
                &runtime,
                &FileUploadRequest {
                    connection_id: cmd.connection_id,
                    local_path: cmd.local_path,
                    remote_path: cmd.remote_path,
                    recursive: cmd.recursive,
                },
            )?;
            if cmd.json {
                println!("{}", serde_json::to_string_pretty(&payload)?);
            } else {
                print_transfer_text("Uploaded", &payload.data);
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
            ServerSubcommand::Ensure { json } => {
                let payload = ensure_server_running()?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&payload)?);
                } else {
                    println!("Server ready");
                    println!("  transport: {}", payload.data.transport);
                    println!("  endpoint: {}:{}", payload.data.host, payload.data.port);
                    println!("  runtime: {}", payload.data.runtime_path);
                    println!("  db: {}", payload.data.db_path);
                }
            }
        },
        Command::Session(cmd) => {
            let runtime = read_runtime_file(&runtime_file_path()?)?;
            match cmd.command {
                SessionSubcommand::Upsert {
                    session_id,
                    connection_id,
                    session_type,
                    description,
                    state,
                    node_role,
                    remote_host,
                    compute_node,
                    keepalive_secs,
                    json,
                } => {
                    let payload = upsert_session(
                        &runtime,
                        &SessionUpsertRequest {
                            id: session_id,
                            connection_id,
                            session_type,
                            description,
                            state: state.into(),
                            node_role: node_role.into(),
                            remote_host,
                            compute_node,
                            keepalive_secs,
                        },
                    )?;
                    if json {
                        println!("{}", serde_json::to_string_pretty(&payload)?);
                    } else {
                        println!("Session upserted");
                        println!("  id: {}", payload.data.session_id);
                        println!("  created: {}", payload.data.created);
                    }
                }
                SessionSubcommand::List { json } => {
                    let payload = list_sessions(&runtime)?;
                    if json {
                        println!("{}", serde_json::to_string_pretty(&payload)?);
                    } else {
                        print_sessions_text(&payload.data.sessions);
                    }
                }
                SessionSubcommand::Get { session_id, json } => {
                    let payload = get_session(&runtime, &session_id)?;
                    if json {
                        println!("{}", serde_json::to_string_pretty(&payload)?);
                    } else {
                        print_session_detail(&payload.data);
                    }
                }
                SessionSubcommand::Remove { session_id, json } => {
                    let payload = remove_session(&runtime, &session_id)?;
                    if json {
                        println!("{}", serde_json::to_string_pretty(&payload)?);
                    } else {
                        println!("Session removed: {}", payload.data.deleted);
                    }
                }
                SessionSubcommand::Summary { json } => {
                    let payload = summarize_sessions(&runtime)?;
                    if json {
                        println!("{}", serde_json::to_string_pretty(&payload)?);
                    } else {
                        print_session_summary_text(&payload.data);
                    }
                }
            }
        }
    }
    Ok(())
}

fn runtime_file_path() -> Result<PathBuf> {
    if let Ok(override_dir) = env::var("SLURM_ASSISTANT_DATA_DIR") {
        return Ok(PathBuf::from(override_dir).join("runtime.json"));
    }
    if cfg!(windows) {
        let base = env::var("APPDATA").context("APPDATA not set")?;
        return Ok(PathBuf::from(base)
            .join("slurm-assistant")
            .join("runtime.json"));
    }
    if let Ok(xdg_state) = env::var("XDG_STATE_HOME") {
        return Ok(PathBuf::from(xdg_state)
            .join("slurm-assistant")
            .join("runtime.json"));
    }
    let home = env::var("HOME").context("HOME not set")?;
    Ok(PathBuf::from(home)
        .join(".local")
        .join("share")
        .join("slurm-assistant")
        .join("runtime.json"))
}

fn read_runtime_file(path: &Path) -> Result<RuntimeFile> {
    let bytes = fs::read(path)
        .with_context(|| format!("failed to read runtime file {}", path.display()))?;
    let runtime = serde_json::from_slice::<RuntimeFile>(&bytes)
        .with_context(|| format!("failed to parse runtime file {}", path.display()))?;
    Ok(runtime)
}

fn fetch_server_status(runtime: &RuntimeFile) -> Result<SuccessResponse<ServerStatusData>> {
    send_request_json(
        http_client().get(format!(
            "http://{}:{}/v1/server/status",
            runtime.host, runtime.port
        )),
        runtime,
        "failed to decode server status response",
    )
}

fn ensure_server_running() -> Result<SuccessResponse<ServerStatusData>> {
    let runtime_path = runtime_file_path()?;

    if let Ok(runtime) = read_runtime_file(&runtime_path) {
        if let Ok(payload) = fetch_server_status(&runtime) {
            return Ok(payload);
        }
    }

    start_server_background()?;

    let deadline = Instant::now() + Duration::from_secs(8);
    let mut last_error: Option<anyhow::Error> = None;
    while Instant::now() < deadline {
        match read_runtime_file(&runtime_path) {
            Ok(runtime) => match fetch_server_status(&runtime) {
                Ok(payload) => return Ok(payload),
                Err(err) => last_error = Some(err),
            },
            Err(err) => last_error = Some(err),
        }
        thread::sleep(Duration::from_millis(200));
    }

    if let Some(err) = last_error {
        bail!("server ensure failed: {err}");
    }
    bail!("server ensure failed: timeout waiting for server startup");
}

fn start_server_background() -> Result<()> {
    ProcessCommand::new("slurm-server")
        .arg("serve")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("failed to start slurm-server (is it installed and in PATH?)")?;
    Ok(())
}

fn add_connection(
    runtime: &RuntimeFile,
    request: &ConnectionAddRequest,
) -> Result<SuccessResponse<slurm_proto::ConnectionAddData>> {
    send_request_json(
        http_client()
            .post(format!(
                "http://{}:{}/v1/connections/add",
                runtime.host, runtime.port
            ))
            .json(request),
        runtime,
        "failed to decode connection add response",
    )
}

fn list_connections(runtime: &RuntimeFile) -> Result<SuccessResponse<ConnectionListData>> {
    send_request_json(
        http_client().get(format!(
            "http://{}:{}/v1/connections/list",
            runtime.host, runtime.port
        )),
        runtime,
        "failed to decode connection list response",
    )
}

fn get_connection(
    runtime: &RuntimeFile,
    connection_id: &str,
) -> Result<SuccessResponse<ConnectionRecord>> {
    send_request_json(
        http_client().get(format!(
            "http://{}:{}/v1/connections/{}",
            runtime.host, runtime.port, connection_id
        )),
        runtime,
        "failed to decode connection get response",
    )
}

fn remove_connection(
    runtime: &RuntimeFile,
    connection_id: &str,
) -> Result<SuccessResponse<ConnectionDeleteData>> {
    send_request_json(
        http_client().delete(format!(
            "http://{}:{}/v1/connections/{}",
            runtime.host, runtime.port, connection_id
        )),
        runtime,
        "failed to decode connection remove response",
    )
}

fn upsert_session(
    runtime: &RuntimeFile,
    request: &SessionUpsertRequest,
) -> Result<SuccessResponse<slurm_proto::SessionUpsertData>> {
    send_request_json(
        http_client()
            .post(format!(
                "http://{}:{}/v1/sessions/upsert",
                runtime.host, runtime.port
            ))
            .json(request),
        runtime,
        "failed to decode session upsert response",
    )
}

fn list_sessions(runtime: &RuntimeFile) -> Result<SuccessResponse<SessionListData>> {
    send_request_json(
        http_client().get(format!(
            "http://{}:{}/v1/sessions/list",
            runtime.host, runtime.port
        )),
        runtime,
        "failed to decode session list response",
    )
}

fn get_session(runtime: &RuntimeFile, session_id: &str) -> Result<SuccessResponse<SessionRecord>> {
    send_request_json(
        http_client().get(format!(
            "http://{}:{}/v1/sessions/{}",
            runtime.host, runtime.port, session_id
        )),
        runtime,
        "failed to decode session get response",
    )
}

fn remove_session(
    runtime: &RuntimeFile,
    session_id: &str,
) -> Result<SuccessResponse<SessionDeleteData>> {
    send_request_json(
        http_client().delete(format!(
            "http://{}:{}/v1/sessions/{}",
            runtime.host, runtime.port, session_id
        )),
        runtime,
        "failed to decode session remove response",
    )
}

fn summarize_sessions(runtime: &RuntimeFile) -> Result<SuccessResponse<SessionSummaryData>> {
    send_request_json(
        http_client().get(format!(
            "http://{}:{}/v1/sessions/summary",
            runtime.host, runtime.port
        )),
        runtime,
        "failed to decode session summary response",
    )
}

fn exec_run(
    runtime: &RuntimeFile,
    request: &ExecRunRequest,
) -> Result<SuccessResponse<ExecRunData>> {
    send_request_json(
        http_client()
            .post(format!(
                "http://{}:{}/v1/exec/run",
                runtime.host, runtime.port
            ))
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
            .post(format!(
                "http://{}:{}/v1/slurm/jobs",
                runtime.host, runtime.port
            ))
            .json(request),
        runtime,
        "failed to decode jobs response",
    )
}

fn log_query(
    runtime: &RuntimeFile,
    request: &SlurmLogRequest,
) -> Result<SuccessResponse<SlurmLogData>> {
    send_request_json(
        http_client()
            .post(format!(
                "http://{}:{}/v1/slurm/log",
                runtime.host, runtime.port
            ))
            .json(request),
        runtime,
        "failed to decode log response",
    )
}

fn cancel_query(
    runtime: &RuntimeFile,
    request: &SlurmCancelRequest,
) -> Result<SuccessResponse<SlurmCancelData>> {
    send_request_json(
        http_client()
            .post(format!(
                "http://{}:{}/v1/slurm/cancel",
                runtime.host, runtime.port
            ))
            .json(request),
        runtime,
        "failed to decode cancel response",
    )
}

fn submit_query(
    runtime: &RuntimeFile,
    request: &SlurmSubmitRequest,
) -> Result<SuccessResponse<SlurmSubmitData>> {
    send_request_json(
        http_client()
            .post(format!(
                "http://{}:{}/v1/slurm/submit",
                runtime.host, runtime.port
            ))
            .json(request),
        runtime,
        "failed to decode submit response",
    )
}

fn upload_query(
    runtime: &RuntimeFile,
    request: &FileUploadRequest,
) -> Result<SuccessResponse<FileTransferData>> {
    send_request_json(
        http_client()
            .post(format!(
                "http://{}:{}/v1/files/upload",
                runtime.host, runtime.port
            ))
            .json(request),
        runtime,
        "failed to decode upload response",
    )
}

fn download_query(
    runtime: &RuntimeFile,
    request: &FileDownloadRequest,
) -> Result<SuccessResponse<FileTransferData>> {
    send_request_json(
        http_client()
            .post(format!(
                "http://{}:{}/v1/files/download",
                runtime.host, runtime.port
            ))
            .json(request),
        runtime,
        "failed to decode download response",
    )
}

fn status_gpu_query(
    runtime: &RuntimeFile,
    request: &SlurmStatusGpuRequest,
) -> Result<SuccessResponse<SlurmStatusGpuData>> {
    send_request_json(
        http_client()
            .post(format!(
                "http://{}:{}/v1/slurm/status_gpu",
                runtime.host, runtime.port
            ))
            .json(request),
        runtime,
        "failed to decode status gpu response",
    )
}

fn find_gpu_query(
    runtime: &RuntimeFile,
    request: &SlurmFindGpuRequest,
) -> Result<SuccessResponse<SlurmFindGpuData>> {
    send_request_json(
        http_client()
            .post(format!(
                "http://{}:{}/v1/slurm/find_gpu",
                runtime.host, runtime.port
            ))
            .json(request),
        runtime,
        "failed to decode find gpu response",
    )
}

fn node_info_query(runtime: &RuntimeFile, connection_id: &str, node: &str) -> Result<NodeInfoData> {
    let payload = exec_run(
        runtime,
        &ExecRunRequest {
            connection_id: connection_id.to_string(),
            command: format!("scontrol show node {node}"),
            timeout_secs: 30,
        },
    )?;
    if payload.data.exit_code != 0 {
        bail!("node-info command failed: {}", payload.data.stderr.trim());
    }
    Ok(NodeInfoData {
        node: node.to_string(),
        raw_output: payload.data.stdout,
    })
}

fn node_jobs_query(runtime: &RuntimeFile, connection_id: &str, node: &str) -> Result<NodeJobsData> {
    let running = exec_run(
        runtime,
        &ExecRunRequest {
            connection_id: connection_id.to_string(),
            command: format!("squeue -w {node} -t RUNNING -h -o '%i|%j|%u|%T|%M|%m'"),
            timeout_secs: 30,
        },
    )?;
    if running.data.exit_code != 0 {
        bail!(
            "node-jobs running query failed: {}",
            running.data.stderr.trim()
        );
    }

    let pending = exec_run(
        runtime,
        &ExecRunRequest {
            connection_id: connection_id.to_string(),
            command: "squeue -t PENDING -h -o '%i|%j|%u|%T|%M|%P|%m'".to_string(),
            timeout_secs: 30,
        },
    )?;
    if pending.data.exit_code != 0 {
        bail!(
            "node-jobs pending query failed: {}",
            pending.data.stderr.trim()
        );
    }

    let node_partitions = exec_run(
        runtime,
        &ExecRunRequest {
            connection_id: connection_id.to_string(),
            command: format!("sinfo -N -n {node} -h -o '%P'"),
            timeout_secs: 30,
        },
    )?;
    if node_partitions.data.exit_code != 0 {
        bail!(
            "node-jobs partition query failed: {}",
            node_partitions.data.stderr.trim()
        );
    }

    Ok(NodeJobsData {
        node: node.to_string(),
        running_jobs: parse_node_running_jobs(&running.data.stdout)?,
        pending_jobs: parse_node_pending_jobs(&pending.data.stdout, &node_partitions.data.stdout)?,
    })
}

fn partition_info_query(
    runtime: &RuntimeFile,
    connection_id: &str,
    partition: Option<&str>,
) -> Result<PartitionInfoData> {
    let (nodes_command, jobs_command) = if let Some(partition) = partition {
        (
            format!("sinfo -p {partition} -N -h -o '%N|%C|%G|%m'"),
            format!("squeue -p {partition} -t RUNNING -h -o '%i|%N|%b|%M'"),
        )
    } else {
        (
            "sinfo -N -h -o '%N|%P|%C|%G|%m'".to_string(),
            "squeue -t RUNNING -h -o '%i|%N|%b|%M'".to_string(),
        )
    };

    let nodes = exec_run(
        runtime,
        &ExecRunRequest {
            connection_id: connection_id.to_string(),
            command: nodes_command,
            timeout_secs: 30,
        },
    )?;
    if nodes.data.exit_code != 0 {
        bail!(
            "partition-info node query failed: {}",
            nodes.data.stderr.trim()
        );
    }

    let jobs = exec_run(
        runtime,
        &ExecRunRequest {
            connection_id: connection_id.to_string(),
            command: jobs_command,
            timeout_secs: 30,
        },
    )?;
    if jobs.data.exit_code != 0 {
        bail!(
            "partition-info jobs query failed: {}",
            jobs.data.stderr.trim()
        );
    }

    parse_partition_info(&nodes.data.stdout, &jobs.data.stdout, partition)
}

fn http_client() -> Client {
    Client::new()
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

fn print_status_gpu_text(data: &SlurmStatusGpuData) {
    if data.available_nodes.is_empty() && data.drain_nodes.is_empty() {
        println!("No GPU nodes found");
        println!();
        println!("Summary");
        println!("  available_nodes: 0");
        println!("  total_gpu: 0");
        println!("  idle_gpu: 0");
        return;
    }

    if !data.available_nodes.is_empty() {
        println!("[AVAILABLE] GPU nodes ({})", data.available_nodes.len());
        print_gpu_table(&data.available_nodes);
    }
    if !data.drain_nodes.is_empty() {
        if !data.available_nodes.is_empty() {
            println!();
        }
        println!("[DRAIN] GPU nodes ({})", data.drain_nodes.len());
        print_gpu_table(&data.drain_nodes);
    }
    println!();
    print_gpu_summary(
        data.summary.available_nodes,
        data.summary.total_gpu,
        data.summary.idle_gpu,
    );
}

fn print_find_gpu_text(data: &SlurmFindGpuData) {
    if data.available_nodes.is_empty() && data.busy_nodes.is_empty() && data.drain_nodes.is_empty()
    {
        println!("No matching GPU nodes found");
        println!();
        print_gpu_summary(0, 0, 0);
        return;
    }

    if !data.available_nodes.is_empty() {
        println!(
            "[AVAILABLE] Nodes with idle GPU ({})",
            data.available_nodes.len()
        );
        print_gpu_table(&data.available_nodes);
    }
    if !data.busy_nodes.is_empty() {
        if !data.available_nodes.is_empty() {
            println!();
        }
        println!("[BUSY] Nodes without idle GPU ({})", data.busy_nodes.len());
        print_gpu_table(&data.busy_nodes);
    }
    if !data.drain_nodes.is_empty() {
        if !data.available_nodes.is_empty() || !data.busy_nodes.is_empty() {
            println!();
        }
        println!("[DRAIN] Unavailable GPU nodes ({})", data.drain_nodes.len());
        print_gpu_table(&data.drain_nodes);
    }
    println!();
    print_gpu_summary(
        data.summary.available_nodes,
        data.summary.total_gpu,
        data.summary.idle_gpu,
    );
}

fn print_gpu_table(nodes: &[SlurmGpuNode]) {
    println!(
        "{:<20} {:<12} {:<15} {:<15} GPU TYPE",
        "NODE", "PARTITION", "GPU IDLE/TOTAL", "CPU IDLE/TOTAL"
    );
    for node in nodes {
        println!(
            "{:<20} {:<12} {:<15} {:<15} {}",
            truncate_for_table(&node.node, 20),
            truncate_for_table(&node.partition, 12),
            format!("{}/{}", node.gpu_idle, node.gpu_total),
            format!("{}/{}", node.cpu_idle, node.cpu_total),
            node.gpu_type
        );
    }
}

fn print_gpu_summary(available_nodes: u32, total_gpu: u32, idle_gpu: u32) {
    println!("Summary");
    println!("  available_nodes: {}", available_nodes);
    println!("  total_gpu: {}", total_gpu);
    println!("  idle_gpu: {}", idle_gpu);
}

fn print_transfer_text(action: &str, data: &FileTransferData) {
    println!("{} {}", action, data.source_path);
    println!("  to: {}", data.destination_path);
    println!("  recursive: {}", data.recursive);
}

fn print_connection_detail(connection: &ConnectionRecord) {
    println!("Connection");
    println!("  id: {}", connection.id);
    println!("  label: {}", connection.label);
    println!(
        "  kind: {}",
        format!("{:?}", connection.kind).to_lowercase()
    );
    println!(
        "  endpoint: {}",
        match (&connection.host, connection.port, &connection.username) {
            (Some(host), Some(port), Some(user)) => format!("{user}@{host}:{port}"),
            _ => "local".to_string(),
        }
    );
    println!(
        "  jump_host: {}",
        connection.jump_host.as_deref().unwrap_or("-")
    );
    println!(
        "  default_keepalive_secs: {}",
        connection
            .default_keepalive_secs
            .map(|v| v.to_string())
            .unwrap_or_else(|| "-".to_string())
    );
}

fn print_session_detail(session: &SessionRecord) {
    println!("Session");
    println!("  id: {}", session.id);
    println!("  connection: {}", session.connection_id);
    println!("  type: {}", session.session_type);
    println!("  state: {:?}", session.state);
    println!("  node_role: {:?}", session.node_role);
    println!(
        "  description: {}",
        session.description.as_deref().unwrap_or("-")
    );
    println!(
        "  remote_host: {}",
        session.remote_host.as_deref().unwrap_or("-")
    );
    println!(
        "  compute_node: {}",
        session.compute_node.as_deref().unwrap_or("-")
    );
    println!(
        "  keepalive_secs: {}",
        session
            .keepalive_secs
            .map(|v| v.to_string())
            .unwrap_or_else(|| "-".to_string())
    );
    println!("  created_at: {}", session.created_at);
    println!("  last_seen_at: {}", session.last_seen_at);
}

fn print_sessions_text(sessions: &[SessionRecord]) {
    if sessions.is_empty() {
        println!("No sessions found");
        return;
    }
    println!(
        "{:<18} {:<20} {:<8} {:<8} {:<16} {:<10} Description",
        "SESSION_ID", "CONNECTION", "TYPE", "STATE", "COMPUTE_NODE", "KEEPALIVE"
    );
    for session in sessions {
        println!(
            "{:<18} {:<20} {:<8} {:<8} {:<16} {:<10} {}",
            truncate_for_table(&session.id, 18),
            truncate_for_table(&session.connection_id, 20),
            truncate_for_table(&session.session_type, 8),
            truncate_for_table(&format!("{:?}", session.state).to_lowercase(), 8),
            truncate_for_table(session.compute_node.as_deref().unwrap_or("-"), 16),
            session
                .keepalive_secs
                .map(|v| v.to_string())
                .unwrap_or_else(|| "-".to_string()),
            session.description.as_deref().unwrap_or("-")
        );
    }
}

fn print_session_summary_text(summary: &SessionSummaryData) {
    println!("Active sessions: {}", summary.total_active);
    if summary.connections.is_empty() {
        return;
    }
    println!(
        "{:<20} {:<7} {:<18} {:<16} {:<10} Description",
        "CONNECTION", "ACTIVE", "CURRENT_SESSION", "COMPUTE_NODE", "KEEPALIVE"
    );
    for item in &summary.connections {
        println!(
            "{:<20} {:<7} {:<18} {:<16} {:<10} {}",
            truncate_for_table(&item.connection_id, 20),
            item.active_count,
            truncate_for_table(item.current_session_id.as_deref().unwrap_or("-"), 18),
            truncate_for_table(item.current_compute_node.as_deref().unwrap_or("-"), 16),
            item.current_keepalive_secs
                .map(|v| v.to_string())
                .unwrap_or_else(|| "-".to_string()),
            item.current_description.as_deref().unwrap_or("-")
        );
    }
}

fn parse_cpu_alloc(alloc: &str) -> (u32, u32, u32, u32) {
    let parts: Vec<&str> = alloc.split('/').collect();
    if parts.len() != 4 {
        return (0, 0, 0, 0);
    }
    (
        parts[0].parse().unwrap_or(0),
        parts[1].parse().unwrap_or(0),
        parts[2].parse().unwrap_or(0),
        parts[3].parse().unwrap_or(0),
    )
}

fn parse_gpu_gres_local(gres: &str) -> (u32, Option<String>) {
    let lower = gres.to_ascii_lowercase();
    if let Some(captures) = regex::Regex::new(r"gpu:([a-zA-Z_]\w*):(\d+)")
        .ok()
        .and_then(|re| re.captures(&lower))
    {
        return (
            captures
                .get(2)
                .and_then(|m| m.as_str().parse().ok())
                .unwrap_or(0),
            captures.get(1).map(|m| m.as_str().to_ascii_uppercase()),
        );
    }
    if let Some(captures) = regex::Regex::new(r"gpu:(\d+)")
        .ok()
        .and_then(|re| re.captures(&lower))
    {
        return (
            captures
                .get(1)
                .and_then(|m| m.as_str().parse().ok())
                .unwrap_or(0),
            Some("UNKNOWN".to_string()),
        );
    }
    (0, None)
}

fn parse_node_running_jobs(stdout: &str) -> Result<Vec<NodeJobEntry>> {
    let mut jobs = Vec::new();
    for line in stdout.lines().filter(|line| !line.trim().is_empty()) {
        let parts: Vec<&str> = line.split('|').collect();
        if parts.len() < 6 {
            bail!("failed to parse running node-jobs row: {line}");
        }
        jobs.push(NodeJobEntry {
            job_id: parts[0].to_string(),
            name: parts[1].to_string(),
            user: parts[2].to_string(),
            status: parts[3].to_string(),
            time: parts[4].to_string(),
            mem: parts[5].to_string(),
        });
    }
    Ok(jobs)
}

fn parse_node_pending_jobs(stdout: &str, partitions_stdout: &str) -> Result<Vec<NodeJobEntry>> {
    let partitions: Vec<&str> = partitions_stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    let mut jobs = Vec::new();
    for line in stdout.lines().filter(|line| !line.trim().is_empty()) {
        let parts: Vec<&str> = line.split('|').collect();
        if parts.len() < 7 {
            bail!("failed to parse pending node-jobs row: {line}");
        }
        if partitions.iter().any(|partition| partition == &parts[5]) {
            jobs.push(NodeJobEntry {
                job_id: parts[0].to_string(),
                name: parts[1].to_string(),
                user: parts[2].to_string(),
                status: parts[3].to_string(),
                time: parts[4].to_string(),
                mem: parts[6].to_string(),
            });
        }
    }
    Ok(jobs)
}

fn parse_partition_info(
    nodes_stdout: &str,
    jobs_stdout: &str,
    fixed_partition: Option<&str>,
) -> Result<PartitionInfoData> {
    use std::collections::BTreeMap;

    let mut partitions: BTreeMap<String, BTreeMap<String, PartitionNodeInfo>> = BTreeMap::new();
    for line in nodes_stdout.lines().filter(|line| !line.trim().is_empty()) {
        let parts: Vec<&str> = line.split('|').collect();
        let (node, partition, cpu, gres, mem) = if let Some(fixed_partition) = fixed_partition {
            if parts.len() < 4 {
                bail!("failed to parse partition node row: {line}");
            }
            (parts[0], fixed_partition, parts[1], parts[2], parts[3])
        } else {
            if parts.len() < 5 {
                bail!("failed to parse partition node row: {line}");
            }
            (parts[0], parts[1], parts[2], parts[3], parts[4])
        };
        let (_, cpu_idle, _, cpu_total) = parse_cpu_alloc(cpu);
        let (gpu_total, gpu_type) = parse_gpu_gres_local(gres);
        partitions.entry(partition.to_string()).or_default().insert(
            node.to_string(),
            PartitionNodeInfo {
                node: node.to_string(),
                cpu_idle,
                cpu_total,
                jobs: 0,
                mem: mem.to_string(),
                gpu_idle: gpu_total,
                gpu_total,
                gpu_type,
            },
        );
    }

    let gpu_count_regex = regex::Regex::new(r"gpu:\w*:?(\d+)")
        .context("failed to compile partition-info GPU usage regex")?;

    for line in jobs_stdout.lines().filter(|line| !line.trim().is_empty()) {
        let parts: Vec<&str> = line.split('|').collect();
        if parts.len() < 4 {
            bail!("failed to parse partition jobs row: {line}");
        }
        let node_list = parts[1];
        let gres = parts[2].to_ascii_lowercase();
        let used_gpu = gpu_count_regex
            .captures(&gres)
            .and_then(|captures| captures.get(1))
            .and_then(|value| value.as_str().parse::<u32>().ok())
            .unwrap_or(0);
        for node in node_list
            .split(',')
            .map(str::trim)
            .filter(|node| !node.is_empty())
        {
            for nodes in partitions.values_mut() {
                if let Some(info) = nodes.get_mut(node) {
                    info.jobs += 1;
                    info.gpu_idle = info.gpu_total.saturating_sub(used_gpu.min(info.gpu_total));
                }
            }
        }
    }

    Ok(PartitionInfoData {
        partitions: partitions
            .into_iter()
            .map(|(partition, nodes)| {
                let mut gpu_nodes = Vec::new();
                let mut cpu_nodes = Vec::new();
                for (_, node) in nodes {
                    if node.gpu_total > 0 {
                        gpu_nodes.push(node);
                    } else {
                        cpu_nodes.push(node);
                    }
                }
                gpu_nodes.sort_by(|a, b| a.node.cmp(&b.node));
                cpu_nodes.sort_by(|a, b| a.node.cmp(&b.node));
                PartitionInfoSection {
                    partition,
                    gpu_nodes,
                    cpu_nodes,
                }
            })
            .collect(),
    })
}

fn print_node_jobs_text(data: &NodeJobsData) {
    println!("Node: {}", data.node);
    println!();
    println!("[RUNNING] Running jobs ({})", data.running_jobs.len());
    if data.running_jobs.is_empty() {
        println!("  None");
    } else {
        println!(
            "{:<10} {:<25} {:<12} {:<12} Memory",
            "JOBID", "Name", "User", "Runtime"
        );
        for job in &data.running_jobs {
            println!(
                "{:<10} {:<25} {:<12} {:<12} {}",
                job.job_id,
                truncate_for_table(&job.name, 25),
                truncate_for_table(&job.user, 12),
                truncate_for_table(&job.time, 12),
                job.mem
            );
        }
    }
    println!();
    println!("[PENDING] Pending jobs ({})", data.pending_jobs.len());
    if data.pending_jobs.is_empty() {
        println!("  None");
    } else {
        println!(
            "{:<10} {:<25} {:<12} {:<12} Memory",
            "JOBID", "Name", "User", "WaitTime"
        );
        for job in &data.pending_jobs {
            println!(
                "{:<10} {:<25} {:<12} {:<12} {}",
                job.job_id,
                truncate_for_table(&job.name, 25),
                truncate_for_table(&job.user, 12),
                truncate_for_table(&job.time, 12),
                job.mem
            );
        }
    }
}

fn print_partition_info_text(data: &PartitionInfoData) {
    for section in &data.partitions {
        println!("============================================================");
        println!("Partition: {}", section.partition);
        println!("============================================================");
        if !section.gpu_nodes.is_empty() {
            println!();
            println!("[GPU Nodes] ({})", section.gpu_nodes.len());
            println!(
                "{:<18} {:<14} {:<14} {:<8} Memory",
                "Node", "GPU Idle/Total", "CPU Idle/Total", "Jobs"
            );
            for node in &section.gpu_nodes {
                println!(
                    "{:<18} {:<14} {:<14} {:<8} {}",
                    truncate_for_table(&node.node, 18),
                    format!("{}/{}", node.gpu_idle, node.gpu_total),
                    format!("{}/{}", node.cpu_idle, node.cpu_total),
                    node.jobs,
                    node.mem
                );
            }
        }
        if !section.cpu_nodes.is_empty() {
            println!();
            println!("[CPU Nodes] ({})", section.cpu_nodes.len());
            println!(
                "{:<18} {:<14} {:<8} Memory",
                "Node", "CPU Idle/Total", "Jobs"
            );
            for node in &section.cpu_nodes {
                println!(
                    "{:<18} {:<14} {:<8} {}",
                    truncate_for_table(&node.node, 18),
                    format!("{}/{}", node.cpu_idle, node.cpu_total),
                    node.jobs,
                    node.mem
                );
            }
        }
        println!();
    }
}

fn build_salloc_command(
    partition: &str,
    gres: Option<&str>,
    cpus: Option<u32>,
    time: Option<&str>,
    mem: Option<&str>,
    nodelist: Option<&str>,
    max_wait: Option<u32>,
) -> String {
    let mut parts = vec![
        "salloc".to_string(),
        "-p".to_string(),
        partition.to_string(),
    ];
    if let Some(cpus) = cpus {
        parts.push(format!("--cpus-per-task={cpus}"));
    }
    if let Some(gres) = gres {
        parts.push(format!("--gres={gres}"));
    }
    if let Some(time) = time {
        parts.push(format!("--time={time}"));
    }
    if let Some(mem) = mem {
        parts.push(format!("--mem={mem}"));
    }
    if let Some(nodelist) = nodelist {
        parts.push("-w".to_string());
        parts.push(nodelist.to_string());
    }
    if let Some(max_wait) = max_wait {
        parts.push(format!("--wait={max_wait}"));
    }
    parts.join(" ")
}

fn build_srun_command(
    command: &[String],
    partition: Option<&str>,
    gres: Option<&str>,
    cpus: Option<u32>,
    time: Option<&str>,
    mem: Option<&str>,
    nodelist: Option<&str>,
) -> Result<String> {
    if command.is_empty() {
        bail!("must provide command for srun");
    }
    let mut parts = vec!["srun".to_string()];
    if let Some(partition) = partition {
        parts.push("-p".to_string());
        parts.push(partition.to_string());
    }
    if let Some(gres) = gres {
        parts.push(format!("--gres={gres}"));
    }
    if let Some(cpus) = cpus {
        parts.push(format!("--cpus-per-task={cpus}"));
    }
    if let Some(time) = time {
        parts.push(format!("--time={time}"));
    }
    if let Some(mem) = mem {
        parts.push(format!("--mem={mem}"));
    }
    if let Some(nodelist) = nodelist {
        parts.push("-w".to_string());
        parts.push(nodelist.to_string());
    }
    parts.extend(command.iter().cloned());
    Ok(parts.join(" "))
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
    let body = response
        .text()
        .context("failed to read server response body")?;
    if !status.is_success() {
        bail!("server returned {}: {}", status, body);
    }

    let payload =
        serde_json::from_str::<T>(&body).with_context(|| decode_error_context.to_string())?;
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

    #[test]
    fn gpu_summary_text_mentions_counts() {
        let data = SlurmStatusGpuData {
            available_nodes: vec![SlurmGpuNode {
                node: "gpu-a10-3".to_string(),
                partition: "gpu-a10".to_string(),
                gpu_idle: 1,
                gpu_total: 2,
                gpu_type: "A10".to_string(),
                cpu_idle: 16,
                cpu_total: 32,
            }],
            drain_nodes: vec![],
            summary: slurm_proto::SlurmGpuSummary {
                available_nodes: 1,
                total_gpu: 2,
                idle_gpu: 1,
            },
        };
        assert_eq!(data.summary.available_nodes, 1);
        assert_eq!(data.available_nodes[0].gpu_type, "A10");
    }

    #[test]
    fn build_salloc_command_renders_optional_flags() {
        let command = build_salloc_command(
            "gpu-a10",
            Some("gpu:1"),
            Some(8),
            Some("00:30:00"),
            Some("16G"),
            Some("gpu-a10-3"),
            Some(5),
        );
        assert_eq!(
            command,
            "salloc -p gpu-a10 --cpus-per-task=8 --gres=gpu:1 --time=00:30:00 --mem=16G -w gpu-a10-3 --wait=5"
        );
    }

    #[test]
    fn build_srun_command_requires_payload() {
        let err = build_srun_command(&[], None, None, None, None, None, None).unwrap_err();
        assert!(err.to_string().contains("must provide command"));
    }

    #[test]
    fn build_srun_command_renders_flags() {
        let command = build_srun_command(
            &["python".to_string(), "train.py".to_string()],
            Some("gpu-a10"),
            Some("gpu:1"),
            Some(8),
            Some("01:00:00"),
            Some("32G"),
            Some("gpu-a10-7"),
        )
        .unwrap();
        assert_eq!(
            command,
            "srun -p gpu-a10 --gres=gpu:1 --cpus-per-task=8 --time=01:00:00 --mem=32G -w gpu-a10-7 python train.py"
        );
    }

    #[test]
    fn parse_node_running_jobs_decodes_rows() {
        let jobs = parse_node_running_jobs("123|train|alice|RUNNING|00:12|8G\n").unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].job_id, "123");
        assert_eq!(jobs[0].mem, "8G");
    }

    #[test]
    fn parse_partition_info_groups_gpu_and_cpu_nodes() {
        let nodes = "\
gpu-a10-1|gpu-a10|28/20/0/48|gpu:a10:2|128000\n\
cpu48c-1|cpu48c|4/44/0/48|(null)|64000\n";
        let jobs = "9001|gpu-a10-1|gpu:1|00:10\n";
        let parsed = parse_partition_info(nodes, jobs, None).unwrap();
        assert_eq!(parsed.partitions.len(), 2);
        assert_eq!(
            parsed.partitions[0].gpu_nodes.len() + parsed.partitions[0].cpu_nodes.len(),
            1
        );
        let gpu_section = parsed
            .partitions
            .iter()
            .find(|section| section.partition == "gpu-a10")
            .unwrap();
        assert_eq!(gpu_section.gpu_nodes[0].gpu_idle, 1);
        assert_eq!(gpu_section.gpu_nodes[0].jobs, 1);
    }
}
