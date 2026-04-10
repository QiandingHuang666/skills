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
    #[serde(default = "default_server_api_version")]
    pub api_version: u32,
    #[serde(default)]
    pub capabilities: Vec<String>,
}

fn default_server_api_version() -> u32 {
    0
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
    ResourceNode,
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
    pub default_keepalive_secs: Option<u64>,
    pub health_state: Option<String>,
    pub health_message: Option<String>,
    pub last_health_checked_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConnectionAddRequest {
    pub label: String,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub username: Option<String>,
    pub kind: ConnectionKind,
    pub jump_host: Option<String>,
    pub default_keepalive_secs: Option<u64>,
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
pub struct ConnectionDeleteData {
    pub deleted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    Active,
    Idle,
    Closed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionNodeRole {
    Login,
    Compute,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionRecord {
    pub id: String,
    pub connection_id: String,
    pub session_type: String,
    pub description: Option<String>,
    pub state: SessionState,
    pub node_role: SessionNodeRole,
    pub remote_host: Option<String>,
    pub compute_node: Option<String>,
    pub keepalive_secs: Option<u64>,
    pub created_at: String,
    pub last_seen_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionUpsertRequest {
    pub id: String,
    pub connection_id: String,
    pub session_type: String,
    pub description: Option<String>,
    pub state: SessionState,
    pub node_role: SessionNodeRole,
    pub remote_host: Option<String>,
    pub compute_node: Option<String>,
    pub keepalive_secs: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionUpsertData {
    pub session_id: String,
    pub created: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionDeleteData {
    pub deleted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionListData {
    pub sessions: Vec<SessionRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionConnectionSummary {
    pub connection_id: String,
    pub active_count: u32,
    pub current_session_id: Option<String>,
    pub current_description: Option<String>,
    pub current_node_role: Option<SessionNodeRole>,
    pub current_compute_node: Option<String>,
    pub current_keepalive_secs: Option<u64>,
    pub last_seen_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionSummaryData {
    pub total_active: u32,
    pub connections: Vec<SessionConnectionSummary>,
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
pub struct SlurmJobsRequest {
    pub connection_id: String,
    pub job_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SlurmStatusGpuRequest {
    pub connection_id: String,
    pub partition: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SlurmFindGpuRequest {
    pub connection_id: String,
    pub gpu_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SlurmGpuNode {
    pub node: String,
    pub partition: String,
    pub gpu_idle: u32,
    pub gpu_total: u32,
    pub gpu_type: String,
    pub cpu_idle: u32,
    pub cpu_total: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SlurmGpuSummary {
    pub available_nodes: u32,
    pub total_gpu: u32,
    pub idle_gpu: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SlurmStatusGpuData {
    pub available_nodes: Vec<SlurmGpuNode>,
    pub drain_nodes: Vec<SlurmGpuNode>,
    pub summary: SlurmGpuSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SlurmFindGpuData {
    pub available_nodes: Vec<SlurmGpuNode>,
    pub busy_nodes: Vec<SlurmGpuNode>,
    pub drain_nodes: Vec<SlurmGpuNode>,
    pub summary: SlurmGpuSummary,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SlurmLogRequest {
    pub connection_id: String,
    pub job_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SlurmLogData {
    pub job_id: String,
    pub found: bool,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SlurmCancelRequest {
    pub connection_id: String,
    pub job_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SlurmCancelData {
    pub cancelled: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SlurmSubmitRequest {
    pub connection_id: String,
    pub script_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SlurmSubmitData {
    pub job_id: String,
    pub raw_output: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileUploadRequest {
    pub connection_id: String,
    pub local_path: String,
    pub remote_path: String,
    pub recursive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileDownloadRequest {
    pub connection_id: String,
    pub remote_path: String,
    pub local_path: String,
    pub recursive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileTransferData {
    pub source_path: String,
    pub destination_path: String,
    pub recursive: bool,
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
            default_keepalive_secs: Some(1800),
        };
        let json = serde_json::to_string(&value).unwrap();
        let parsed: ConnectionAddRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, value);
    }

    #[test]
    fn connection_delete_response_roundtrip() {
        let value = SuccessResponse::new(ConnectionDeleteData { deleted: true });
        let json = serde_json::to_string(&value).unwrap();
        let parsed: SuccessResponse<ConnectionDeleteData> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, value);
    }

    #[test]
    fn session_upsert_request_roundtrip() {
        let value = SessionUpsertRequest {
            id: "sess_gzu_a10".to_string(),
            connection_id: "conn_gzu_cluster".to_string(),
            session_type: "alloc".to_string(),
            description: Some("gzu a10 interactive".to_string()),
            state: SessionState::Active,
            node_role: SessionNodeRole::Compute,
            remote_host: Some("210.40.56.85".to_string()),
            compute_node: Some("gpu-a10-01".to_string()),
            keepalive_secs: Some(1800),
        };
        let json = serde_json::to_string(&value).unwrap();
        let parsed: SessionUpsertRequest = serde_json::from_str(&json).unwrap();
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
    fn slurm_jobs_request_roundtrip() {
        let value = SlurmJobsRequest {
            connection_id: "conn_gzu_cluster".to_string(),
            job_id: Some("57373".to_string()),
        };
        let json = serde_json::to_string(&value).unwrap();
        let parsed: SlurmJobsRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, value);
    }

    #[test]
    fn slurm_status_gpu_response_roundtrip() {
        let value = SuccessResponse::new(SlurmStatusGpuData {
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
            summary: SlurmGpuSummary {
                available_nodes: 1,
                total_gpu: 2,
                idle_gpu: 1,
            },
        });
        let json = serde_json::to_string(&value).unwrap();
        let parsed: SuccessResponse<SlurmStatusGpuData> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, value);
    }

    #[test]
    fn slurm_find_gpu_request_roundtrip() {
        let value = SlurmFindGpuRequest {
            connection_id: "conn_gzu_cluster".to_string(),
            gpu_type: Some("a10".to_string()),
        };
        let json = serde_json::to_string(&value).unwrap();
        let parsed: SlurmFindGpuRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, value);
    }

    #[test]
    fn slurm_log_response_roundtrip() {
        let value = SuccessResponse::new(SlurmLogData {
            job_id: "57373".to_string(),
            found: true,
            content: "training started\n".to_string(),
        });
        let json = serde_json::to_string(&value).unwrap();
        let parsed: SuccessResponse<SlurmLogData> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, value);
    }

    #[test]
    fn slurm_cancel_request_roundtrip() {
        let value = SlurmCancelRequest {
            connection_id: "conn_gzu_cluster".to_string(),
            job_ids: vec!["60001".to_string(), "60002".to_string()],
        };
        let json = serde_json::to_string(&value).unwrap();
        let parsed: SlurmCancelRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, value);
    }

    #[test]
    fn slurm_submit_response_roundtrip() {
        let value = SuccessResponse::new(SlurmSubmitData {
            job_id: "60001".to_string(),
            raw_output: "Submitted batch job 60001".to_string(),
        });
        let json = serde_json::to_string(&value).unwrap();
        let parsed: SuccessResponse<SlurmSubmitData> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, value);
    }

    #[test]
    fn file_upload_request_roundtrip() {
        let value = FileUploadRequest {
            connection_id: "conn_gzu_cluster".to_string(),
            local_path: "/tmp/train.py".to_string(),
            remote_path: "~/train.py".to_string(),
            recursive: false,
        };
        let json = serde_json::to_string(&value).unwrap();
        let parsed: FileUploadRequest = serde_json::from_str(&json).unwrap();
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

    #[test]
    fn server_status_backward_compat_defaults() {
        let json = r#"{
          "pid": 1,
          "started_at": "123Z",
          "transport": "tcp",
          "host": "127.0.0.1",
          "port": 49380,
          "db_path": "state.db",
          "runtime_path": "runtime.json"
        }"#;
        let parsed: ServerStatusData = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.api_version, 0);
        assert!(parsed.capabilities.is_empty());
    }
}
