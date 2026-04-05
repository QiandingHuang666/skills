use std::{
    env, fs,
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};
use clap::{Args, Parser, Subcommand};
use reqwest::blocking::Client;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use slurm_proto::{
    ConnectionAddRequest, ConnectionDeleteData, ConnectionKind, ConnectionListData, ConnectionRecord,
    ExecRunData, ExecRunRequest, FileDownloadRequest, FileTransferData, FileUploadRequest,
    RuntimeFile, ServerStatusData, SlurmCancelData, SlurmCancelRequest, SlurmFindGpuData,
    SlurmFindGpuRequest, SlurmGpuNode, SlurmJobsData, SlurmJobsRequest, SlurmLogData,
    SlurmLogRequest, SlurmStatusGpuData, SlurmStatusGpuRequest, SlurmSubmitData,
    SlurmSubmitRequest, SuccessResponse,
};

#[derive(Debug, Parser)]
#[command(name = "slurm-client", about = "Rust client scaffold for slurm-assistant")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Cancel(CancelCommand),
    Connection(ConnectionCommand),
    Download(DownloadCommand),
    Exec(ExecCommand),
    FindGpu(FindGpuCommand),
    Jobs(JobsCommand),
    Log(LogCommand),
    Status(StatusCommand),
    Submit(SubmitCommand),
    Upload(UploadCommand),
    Server(ServerCommand),
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

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
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
                ConnectionSubcommand::Get { connection_id, json } => {
                    let payload = get_connection(&runtime, &connection_id)?;
                    if json {
                        println!("{}", serde_json::to_string_pretty(&payload)?);
                    } else {
                        print_connection_detail(&payload.data);
                    }
                }
                ConnectionSubcommand::Remove { connection_id, json } => {
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

fn get_connection(runtime: &RuntimeFile, connection_id: &str) -> Result<SuccessResponse<ConnectionRecord>> {
    send_request_json(
        http_client()
            .get(format!(
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
        http_client()
            .delete(format!(
                "http://{}:{}/v1/connections/{}",
                runtime.host, runtime.port, connection_id
            )),
        runtime,
        "failed to decode connection remove response",
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

fn log_query(
    runtime: &RuntimeFile,
    request: &SlurmLogRequest,
) -> Result<SuccessResponse<SlurmLogData>> {
    send_request_json(
        http_client()
            .post(format!("http://{}:{}/v1/slurm/log", runtime.host, runtime.port))
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
            .post(format!("http://{}:{}/v1/slurm/cancel", runtime.host, runtime.port))
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
            .post(format!("http://{}:{}/v1/slurm/submit", runtime.host, runtime.port))
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
            .post(format!("http://{}:{}/v1/files/upload", runtime.host, runtime.port))
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
            .post(format!("http://{}:{}/v1/files/download", runtime.host, runtime.port))
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
            .post(format!("http://{}:{}/v1/slurm/status_gpu", runtime.host, runtime.port))
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
            .post(format!("http://{}:{}/v1/slurm/find_gpu", runtime.host, runtime.port))
            .json(request),
        runtime,
        "failed to decode find gpu response",
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
    if data.available_nodes.is_empty() && data.busy_nodes.is_empty() && data.drain_nodes.is_empty() {
        println!("No matching GPU nodes found");
        println!();
        print_gpu_summary(0, 0, 0);
        return;
    }

    if !data.available_nodes.is_empty() {
        println!("[AVAILABLE] Nodes with idle GPU ({})", data.available_nodes.len());
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
        "{:<20} {:<12} {:<15} {:<15} {}",
        "NODE", "PARTITION", "GPU IDLE/TOTAL", "CPU IDLE/TOTAL", "GPU TYPE"
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
    println!("  kind: {}", format!("{:?}", connection.kind).to_lowercase());
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
}
