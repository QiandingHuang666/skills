use std::{
    env, fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
use reqwest::blocking::Client;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use slurm_proto::{RuntimeFile, ServerStatusData, SuccessResponse};

#[derive(Debug, Parser)]
#[command(name = "slurm-client", about = "Rust client scaffold for slurm-assistant")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
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

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
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
    let client = Client::new();
    let response = client
        .get(format!("http://{}:{}/v1/server/status", runtime.host, runtime.port))
        .header(AUTHORIZATION, format!("Bearer {}", runtime.token))
        .header(CONTENT_TYPE, "application/json")
        .send()
        .context("failed to contact local server")?
        .error_for_status()
        .context("server returned non-success status")?;
    let payload = response
        .json::<SuccessResponse<ServerStatusData>>()
        .context("failed to decode server status response")?;
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
}
