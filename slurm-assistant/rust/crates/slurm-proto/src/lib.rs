use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    Unauthorized,
    ServerUnavailable,
    InvalidRequest,
    ConnectionNotFound,
    SessionNotFound,
    ExecFailed,
    SshFailed,
    Timeout,
    NotSupported,
    InternalError,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ErrorBody {
    pub code: ErrorCode,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ErrorResponse {
    pub ok: bool,
    pub error: ErrorBody,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SuccessResponse<T> {
    pub ok: bool,
    pub data: T,
}

impl<T> SuccessResponse<T> {
    pub fn new(data: T) -> Self {
        Self { ok: true, data }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PingData {
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServerStatusData {
    pub pid: u32,
    pub started_at: String,
    pub transport: String,
    pub host: String,
    pub port: u16,
    pub db_path: String,
    pub runtime_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeFile {
    pub version: u32,
    pub transport: String,
    pub host: String,
    pub port: u16,
    pub token: String,
    pub pid: u32,
    pub started_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionKind {
    Local,
    Cluster,
    Instance,
    Server,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConnectionRecord {
    pub id: String,
    pub label: String,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub username: Option<String>,
    pub kind: ConnectionKind,
    pub jump_host: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConnectionAddRequest {
    pub label: String,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub username: Option<String>,
    pub kind: ConnectionKind,
    pub jump_host: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConnectionAddData {
    pub connection_id: String,
    pub created: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConnectionListData {
    pub connections: Vec<ConnectionRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExecRunRequest {
    pub connection_id: String,
    pub command: String,
    pub timeout_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExecRunData {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SlurmJob {
    pub job_id: String,
    pub partition: String,
    pub name: String,
    pub user: String,
    pub state: String,
    pub time: String,
    pub nodes: u32,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SlurmJobsData {
    pub jobs: Vec<SlurmJob>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ping_response_roundtrip() {
        let value = SuccessResponse::new(PingData {
            message: "pong".to_string(),
        });
        let json = serde_json::to_string(&value).unwrap();
        let parsed: SuccessResponse<PingData> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, value);
    }

    #[test]
    fn connection_add_request_roundtrip() {
        let value = ConnectionAddRequest {
            label: "gzu-cluster".to_string(),
            host: Some("210.40.56.85".to_string()),
            port: Some(21563),
            username: Some("qiandingh".to_string()),
            kind: ConnectionKind::Cluster,
            jump_host: None,
        };
        let json = serde_json::to_string(&value).unwrap();
        let parsed: ConnectionAddRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, value);
    }

    #[test]
    fn exec_run_response_roundtrip() {
        let value = SuccessResponse::new(ExecRunData {
            stdout: "workstation\n".to_string(),
            stderr: String::new(),
            exit_code: 0,
        });
        let json = serde_json::to_string(&value).unwrap();
        let parsed: SuccessResponse<ExecRunData> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, value);
    }

    #[test]
    fn slurm_jobs_response_roundtrip() {
        let value = SuccessResponse::new(SlurmJobsData {
            jobs: vec![SlurmJob {
                job_id: "57373".to_string(),
                partition: "gpu-a10".to_string(),
                name: "interactive".to_string(),
                user: "qiandingh".to_string(),
                state: "R".to_string(),
                time: "17:08:48".to_string(),
                nodes: 1,
                reason: "gpu-a10-13".to_string(),
            }],
        });
        let json = serde_json::to_string(&value).unwrap();
        let parsed: SuccessResponse<SlurmJobsData> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, value);
    }

    #[test]
    fn error_response_roundtrip() {
        let value = ErrorResponse {
            ok: false,
            error: ErrorBody {
                code: ErrorCode::ConnectionNotFound,
                message: "Connection not found: conn_xxx".to_string(),
            },
        };
        let json = serde_json::to_string(&value).unwrap();
        let parsed: ErrorResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, value);
    }

    #[test]
    fn runtime_file_roundtrip() {
        let value = RuntimeFile {
            version: 1,
            transport: "tcp".to_string(),
            host: "127.0.0.1".to_string(),
            port: 47831,
            token: "token".to_string(),
            pid: 12345,
            started_at: "2026-04-05T12:34:56Z".to_string(),
        };
        let json = serde_json::to_string(&value).unwrap();
        let parsed: RuntimeFile = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, value);
    }
}
