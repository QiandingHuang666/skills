use std::{
    env,
    fs,
    path::{Path, PathBuf},
    process::Command as ProcessCommand,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use axum::{
    extract::State,
    http::{header::AUTHORIZATION, HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use clap::{Parser, Subcommand};
use rusqlite::{params, Connection};
use slurm_proto::{
    ConnectionAddData, ConnectionAddRequest, ConnectionKind, ConnectionListData, ConnectionRecord,
    ErrorBody, ErrorCode, ErrorResponse, ExecRunData, ExecRunRequest, PingData, RuntimeFile,
    ServerStatusData, SlurmJob, SlurmJobsData, SlurmJobsRequest, SuccessResponse,
};
use wait_timeout::ChildExt;

#[derive(Debug, Parser)]
#[command(name = "slurm-server", about = "Rust server scaffold for slurm-assistant")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Serve {
        #[arg(long, default_value_t = 0)]
        port: u16,
    },
    Status,
}

#[derive(Debug, Clone)]
struct Paths {
    db_path: PathBuf,
    runtime_path: PathBuf,
}

#[derive(Debug, Clone)]
struct ServerState {
    token: String,
    status: ServerStatusData,
    db_path: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Serve { port } => serve(port).await,
        Command::Status => print_local_status(),
    }
}

async fn serve(requested_port: u16) -> Result<()> {
    let paths = resolve_paths()?;
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", requested_port))
        .await
        .with_context(|| format!("failed to bind localhost:{}", requested_port))?;
    let addr = listener.local_addr().context("failed to read local address")?;

    let runtime = RuntimeFile {
        version: 1,
        transport: "tcp".to_string(),
        host: "127.0.0.1".to_string(),
        port: addr.port(),
        token: generate_token(),
        pid: std::process::id(),
        started_at: now_iso_like(),
    };
    init_db(&paths.db_path)?;
    write_runtime_file(&paths.runtime_path, &runtime)?;

    let state = ServerState {
        token: runtime.token.clone(),
        status: ServerStatusData {
            pid: runtime.pid,
            started_at: runtime.started_at.clone(),
            transport: runtime.transport.clone(),
            host: runtime.host.clone(),
            port: runtime.port,
            db_path: paths.db_path.display().to_string(),
            runtime_path: paths.runtime_path.display().to_string(),
        },
        db_path: paths.db_path.clone(),
    };

    let app = app_router(state);
    axum::serve(listener, app)
        .await
        .context("server exited unexpectedly")
}

fn print_local_status() -> Result<()> {
    let paths = resolve_paths()?;
    let runtime = read_runtime_file(&paths.runtime_path)?;
    let status = ServerStatusData {
        pid: runtime.pid,
        started_at: runtime.started_at,
        transport: runtime.transport,
        host: runtime.host,
        port: runtime.port,
        db_path: paths.db_path.display().to_string(),
        runtime_path: paths.runtime_path.display().to_string(),
    };
    println!("{}", serde_json::to_string_pretty(&SuccessResponse::new(status))?);
    Ok(())
}

fn app_router(state: ServerState) -> Router {
    Router::new()
        .route("/v1/ping", get(handle_ping))
        .route("/v1/server/status", get(handle_status))
        .route("/v1/connections/list", get(handle_connections_list))
        .route("/v1/connections/add", post(handle_connections_add))
        .route("/v1/exec/run", post(handle_exec_run))
        .route("/v1/slurm/jobs", post(handle_slurm_jobs))
        .with_state(state)
}

async fn handle_ping(State(state): State<ServerState>, headers: HeaderMap) -> impl IntoResponse {
    if !is_authorized(&headers, &state.token) {
        return unauthorized_response();
    }
    (
        StatusCode::OK,
        Json(SuccessResponse::new(PingData {
            message: "pong".to_string(),
        })),
    )
        .into_response()
}

async fn handle_status(State(state): State<ServerState>, headers: HeaderMap) -> impl IntoResponse {
    if !is_authorized(&headers, &state.token) {
        return unauthorized_response();
    }
    (StatusCode::OK, Json(SuccessResponse::new(state.status))).into_response()
}

async fn handle_connections_list(
    State(state): State<ServerState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if !is_authorized(&headers, &state.token) {
        return unauthorized_response();
    }
    match list_connections_from_db(&state.db_path) {
        Ok(connections) => (
            StatusCode::OK,
            Json(SuccessResponse::new(ConnectionListData { connections })),
        )
            .into_response(),
        Err(err) => internal_error_response(err),
    }
}

async fn handle_connections_add(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Json(request): Json<ConnectionAddRequest>,
) -> impl IntoResponse {
    if !is_authorized(&headers, &state.token) {
        return unauthorized_response();
    }
    match add_connection_to_db(&state.db_path, &request) {
        Ok(data) => (StatusCode::OK, Json(SuccessResponse::new(data))).into_response(),
        Err(err) => internal_error_response(err),
    }
}

async fn handle_exec_run(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Json(request): Json<ExecRunRequest>,
) -> impl IntoResponse {
    if !is_authorized(&headers, &state.token) {
        return unauthorized_response();
    }
    match execute_command(&state.db_path, &request) {
        Ok(data) => (StatusCode::OK, Json(SuccessResponse::new(data))).into_response(),
        Err(err) => internal_error_response(err),
    }
}

async fn handle_slurm_jobs(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Json(request): Json<SlurmJobsRequest>,
) -> impl IntoResponse {
    if !is_authorized(&headers, &state.token) {
        return unauthorized_response();
    }
    match query_slurm_jobs(&state.db_path, &request) {
        Ok(data) => (StatusCode::OK, Json(SuccessResponse::new(data))).into_response(),
        Err(err) => internal_error_response(err),
    }
}

fn is_authorized(headers: &HeaderMap, token: &str) -> bool {
    let Some(value) = headers.get(AUTHORIZATION) else {
        return false;
    };
    let Ok(value) = value.to_str() else {
        return false;
    };
    value == format!("Bearer {}", token)
}

fn unauthorized_response() -> axum::response::Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(ErrorResponse {
            ok: false,
            error: ErrorBody {
                code: ErrorCode::Unauthorized,
                message: "Invalid or missing token".to_string(),
            },
        }),
    )
        .into_response()
}

fn internal_error_response(err: anyhow::Error) -> axum::response::Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse {
            ok: false,
            error: ErrorBody {
                code: ErrorCode::InternalError,
                message: err.to_string(),
            },
        }),
    )
        .into_response()
}

fn resolve_paths() -> Result<Paths> {
    let data_dir = if let Ok(override_dir) = env::var("SLURM_ASSISTANT_DATA_DIR") {
        PathBuf::from(override_dir)
    } else if cfg!(windows) {
        let base = env::var("APPDATA").context("APPDATA not set")?;
        PathBuf::from(base).join("slurm-assistant")
    } else if let Ok(xdg_state) = env::var("XDG_STATE_HOME") {
        PathBuf::from(xdg_state).join("slurm-assistant")
    } else {
        let home = env::var("HOME").context("HOME not set")?;
        PathBuf::from(home).join(".local").join("share").join("slurm-assistant")
    };

    Ok(Paths {
        db_path: data_dir.join("state.db"),
        runtime_path: data_dir.join("runtime.json"),
    })
}

fn write_runtime_file(path: &Path, runtime: &RuntimeFile) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create runtime dir {}", parent.display()))?;
    }
    fs::write(path, serde_json::to_vec_pretty(runtime)?)
        .with_context(|| format!("failed to write runtime file {}", path.display()))?;
    Ok(())
}

fn read_runtime_file(path: &Path) -> Result<RuntimeFile> {
    let bytes = fs::read(path).with_context(|| format!("failed to read runtime file {}", path.display()))?;
    let runtime = serde_json::from_slice::<RuntimeFile>(&bytes)
        .with_context(|| format!("failed to parse runtime file {}", path.display()))?;
    Ok(runtime)
}

fn init_db(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create db dir {}", parent.display()))?;
    }
    let conn = open_db(path)?;
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS connections (
          id TEXT PRIMARY KEY,
          label TEXT NOT NULL UNIQUE,
          host TEXT,
          port INTEGER,
          username TEXT,
          kind TEXT NOT NULL,
          jump_host TEXT,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL
        );
        "#,
    )
    .context("failed to initialize schema")?;
    Ok(())
}

fn open_db(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path).with_context(|| format!("failed to open db {}", path.display()))?;
    conn.pragma_update(None, "journal_mode", "WAL")
        .context("failed to enable WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")
        .context("failed to set synchronous")?;
    conn.busy_timeout(std::time::Duration::from_millis(5000))
        .context("failed to set busy timeout")?;
    Ok(conn)
}

fn add_connection_to_db(path: &Path, request: &ConnectionAddRequest) -> Result<ConnectionAddData> {
    let conn = open_db(path)?;
    let now = now_iso_like();
    let connection_id = connection_id_for_label(&request.label);
    let updated = conn
        .execute(
            r#"
            INSERT INTO connections (id, label, host, port, username, kind, jump_host, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)
            ON CONFLICT(label) DO UPDATE SET
              host=excluded.host,
              port=excluded.port,
              username=excluded.username,
              kind=excluded.kind,
              jump_host=excluded.jump_host,
              updated_at=excluded.updated_at
            "#,
            params![
                connection_id,
                request.label,
                request.host,
                request.port.map(|p| p as i64),
                request.username,
                connection_kind_as_str(&request.kind),
                request.jump_host,
                now,
            ],
        )
        .context("failed to insert or update connection")?;

    Ok(ConnectionAddData {
        connection_id,
        created: updated > 0,
    })
}

fn list_connections_from_db(path: &Path) -> Result<Vec<ConnectionRecord>> {
    let conn = open_db(path)?;
    let mut stmt = conn
        .prepare(
            r#"
            SELECT id, label, host, port, username, kind, jump_host
            FROM connections
            ORDER BY label
            "#,
        )
        .context("failed to prepare list query")?;
    let rows = stmt
        .query_map([], |row| {
            Ok(ConnectionRecord {
                id: row.get(0)?,
                label: row.get(1)?,
                host: row.get(2)?,
                port: row.get::<_, Option<i64>>(3)?.map(|v| v as u16),
                username: row.get(4)?,
                kind: parse_connection_kind(&row.get::<_, String>(5)?),
                jump_host: row.get(6)?,
            })
        })
        .context("failed to query connections")?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row.context("failed to decode connection row")?);
    }
    Ok(out)
}

fn get_connection_from_db(path: &Path, connection_id: &str) -> Result<ConnectionRecord> {
    let conn = open_db(path)?;
    let mut stmt = conn
        .prepare(
            r#"
            SELECT id, label, host, port, username, kind, jump_host
            FROM connections
            WHERE id = ?1
            "#,
        )
        .context("failed to prepare connection lookup query")?;
    let record = stmt.query_row([connection_id], |row| {
        Ok(ConnectionRecord {
            id: row.get(0)?,
            label: row.get(1)?,
            host: row.get(2)?,
            port: row.get::<_, Option<i64>>(3)?.map(|v| v as u16),
            username: row.get(4)?,
            kind: parse_connection_kind(&row.get::<_, String>(5)?),
            jump_host: row.get(6)?,
        })
    });
    match record {
        Ok(conn) => Ok(conn),
        Err(rusqlite::Error::QueryReturnedNoRows) => Err(anyhow::anyhow!("connection not found: {connection_id}")),
        Err(err) => Err(err).context("failed to decode connection record"),
    }
}

fn execute_command(path: &Path, request: &ExecRunRequest) -> Result<ExecRunData> {
    let connection = get_connection_from_db(path, &request.connection_id)?;
    let (program, args) = build_exec_program(&connection, &request.command)?;
    run_process(program, &args, request.timeout_secs)
}

fn query_slurm_jobs(path: &Path, request: &SlurmJobsRequest) -> Result<SlurmJobsData> {
    let connection = get_connection_from_db(path, &request.connection_id)?;
    let username = connection
        .username
        .clone()
        .or_else(local_username)
        .ok_or_else(|| anyhow::anyhow!("missing username for slurm jobs query"))?;

    let command = if let Some(job_id) = &request.job_id {
        format!("squeue -j {job_id} -h -o '%i|%P|%j|%u|%T|%M|%D|%R'")
    } else {
        format!("squeue -u {username} -h -o '%i|%P|%j|%u|%T|%M|%D|%R'")
    };

    let (program, args) = build_exec_program(&connection, &command)?;
    let output = run_process(program, &args, 30)?;
    if output.exit_code != 0 {
        return Err(anyhow::anyhow!(
            "squeue failed with exit code {}: {}",
            output.exit_code,
            output.stderr.trim()
        ));
    }

    Ok(SlurmJobsData {
        jobs: parse_squeue_jobs_output(&output.stdout)?,
    })
}

fn build_exec_program(connection: &ConnectionRecord, command: &str) -> Result<(String, Vec<String>)> {
    match connection.kind {
        ConnectionKind::Local => {
            if cfg!(windows) {
                Ok((
                    "cmd".to_string(),
                    vec!["/C".to_string(), command.to_string()],
                ))
            } else {
                Ok((
                    "sh".to_string(),
                    vec!["-lc".to_string(), command.to_string()],
                ))
            }
        }
        ConnectionKind::Cluster | ConnectionKind::Instance | ConnectionKind::Server => {
            let host = connection
                .host
                .clone()
                .ok_or_else(|| anyhow::anyhow!("remote connection missing host"))?;
            let username = connection
                .username
                .clone()
                .ok_or_else(|| anyhow::anyhow!("remote connection missing username"))?;
            let mut args = vec!["-o".to_string(), "StrictHostKeyChecking=accept-new".to_string()];
            if let Some(port) = connection.port {
                args.push("-p".to_string());
                args.push(port.to_string());
            }
            if let Some(jump_host) = &connection.jump_host {
                args.push("-J".to_string());
                args.push(jump_host.clone());
            }
            args.push(format!("{username}@{host}"));
            args.push(command.to_string());
            Ok(("ssh".to_string(), args))
        }
    }
}

fn local_username() -> Option<String> {
    env::var("USER")
        .ok()
        .filter(|value| !value.is_empty())
        .or_else(|| env::var("USERNAME").ok().filter(|value| !value.is_empty()))
}

fn parse_squeue_jobs_output(stdout: &str) -> Result<Vec<SlurmJob>> {
    let mut jobs = Vec::new();
    for (index, raw_line) in stdout.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.splitn(8, '|').collect();
        if parts.len() != 8 {
            return Err(anyhow::anyhow!(
                "failed to parse squeue line {}: expected 8 fields, got {}",
                index + 1,
                parts.len()
            ));
        }
        jobs.push(SlurmJob {
            job_id: parts[0].to_string(),
            partition: parts[1].to_string(),
            name: parts[2].to_string(),
            user: parts[3].to_string(),
            state: parts[4].to_string(),
            time: parts[5].to_string(),
            nodes: parts[6]
                .parse::<u32>()
                .with_context(|| format!("failed to parse node count on line {}", index + 1))?,
            reason: parts[7].to_string(),
        });
    }
    Ok(jobs)
}

fn run_process(program: String, args: &[String], timeout_secs: u64) -> Result<ExecRunData> {
    let mut child = ProcessCommand::new(&program)
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .with_context(|| format!("failed to spawn process: {} {:?}", program, args))?;

    let timeout = std::time::Duration::from_secs(timeout_secs.max(1));
    let status = child
        .wait_timeout(timeout)
        .context("failed while waiting for process")?;

    if status.is_none() {
        child.kill().ok();
        let _ = child.wait();
        return Ok(ExecRunData {
            stdout: String::new(),
            stderr: "Command timed out".to_string(),
            exit_code: 124,
        });
    }

    let output = child
        .wait_with_output()
        .context("failed to collect process output")?;
    Ok(ExecRunData {
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        exit_code: output.status.code().unwrap_or(-1),
    })
}

fn connection_id_for_label(label: &str) -> String {
    let mut out = String::with_capacity(label.len());
    let mut last_dash = false;
    for ch in label.chars() {
        let c = ch.to_ascii_lowercase();
        if c.is_ascii_alphanumeric() {
            out.push(c);
            last_dash = false;
        } else if !last_dash {
            out.push('_');
            last_dash = true;
        }
    }
    let out = out.trim_matches('_');
    if out.is_empty() {
        "conn_default".to_string()
    } else {
        format!("conn_{out}")
    }
}

fn connection_kind_as_str(kind: &slurm_proto::ConnectionKind) -> &'static str {
    match kind {
        slurm_proto::ConnectionKind::Local => "local",
        slurm_proto::ConnectionKind::Cluster => "cluster",
        slurm_proto::ConnectionKind::Instance => "instance",
        slurm_proto::ConnectionKind::Server => "server",
    }
}

fn parse_connection_kind(value: &str) -> slurm_proto::ConnectionKind {
    match value {
        "local" => slurm_proto::ConnectionKind::Local,
        "cluster" => slurm_proto::ConnectionKind::Cluster,
        "instance" => slurm_proto::ConnectionKind::Instance,
        "server" => slurm_proto::ConnectionKind::Server,
        _ => slurm_proto::ConnectionKind::Server,
    }
}

fn now_iso_like() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{secs}Z")
}

fn generate_token() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("sa-{}-{nanos}", std::process::id())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::util::ServiceExt;

    #[test]
    fn runtime_status_contains_expected_defaults() {
        let paths = resolve_paths().unwrap();
        let status = ServerStatusData {
            pid: std::process::id(),
            started_at: now_iso_like(),
            transport: "tcp".to_string(),
            host: "127.0.0.1".to_string(),
            port: 47831,
            db_path: paths.db_path.display().to_string(),
            runtime_path: paths.runtime_path.display().to_string(),
        };
        assert_eq!(status.transport, "tcp");
        assert_eq!(status.host, "127.0.0.1");
        assert!(status.port > 0);
        assert!(status.db_path.contains("state.db"));
        assert!(status.runtime_path.contains("runtime.json"));
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
        write_runtime_file(&runtime_path, &runtime).unwrap();
        let read_back = read_runtime_file(&runtime_path).unwrap();
        assert_eq!(read_back, runtime);
    }

    #[tokio::test]
    async fn ping_requires_valid_token() {
        let state = ServerState {
            token: "token".to_string(),
            status: ServerStatusData {
                pid: 1,
                started_at: "123Z".to_string(),
                transport: "tcp".to_string(),
                host: "127.0.0.1".to_string(),
                port: 1,
                db_path: "state.db".to_string(),
                runtime_path: "runtime.json".to_string(),
            },
            db_path: PathBuf::from("state.db"),
        };
        let app = app_router(state);

        let response = app
            .clone()
            .oneshot(Request::builder().uri("/v1/ping").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/v1/ping")
                    .header(AUTHORIZATION, "Bearer token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn sqlite_wal_enabled() {
        let temp = tempfile::tempdir().unwrap();
        let db_path = temp.path().join("state.db");
        init_db(&db_path).unwrap();
        let conn = open_db(&db_path).unwrap();
        let mode: String = conn
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .unwrap();
        assert_eq!(mode.to_ascii_lowercase(), "wal");
    }

    #[test]
    fn connection_insert_and_list() {
        let temp = tempfile::tempdir().unwrap();
        let db_path = temp.path().join("state.db");
        init_db(&db_path).unwrap();
        let data = add_connection_to_db(
            &db_path,
            &ConnectionAddRequest {
                label: "gzu-cluster".to_string(),
                host: Some("210.40.56.85".to_string()),
                port: Some(21563),
                username: Some("qiandingh".to_string()),
                kind: slurm_proto::ConnectionKind::Cluster,
                jump_host: None,
            },
        )
        .unwrap();
        assert_eq!(data.connection_id, "conn_gzu_cluster");

        let connections = list_connections_from_db(&db_path).unwrap();
        assert_eq!(connections.len(), 1);
        assert_eq!(connections[0].label, "gzu-cluster");
        assert_eq!(connections[0].host.as_deref(), Some("210.40.56.85"));
    }

    #[test]
    fn local_exec_returns_stdout_and_exit_code() {
        let temp = tempfile::tempdir().unwrap();
        let db_path = temp.path().join("state.db");
        init_db(&db_path).unwrap();
        let data = add_connection_to_db(
            &db_path,
            &ConnectionAddRequest {
                label: "local".to_string(),
                host: None,
                port: None,
                username: None,
                kind: ConnectionKind::Local,
                jump_host: None,
            },
        )
        .unwrap();

        let command = if cfg!(windows) {
            "echo hello"
        } else {
            "printf 'hello\\n'"
        };
        let result = execute_command(
            &db_path,
            &ExecRunRequest {
                connection_id: data.connection_id,
                command: command.to_string(),
                timeout_secs: 5,
            },
        )
        .unwrap();

        assert!(result.stdout.contains("hello"));
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn remote_exec_program_uses_ssh_target() {
        let connection = ConnectionRecord {
            id: "conn_gzu_cluster".to_string(),
            label: "gzu-cluster".to_string(),
            host: Some("210.40.56.85".to_string()),
            port: Some(21563),
            username: Some("qiandingh".to_string()),
            kind: ConnectionKind::Cluster,
            jump_host: None,
        };
        let (program, args) = build_exec_program(&connection, "hostname").unwrap();
        assert_eq!(program, "ssh");
        assert!(args.iter().any(|arg| arg == "qiandingh@210.40.56.85"));
        assert!(args.iter().any(|arg| arg == "21563"));
        assert_eq!(args.last().map(String::as_str), Some("hostname"));
    }

    #[test]
    fn exec_timeout_returns_124() {
        let (program, args) = if cfg!(windows) {
            (
                "cmd".to_string(),
                vec![
                    "/C".to_string(),
                    "ping -n 3 127.0.0.1 >NUL".to_string(),
                ],
            )
        } else {
            (
                "sh".to_string(),
                vec!["-lc".to_string(), "sleep 2".to_string()],
            )
        };

        let result = run_process(program, &args, 1).unwrap();
        assert_eq!(result.exit_code, 124);
        assert!(result.stderr.contains("timed out"));
    }

    #[test]
    fn parse_squeue_jobs_output_decodes_multiple_rows() {
        let output = "\
57373|gpu-a10|interactive|qiandingh|RUNNING|17:08:48|1|gpu-a10-13\n\
57374|cpu|train-job|qiandingh|PENDING|00:00|2|Priority\n";
        let jobs = parse_squeue_jobs_output(output).unwrap();
        assert_eq!(jobs.len(), 2);
        assert_eq!(jobs[0].job_id, "57373");
        assert_eq!(jobs[0].nodes, 1);
        assert_eq!(jobs[1].state, "PENDING");
        assert_eq!(jobs[1].reason, "Priority");
    }

    #[test]
    fn parse_squeue_jobs_output_rejects_bad_rows() {
        let err = parse_squeue_jobs_output("57373|gpu-a10|interactive").unwrap_err();
        assert!(err.to_string().contains("expected 8 fields"));
    }
}
