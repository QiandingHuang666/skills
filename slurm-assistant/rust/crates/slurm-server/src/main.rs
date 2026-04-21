use std::{
    collections::BTreeMap,
    env, fs,
    io::Read,
    path::{Path, PathBuf},
    process::Command as ProcessCommand,
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use axum::{
    Json, Router,
    extract::{Path as AxumPath, State},
    http::{HeaderMap, StatusCode, header::AUTHORIZATION},
    response::IntoResponse,
    routing::{get, post},
};
use clap::{Parser, Subcommand};
use rusqlite::{Connection, params};
use slurm_proto::{
    ConnectionAddData, ConnectionAddRequest, ConnectionDeleteData, ConnectionKind,
    ConnectionListData, ConnectionRecord, ErrorBody, ErrorCode, ErrorResponse, ExecRunData,
    ExecRunRequest, FileDownloadRequest, FileTransferData, FileUploadRequest, PingData,
    RuntimeFile, ServerStatusData, SlurmCancelData, SlurmCancelRequest, SlurmFindGpuData,
    SlurmFindGpuRequest, SlurmGpuNode, SlurmGpuSummary, SlurmJob, SlurmJobsData,
    SlurmJobsRequest, SlurmLogData, SlurmLogRequest, SlurmStatusGpuData, SlurmStatusGpuRequest,
    SlurmSubmitData, SlurmSubmitRequest, SuccessResponse, SessionConnectionSummary,
    SessionDeleteData, SessionListData, SessionNodeRole, SessionRecord, SessionState,
    SessionSummaryData, SessionUpsertData, SessionUpsertRequest,
};
use wait_timeout::ChildExt;

const SERVER_API_VERSION: u32 = 1;

#[derive(Debug, Parser)]
#[command(
    name = "slurm-server",
    about = "Rust server scaffold for slurm-assistant"
)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedGpuNode {
    node: String,
    partition: String,
    gpu_idle: u32,
    gpu_total: u32,
    gpu_type: String,
    cpu_idle: u32,
    cpu_total: u32,
    is_drain: bool,
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
    let addr = listener
        .local_addr()
        .context("failed to read local address")?;

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
    refresh_resource_node_health(&paths.db_path)?;
    write_runtime_file(&paths.runtime_path, &runtime)?;

    spawn_resource_node_health_task(paths.db_path.clone());

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
            api_version: SERVER_API_VERSION,
            capabilities: server_capabilities(),
        },
        db_path: paths.db_path.clone(),
    };

    let app = app_router(state);
    axum::serve(listener, app)
        .await
        .context("server exited unexpectedly")
}

fn spawn_resource_node_health_task(db_path: PathBuf) {
    tokio::spawn(async move {
        loop {
            let path = db_path.clone();
            let _ = tokio::task::spawn_blocking(move || refresh_resource_node_health(&path)).await;
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        }
    });
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
        api_version: SERVER_API_VERSION,
        capabilities: server_capabilities(),
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&SuccessResponse::new(status))?
    );
    Ok(())
}

fn server_capabilities() -> Vec<String> {
    vec![
        "connections".to_string(),
        "exec".to_string(),
        "files".to_string(),
        "server".to_string(),
        "sessions".to_string(),
        "slurm".to_string(),
    ]
}

fn app_router(state: ServerState) -> Router {
    Router::new()
        .route("/v1/ping", get(handle_ping))
        .route("/v1/server/status", get(handle_status))
        .route("/v1/connections/list", get(handle_connections_list))
        .route("/v1/connections/add", post(handle_connections_add))
        .route(
            "/v1/connections/{id}",
            get(handle_connections_get).delete(handle_connections_delete),
        )
        .route("/v1/sessions/list", get(handle_sessions_list))
        .route("/v1/sessions/summary", get(handle_sessions_summary))
        .route("/v1/sessions/upsert", post(handle_sessions_upsert))
        .route("/v1/sessions/{id}", get(handle_sessions_get).delete(handle_sessions_delete))
        .route("/v1/exec/run", post(handle_exec_run))
        .route("/v1/slurm/status_gpu", post(handle_slurm_status_gpu))
        .route("/v1/slurm/find_gpu", post(handle_slurm_find_gpu))
        .route("/v1/slurm/jobs", post(handle_slurm_jobs))
        .route("/v1/slurm/cancel", post(handle_slurm_cancel))
        .route("/v1/slurm/log", post(handle_slurm_log))
        .route("/v1/slurm/submit", post(handle_slurm_submit))
        .route("/v1/files/upload", post(handle_files_upload))
        .route("/v1/files/download", post(handle_files_download))
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

async fn handle_connections_get(
    State(state): State<ServerState>,
    headers: HeaderMap,
    AxumPath(connection_id): AxumPath<String>,
) -> impl IntoResponse {
    if !is_authorized(&headers, &state.token) {
        return unauthorized_response();
    }
    match get_connection_from_db(&state.db_path, &connection_id) {
        Ok(data) => (StatusCode::OK, Json(SuccessResponse::new(data))).into_response(),
        Err(err) => internal_error_response(err),
    }
}

async fn handle_connections_delete(
    State(state): State<ServerState>,
    headers: HeaderMap,
    AxumPath(connection_id): AxumPath<String>,
) -> impl IntoResponse {
    if !is_authorized(&headers, &state.token) {
        return unauthorized_response();
    }
    match delete_connection_from_db(&state.db_path, &connection_id) {
        Ok(data) => (StatusCode::OK, Json(SuccessResponse::new(data))).into_response(),
        Err(err) => internal_error_response(err),
    }
}

async fn handle_sessions_list(
    State(state): State<ServerState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if !is_authorized(&headers, &state.token) {
        return unauthorized_response();
    }
    match list_sessions_from_db(&state.db_path) {
        Ok(sessions) => (
            StatusCode::OK,
            Json(SuccessResponse::new(SessionListData { sessions })),
        )
            .into_response(),
        Err(err) => internal_error_response(err),
    }
}

async fn handle_sessions_summary(
    State(state): State<ServerState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if !is_authorized(&headers, &state.token) {
        return unauthorized_response();
    }
    match summarize_active_sessions(&state.db_path) {
        Ok(data) => (StatusCode::OK, Json(SuccessResponse::new(data))).into_response(),
        Err(err) => internal_error_response(err),
    }
}

async fn handle_sessions_get(
    State(state): State<ServerState>,
    headers: HeaderMap,
    AxumPath(session_id): AxumPath<String>,
) -> impl IntoResponse {
    if !is_authorized(&headers, &state.token) {
        return unauthorized_response();
    }
    match get_session_from_db(&state.db_path, &session_id) {
        Ok(data) => (StatusCode::OK, Json(SuccessResponse::new(data))).into_response(),
        Err(err) => internal_error_response(err),
    }
}

async fn handle_sessions_upsert(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Json(request): Json<SessionUpsertRequest>,
) -> impl IntoResponse {
    if !is_authorized(&headers, &state.token) {
        return unauthorized_response();
    }
    match upsert_session_to_db(&state.db_path, &request) {
        Ok(data) => (StatusCode::OK, Json(SuccessResponse::new(data))).into_response(),
        Err(err) => internal_error_response(err),
    }
}

async fn handle_sessions_delete(
    State(state): State<ServerState>,
    headers: HeaderMap,
    AxumPath(session_id): AxumPath<String>,
) -> impl IntoResponse {
    if !is_authorized(&headers, &state.token) {
        return unauthorized_response();
    }
    match delete_session_from_db(&state.db_path, &session_id) {
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

async fn handle_slurm_status_gpu(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Json(request): Json<SlurmStatusGpuRequest>,
) -> impl IntoResponse {
    if !is_authorized(&headers, &state.token) {
        return unauthorized_response();
    }
    match query_slurm_status_gpu(&state.db_path, &request) {
        Ok(data) => (StatusCode::OK, Json(SuccessResponse::new(data))).into_response(),
        Err(err) => internal_error_response(err),
    }
}

async fn handle_slurm_find_gpu(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Json(request): Json<SlurmFindGpuRequest>,
) -> impl IntoResponse {
    if !is_authorized(&headers, &state.token) {
        return unauthorized_response();
    }
    match query_slurm_find_gpu(&state.db_path, &request) {
        Ok(data) => (StatusCode::OK, Json(SuccessResponse::new(data))).into_response(),
        Err(err) => internal_error_response(err),
    }
}

async fn handle_slurm_log(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Json(request): Json<SlurmLogRequest>,
) -> impl IntoResponse {
    if !is_authorized(&headers, &state.token) {
        return unauthorized_response();
    }
    match query_slurm_log(&state.db_path, &request) {
        Ok(data) => (StatusCode::OK, Json(SuccessResponse::new(data))).into_response(),
        Err(err) => internal_error_response(err),
    }
}

async fn handle_slurm_cancel(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Json(request): Json<SlurmCancelRequest>,
) -> impl IntoResponse {
    if !is_authorized(&headers, &state.token) {
        return unauthorized_response();
    }
    match query_slurm_cancel(&state.db_path, &request) {
        Ok(data) => (StatusCode::OK, Json(SuccessResponse::new(data))).into_response(),
        Err(err) => internal_error_response(err),
    }
}

async fn handle_slurm_submit(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Json(request): Json<SlurmSubmitRequest>,
) -> impl IntoResponse {
    if !is_authorized(&headers, &state.token) {
        return unauthorized_response();
    }
    match query_slurm_submit(&state.db_path, &request) {
        Ok(data) => (StatusCode::OK, Json(SuccessResponse::new(data))).into_response(),
        Err(err) => internal_error_response(err),
    }
}

async fn handle_files_upload(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Json(request): Json<FileUploadRequest>,
) -> impl IntoResponse {
    if !is_authorized(&headers, &state.token) {
        return unauthorized_response();
    }
    match handle_upload(&state.db_path, &request) {
        Ok(data) => (StatusCode::OK, Json(SuccessResponse::new(data))).into_response(),
        Err(err) => internal_error_response(err),
    }
}

async fn handle_files_download(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Json(request): Json<FileDownloadRequest>,
) -> impl IntoResponse {
    if !is_authorized(&headers, &state.token) {
        return unauthorized_response();
    }
    match handle_download(&state.db_path, &request) {
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
        PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("slurm-assistant")
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
    let bytes = fs::read(path)
        .with_context(|| format!("failed to read runtime file {}", path.display()))?;
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
          default_keepalive_secs INTEGER,
          health_state TEXT,
          health_message TEXT,
          last_health_checked_at TEXT,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS sessions (
          id TEXT PRIMARY KEY,
          connection_id TEXT NOT NULL,
          session_type TEXT NOT NULL,
          description TEXT,
          state TEXT NOT NULL,
          node_role TEXT NOT NULL,
          remote_host TEXT,
          compute_node TEXT,
          keepalive_secs INTEGER,
          created_at TEXT NOT NULL,
          last_seen_at TEXT NOT NULL,
          FOREIGN KEY(connection_id) REFERENCES connections(id)
        );
        "#,
    )
    .context("failed to initialize schema")?;
    ensure_column_exists(&conn, "connections", "default_keepalive_secs", "INTEGER")
        .context("failed to migrate default_keepalive_secs column")?;
    ensure_column_exists(&conn, "connections", "health_state", "TEXT")
        .context("failed to migrate health_state column")?;
    ensure_column_exists(&conn, "connections", "health_message", "TEXT")
        .context("failed to migrate health_message column")?;
    ensure_column_exists(&conn, "connections", "last_health_checked_at", "TEXT")
        .context("failed to migrate last_health_checked_at column")?;
    Ok(())
}

fn open_db(path: &Path) -> Result<Connection> {
    let conn =
        Connection::open(path).with_context(|| format!("failed to open db {}", path.display()))?;
    conn.pragma_update(None, "journal_mode", "WAL")
        .context("failed to enable WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")
        .context("failed to set synchronous")?;
    conn.busy_timeout(std::time::Duration::from_millis(5000))
        .context("failed to set busy timeout")?;
    Ok(conn)
}

fn ensure_column_exists(conn: &Connection, table: &str, column: &str, sql_type: &str) -> Result<()> {
    let mut stmt = conn
        .prepare(&format!("PRAGMA table_info({table})"))
        .with_context(|| format!("failed to inspect table {table}"))?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let name: String = row.get(1)?;
        if name == column {
            return Ok(());
        }
    }
    conn.execute(
        &format!("ALTER TABLE {table} ADD COLUMN {column} {sql_type}"),
        [],
    )
    .with_context(|| format!("failed to add {column} on {table}"))?;
    Ok(())
}

fn add_connection_to_db(path: &Path, request: &ConnectionAddRequest) -> Result<ConnectionAddData> {
    let conn = open_db(path)?;
    let now = now_iso_like();
    let connection_id = connection_id_for_label(&request.label);
    let keepalive = normalize_keepalive_secs(request.default_keepalive_secs);
    if matches!(request.kind, ConnectionKind::ResourceNode) {
        if request.host.as_deref().unwrap_or("").is_empty() {
            return Err(anyhow::anyhow!(
                "resource_node connection requires --host"
            ));
        }
        if request.username.as_deref().unwrap_or("").is_empty() {
            return Err(anyhow::anyhow!(
                "resource_node connection requires --user"
            ));
        }
        if request.jump_host.as_deref().unwrap_or("").is_empty() {
            return Err(anyhow::anyhow!(
                "resource_node connection requires --jump-host"
            ));
        }
    }
    let updated = conn
        .execute(
            r#"
            INSERT INTO connections (id, label, host, port, username, kind, jump_host, default_keepalive_secs, health_state, health_message, last_health_checked_at, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL, NULL, NULL, ?9, ?9)
            ON CONFLICT(label) DO UPDATE SET
              host=excluded.host,
              port=excluded.port,
              username=excluded.username,
              kind=excluded.kind,
              jump_host=excluded.jump_host,
              default_keepalive_secs=excluded.default_keepalive_secs,
              health_state=NULL,
              health_message=NULL,
              last_health_checked_at=NULL,
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
                keepalive.map(|v| v as i64),
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
            SELECT id, label, host, port, username, kind, jump_host, default_keepalive_secs, health_state, health_message, last_health_checked_at
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
                default_keepalive_secs: row.get::<_, Option<i64>>(7)?.map(|v| v as u64),
                health_state: row.get(8)?,
                health_message: row.get(9)?,
                last_health_checked_at: row.get(10)?,
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
            SELECT id, label, host, port, username, kind, jump_host, default_keepalive_secs, health_state, health_message, last_health_checked_at
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
            default_keepalive_secs: row.get::<_, Option<i64>>(7)?.map(|v| v as u64),
            health_state: row.get(8)?,
            health_message: row.get(9)?,
            last_health_checked_at: row.get(10)?,
        })
    });
    match record {
        Ok(conn) => Ok(conn),
        Err(rusqlite::Error::QueryReturnedNoRows) => {
            Err(anyhow::anyhow!("connection not found: {connection_id}"))
        }
        Err(err) => Err(err).context("failed to decode connection record"),
    }
}

fn delete_connection_from_db(path: &Path, connection_id: &str) -> Result<ConnectionDeleteData> {
    let conn = open_db(path)?;
    let deleted = conn
        .execute("DELETE FROM connections WHERE id = ?1", [connection_id])
        .with_context(|| format!("failed to delete connection {connection_id}"))?;
    Ok(ConnectionDeleteData {
        deleted: deleted > 0,
    })
}

fn upsert_session_to_db(path: &Path, request: &SessionUpsertRequest) -> Result<SessionUpsertData> {
    if request.id.trim().is_empty() {
        return Err(anyhow::anyhow!("session id must not be empty"));
    }
    if request.session_type.trim().is_empty() {
        return Err(anyhow::anyhow!("session type must not be empty"));
    }
    get_connection_from_db(path, &request.connection_id)?;

    let conn = open_db(path)?;
    let now = now_iso_like();
    let description = normalize_description(request.description.clone())?;
    let keepalive = normalize_keepalive_secs(request.keepalive_secs);
    let updated = conn
        .execute(
            r#"
            INSERT INTO sessions (
              id, connection_id, session_type, description, state, node_role, remote_host, compute_node, keepalive_secs, created_at, last_seen_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)
            ON CONFLICT(id) DO UPDATE SET
              connection_id=excluded.connection_id,
              session_type=excluded.session_type,
              description=excluded.description,
              state=excluded.state,
              node_role=excluded.node_role,
              remote_host=excluded.remote_host,
              compute_node=excluded.compute_node,
              keepalive_secs=excluded.keepalive_secs,
              last_seen_at=excluded.last_seen_at
            "#,
            params![
                request.id,
                request.connection_id,
                request.session_type,
                description,
                session_state_as_str(&request.state),
                session_node_role_as_str(&request.node_role),
                request.remote_host,
                request.compute_node,
                keepalive.map(|v| v as i64),
                now,
            ],
        )
        .context("failed to insert or update session")?;

    Ok(SessionUpsertData {
        session_id: request.id.clone(),
        created: updated > 0,
    })
}

fn list_sessions_from_db(path: &Path) -> Result<Vec<SessionRecord>> {
    let conn = open_db(path)?;
    let mut stmt = conn
        .prepare(
            r#"
            SELECT id, connection_id, session_type, description, state, node_role, remote_host, compute_node, keepalive_secs, created_at, last_seen_at
            FROM sessions
            ORDER BY last_seen_at DESC, id ASC
            "#,
        )
        .context("failed to prepare sessions list query")?;
    let rows = stmt
        .query_map([], |row| {
            Ok(SessionRecord {
                id: row.get(0)?,
                connection_id: row.get(1)?,
                session_type: row.get(2)?,
                description: row.get(3)?,
                state: parse_session_state(&row.get::<_, String>(4)?),
                node_role: parse_session_node_role(&row.get::<_, String>(5)?),
                remote_host: row.get(6)?,
                compute_node: row.get(7)?,
                keepalive_secs: row.get::<_, Option<i64>>(8)?.map(|v| v as u64),
                created_at: row.get(9)?,
                last_seen_at: row.get(10)?,
            })
        })
        .context("failed to query sessions")?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.context("failed to decode session row")?);
    }
    Ok(out)
}

fn get_session_from_db(path: &Path, session_id: &str) -> Result<SessionRecord> {
    let conn = open_db(path)?;
    let mut stmt = conn
        .prepare(
            r#"
            SELECT id, connection_id, session_type, description, state, node_role, remote_host, compute_node, keepalive_secs, created_at, last_seen_at
            FROM sessions
            WHERE id = ?1
            "#,
        )
        .context("failed to prepare session lookup query")?;
    let record = stmt.query_row([session_id], |row| {
        Ok(SessionRecord {
            id: row.get(0)?,
            connection_id: row.get(1)?,
            session_type: row.get(2)?,
            description: row.get(3)?,
            state: parse_session_state(&row.get::<_, String>(4)?),
            node_role: parse_session_node_role(&row.get::<_, String>(5)?),
            remote_host: row.get(6)?,
            compute_node: row.get(7)?,
            keepalive_secs: row.get::<_, Option<i64>>(8)?.map(|v| v as u64),
            created_at: row.get(9)?,
            last_seen_at: row.get(10)?,
        })
    });
    match record {
        Ok(session) => Ok(session),
        Err(rusqlite::Error::QueryReturnedNoRows) => {
            Err(anyhow::anyhow!("session not found: {session_id}"))
        }
        Err(err) => Err(err).context("failed to decode session record"),
    }
}

fn delete_session_from_db(path: &Path, session_id: &str) -> Result<SessionDeleteData> {
    let conn = open_db(path)?;
    let deleted = conn
        .execute("DELETE FROM sessions WHERE id = ?1", [session_id])
        .with_context(|| format!("failed to delete session {session_id}"))?;
    Ok(SessionDeleteData {
        deleted: deleted > 0,
    })
}

fn summarize_active_sessions(path: &Path) -> Result<SessionSummaryData> {
    let sessions = list_sessions_from_db(path)?;
    let mut map: BTreeMap<String, SessionConnectionSummary> = BTreeMap::new();
    let mut total_active = 0_u32;
    for session in sessions
        .into_iter()
        .filter(|s| matches!(s.state, SessionState::Active))
    {
        total_active += 1;
        let entry = map
            .entry(session.connection_id.clone())
            .or_insert(SessionConnectionSummary {
                connection_id: session.connection_id.clone(),
                active_count: 0,
                current_session_id: None,
                current_description: None,
                current_node_role: None,
                current_compute_node: None,
                current_keepalive_secs: None,
                last_seen_at: None,
            });
        entry.active_count += 1;
        if entry
            .last_seen_at
            .as_ref()
            .map(|value| value < &session.last_seen_at)
            .unwrap_or(true)
        {
            entry.current_session_id = Some(session.id.clone());
            entry.current_description = session.description.clone();
            entry.current_node_role = Some(session.node_role.clone());
            entry.current_compute_node = session.compute_node.clone();
            entry.current_keepalive_secs = session.keepalive_secs;
            entry.last_seen_at = Some(session.last_seen_at);
        }
    }

    Ok(SessionSummaryData {
        total_active,
        connections: map.into_values().collect(),
    })
}

fn refresh_resource_node_health(path: &Path) -> Result<()> {
    let conn = open_db(path)?;
    let mut stmt = conn.prepare(
        r#"
        SELECT id, host, port, username, jump_host
        FROM connections
        WHERE kind = 'resource_node'
        "#,
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, Option<String>>(1)?,
            row.get::<_, Option<i64>>(2)?.map(|v| v as u16),
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<String>>(4)?,
        ))
    })?;

    for row in rows {
        let (connection_id, host, port, username, jump_host) = row?;
        let checked_at = now_iso_like();
        let (state, message) = check_resource_node_state(
            host.as_deref(),
            port,
            username.as_deref(),
            jump_host.as_deref(),
        );
        conn.execute(
            r#"
            UPDATE connections
            SET health_state = ?2,
                health_message = ?3,
                last_health_checked_at = ?4
            WHERE id = ?1
            "#,
            params![connection_id, state, message, checked_at],
        )?;
    }
    Ok(())
}

fn check_resource_node_state(
    host: Option<&str>,
    port: Option<u16>,
    username: Option<&str>,
    jump_host: Option<&str>,
) -> (String, String) {
    let (Some(host), Some(username), Some(jump_host)) = (host, username, jump_host) else {
        return (
            "invalid".to_string(),
            "resource_node missing host/user/jump_host".to_string(),
        );
    };

    let mut connect_args = vec![
        "-o".to_string(),
        "StrictHostKeyChecking=accept-new".to_string(),
        "-J".to_string(),
        jump_host.to_string(),
    ];
    if let Some(port) = port {
        connect_args.push("-p".to_string());
        connect_args.push(port.to_string());
    }
    connect_args.push(format!("{username}@{host}"));
    connect_args.push("hostname".to_string());

    match run_process("ssh".to_string(), &connect_args, 20) {
        Ok(output) if output.exit_code == 0 => {}
        Ok(output) => {
            return (
                "offline".to_string(),
                format!("ssh failed: {}", output.stderr.trim()),
            );
        }
        Err(err) => {
            return ("offline".to_string(), format!("ssh error: {err}"));
        }
    }

    let check_command = format!(
        "squeue -h -u {username} -o '%N' | xargs -r -n1 scontrol show hostnames | grep -Fx '{host}' >/dev/null"
    );
    let check_args = vec![
        "-o".to_string(),
        "StrictHostKeyChecking=accept-new".to_string(),
        jump_host.to_string(),
        check_command,
    ];
    match run_process("ssh".to_string(), &check_args, 20) {
        Ok(output) if output.exit_code == 0 => (
            "online".to_string(),
            "resource node reachable and active in user queue".to_string(),
        ),
        Ok(_) => (
            "released".to_string(),
            "node reachable but not present in current user jobs".to_string(),
        ),
        Err(err) => (
            "unknown".to_string(),
            format!("failed to check queue state: {err}"),
        ),
    }
}

fn execute_command(path: &Path, request: &ExecRunRequest) -> Result<ExecRunData> {
    let connection = get_connection_from_db(path, &request.connection_id)?;
    let (program, args) = build_exec_program(path, &connection, &request.command)?;
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

    let (program, args) = build_exec_program(path, &connection, &command)?;
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

fn query_slurm_status_gpu(
    path: &Path,
    request: &SlurmStatusGpuRequest,
) -> Result<SlurmStatusGpuData> {
    let connection = get_connection_from_db(path, &request.connection_id)?;
    let nodes = query_scontrol_gpu_nodes(path, &connection)?;

    let matches_partition = |node: &ParsedGpuNode| {
        request
            .partition
            .as_ref()
            .map(|partition| matches_token(&node.partition, partition))
            .unwrap_or(true)
    };

    let mut available_nodes = Vec::new();
    let mut drain_nodes = Vec::new();
    for node in nodes.into_iter().filter(matches_partition) {
        if node.is_drain {
            drain_nodes.push(gpu_node_public(&node));
        } else {
            available_nodes.push(gpu_node_public(&node));
        }
    }

    Ok(SlurmStatusGpuData {
        summary: gpu_summary(&available_nodes),
        available_nodes,
        drain_nodes,
    })
}

fn query_slurm_find_gpu(path: &Path, request: &SlurmFindGpuRequest) -> Result<SlurmFindGpuData> {
    let connection = get_connection_from_db(path, &request.connection_id)?;
    let nodes = query_scontrol_gpu_nodes(path, &connection)?;

    let matches_gpu_type = |node: &ParsedGpuNode| {
        request
            .gpu_type
            .as_ref()
            .map(|gpu_type| node.gpu_type.eq_ignore_ascii_case(gpu_type))
            .unwrap_or(true)
    };

    let mut available_nodes = Vec::new();
    let mut busy_nodes = Vec::new();
    let mut drain_nodes = Vec::new();
    for node in nodes.into_iter().filter(matches_gpu_type) {
        let public = gpu_node_public(&node);
        if node.is_drain {
            drain_nodes.push(public);
        } else if node.gpu_idle > 0 {
            available_nodes.push(public);
        } else {
            busy_nodes.push(public);
        }
    }

    let mut summary_nodes = available_nodes.clone();
    summary_nodes.extend(busy_nodes.clone());
    Ok(SlurmFindGpuData {
        summary: gpu_summary(&summary_nodes),
        available_nodes,
        busy_nodes,
        drain_nodes,
    })
}

fn query_slurm_log(path: &Path, request: &SlurmLogRequest) -> Result<SlurmLogData> {
    let connection = get_connection_from_db(path, &request.connection_id)?;
    let command = build_slurm_log_command(&request.job_id)?;
    let (program, args) = build_exec_program(path, &connection, &command)?;
    let output = run_process(program, &args, 30)?;
    if output.exit_code != 0 {
        return Err(anyhow::anyhow!(
            "log query failed with exit code {}: {}",
            output.exit_code,
            output.stderr.trim()
        ));
    }

    let content = output.stdout;
    let found = content != "Log file not found";
    Ok(SlurmLogData {
        job_id: request.job_id.clone(),
        found,
        content,
    })
}

fn query_slurm_cancel(path: &Path, request: &SlurmCancelRequest) -> Result<SlurmCancelData> {
    let connection = get_connection_from_db(path, &request.connection_id)?;
    let command = build_scancel_command(&request.job_ids)?;
    let (program, args) = build_exec_program(path, &connection, &command)?;
    let output = run_process(program, &args, 30)?;
    if output.exit_code != 0 {
        return Err(anyhow::anyhow!(
            "scancel failed with exit code {}: {}",
            output.exit_code,
            output.stderr.trim()
        ));
    }
    Ok(SlurmCancelData {
        cancelled: request.job_ids.clone(),
    })
}

fn query_slurm_submit(path: &Path, request: &SlurmSubmitRequest) -> Result<SlurmSubmitData> {
    let connection = get_connection_from_db(path, &request.connection_id)?;
    let command = build_sbatch_command(&request.script_path)?;
    let (program, args) = build_exec_program(path, &connection, &command)?;
    let output = run_process(program, &args, 30)?;
    if output.exit_code != 0 {
        return Err(anyhow::anyhow!(
            "sbatch failed with exit code {}: {}",
            output.exit_code,
            output.stderr.trim()
        ));
    }
    let raw_output = output.stdout.trim().to_string();
    let job_id = parse_submitted_job_id(&raw_output)?;
    Ok(SlurmSubmitData { job_id, raw_output })
}

fn handle_upload(path: &Path, request: &FileUploadRequest) -> Result<FileTransferData> {
    let connection = get_connection_from_db(path, &request.connection_id)?;
    transfer_path(
        path,
        &connection,
        &request.local_path,
        &request.remote_path,
        request.recursive,
        false,
    )?;
    Ok(FileTransferData {
        source_path: request.local_path.clone(),
        destination_path: request.remote_path.clone(),
        recursive: request.recursive,
    })
}

fn handle_download(path: &Path, request: &FileDownloadRequest) -> Result<FileTransferData> {
    let connection = get_connection_from_db(path, &request.connection_id)?;
    transfer_path(
        path,
        &connection,
        &request.remote_path,
        &request.local_path,
        request.recursive,
        true,
    )?;
    Ok(FileTransferData {
        source_path: request.remote_path.clone(),
        destination_path: request.local_path.clone(),
        recursive: request.recursive,
    })
}

fn build_exec_program(
    db_path: &Path,
    connection: &ConnectionRecord,
    command: &str,
) -> Result<(String, Vec<String>)> {
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
        ConnectionKind::Cluster
        | ConnectionKind::Instance
        | ConnectionKind::Server
        | ConnectionKind::ResourceNode => {
            let host = connection
                .host
                .clone()
                .ok_or_else(|| anyhow::anyhow!("remote connection missing host"))?;
            let username = connection
                .username
                .clone()
                .ok_or_else(|| anyhow::anyhow!("remote connection missing username"))?;
            let mut args = vec![
                "-o".to_string(),
                "StrictHostKeyChecking=accept-new".to_string(),
            ];
            if let Some(port) = connection.port {
                args.push("-p".to_string());
                args.push(port.to_string());
            }
            if let Some(jump_host) = &connection.jump_host {
                args.push("-J".to_string());
                args.push(resolve_jump_host_value(db_path, jump_host)?);
            }
            args.push(format!("{username}@{host}"));
            args.push(command.to_string());
            Ok(("ssh".to_string(), args))
        }
    }
}

fn build_slurm_log_command(job_id: &str) -> Result<String> {
    if job_id.is_empty()
        || !job_id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
    {
        return Err(anyhow::anyhow!("invalid job id for log query: {job_id}"));
    }
    let log_file = format!("slurm-{job_id}.out");
    Ok(format!(
        "test -f {log_file} && cat {log_file} || printf 'Log file not found'"
    ))
}

fn build_scancel_command(job_ids: &[String]) -> Result<String> {
    if job_ids.is_empty() {
        return Err(anyhow::anyhow!("must provide at least one job id"));
    }
    for job_id in job_ids {
        if job_id.is_empty()
            || !job_id
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
        {
            return Err(anyhow::anyhow!("invalid job id for cancel query: {job_id}"));
        }
    }
    Ok(format!("scancel {}", job_ids.join(" ")))
}

fn build_sbatch_command(script_path: &str) -> Result<String> {
    if script_path.trim().is_empty() {
        return Err(anyhow::anyhow!("script path must not be empty"));
    }
    if script_path.contains('\'') {
        return Err(anyhow::anyhow!(
            "script path must not contain single quotes"
        ));
    }
    let rendered_path = if script_path == "~" {
        "$HOME".to_string()
    } else if let Some(rest) = script_path.strip_prefix("~/") {
        format!("$HOME/'{rest}'")
    } else {
        format!("'{script_path}'")
    };
    Ok(format!("sbatch {rendered_path}"))
}

fn parse_submitted_job_id(raw_output: &str) -> Result<String> {
    let captures = regex::Regex::new(r"Submitted batch job (\d+)")
        .context("failed to compile submit regex")?
        .captures(raw_output)
        .ok_or_else(|| {
            anyhow::anyhow!("failed to parse submitted job id from output: {raw_output}")
        })?;
    Ok(captures
        .get(1)
        .ok_or_else(|| anyhow::anyhow!("submit output missing job id capture"))?
        .as_str()
        .to_string())
}

fn transfer_path(
    db_path: &Path,
    connection: &ConnectionRecord,
    src: &str,
    dst: &str,
    recursive: bool,
    download: bool,
) -> Result<()> {
    match connection.kind {
        ConnectionKind::Local => local_transfer(src, dst, recursive),
        ConnectionKind::Cluster
        | ConnectionKind::Instance
        | ConnectionKind::Server
        | ConnectionKind::ResourceNode => {
            let (program, args) =
                build_scp_program(db_path, connection, src, dst, recursive, download)?;
            let output = run_process(program, &args, 300)?;
            if output.exit_code != 0 {
                return Err(anyhow::anyhow!(
                    "scp failed with exit code {}: {}",
                    output.exit_code,
                    output.stderr.trim()
                ));
            }
            Ok(())
        }
    }
}

fn build_scp_program(
    db_path: &Path,
    connection: &ConnectionRecord,
    src: &str,
    dst: &str,
    recursive: bool,
    download: bool,
) -> Result<(String, Vec<String>)> {
    let host = connection
        .host
        .clone()
        .ok_or_else(|| anyhow::anyhow!("remote connection missing host"))?;
    let username = connection
        .username
        .clone()
        .ok_or_else(|| anyhow::anyhow!("remote connection missing username"))?;

    let mut args = vec![
        "-o".to_string(),
        "StrictHostKeyChecking=accept-new".to_string(),
    ];
    if let Some(port) = connection.port {
        args.push("-P".to_string());
        args.push(port.to_string());
    }
    if let Some(jump_host) = &connection.jump_host {
        args.push("-J".to_string());
        args.push(resolve_jump_host_value(db_path, jump_host)?);
    }
    if recursive {
        args.push("-r".to_string());
    }

    let remote_prefix = format!("{username}@{host}:");
    let source = if download {
        format!("{remote_prefix}{src}")
    } else {
        src.to_string()
    };
    let destination = if download {
        dst.to_string()
    } else {
        format!("{remote_prefix}{dst}")
    };
    args.push(source);
    args.push(destination);
    Ok(("scp".to_string(), args))
}

fn local_transfer(src: &str, dst: &str, recursive: bool) -> Result<()> {
    let src_path = PathBuf::from(src);
    let dst_path = PathBuf::from(dst);
    if src_path.is_dir() {
        if !recursive {
            return Err(anyhow::anyhow!(
                "source is a directory; use recursive transfer for {}",
                src_path.display()
            ));
        }
        copy_dir_recursive(&src_path, &dst_path)
    } else {
        copy_file_like_cp(&src_path, &dst_path)
    }
}

fn copy_file_like_cp(src: &Path, dst: &Path) -> Result<()> {
    let target = if dst.exists() && dst.is_dir() {
        dst.join(
            src.file_name()
                .ok_or_else(|| anyhow::anyhow!("source missing file name: {}", src.display()))?,
        )
    } else {
        dst.to_path_buf()
    };
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create parent dir {}", parent.display()))?;
    }
    fs::copy(src, &target)
        .with_context(|| format!("failed to copy {} -> {}", src.display(), target.display()))?;
    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)
        .with_context(|| format!("failed to create destination dir {}", dst.display()))?;
    for entry in
        fs::read_dir(src).with_context(|| format!("failed to read dir {}", src.display()))?
    {
        let entry = entry.with_context(|| format!("failed to read entry in {}", src.display()))?;
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to inspect {}", entry.path().display()))?;
        let target = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_recursive(&entry.path(), &target)?;
        } else {
            copy_file_like_cp(&entry.path(), &target)?;
        }
    }
    Ok(())
}

fn local_username() -> Option<String> {
    env::var("USER")
        .ok()
        .filter(|value| !value.is_empty())
        .or_else(|| env::var("USERNAME").ok().filter(|value| !value.is_empty()))
}

fn query_scontrol_gpu_nodes(path: &Path, connection: &ConnectionRecord) -> Result<Vec<ParsedGpuNode>> {
    let (program, args) = build_exec_program(path, connection, "scontrol show node")?;
    let output = run_process(program, &args, 30)?;
    if output.exit_code != 0 {
        return Err(anyhow::anyhow!(
            "scontrol show node failed with exit code {}: {}",
            output.exit_code,
            output.stderr.trim()
        ));
    }
    parse_scontrol_gpu_nodes_output(&output.stdout)
}

fn resolve_jump_host_value(db_path: &Path, raw_jump_host: &str) -> Result<String> {
    let trimmed = raw_jump_host.trim();
    if trimmed.is_empty() {
        return Err(anyhow::anyhow!("jump host must not be empty"));
    }
    if trimmed.contains('@') || trimmed.contains(':') {
        return Ok(trimmed.to_string());
    }

    if let Ok(conn) = get_connection_from_db(db_path, trimmed) {
        let host = conn.host.ok_or_else(|| {
            anyhow::anyhow!("jump host connection `{trimmed}` missing host")
        })?;
        let user = conn.username.ok_or_else(|| {
            anyhow::anyhow!("jump host connection `{trimmed}` missing username")
        })?;
        if let Some(port) = conn.port {
            return Ok(format!("{user}@{host}:{port}"));
        }
        return Ok(format!("{user}@{host}"));
    }

    Ok(trimmed.to_string())
}

fn matches_token(haystack: &str, needle: &str) -> bool {
    haystack
        .split(',')
        .map(|token| token.trim())
        .any(|token| token.eq_ignore_ascii_case(needle))
}

fn parse_gpu_gres(gres: &str) -> (u32, String) {
    let lower = gres.to_ascii_lowercase();
    let typed = regex_extract(&lower, r"gpu:([a-zA-Z_]\w*):(\d+)");
    if let Some((gpu_type, count)) = typed
        && let Ok(count) = count.parse::<u32>()
    {
        return (count, gpu_type.to_string());
    }
    let simple = regex_extract_single(&lower, r"gpu:(\d+)");
    if let Some(count) = simple.and_then(|value| value.parse::<u32>().ok()) {
        return (count, "unknown".to_string());
    }
    (0, String::new())
}

fn regex_extract<'a>(value: &'a str, pattern: &'a str) -> Option<(&'a str, &'a str)> {
    let captures = regex::Regex::new(pattern).ok()?.captures(value)?;
    Some((captures.get(1)?.as_str(), captures.get(2)?.as_str()))
}

fn regex_extract_single<'a>(value: &'a str, pattern: &'a str) -> Option<&'a str> {
    let captures = regex::Regex::new(pattern).ok()?.captures(value)?;
    Some(captures.get(1)?.as_str())
}

fn parse_scontrol_gpu_nodes_output(stdout: &str) -> Result<Vec<ParsedGpuNode>> {
    let mut nodes = Vec::new();
    let mut current_node: Option<String> = None;
    let mut current_gres: Option<String> = None;
    let mut current_alloc_tres: Option<String> = None;
    let mut current_partition: Option<String> = None;
    let mut current_cpu_alloc: Option<u32> = None;
    let mut current_cpu_total: Option<u32> = None;
    let mut current_state: Option<String> = None;

    let flush_current = |nodes: &mut Vec<ParsedGpuNode>,
                         current_node: &mut Option<String>,
                         current_gres: &mut Option<String>,
                         current_alloc_tres: &mut Option<String>,
                         current_partition: &mut Option<String>,
                         current_cpu_alloc: &mut Option<u32>,
                         current_cpu_total: &mut Option<u32>,
                         current_state: &mut Option<String>| {
        let Some(node_name) = current_node.take() else {
            return;
        };
        let Some(gres) = current_gres.take() else {
            return;
        };
        if !gres.to_ascii_lowercase().contains("gpu") {
            return;
        }

        let (gpu_total, gpu_type) = parse_gpu_gres(&gres);
        if gpu_total == 0 {
            return;
        }

        let gpu_alloc = current_alloc_tres
            .as_ref()
            .and_then(|value| regex_extract_single(value, r"gres/gpu=(\d+)"))
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(0);
        let cpu_total = current_cpu_total.unwrap_or(0);
        let cpu_alloc = current_cpu_alloc.unwrap_or(0);
        let cpu_idle = cpu_total.saturating_sub(cpu_alloc);
        let state = current_state
            .take()
            .unwrap_or_else(|| "UNKNOWN".to_string());
        nodes.push(ParsedGpuNode {
            node: node_name,
            partition: current_partition
                .take()
                .unwrap_or_else(|| "unknown".to_string()),
            gpu_idle: gpu_total.saturating_sub(gpu_alloc),
            gpu_total,
            gpu_type: if gpu_type.is_empty() {
                "GPU".to_string()
            } else {
                gpu_type.to_ascii_uppercase()
            },
            cpu_idle,
            cpu_total,
            is_drain: state.to_ascii_uppercase().contains("DRAIN"),
        });
        current_alloc_tres.take();
        current_cpu_alloc.take();
        current_cpu_total.take();
    };

    for raw_line in stdout.lines() {
        let line = raw_line.trim();
        if let Some(rest) = line.strip_prefix("NodeName=") {
            flush_current(
                &mut nodes,
                &mut current_node,
                &mut current_gres,
                &mut current_alloc_tres,
                &mut current_partition,
                &mut current_cpu_alloc,
                &mut current_cpu_total,
                &mut current_state,
            );
            let node_name = rest.split_whitespace().next().unwrap_or_default();
            if !node_name.is_empty() {
                current_node = Some(node_name.to_string());
            }
            continue;
        }
        if let Some(state) = line.strip_prefix("State=") {
            current_state = Some(state.to_string());
            continue;
        }
        if let Some(gres) = line.strip_prefix("Gres=") {
            current_gres = Some(gres.to_string());
            continue;
        }
        if let Some(partition) = line.strip_prefix("Partitions=") {
            current_partition = Some(partition.to_string());
            continue;
        }
        if let Some(alloc_tres) = line.strip_prefix("AllocTRES=") {
            current_alloc_tres = Some(alloc_tres.to_string());
            continue;
        }
        if line.contains("CPUAlloc=") {
            current_cpu_alloc = regex_extract_single(line, r"CPUAlloc=(\d+)")
                .and_then(|value| value.parse::<u32>().ok());
        }
        if line.contains("CPUTot=") {
            current_cpu_total = regex_extract_single(line, r"CPUTot=(\d+)")
                .and_then(|value| value.parse::<u32>().ok());
        }
    }

    flush_current(
        &mut nodes,
        &mut current_node,
        &mut current_gres,
        &mut current_alloc_tres,
        &mut current_partition,
        &mut current_cpu_alloc,
        &mut current_cpu_total,
        &mut current_state,
    );
    Ok(nodes)
}

fn gpu_node_public(node: &ParsedGpuNode) -> SlurmGpuNode {
    SlurmGpuNode {
        node: node.node.clone(),
        partition: node.partition.clone(),
        gpu_idle: node.gpu_idle,
        gpu_total: node.gpu_total,
        gpu_type: node.gpu_type.clone(),
        cpu_idle: node.cpu_idle,
        cpu_total: node.cpu_total,
    }
}

fn gpu_summary(nodes: &[SlurmGpuNode]) -> SlurmGpuSummary {
    SlurmGpuSummary {
        available_nodes: nodes.len() as u32,
        total_gpu: nodes.iter().map(|node| node.gpu_total).sum(),
        idle_gpu: nodes.iter().map(|node| node.gpu_idle).sum(),
    }
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

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow::anyhow!("failed to capture stdout"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| anyhow::anyhow!("failed to capture stderr"))?;
    let stdout_handle = thread::spawn(move || -> Vec<u8> {
        let mut reader = stdout;
        let mut buf = Vec::new();
        let _ = reader.read_to_end(&mut buf);
        buf
    });
    let stderr_handle = thread::spawn(move || -> Vec<u8> {
        let mut reader = stderr;
        let mut buf = Vec::new();
        let _ = reader.read_to_end(&mut buf);
        buf
    });

    let timeout = std::time::Duration::from_secs(timeout_secs.max(1));
    let status = child
        .wait_timeout(timeout)
        .context("failed while waiting for process")?;

    if status.is_none() {
        child.kill().ok();
        let _ = child.wait();
    }

    let stdout = stdout_handle
        .join()
        .map_err(|_| anyhow::anyhow!("stdout reader thread panicked"))?;
    let mut stderr = stderr_handle
        .join()
        .map_err(|_| anyhow::anyhow!("stderr reader thread panicked"))?;

    if status.is_none() {
        if !stderr.is_empty() {
            stderr.extend_from_slice(b"\n");
        }
        stderr.extend_from_slice(b"Command timed out");
        return Ok(ExecRunData {
            stdout: String::from_utf8_lossy(&stdout).to_string(),
            stderr: String::from_utf8_lossy(&stderr).to_string(),
            exit_code: 124,
        });
    }

    Ok(ExecRunData {
        stdout: String::from_utf8_lossy(&stdout).to_string(),
        stderr: String::from_utf8_lossy(&stderr).to_string(),
        exit_code: status.and_then(|value| value.code()).unwrap_or(-1),
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
        slurm_proto::ConnectionKind::ResourceNode => "resource_node",
    }
}

fn parse_connection_kind(value: &str) -> slurm_proto::ConnectionKind {
    match value {
        "local" => slurm_proto::ConnectionKind::Local,
        "cluster" => slurm_proto::ConnectionKind::Cluster,
        "instance" => slurm_proto::ConnectionKind::Instance,
        "server" => slurm_proto::ConnectionKind::Server,
        "resource_node" => slurm_proto::ConnectionKind::ResourceNode,
        _ => slurm_proto::ConnectionKind::Server,
    }
}

fn normalize_keepalive_secs(value: Option<u64>) -> Option<u64> {
    value.map(|secs| secs.clamp(60, 86_400))
}

fn normalize_description(value: Option<String>) -> Result<Option<String>> {
    let Some(raw) = value else {
        return Ok(None);
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed.chars().count() > 256 {
        return Err(anyhow::anyhow!(
            "session description too long (max 256 characters)"
        ));
    }
    Ok(Some(trimmed.to_string()))
}

fn session_state_as_str(state: &SessionState) -> &'static str {
    match state {
        SessionState::Active => "active",
        SessionState::Idle => "idle",
        SessionState::Closed => "closed",
    }
}

fn parse_session_state(value: &str) -> SessionState {
    match value {
        "active" => SessionState::Active,
        "idle" => SessionState::Idle,
        "closed" => SessionState::Closed,
        _ => SessionState::Idle,
    }
}

fn session_node_role_as_str(role: &SessionNodeRole) -> &'static str {
    match role {
        SessionNodeRole::Login => "login",
        SessionNodeRole::Compute => "compute",
        SessionNodeRole::Unknown => "unknown",
    }
}

fn parse_session_node_role(value: &str) -> SessionNodeRole {
    match value {
        "login" => SessionNodeRole::Login,
        "compute" => SessionNodeRole::Compute,
        "unknown" => SessionNodeRole::Unknown,
        _ => SessionNodeRole::Unknown,
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
    use axum::body::to_bytes;
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
            api_version: SERVER_API_VERSION,
            capabilities: server_capabilities(),
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
                api_version: SERVER_API_VERSION,
                capabilities: server_capabilities(),
            },
            db_path: PathBuf::from("state.db"),
        };
        let app = app_router(state);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/v1/ping")
                    .body(Body::empty())
                    .unwrap(),
            )
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
                default_keepalive_secs: None,
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
                default_keepalive_secs: None,
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
            default_keepalive_secs: None,
            health_state: None,
            health_message: None,
            last_health_checked_at: None,
        };
        let (program, args) =
            build_exec_program(Path::new("state.db"), &connection, "hostname").unwrap();
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
                vec!["/C".to_string(), "ping -n 3 127.0.0.1 >NUL".to_string()],
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

    #[test]
    fn parse_gpu_gres_supports_typed_and_simple_forms() {
        assert_eq!(parse_gpu_gres("gpu:a100:4"), (4, "a100".to_string()));
        assert_eq!(parse_gpu_gres("gpu:a40:2(S:0)"), (2, "a40".to_string()));
        assert_eq!(parse_gpu_gres("gpu:8"), (8, "unknown".to_string()));
        assert_eq!(parse_gpu_gres("cpu"), (0, String::new()));
    }

    #[test]
    fn parse_scontrol_gpu_nodes_output_classifies_available_and_drain() {
        let output = "\
NodeName=gpu-a10-3 Arch=x86_64\n\
   State=IDLE\n\
   Gres=gpu:a10:2\n\
   Partitions=gpu-a10\n\
   CPUAlloc=16 CPUTot=32\n\
   AllocTRES=cpu=16,gres/gpu=1\n\
NodeName=gpu-a40-9 Arch=x86_64\n\
   State=DRAIN\n\
   Gres=gpu:a40:4\n\
   Partitions=gpu-a40\n\
   CPUAlloc=0 CPUTot=64\n\
   AllocTRES=\n";
        let nodes = parse_scontrol_gpu_nodes_output(output).unwrap();
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0].gpu_idle, 1);
        assert_eq!(nodes[0].cpu_idle, 16);
        assert!(!nodes[0].is_drain);
        assert!(nodes[1].is_drain);
        assert_eq!(nodes[1].gpu_type, "A40");
    }

    #[test]
    fn matches_token_uses_exact_partition_segments() {
        assert!(matches_token("gpu-a10", "gpu-a10"));
        assert!(matches_token("gpu-a10,gpu-share", "gpu-share"));
        assert!(!matches_token("gpu-a100", "gpu-a10"));
    }

    #[test]
    fn build_slurm_log_command_rejects_invalid_job_id() {
        let err = build_slurm_log_command("57373;rm -rf /").unwrap_err();
        assert!(err.to_string().contains("invalid job id"));
    }

    #[test]
    fn log_missing_file_is_well_formed() {
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
                default_keepalive_secs: None,
            },
        )
        .unwrap();

        let result = query_slurm_log(
            &db_path,
            &SlurmLogRequest {
                connection_id: data.connection_id,
                job_id: "999999999".to_string(),
            },
        )
        .unwrap();

        assert_eq!(result.job_id, "999999999");
        assert!(!result.found);
        assert_eq!(result.content, "Log file not found");
    }

    #[test]
    fn build_scancel_command_validates_job_ids() {
        assert_eq!(
            build_scancel_command(&["60001".to_string(), "60002".to_string()]).unwrap(),
            "scancel 60001 60002"
        );
        let err = build_scancel_command(&["60001;rm".to_string()]).unwrap_err();
        assert!(err.to_string().contains("invalid job id"));
        let err = build_scancel_command(&[]).unwrap_err();
        assert!(err.to_string().contains("at least one job id"));
    }

    #[test]
    fn build_sbatch_command_validates_script_path() {
        assert_eq!(build_sbatch_command("job.sh").unwrap(), "sbatch 'job.sh'");
        assert_eq!(
            build_sbatch_command("~/job.sh").unwrap(),
            "sbatch $HOME/'job.sh'"
        );
        let err = build_sbatch_command("").unwrap_err();
        assert!(err.to_string().contains("must not be empty"));
        let err = build_sbatch_command("bad'quote.sh").unwrap_err();
        assert!(err.to_string().contains("single quotes"));
    }

    #[test]
    fn parse_submitted_job_id_extracts_job_id() {
        assert_eq!(
            parse_submitted_job_id("Submitted batch job 60001").unwrap(),
            "60001"
        );
        let err = parse_submitted_job_id("submission failed").unwrap_err();
        assert!(err.to_string().contains("failed to parse submitted job id"));
    }

    #[test]
    fn build_scp_program_uses_remote_prefixes() {
        let connection = ConnectionRecord {
            id: "conn_gzu_cluster".to_string(),
            label: "gzu-cluster".to_string(),
            host: Some("210.40.56.85".to_string()),
            port: Some(21563),
            username: Some("qiandingh".to_string()),
            kind: ConnectionKind::Cluster,
            jump_host: None,
            default_keepalive_secs: None,
            health_state: None,
            health_message: None,
            last_health_checked_at: None,
        };
        let (program, args) = build_scp_program(
            Path::new("state.db"),
            &connection,
            "/tmp/train.py",
            "~/train.py",
            false,
            false,
        )
        .unwrap();
        assert_eq!(program, "scp");
        assert_eq!(
            args.last().map(String::as_str),
            Some("qiandingh@210.40.56.85:~/train.py")
        );

        let (_, download_args) = build_scp_program(
            Path::new("state.db"),
            &connection,
            "~/slurm.out",
            "/tmp/slurm.out",
            false,
            true,
        )
        .unwrap();
        assert!(
            download_args
                .iter()
                .any(|value| value == "qiandingh@210.40.56.85:~/slurm.out")
        );
    }

    #[test]
    fn resolve_jump_host_connection_id_to_target() {
        let temp = tempfile::tempdir().unwrap();
        let db_path = temp.path().join("state.db");
        init_db(&db_path).unwrap();

        add_connection_to_db(
            &db_path,
            &ConnectionAddRequest {
                label: "gzu-hpc".to_string(),
                host: Some("210.40.56.85".to_string()),
                port: Some(21563),
                username: Some("qiandingh".to_string()),
                kind: ConnectionKind::Instance,
                jump_host: None,
                default_keepalive_secs: None,
            },
        )
        .unwrap();

        let resolved = resolve_jump_host_value(&db_path, "conn_gzu_hpc").unwrap();
        assert_eq!(resolved, "qiandingh@210.40.56.85:21563");
    }

    #[test]
    fn build_exec_program_resolves_jump_host_connection_id() {
        let temp = tempfile::tempdir().unwrap();
        let db_path = temp.path().join("state.db");
        init_db(&db_path).unwrap();

        add_connection_to_db(
            &db_path,
            &ConnectionAddRequest {
                label: "gzu-hpc".to_string(),
                host: Some("210.40.56.85".to_string()),
                port: Some(21563),
                username: Some("qiandingh".to_string()),
                kind: ConnectionKind::Instance,
                jump_host: None,
                default_keepalive_secs: None,
            },
        )
        .unwrap();

        let node_conn = ConnectionRecord {
            id: "conn_gpu_a100_8card_1".to_string(),
            label: "gpu-a100-8card-1".to_string(),
            host: Some("gpu-a100-8card-1".to_string()),
            port: Some(22),
            username: Some("qiandingh".to_string()),
            kind: ConnectionKind::ResourceNode,
            jump_host: Some("conn_gzu_hpc".to_string()),
            default_keepalive_secs: None,
            health_state: None,
            health_message: None,
            last_health_checked_at: None,
        };
        let (_program, args) = build_exec_program(&db_path, &node_conn, "hostname").unwrap();
        assert!(
            args.iter()
                .any(|arg| arg == "qiandingh@210.40.56.85:21563")
        );
    }

    #[test]
    fn local_transfer_roundtrip_file() {
        let temp = tempfile::tempdir().unwrap();
        let src = temp.path().join("src.txt");
        let dst_dir = temp.path().join("dst");
        fs::create_dir_all(&dst_dir).unwrap();
        fs::write(&src, "hello").unwrap();

        local_transfer(src.to_str().unwrap(), dst_dir.to_str().unwrap(), false).unwrap();

        let copied = dst_dir.join("src.txt");
        assert_eq!(fs::read_to_string(copied).unwrap(), "hello");
    }

    #[test]
    fn connection_get_and_delete_roundtrip() {
        let temp = tempfile::tempdir().unwrap();
        let db_path = temp.path().join("state.db");
        init_db(&db_path).unwrap();
        let created = add_connection_to_db(
            &db_path,
            &ConnectionAddRequest {
                label: "gzu-cluster".to_string(),
                host: Some("210.40.56.85".to_string()),
                port: Some(21563),
                username: Some("qiandingh".to_string()),
                kind: ConnectionKind::Cluster,
                jump_host: None,
                default_keepalive_secs: None,
            },
        )
        .unwrap();

        let record = get_connection_from_db(&db_path, &created.connection_id).unwrap();
        assert_eq!(record.label, "gzu-cluster");

        let deleted = delete_connection_from_db(&db_path, &created.connection_id).unwrap();
        assert!(deleted.deleted);
        let missing = get_connection_from_db(&db_path, &created.connection_id).unwrap_err();
        assert!(missing.to_string().contains("connection not found"));
    }

    #[test]
    fn connection_default_keepalive_is_persisted() {
        let temp = tempfile::tempdir().unwrap();
        let db_path = temp.path().join("state.db");
        init_db(&db_path).unwrap();
        let created = add_connection_to_db(
            &db_path,
            &ConnectionAddRequest {
                label: "gzu-cluster".to_string(),
                host: Some("210.40.56.85".to_string()),
                port: Some(21563),
                username: Some("qiandingh".to_string()),
                kind: ConnectionKind::Cluster,
                jump_host: None,
                default_keepalive_secs: Some(1800),
            },
        )
        .unwrap();
        let record = get_connection_from_db(&db_path, &created.connection_id).unwrap();
        assert_eq!(record.default_keepalive_secs, Some(1800));
    }

    #[test]
    fn sessions_crud_and_summary_roundtrip() {
        let temp = tempfile::tempdir().unwrap();
        let db_path = temp.path().join("state.db");
        init_db(&db_path).unwrap();
        let created = add_connection_to_db(
            &db_path,
            &ConnectionAddRequest {
                label: "gzu-cluster".to_string(),
                host: Some("210.40.56.85".to_string()),
                port: Some(21563),
                username: Some("qiandingh".to_string()),
                kind: ConnectionKind::Cluster,
                jump_host: None,
                default_keepalive_secs: Some(1800),
            },
        )
        .unwrap();

        upsert_session_to_db(
            &db_path,
            &SessionUpsertRequest {
                id: "sess_a".to_string(),
                connection_id: created.connection_id.clone(),
                session_type: "alloc".to_string(),
                description: Some("interactive a10".to_string()),
                state: SessionState::Active,
                node_role: SessionNodeRole::Compute,
                remote_host: Some("210.40.56.85".to_string()),
                compute_node: Some("gpu-a10-01".to_string()),
                keepalive_secs: Some(1800),
            },
        )
        .unwrap();

        let list = list_sessions_from_db(&db_path).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].description.as_deref(), Some("interactive a10"));

        let summary = summarize_active_sessions(&db_path).unwrap();
        assert_eq!(summary.total_active, 1);
        assert_eq!(summary.connections.len(), 1);
        assert_eq!(
            summary.connections[0].current_compute_node.as_deref(),
            Some("gpu-a10-01")
        );

        let deleted = delete_session_from_db(&db_path, "sess_a").unwrap();
        assert!(deleted.deleted);
        assert!(list_sessions_from_db(&db_path).unwrap().is_empty());
    }

    #[test]
    fn init_db_migrates_default_keepalive_column() {
        let temp = tempfile::tempdir().unwrap();
        let db_path = temp.path().join("state.db");
        let conn = Connection::open(&db_path).unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE connections (
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
        .unwrap();
        drop(conn);

        init_db(&db_path).unwrap();
        let conn = open_db(&db_path).unwrap();
        let exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('connections') WHERE name='default_keepalive_secs'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(exists, 1);
    }

    #[test]
    fn resource_node_requires_jump_host() {
        let temp = tempfile::tempdir().unwrap();
        let db_path = temp.path().join("state.db");
        init_db(&db_path).unwrap();
        let err = add_connection_to_db(
            &db_path,
            &ConnectionAddRequest {
                label: "gpu-a100-temp".to_string(),
                host: Some("gpu-a100-01".to_string()),
                port: Some(22),
                username: Some("qiandingh".to_string()),
                kind: ConnectionKind::ResourceNode,
                jump_host: None,
                default_keepalive_secs: Some(1200),
            },
        )
        .unwrap_err();
        assert!(err.to_string().contains("requires --jump-host"));
    }

    #[test]
    fn resource_node_health_check_rejects_incomplete_record() {
        let (state, message) = check_resource_node_state(None, None, None, None);
        assert_eq!(state, "invalid");
        assert!(message.contains("missing"));
    }

    #[tokio::test]
    async fn sessions_api_roundtrip() {
        let temp = tempfile::tempdir().unwrap();
        let db_path = temp.path().join("state.db");
        init_db(&db_path).unwrap();
        let conn = add_connection_to_db(
            &db_path,
            &ConnectionAddRequest {
                label: "gzu-cluster".to_string(),
                host: Some("210.40.56.85".to_string()),
                port: Some(21563),
                username: Some("qiandingh".to_string()),
                kind: ConnectionKind::Cluster,
                jump_host: None,
                default_keepalive_secs: Some(1800),
            },
        )
        .unwrap();

        let state = ServerState {
            token: "token".to_string(),
            status: ServerStatusData {
                pid: 1,
                started_at: "123Z".to_string(),
                transport: "tcp".to_string(),
                host: "127.0.0.1".to_string(),
                port: 1,
                db_path: db_path.display().to_string(),
                runtime_path: "runtime.json".to_string(),
                api_version: SERVER_API_VERSION,
                capabilities: server_capabilities(),
            },
            db_path: db_path.clone(),
        };
        let app = app_router(state);

        let upsert = SessionUpsertRequest {
            id: "sess_api_1".to_string(),
            connection_id: conn.connection_id,
            session_type: "ssh".to_string(),
            description: Some("api test".to_string()),
            state: SessionState::Active,
            node_role: SessionNodeRole::Login,
            remote_host: Some("210.40.56.85".to_string()),
            compute_node: None,
            keepalive_secs: Some(900),
        };
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/sessions/upsert")
                    .header(AUTHORIZATION, "Bearer token")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&upsert).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/v1/sessions/summary")
                    .header(AUTHORIZATION, "Bearer token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: SuccessResponse<SessionSummaryData> = serde_json::from_slice(&body).unwrap();
        assert_eq!(payload.data.total_active, 1);
        assert_eq!(payload.data.connections.len(), 1);
    }
}
