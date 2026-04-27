use std::sync::Arc;

use axum::{
    extract::{OriginalUri, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, Utc};
use serde_json::json;
use uuid::Uuid;

use crate::{
    error::AppError,
    types::{
        AgentResponse, AgentTokenCreateRequest, AgentTokenDetailResponse, AgentTokenResponse,
        PaginationParams,
    },
    AppState,
};

use super::{generate_secure_token, get_user_and_org_by_slug, paginate_params, paginated_response};

fn token_preview(token: &str) -> String {
    if token.len() > 8 {
        format!("{}...", &token[..8])
    } else {
        token.to_string()
    }
}

fn agent_runtime_counts(agents: &[crate::db::Agent]) -> (i64, i64, i64) {
    let agents_count = agents.len() as i64;
    let connected_count = agents.iter().filter(|a| a.status == "connected").count() as i64;
    let running_count = agents.iter().filter(|a| a.current_job_id.is_some()).count() as i64;
    (agents_count, connected_count, running_count)
}

async fn map_agent_response(state: &Arc<AppState>, agent: crate::db::Agent) -> AgentResponse {
    let queue_name = if let Some(qid) = agent.queue_id {
        state.db.get_queue_by_id(qid).await.ok().map(|q| q.name)
    } else {
        None
    };

    let user_agent = format!("glacier-agent/{} ({}/{})", agent.version, agent.os, agent.arch);

    AgentResponse {
        id: agent.id,
        uuid: agent.uuid,
        name: agent.name,
        hostname: agent.hostname,
        os: agent.os,
        arch: agent.arch,
        version: agent.version,
        connection_state: agent.status,
        user_agent,
        meta_data: agent.tags,
        priority: agent.priority,
        queue_id: agent.queue_id,
        queue_name,
        last_seen: agent
            .last_seen
            .map(|t| DateTime::<Utc>::from_naive_utc_and_offset(t, Utc).to_rfc3339()),
        last_heartbeat: agent
            .last_heartbeat
            .map(|t| DateTime::<Utc>::from_naive_utc_and_offset(t, Utc).to_rfc3339()),
        current_job_id: agent.current_job_id,
        created_at: DateTime::<Utc>::from_naive_utc_and_offset(agent.created_at, Utc).to_rfc3339(),
    }
}

pub async fn list_user_agent_tokens(
    State(state): State<Arc<AppState>>,
    Path(org_slug): Path<String>,
    Query(pagination): Query<PaginationParams>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let (_user, org_id) = get_user_and_org_by_slug(&state, &headers, &org_slug).await?;
    let (page, per_page, limit, offset) = paginate_params(&pagination);
    let total = state.db.count_agent_tokens_by_organization(org_id).await?;
    let tokens = state.db.get_agent_tokens_by_organization_paginated(org_id, limit, offset).await?;

    let mut response: Vec<AgentTokenResponse> = Vec::new();

    for token in tokens {
        let agents = state.db.get_agents_by_registration_token(token.id).await?;
        let (agents_count, connected_count, running_count) = agent_runtime_counts(&agents);

        response.push(AgentTokenResponse {
            id: token.id,
            name: token.name,
            description: token.description,
            token_preview: token_preview(&token.token),
            token: None,
            expires_at: token.expires_at.map(|t| DateTime::<Utc>::from_naive_utc_and_offset(t, Utc).to_rfc3339()),
            created_at: DateTime::<Utc>::from_naive_utc_and_offset(token.created_at, Utc).to_rfc3339(),
            agents_count,
            connected_count,
            running_count,
        });
    }

    Ok(paginated_response(response, page, per_page, total, uri.path()))
}

pub async fn create_user_agent_token(
    State(state): State<Arc<AppState>>,
    Path(org_slug): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<AgentTokenCreateRequest>,
) -> Result<impl IntoResponse, AppError> {
    let (user, org_id) = get_user_and_org_by_slug(&state, &headers, &org_slug).await?;

    if payload.name.is_empty() {
        return Err(AppError::Http(StatusCode::BAD_REQUEST, "Name is required".into()));
    }

    let token_value = generate_secure_token(32);
    let token = state.db
        .create_agent_token_for_user(user.id, Some(org_id), &token_value, &payload.name, payload.description.as_deref())
        .await?;

    tracing::info!("Agent token created: {} (id: {})", token.name.as_deref().unwrap_or(""), token.id);

    let response = AgentTokenResponse {
        id: token.id,
        name: token.name,
        description: token.description,
        token_preview: token_preview(&token_value),
        token: Some(token_value),
        expires_at: token.expires_at.map(|t| DateTime::<Utc>::from_naive_utc_and_offset(t, Utc).to_rfc3339()),
        created_at: DateTime::<Utc>::from_naive_utc_and_offset(token.created_at, Utc).to_rfc3339(),
        agents_count: 0,
        connected_count: 0,
        running_count: 0,
    };

    Ok((StatusCode::CREATED, Json(response)))
}

pub async fn get_user_agent_token(
    State(state): State<Arc<AppState>>,
    Path((org_slug, token_id)): Path<(String, Uuid)>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let (_user, org_id) = get_user_and_org_by_slug(&state, &headers, &org_slug).await?;

    let token = state.db.get_agent_token_by_id(token_id).await
        .map_err(|_| AppError::Http(StatusCode::NOT_FOUND, "Token not found".into()))?;

    if token.organization_id != Some(org_id) {
        return Err(AppError::Http(StatusCode::NOT_FOUND, "Token not found".into()));
    }

    let agents = state.db.get_agents_by_registration_token(token.id).await?;
    let (agents_count, connected_count, running_count) = agent_runtime_counts(&agents);

    let mut agents_resp: Vec<AgentResponse> = Vec::new();
    for agent in agents {
        agents_resp.push(map_agent_response(&state, agent).await);
    }

    let response = AgentTokenDetailResponse {
        token: AgentTokenResponse {
            id: token.id,
            name: token.name,
            description: token.description,
            token_preview: token_preview(&token.token),
            token: None,
            expires_at: token.expires_at.map(|t| DateTime::<Utc>::from_naive_utc_and_offset(t, Utc).to_rfc3339()),
            created_at: DateTime::<Utc>::from_naive_utc_and_offset(token.created_at, Utc).to_rfc3339(),
            agents_count,
            connected_count,
            running_count,
        },
        agents: agents_resp,
    };

    Ok(Json(response))
}

pub async fn delete_user_agent_token(
    State(state): State<Arc<AppState>>,
    Path((org_slug, token_id)): Path<(String, Uuid)>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let (user, org_id) = get_user_and_org_by_slug(&state, &headers, &org_slug).await?;

    let token = state.db.get_agent_token_by_id(token_id).await
        .map_err(|_| AppError::Http(StatusCode::NOT_FOUND, "Token not found".into()))?;

    if token.organization_id != Some(org_id) {
        return Err(AppError::Http(StatusCode::NOT_FOUND, "Token not found".into()));
    }

    let (deleted, agents_deleted) = state.db.delete_agent_token(token_id, user.id).await?;
    if !deleted {
        return Err(AppError::Http(StatusCode::NOT_FOUND, "Token not found".into()));
    }

    tracing::info!("Agent token deleted: {} (deleted {} agents)", token_id, agents_deleted);
    Ok(Json(json!({ "message": "Token deleted successfully", "agents_deleted": agents_deleted })))
}

pub async fn list_user_agents(
    State(state): State<Arc<AppState>>,
    Path(org_slug): Path<String>,
    Query(pagination): Query<PaginationParams>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let (_user, org_id) = get_user_and_org_by_slug(&state, &headers, &org_slug).await?;
    let (page, per_page, limit, offset) = paginate_params(&pagination);
    let total = state.db.count_agents_by_organization(org_id).await?;
    let agents = state.db.get_agents_by_organization_paginated(org_id, limit, offset).await?;

    let mut response: Vec<AgentResponse> = Vec::new();
    for agent in agents {
        response.push(map_agent_response(&state, agent).await);
    }

    Ok(paginated_response(response, page, per_page, total, uri.path()))
}
