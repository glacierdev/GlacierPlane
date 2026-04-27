use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct AgentRegisterRequest {
    pub name: String,
    pub hostname: String,
    pub os: String,
    pub arch: String,
    #[serde(default)]
    pub script_eval_enabled: bool,
    #[serde(default)]
    pub ignore_in_dispatches: bool,
    #[serde(default)]
    pub priority: Option<String>,
    pub version: String,
    pub build: String,
    #[serde(default, rename = "meta_data")]
    pub tags: Vec<String>,
    #[serde(default)]
    pub pid: Option<i32>,
    #[serde(default)]
    pub machine_id: Option<String>,
    #[serde(default)]
    pub features: Vec<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct AgentConnectRequest {
    #[serde(default, rename = "meta_data")]
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub priority: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AgentRegisterResponse {
    pub id: Uuid,
    pub name: String,
    pub access_token: String,
    pub endpoint: String,
    pub request_headers: serde_json::Map<String, Value>,
    pub ping_interval: i32,
    pub job_status_interval: i32,
    pub heartbeat_interval: i32,
    #[serde(rename = "meta_data")]
    pub tags: Vec<String>,
}

#[derive(Debug, Serialize, Default)]
pub struct PingResponse {
    pub action: Option<String>,
    pub message: Option<String>,
    pub job: Option<JobResponse>,
    pub endpoint: Option<String>,
    #[serde(default)]
    pub request_headers: serde_json::Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CommandStep {
    pub label: Option<String>,
    pub command: Option<Value>,
    #[serde(default)]
    pub env: Option<Value>,
    #[serde(default)]
    pub agents: Option<Value>,
    #[serde(default)]
    pub key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct JobResponse {
    pub id: Uuid,
    pub endpoint: String,
    pub state: Option<String>,
    pub env: serde_json::Map<String, Value>,
    pub step: CommandStep,
    #[serde(default)]
    pub chunks_max_size_bytes: Option<u64>,
    #[serde(default)]
    pub chunks_interval_seconds: Option<i32>,
    #[serde(default)]
    pub log_max_size_bytes: Option<u64>,
    pub token: Option<String>,
    pub exit_status: Option<String>,
    pub signal: Option<String>,
    pub signal_reason: Option<String>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub runnable_at: Option<String>,
    pub chunks_failed_count: Option<i32>,
    #[serde(rename = "traceparent")]
    pub trace_parent: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct JobStartRequest {
    pub started_at: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct JobFinishRequest {
    pub exit_status: Option<String>,
    pub signal: Option<String>,
    pub signal_reason: Option<String>,
    pub finished_at: Option<String>,
    pub chunks_failed_count: Option<i32>,
    #[serde(default)]
    pub ignore_agent_in_dispatches: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UploadChunkParams {
    pub sequence: i32,
    pub offset: i64,
    pub size: i32,
}

#[derive(Debug, Deserialize)]
pub struct MetadataExistsRequest {
    pub key: String,
}

#[derive(Debug, Deserialize)]
pub struct MetadataSetRequest {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Deserialize)]
pub struct MetadataGetRequest {
    pub key: String,
}

#[derive(Debug, Deserialize)]
pub struct PipelineUploadRequest {
    pub pipeline: Option<Value>,
    pub steps: Option<Vec<Value>>,
    #[serde(default)]
    #[allow(dead_code)]
    pub replace: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PipelineStep {
    pub label: Option<String>,
    pub command: Option<Value>,
    pub commands: Option<Vec<String>>,
    pub env: Option<Value>,
    pub agents: Option<Value>,
    pub depends_on: Option<Value>,
    pub timeout_in_minutes: Option<i32>,
    pub retry: Option<Value>,
    pub plugins: Option<Value>,
    #[serde(rename = "if")]
    pub condition: Option<String>,
    pub key: Option<String>,
    pub soft_fail: Option<Value>,
    pub allow_dependency_failure: Option<bool>,
}
