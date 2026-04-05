use std::{
    env,
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use axum::{
    extract::State,
    http::{header::AUTHORIZATION, HeaderMap, StatusCode},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use clap::{Parser, Subcommand};
use slurm_proto::{ErrorBody, ErrorCode, ErrorResponse, PingData, RuntimeFile, ServerStatusData, SuccessResponse};

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
}
