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
        PaginationParams, QueueAgentTokenResponse, QueueCreateRequest, QueueDetailResponse,
        QueueResponse, QueueUpdateRequest, QueueWithStatsResponse,
    },
    AppState,
};

use super::{get_user_and_org_by_slug, paginate_params, paginated_response};

pub async fn list_user_queues(
    State(state): State<Arc<AppState>>,
    Path(org_slug): Path<String>,
    Query(pagination): Query<PaginationParams>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let (_user, org_id) = get_user_and_org_by_slug(&state, &headers, &org_slug).await?;
    let (page, per_page, limit, offset) = paginate_params(&pagination);
    let total = state.db.count_queues_by_organization(org_id).await?;
    let queues = state
        .db
        .get_queues_by_organization_paginated(org_id, limit, offset)
        .await?;

    let mut response: Vec<QueueWithStatsResponse> = Vec::new();

    for queue in queues {
        let registrations = state.db.get_agents_by_queue(queue.id).await?;

        let unique_tokens: std::collections::HashSet<Uuid> = registrations
            .iter()
            .filter_map(|a| a.registration_token_id)
            .collect();
        let agents_count = unique_tokens.len() as i64;
        let connected_count = registrations
            .iter()
            .filter(|a| a.status == "connected")
            .count() as i64;
        let running_count = registrations
            .iter()
            .filter(|a| a.current_job_id.is_some())
            .count() as i64;

        let pipeline_name = if let Some(pid) = queue.pipeline_id {
            state
                .db
                .get_pipeline_by_id(pid)
                .await
                .ok()
                .and_then(|p| p.name)
        } else {
            None
        };

        response.push(QueueWithStatsResponse {
            id: queue.id,
            user_id: queue.user_id,
            pipeline_id: queue.pipeline_id,
            pipeline_name,
            name: queue.name,
            key: queue.key,
            description: queue.description,
            is_default: queue.is_default,
            created_at: DateTime::<Utc>::from_naive_utc_and_offset(queue.created_at, Utc)
                .to_rfc3339(),
            updated_at: DateTime::<Utc>::from_naive_utc_and_offset(queue.updated_at, Utc)
                .to_rfc3339(),
            agents_count,
            connected_count,
            running_count,
        });
    }

    Ok(paginated_response(
        response,
        page,
        per_page,
        total,
        uri.path(),
    ))
}

pub async fn get_user_queue(
    State(state): State<Arc<AppState>>,
    Path((org_slug, queue_id)): Path<(String, Uuid)>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let (_user, org_id) = get_user_and_org_by_slug(&state, &headers, &org_slug).await?;
    let queue = state
        .db
        .get_queue_by_id_and_org(queue_id, org_id)
        .await
        .map_err(|_| AppError::Http(StatusCode::NOT_FOUND, "Queue not found".into()))?;

    let registrations = state.db.get_agents_by_queue(queue.id).await?;

    let pipeline_name = if let Some(pid) = queue.pipeline_id {
        state
            .db
            .get_pipeline_by_id(pid)
            .await
            .ok()
            .and_then(|p| p.name)
    } else {
        None
    };

    let mut token_groups: std::collections::HashMap<Uuid, Vec<&crate::db::Agent>> =
        std::collections::HashMap::new();
    for reg in &registrations {
        if let Some(token_id) = reg.registration_token_id {
            token_groups.entry(token_id).or_default().push(reg);
        }
    }

    let mut agents_resp: Vec<QueueAgentTokenResponse> = Vec::new();
    for (token_id, regs) in &token_groups {
        if let Ok(token) = state.db.get_agent_token_by_id(*token_id).await {
            let token_preview = if token.token.len() > 8 {
                format!("{}...", &token.token[..8])
            } else {
                token.token.clone()
            };

            agents_resp.push(QueueAgentTokenResponse {
                id: token.id,
                name: token.name,
                description: token.description,
                token_preview,
                created_at: DateTime::<Utc>::from_naive_utc_and_offset(token.created_at, Utc)
                    .to_rfc3339(),
                registrations_count: regs.len() as i64,
                connected_count: regs.iter().filter(|a| a.status == "connected").count() as i64,
                running_count: regs.iter().filter(|a| a.current_job_id.is_some()).count() as i64,
            });
        }
    }

    let response = QueueDetailResponse {
        queue: QueueResponse {
            id: queue.id,
            user_id: queue.user_id,
            pipeline_id: queue.pipeline_id,
            name: queue.name,
            key: queue.key,
            description: queue.description,
            is_default: queue.is_default,
            created_at: DateTime::<Utc>::from_naive_utc_and_offset(queue.created_at, Utc)
                .to_rfc3339(),
            updated_at: DateTime::<Utc>::from_naive_utc_and_offset(queue.updated_at, Utc)
                .to_rfc3339(),
        },
        agents: agents_resp,
        pipeline_name,
    };

    Ok(Json(response))
}

pub async fn create_user_queue(
    State(state): State<Arc<AppState>>,
    Path(org_slug): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<QueueCreateRequest>,
) -> Result<impl IntoResponse, AppError> {
    let (user, org_id) = get_user_and_org_by_slug(&state, &headers, &org_slug).await?;

    if payload.name.is_empty() || payload.key.is_empty() {
        return Err(AppError::Http(
            StatusCode::BAD_REQUEST,
            "Name and key are required".into(),
        ));
    }
    if !payload
        .key
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return Err(AppError::Http(
            StatusCode::BAD_REQUEST,
            "Key must contain only alphanumeric characters, hyphens, and underscores".into(),
        ));
    }
    if state
        .db
        .get_queue_by_key_and_org(&payload.key, org_id)
        .await
        .is_ok()
    {
        return Err(AppError::Http(
            StatusCode::CONFLICT,
            "A queue with this key already exists".into(),
        ));
    }
    if let Some(pipeline_id) = payload.pipeline_id {
        state
            .db
            .get_pipeline_by_id_and_org(pipeline_id, org_id)
            .await
            .map_err(|_| AppError::Http(StatusCode::BAD_REQUEST, "Pipeline not found".into()))?;
    }

    let queue = state
        .db
        .create_queue(
            user.id,
            Some(org_id),
            payload.pipeline_id,
            &payload.name,
            &payload.key,
            payload.description.as_deref(),
            false,
        )
        .await?;

    tracing::info!("Queue created: {} (id: {})", queue.key, queue.id);

    let response = QueueResponse {
        id: queue.id,
        user_id: queue.user_id,
        pipeline_id: queue.pipeline_id,
        name: queue.name,
        key: queue.key,
        description: queue.description,
        is_default: queue.is_default,
        created_at: DateTime::<Utc>::from_naive_utc_and_offset(queue.created_at, Utc).to_rfc3339(),
        updated_at: DateTime::<Utc>::from_naive_utc_and_offset(queue.updated_at, Utc).to_rfc3339(),
    };

    Ok((StatusCode::CREATED, Json(response)))
}

pub async fn update_user_queue(
    State(state): State<Arc<AppState>>,
    Path((org_slug, queue_id)): Path<(String, Uuid)>,
    headers: HeaderMap,
    Json(payload): Json<QueueUpdateRequest>,
) -> Result<impl IntoResponse, AppError> {
    let (_user, org_id) = get_user_and_org_by_slug(&state, &headers, &org_slug).await?;

    if payload.name.is_empty() {
        return Err(AppError::Http(
            StatusCode::BAD_REQUEST,
            "Name is required".into(),
        ));
    }

    state
        .db
        .get_queue_by_id_and_org(queue_id, org_id)
        .await
        .map_err(|_| AppError::Http(StatusCode::NOT_FOUND, "Queue not found".into()))?;
    let queue = state
        .db
        .update_queue_by_org(
            queue_id,
            org_id,
            &payload.name,
            payload.description.as_deref(),
        )
        .await?;

    tracing::info!("Queue updated: {} (id: {})", queue.key, queue.id);

    let response = QueueResponse {
        id: queue.id,
        user_id: queue.user_id,
        pipeline_id: queue.pipeline_id,
        name: queue.name,
        key: queue.key,
        description: queue.description,
        is_default: queue.is_default,
        created_at: DateTime::<Utc>::from_naive_utc_and_offset(queue.created_at, Utc).to_rfc3339(),
        updated_at: DateTime::<Utc>::from_naive_utc_and_offset(queue.updated_at, Utc).to_rfc3339(),
    };

    Ok(Json(response))
}

pub async fn delete_user_queue(
    State(state): State<Arc<AppState>>,
    Path((org_slug, queue_id)): Path<(String, Uuid)>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let (_user, org_id) = get_user_and_org_by_slug(&state, &headers, &org_slug).await?;
    let queue = state
        .db
        .get_queue_by_id_and_org(queue_id, org_id)
        .await
        .map_err(|_| AppError::Http(StatusCode::NOT_FOUND, "Queue not found".into()))?;

    if queue.is_default {
        return Err(AppError::Http(
            StatusCode::BAD_REQUEST,
            "Cannot delete default queue".into(),
        ));
    }

    let deleted = state.db.delete_queue_by_org(queue_id, org_id).await?;
    if !deleted {
        return Err(AppError::Http(
            StatusCode::NOT_FOUND,
            "Queue not found or cannot be deleted".into(),
        ));
    }

    tracing::info!("Queue deleted: {}", queue_id);
    Ok(Json(json!({ "message": "Queue deleted successfully" })))
}
