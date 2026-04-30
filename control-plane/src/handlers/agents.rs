use std::sync::Arc;

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Extension, Json,
};
use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

use crate::{
    db::{AccessToken, Agent},
    error::AppError,
    middleware::agent_auth::AuthenticatedAgent,
    types::{AgentConnectRequest, AgentRegisterRequest, AgentRegisterResponse, PingResponse},
    AppState,
};

use super::{convert_job_to_response, extract_registration_token, generate_secure_token};

fn tag_value(tags: &[String], key: &str) -> Option<String> {
    tags.iter()
        .find_map(|tag| tag.strip_prefix(&format!("{key}=")).map(str::to_string))
}

fn parse_priority_from_tags(tags: &[String]) -> Option<i32> {
    tag_value(tags, "priority").and_then(|v| v.parse::<i32>().ok())
}

async fn resolve_queue_id_from_tags(
    state: &Arc<AppState>,
    user_id: Uuid,
    organization_id: Option<Uuid>,
    tags: &[String],
) -> Option<Uuid> {
    let key = tag_value(tags, "queue")?;
    let existing_queue = if let Some(oid) = organization_id {
        state.db.get_queue_by_key_and_org(&key, oid).await
    } else {
        state.db.get_queue_by_key_and_user(&key, user_id).await
    };

    match existing_queue {
        Ok(queue) => {
            tracing::info!(
                "Auto-assigning agent to queue '{}' (id: {})",
                queue.key,
                queue.id
            );
            Some(queue.id)
        }
        Err(sqlx::Error::RowNotFound) => match state
            .db
            .create_queue(user_id, organization_id, None, &key, &key, None, false)
            .await
        {
            Ok(queue) => {
                tracing::info!(
                    "Auto-created queue '{}' (id: {}) and assigning agent",
                    queue.key,
                    queue.id
                );
                Some(queue.id)
            }
            Err(e) => {
                tracing::warn!("Failed to auto-create queue '{}': {}", key, e);
                None
            }
        },
        Err(e) => {
            tracing::warn!("Queue lookup failed for '{}': {}", key, e);
            None
        }
    }
}

pub async fn register_agent(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<AgentRegisterRequest>,
) -> Result<impl IntoResponse, AppError> {
    tracing::info!(
        "Agent registration request from: {} ({})",
        payload.name,
        payload.hostname
    );

    let auth_header = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            AppError::Http(
                StatusCode::UNAUTHORIZED,
                "Missing authorization token".into(),
            )
        })?;

    let token = extract_registration_token(auth_header).ok_or_else(|| {
        AppError::Http(
            StatusCode::UNAUTHORIZED,
            "Invalid authorization header".into(),
        )
    })?;

    let token_record = state
        .db
        .get_agent_token_by_token(&token)
        .await
        .map_err(|_| {
            AppError::Http(
                StatusCode::UNAUTHORIZED,
                "Invalid registration token".into(),
            )
        })?;

    if payload.name.is_empty()
        || payload.hostname.is_empty()
        || payload.os.is_empty()
        || payload.arch.is_empty()
        || payload.version.is_empty()
        || payload.build.is_empty()
    {
        return Err(AppError::Http(
            StatusCode::BAD_REQUEST,
            "Missing required fields".into(),
        ));
    }

    let priority = payload
        .priority
        .as_deref()
        .and_then(|p| p.parse::<i32>().ok())
        .or_else(|| parse_priority_from_tags(&payload.tags));

    let now = Utc::now().naive_utc();
    let user_id = token_record.user_id;
    let organization_id = token_record.organization_id;

    let queue_id = match user_id {
        Some(uid) => resolve_queue_id_from_tags(&state, uid, organization_id, &payload.tags).await,
        None => None,
    };

    let mut agent = Agent {
        id: Uuid::new_v4(),
        uuid: Uuid::new_v4().to_string(),
        name: payload.name.clone(),
        hostname: payload.hostname.clone(),
        os: payload.os.clone(),
        arch: payload.arch.clone(),
        version: payload.version.clone(),
        build: payload.build.clone(),
        tags: Some(payload.tags.clone()),
        priority,
        status: "registered".into(),
        registration_token_id: Some(token_record.id),
        user_id,
        organization_id,
        queue_id,
        last_seen: Some(now),
        last_heartbeat: None,
        current_job_id: None,
        created_at: now,
        updated_at: now,
    };

    state.db.create_agent(&mut agent).await?;

    let access_token_value = generate_secure_token(32);
    let mut access_token = AccessToken {
        id: Uuid::new_v4(),
        agent_id: agent.id,
        token: access_token_value.clone(),
        description: Some("Initial registration token".into()),
        revoked_at: None,
        last_used_at: None,
        created_at: now,
    };

    state.db.create_access_token(&mut access_token).await?;
    tracing::info!(
        "Agent registered successfully: {} (id: {}, user_id: {:?}, queue_id: {:?})",
        agent.name,
        agent.id,
        agent.user_id,
        agent.queue_id
    );

    let response = AgentRegisterResponse {
        id: agent.uuid.parse().unwrap_or(agent.id),
        name: agent.name.clone(),
        access_token: access_token_value,
        endpoint: "".into(),
        request_headers: serde_json::Map::new(),
        ping_interval: 3,
        job_status_interval: 10,
        heartbeat_interval: 60,
        tags: payload.tags.clone(),
    };

    Ok((StatusCode::OK, Json(response)))
}

pub async fn connect_agent(
    State(state): State<Arc<AppState>>,
    Extension(mut agent): Extension<Agent>,
    body: Option<Json<AgentConnectRequest>>,
) -> Result<impl IntoResponse, AppError> {
    tracing::info!("Agent connected: {} (id: {})", agent.name, agent.id);
    agent.status = "connected".into();
    agent.last_seen = Some(Utc::now().naive_utc());

    if let Some(ref req) = body {
        if let Some(ref tags) = req.tags {
            agent.tags = Some(tags.clone());
            if let Some(p) = req.priority.as_deref().and_then(|s| s.parse::<i32>().ok()) {
                agent.priority = Some(p);
                tracing::info!(
                    "Agent {} priority updated from connect body: {}",
                    agent.name,
                    p
                );
            } else if let Some(p) = parse_priority_from_tags(tags) {
                agent.priority = Some(p);
                tracing::info!("Agent {} priority synced from tags: {}", agent.name, p);
            }
        } else if let Some(p) = req.priority.as_deref().and_then(|s| s.parse::<i32>().ok()) {
            agent.priority = Some(p);
            tracing::info!(
                "Agent {} priority updated from connect body: {}",
                agent.name,
                p
            );
        }
    }

    let tag_priority = agent
        .tags
        .as_ref()
        .and_then(|tags| parse_priority_from_tags(tags));
    if tag_priority.is_some() && agent.priority != tag_priority {
        tracing::info!(
            "Re-syncing agent {} priority to {:?}",
            agent.name,
            tag_priority
        );
        agent.priority = tag_priority;
    }

    if let Some(user_id) = agent.user_id {
        let queue_key = agent
            .tags
            .as_ref()
            .and_then(|tags| tag_value(tags, "queue"));
        if queue_key.is_some() {
            if let Some(queue_id) = resolve_queue_id_from_tags(
                &state,
                user_id,
                agent.organization_id,
                agent.tags.as_deref().unwrap_or_default(),
            )
            .await
            {
                if agent.queue_id != Some(queue_id) {
                    tracing::info!("Re-syncing agent {} to queue id {}", agent.name, queue_id);
                }
                agent.queue_id = Some(queue_id);
            }
        } else if agent.queue_id.is_some() {
            tracing::info!("Agent {} has no queue tag, removing from queue", agent.name);
            agent.queue_id = None;
        }
    }

    state.db.update_agent(&agent).await?;
    Ok(StatusCode::OK)
}

pub async fn heartbeat(
    State(state): State<Arc<AppState>>,
    Extension(mut agent): Extension<Agent>,
) -> Result<impl IntoResponse, AppError> {
    let now = Utc::now().naive_utc();
    agent.last_heartbeat = Some(now);
    agent.last_seen = Some(now);
    state.db.update_agent(&agent).await?;
    Ok(Json(json!({ "status": "ok" })))
}

pub async fn disconnect_agent(
    State(state): State<Arc<AppState>>,
    Extension(mut agent): Extension<Agent>,
) -> Result<impl IntoResponse, AppError> {
    agent.status = "disconnected".into();

    if let Some(job_id) = agent.current_job_id {
        if let Ok(mut job) = state.db.get_job_by_id(job_id).await {
            if job.state == "running" {
                job.state = "failed".into();
                job.signal_reason = Some("agent_disconnected".into());
                state.db.update_job(&job).await?;
            }
        }
        agent.current_job_id = None;
    }

    state.db.update_agent(&agent).await?;
    Ok(StatusCode::OK)
}

pub async fn ping(
    State(state): State<Arc<AppState>>,
    Extension(auth_agent): Extension<AuthenticatedAgent>,
) -> Result<impl IntoResponse, AppError> {
    let mut agent = auth_agent.agent;
    tracing::debug!("Ping from agent: {} (id: {})", agent.name, agent.id);
    agent.last_seen = Some(Utc::now().naive_utc());
    state.db.update_agent(&agent).await?;

    if agent.status == "paused" {
        return Ok(Json(PingResponse {
            action: Some("pause".into()),
            ..Default::default()
        }));
    }

    let job = state.dispatcher.match_job_to_agent(&agent).await?;
    let mut response = PingResponse::default();

    if let Some(mut job) = job {
        tracing::info!("Dispatching job {} to agent {}", job.id, agent.name);
        let build = match state.db.get_build_by_id(job.build_id).await {
            Ok(build) => build,
            Err(sqlx::Error::RowNotFound) => {
                tracing::warn!(
                    "Build {} not found for job {}, marking job as failed",
                    job.build_id,
                    job.id
                );
                job.state = "failed".into();
                job.signal_reason = Some("missing_build".into());
                job.agent_id = None;
                state.db.update_job(&job).await?;
                return Ok(Json(response));
            }
            Err(err) => return Err(err.into()),
        };
        let pipeline = state
            .db
            .get_pipeline_by_slug(&build.pipeline_slug)
            .await
            .ok();

        let job_resp = convert_job_to_response(
            &job,
            &build,
            pipeline.as_ref(),
            &agent,
            &auth_agent.access_token,
        )?;
        response.job = Some(job_resp);

        job.state = "accepted".into();
        job.agent_id = Some(agent.id);
        state.db.update_job(&job).await?;
    }

    Ok(Json(response))
}
