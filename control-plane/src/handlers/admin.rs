use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::{error::AppError, AppState};

#[derive(serde::Serialize)]
pub struct AdminAgentTokenResponse {
    pub id: Uuid,
    pub name: Option<String>,
    pub description: Option<String>,
    pub token_preview: String,
    pub expires_at: Option<String>,
    pub created_at: String,
    pub agents_count: i64,
    pub connected_count: i64,
    pub running_count: i64,
}

#[derive(serde::Serialize)]
pub struct AdminAgentResponse {
    pub id: Uuid,
    pub uuid: String,
    pub name: String,
    pub hostname: String,
    pub os: String,
    pub arch: String,
    pub version: String,
    pub status: String,
    pub tags: Option<Vec<String>>,
    pub last_seen: Option<String>,
    pub last_heartbeat: Option<String>,
    pub current_job_id: Option<Uuid>,
    pub created_at: String,
}

#[derive(serde::Serialize)]
pub struct AdminJobResponse {
    pub id: Uuid,
    pub build_id: Uuid,
    pub agent_id: Option<Uuid>,
    pub agent_name: Option<String>,
    pub state: String,
    pub exit_status: Option<String>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub created_at: String,
    pub label: Option<String>,
}

#[derive(serde::Serialize)]
pub struct AdminJobWithLogsResponse {
    pub job: AdminJobResponse,
    pub logs: String,
}

#[derive(serde::Serialize)]
pub struct AdminAgentTokenDetailResponse {
    pub token: AdminAgentTokenResponse,
    pub agents: Vec<AdminAgentResponse>,
    pub jobs: Vec<AdminJobWithLogsResponse>,
}

pub async fn admin_list_tokens(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, AppError> {
    let tokens = state.db.get_all_agent_tokens().await?;

    let mut response = Vec::new();
    for token in tokens {
        let agents = state.db.get_agents_by_registration_token(token.id).await?;
        let agents_count = agents.len() as i64;
        let connected_count = agents.iter().filter(|a| a.status == "connected").count() as i64;
        let running_count = agents.iter().filter(|a| a.current_job_id.is_some()).count() as i64;

        let token_preview = if token.token.len() > 8 {
            format!("{}...", &token.token[..8])
        } else {
            token.token.clone()
        };

        response.push(AdminAgentTokenResponse {
            id: token.id,
            name: token.name,
            description: token.description,
            token_preview,
            expires_at: token
                .expires_at
                .map(|t| DateTime::<Utc>::from_naive_utc_and_offset(t, Utc).to_rfc3339()),
            created_at: DateTime::<Utc>::from_naive_utc_and_offset(token.created_at, Utc)
                .to_rfc3339(),
            agents_count,
            connected_count,
            running_count,
        });
    }

    Ok(Json(response))
}

pub async fn admin_get_token(
    State(state): State<Arc<AppState>>,
    Path(token_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let token = state
        .db
        .get_agent_token_by_id(token_id)
        .await
        .map_err(|_| AppError::Http(StatusCode::NOT_FOUND, "Token not found".into()))?;

    let agents = state.db.get_agents_by_registration_token(token_id).await?;
    let jobs = state.db.get_jobs_for_token_agents(token_id, 50).await?;

    let agents_map: std::collections::HashMap<Uuid, String> =
        agents.iter().map(|a| (a.id, a.name.clone())).collect();

    let agents_count = agents.len() as i64;
    let connected_count = agents.iter().filter(|a| a.status == "connected").count() as i64;
    let running_count = agents.iter().filter(|a| a.current_job_id.is_some()).count() as i64;

    let token_preview = if token.token.len() > 8 {
        format!("{}...", &token.token[..8])
    } else {
        token.token.clone()
    };

    let token_resp = AdminAgentTokenResponse {
        id: token.id,
        name: token.name,
        description: token.description,
        token_preview,
        expires_at: token
            .expires_at
            .map(|t| DateTime::<Utc>::from_naive_utc_and_offset(t, Utc).to_rfc3339()),
        created_at: DateTime::<Utc>::from_naive_utc_and_offset(token.created_at, Utc).to_rfc3339(),
        agents_count,
        connected_count,
        running_count,
    };

    let agents_resp: Vec<AdminAgentResponse> = agents
        .into_iter()
        .map(|a| AdminAgentResponse {
            id: a.id,
            uuid: a.uuid,
            name: a.name,
            hostname: a.hostname,
            os: a.os,
            arch: a.arch,
            version: a.version,
            status: a.status,
            tags: a.tags,
            last_seen: a
                .last_seen
                .map(|t| DateTime::<Utc>::from_naive_utc_and_offset(t, Utc).to_rfc3339()),
            last_heartbeat: a
                .last_heartbeat
                .map(|t| DateTime::<Utc>::from_naive_utc_and_offset(t, Utc).to_rfc3339()),
            current_job_id: a.current_job_id,
            created_at: DateTime::<Utc>::from_naive_utc_and_offset(a.created_at, Utc).to_rfc3339(),
        })
        .collect();

    let mut jobs_with_logs = Vec::new();
    for job in jobs {
        let chunks = state.db.get_log_chunks_for_job(job.id).await?;
        let logs: String = chunks
            .into_iter()
            .map(|c| String::from_utf8_lossy(&c.data).to_string())
            .collect::<Vec<_>>()
            .join("");

        let label = job
            .step_config
            .get("label")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let agent_name = job.agent_id.and_then(|aid| agents_map.get(&aid).cloned());

        let job_resp = AdminJobResponse {
            id: job.id,
            build_id: job.build_id,
            agent_id: job.agent_id,
            agent_name,
            state: job.state,
            exit_status: job.exit_status,
            started_at: job
                .started_at
                .map(|t| DateTime::<Utc>::from_naive_utc_and_offset(t, Utc).to_rfc3339()),
            finished_at: job
                .finished_at
                .map(|t| DateTime::<Utc>::from_naive_utc_and_offset(t, Utc).to_rfc3339()),
            created_at: DateTime::<Utc>::from_naive_utc_and_offset(job.created_at, Utc)
                .to_rfc3339(),
            label,
        };

        jobs_with_logs.push(AdminJobWithLogsResponse {
            job: job_resp,
            logs,
        });
    }

    Ok(Json(AdminAgentTokenDetailResponse {
        token: token_resp,
        agents: agents_resp,
        jobs: jobs_with_logs,
    }))
}
