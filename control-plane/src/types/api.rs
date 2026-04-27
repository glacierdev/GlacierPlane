use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct PaginationParams {
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct BuildFilterParams {
    pub page: Option<u32>,
    pub per_page: Option<u32>,
    pub state: Option<String>,
    pub branch: Option<String>,
    pub commit: Option<String>,
    pub created_from: Option<String>,
    pub created_to: Option<String>,
    pub finished_from: Option<String>,
    pub creator: Option<String>,
}

#[derive(Debug, Default)]
pub struct BuildFilter {
    pub states: Option<Vec<String>>,
    pub branch: Option<String>,
    pub commit: Option<String>,
    pub created_from: Option<NaiveDateTime>,
    pub created_to: Option<NaiveDateTime>,
    pub finished_from: Option<NaiveDateTime>,
    pub creator: Option<String>,
}

impl BuildFilterParams {
    pub fn to_pagination(&self) -> PaginationParams {
        PaginationParams {
            page: self.page,
            per_page: self.per_page,
        }
    }

    pub fn to_filter(&self) -> BuildFilter {
        let states = self.state.as_ref().map(|s| {
            s.split(',')
                .flat_map(|part| {
                    let part = part.trim().to_lowercase();
                    if part == "finished" {
                        vec![
                            "passed".to_string(),
                            "failed".to_string(),
                            "blocked".to_string(),
                            "canceled".to_string(),
                        ]
                    } else {
                        vec![part]
                    }
                })
                .collect()
        });

        let parse_ts = |s: &str| -> Option<NaiveDateTime> {
            chrono::DateTime::parse_from_rfc3339(s)
                .ok()
                .map(|dt| dt.naive_utc())
                .or_else(|| {
                    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
                        .ok()
                        .and_then(|d| d.and_hms_opt(0, 0, 0))
                })
        };

        BuildFilter {
            states,
            branch: self.branch.clone(),
            commit: self.commit.clone(),
            created_from: self.created_from.as_deref().and_then(parse_ts),
            created_to: self.created_to.as_deref().and_then(parse_ts),
            finished_from: self.finished_from.as_deref().and_then(parse_ts),
            creator: self.creator.clone(),
        }
    }

}

#[derive(Debug, Deserialize)]
pub struct BuildCreateRequest {
    pub commit: String,
    pub branch: String,
    pub message: Option<String>,
    pub author: Option<BuildAuthor>,
    pub env: Option<std::collections::HashMap<String, String>>,
    pub meta_data: Option<std::collections::HashMap<String, String>>,
}

#[derive(Debug, Deserialize)]
pub struct BuildAuthor {
    pub name: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UserRegisterRequest {
    pub email: String,
    pub name: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct UserLoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct PipelineCreateRequest {
    pub name: String,
    pub slug: String,
    pub repository_url: String,
    pub description: Option<String>,
    pub default_branch: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PipelineUpdateRequest {
    pub name: String,
    pub description: Option<String>,
    pub repository_url: String,
    pub default_branch: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PipelineResponse {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub repository_url: String,
    pub default_branch: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct PipelineWithStatsResponse {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub repository_url: String,
    pub default_branch: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub total_builds: i64,
    pub passed_builds: i64,
    pub failed_builds: i64,
    pub running_builds: i64,
    pub avg_duration_seconds: f64,
    pub recent_builds: Vec<BuildSummary>,
    pub queues: Vec<PipelineQueueInfo>,
}

#[derive(Debug, Serialize)]
pub struct PipelineQueueInfo {
    pub id: Uuid,
    pub name: String,
    pub key: String,
}

#[derive(Debug, Serialize)]
pub struct BuildSummary {
    pub id: Uuid,
    pub number: i32,
    pub commit: String,
    pub branch: String,
    pub message: Option<String>,
    pub author_name: Option<String>,
    pub state: String,
    pub source: String,
    pub created_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PipelineDetailResponse {
    pub pipeline: PipelineResponse,
    pub stats: PipelineStatsResponse,
    pub builds: Vec<BuildWithJobsResponse>,
}

#[derive(Debug, Serialize)]
pub struct PipelineStatsResponse {
    pub total_builds: i64,
    pub passed_builds: i64,
    pub failed_builds: i64,
    pub running_builds: i64,
    pub avg_duration_seconds: f64,
}

#[derive(Debug, Serialize)]
pub struct BuildWithJobsResponse {
    pub id: Uuid,
    pub number: i32,
    pub pipeline_slug: String,
    pub commit: String,
    pub branch: String,
    pub message: Option<String>,
    pub author_name: Option<String>,
    pub author_email: Option<String>,
    pub state: String,
    pub source: String,
    pub created_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub jobs: Vec<JobSummary>,
}

#[derive(Debug, Serialize)]
pub struct JobSummary {
    pub id: Uuid,
    pub state: String,
    pub exit_status: Option<String>,
    pub label: Option<String>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct JobLogResponse {
    pub content: String,
    pub size: usize,
    pub header_times: Vec<i64>,
}

#[derive(Debug, Deserialize)]
pub struct QueueCreateRequest {
    pub name: String,
    pub key: String,
    pub description: Option<String>,
    pub pipeline_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct QueueUpdateRequest {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct QueueResponse {
    pub id: Uuid,
    pub user_id: Uuid,
    pub pipeline_id: Option<Uuid>,
    pub name: String,
    pub key: String,
    pub description: Option<String>,
    pub is_default: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct QueueWithStatsResponse {
    pub id: Uuid,
    pub user_id: Uuid,
    pub pipeline_id: Option<Uuid>,
    pub pipeline_name: Option<String>,
    pub name: String,
    pub key: String,
    pub description: Option<String>,
    pub is_default: bool,
    pub created_at: String,
    pub updated_at: String,
    pub agents_count: i64,
    pub connected_count: i64,
    pub running_count: i64,
}

#[derive(Debug, Serialize)]
pub struct QueueAgentTokenResponse {
    pub id: Uuid,
    pub name: Option<String>,
    pub description: Option<String>,
    pub token_preview: String,
    pub created_at: String,
    pub registrations_count: i64,
    pub connected_count: i64,
    pub running_count: i64,
}

#[derive(Debug, Serialize)]
pub struct QueueDetailResponse {
    pub queue: QueueResponse,
    pub agents: Vec<QueueAgentTokenResponse>,
    pub pipeline_name: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AgentResponse {
    pub id: Uuid,
    pub uuid: String,
    pub name: String,
    pub hostname: String,
    pub os: String,
    pub arch: String,
    pub version: String,
    pub connection_state: String,
    pub user_agent: String,
    pub meta_data: Option<Vec<String>>,
    pub priority: Option<i32>,
    pub queue_id: Option<Uuid>,
    pub queue_name: Option<String>,
    pub last_seen: Option<String>,
    pub last_heartbeat: Option<String>,
    pub current_job_id: Option<Uuid>,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct AgentTokenCreateRequest {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AgentTokenResponse {
    pub id: Uuid,
    pub name: Option<String>,
    pub description: Option<String>,
    pub token_preview: String,
    pub token: Option<String>,
    pub expires_at: Option<String>,
    pub created_at: String,
    pub agents_count: i64,
    pub connected_count: i64,
    pub running_count: i64,
}

#[derive(Debug, Serialize)]
pub struct AgentTokenDetailResponse {
    pub token: AgentTokenResponse,
    pub agents: Vec<AgentResponse>,
}

#[derive(Debug, Deserialize)]
pub struct OrganizationCreateRequest {
    pub name: String,
    pub slug: String,
}

#[derive(Debug, Serialize)]
pub struct OrganizationResponse {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub role: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct OrganizationMemberResponse {
    pub id: Uuid,
    pub user_id: Uuid,
    pub email: String,
    pub name: String,
    pub role: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateMemberRoleRequest {
    pub role: String,
}

#[derive(Debug, Serialize)]
pub struct OrganizationInvitationResponse {
    pub id: Uuid,
    pub token: String,
    pub invite_url: String,
    pub expires_at: String,
    pub created_at: String,
    pub used: bool,
}

#[derive(Debug, Serialize)]
pub struct OrganizationDetailResponse {
    pub organization: OrganizationResponse,
    pub members: Vec<OrganizationMemberResponse>,
    pub invitations: Vec<OrganizationInvitationResponse>,
}
