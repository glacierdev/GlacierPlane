use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Agent {
    pub id: Uuid,
    pub uuid: String,
    pub name: String,
    pub hostname: String,
    pub os: String,
    pub arch: String,
    pub version: String,
    pub build: String,
    pub tags: Option<Vec<String>>,
    pub priority: Option<i32>,
    pub status: String,
    pub registration_token_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub organization_id: Option<Uuid>,
    pub queue_id: Option<Uuid>,
    pub last_seen: Option<NaiveDateTime>,
    pub last_heartbeat: Option<NaiveDateTime>,
    pub current_job_id: Option<Uuid>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AgentToken {
    pub id: Uuid,
    pub token: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub user_id: Option<Uuid>,
    pub organization_id: Option<Uuid>,
    pub expires_at: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AccessToken {
    pub id: Uuid,
    pub agent_id: Uuid,
    pub token: String,
    pub description: Option<String>,
    pub revoked_at: Option<NaiveDateTime>,
    pub last_used_at: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Job {
    pub id: Uuid,
    pub build_id: Uuid,
    pub step_config: Value,
    pub state: String,
    pub agent_id: Option<Uuid>,
    pub job_token: Option<String>,
    pub env: Option<Value>,
    pub depends_on: Option<Vec<Uuid>>,
    pub exit_status: Option<String>,
    pub signal: Option<String>,
    pub signal_reason: Option<String>,
    pub started_at: Option<NaiveDateTime>,
    pub finished_at: Option<NaiveDateTime>,
    pub runnable_at: Option<NaiveDateTime>,
    pub chunks_failed_count: i32,
    #[sqlx(rename = "traceparent")]
    pub trace_parent: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Build {
    pub id: Uuid,
    pub number: i32,
    pub pipeline_slug: String,
    pub commit: String,
    pub branch: String,
    pub tag: Option<String>,
    pub message: Option<String>,
    pub author_name: Option<String>,
    pub author_email: Option<String>,
    pub status: String,
    pub webhook_payload: Option<Value>,
    pub created_at: NaiveDateTime,
    pub started_at: Option<NaiveDateTime>,
    pub finished_at: Option<NaiveDateTime>,
    pub pull_request_number: Option<i32>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct LogChunk {
    pub id: Uuid,
    pub job_id: Uuid,
    pub sequence: i32,
    #[sqlx(rename = "byte_offset")]
    pub offset: i64,
    pub size: i32,
    pub data: Vec<u8>,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Pipeline {
    pub id: Uuid,
    pub slug: String,
    pub repository_url: String,
    pub webhook_secret: Option<String>,
    pub config_cache: Option<Value>,
    pub user_id: Option<Uuid>,
    pub organization_id: Option<Uuid>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub default_branch: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Metadata {
    pub id: Uuid,
    pub build_id: Uuid,
    pub key: String,
    pub value: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct UserSession {
    pub id: Uuid,
    pub user_id: Uuid,
    pub token: String,
    pub expires_at: NaiveDateTime,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Organization {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct OrganizationMember {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub user_id: Uuid,
    pub role: String,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct OrganizationMemberWithUser {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub user_id: Uuid,
    pub role: String,
    pub created_at: NaiveDateTime,
    pub user_email: String,
    pub user_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct OrganizationInvitation {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub token: String,
    pub created_by: Uuid,
    pub expires_at: NaiveDateTime,
    pub used_by: Option<Uuid>,
    pub used_at: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Queue {
    pub id: Uuid,
    pub user_id: Uuid,
    pub organization_id: Option<Uuid>,
    pub pipeline_id: Option<Uuid>,
    pub name: String,
    pub key: String,
    pub description: Option<String>,
    pub is_default: bool,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueWithStats {
    pub id: Uuid,
    pub user_id: Uuid,
    pub organization_id: Option<Uuid>,
    pub pipeline_id: Option<Uuid>,
    pub name: String,
    pub key: String,
    pub description: Option<String>,
    pub is_default: bool,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub agents_count: i64,
    pub connected_count: i64,
    pub running_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PipelineStats {
    pub total_builds: i64,
    pub passed_builds: i64,
    pub failed_builds: i64,
    pub running_builds: i64,
    pub avg_duration_seconds: f64,
}
