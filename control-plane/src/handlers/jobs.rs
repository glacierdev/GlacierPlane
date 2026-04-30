use std::{collections::HashMap, io::Read, sync::Arc};

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Extension, Json,
};
use chrono::{DateTime, Utc};
use flate2::read::GzDecoder;
use serde_json::json;
use uuid::Uuid;

use crate::{
    db::{Agent, Job, LogChunk},
    error::AppError,
    middleware::agent_auth::AuthenticatedAgent,
    types::{
        JobFinishRequest, JobStartRequest, MetadataExistsRequest, MetadataGetRequest,
        MetadataSetRequest, PipelineStep, PipelineUploadRequest, UploadChunkParams,
    },
    AppState,
};

use super::{convert_job_to_response, update_build_status};

async fn load_job_for_agent(
    state: &Arc<AppState>,
    job_id: Uuid,
    agent_id: Uuid,
) -> Result<Job, AppError> {
    let job = state
        .db
        .get_job_by_id(job_id)
        .await
        .map_err(|_| AppError::Http(StatusCode::NOT_FOUND, "Job not found".into()))?;

    if job.agent_id != Some(agent_id) {
        return Err(AppError::Http(
            StatusCode::FORBIDDEN,
            "Job not assigned to this agent".into(),
        ));
    }

    Ok(job)
}

pub async fn get_job(
    State(state): State<Arc<AppState>>,
    Path(job_id): Path<Uuid>,
    Extension(auth_agent): Extension<AuthenticatedAgent>,
) -> Result<impl IntoResponse, AppError> {
    let agent = &auth_agent.agent;
    let job = load_job_for_agent(&state, job_id, agent.id).await?;

    let build = state.db.get_build_by_id(job.build_id).await?;
    let pipeline = state
        .db
        .get_pipeline_by_slug(&build.pipeline_slug)
        .await
        .ok();

    let job_resp = convert_job_to_response(
        &job,
        &build,
        pipeline.as_ref(),
        agent,
        &auth_agent.access_token,
    )?;
    Ok(Json(job_resp))
}

pub async fn accept_job(
    State(state): State<Arc<AppState>>,
    Path(job_id): Path<Uuid>,
    Extension(auth_agent): Extension<AuthenticatedAgent>,
) -> Result<impl IntoResponse, AppError> {
    let agent = &auth_agent.agent;
    let mut job = load_job_for_agent(&state, job_id, agent.id).await?;

    job.state = "accepted".into();
    state.db.update_job(&job).await?;

    let build = state.db.get_build_by_id(job.build_id).await?;
    let pipeline = state
        .db
        .get_pipeline_by_slug(&build.pipeline_slug)
        .await
        .ok();

    let job_resp = convert_job_to_response(
        &job,
        &build,
        pipeline.as_ref(),
        agent,
        &auth_agent.access_token,
    )?;
    Ok(Json(job_resp))
}

pub async fn start_job(
    State(state): State<Arc<AppState>>,
    Path(job_id): Path<Uuid>,
    Extension(mut agent): Extension<Agent>,
    Json(payload): Json<JobStartRequest>,
) -> Result<impl IntoResponse, AppError> {
    tracing::info!("Job {} started by agent {}", job_id, agent.name);
    let mut job = load_job_for_agent(&state, job_id, agent.id).await?;

    let started_at = match payload.started_at {
        Some(ts) => DateTime::parse_from_rfc3339(&ts)
            .map(|t| t.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now())
            .naive_utc(),
        None => Utc::now().naive_utc(),
    };

    job.state = "running".into();
    job.started_at = Some(started_at);
    state.db.update_job(&job).await?;

    agent.current_job_id = Some(job.id);
    state.db.update_agent(&agent).await?;

    Ok(StatusCode::OK)
}

pub async fn finish_job(
    State(state): State<Arc<AppState>>,
    Path(job_id): Path<Uuid>,
    Extension(mut agent): Extension<Agent>,
    Json(payload): Json<JobFinishRequest>,
) -> Result<impl IntoResponse, AppError> {
    tracing::info!(
        "Job {} finished by agent {} with exit_status: {:?}",
        job_id,
        agent.name,
        payload.exit_status
    );
    let mut job = load_job_for_agent(&state, job_id, agent.id).await?;

    let finished_at = match payload.finished_at {
        Some(ts) => DateTime::parse_from_rfc3339(&ts)
            .map(|t| t.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now())
            .naive_utc(),
        None => Utc::now().naive_utc(),
    };

    job.finished_at = Some(finished_at);
    job.exit_status = payload.exit_status.clone();
    job.signal = payload.signal.clone();
    job.signal_reason = payload.signal_reason.clone();
    job.chunks_failed_count = payload
        .chunks_failed_count
        .unwrap_or(job.chunks_failed_count);
    job.state = if payload.exit_status.as_deref() == Some("0") || payload.exit_status.is_none() {
        "finished".into()
    } else {
        "failed".into()
    };

    state.db.update_job(&job).await?;

    agent.current_job_id = None;
    state.db.update_agent(&agent).await?;

    state.dispatcher.check_dependent_jobs(&job).await?;
    update_build_status(&state.db, job.build_id, state.github.as_ref()).await?;

    Ok(StatusCode::OK)
}

pub async fn upload_chunk(
    State(state): State<Arc<AppState>>,
    Path(job_id): Path<Uuid>,
    Query(params): Query<UploadChunkParams>,
    Extension(agent): Extension<Agent>,
    body: axum::body::Bytes,
) -> Result<impl IntoResponse, AppError> {
    let _job = load_job_for_agent(&state, job_id, agent.id).await?;

    let body_len = body.len();
    tracing::info!(
        "upload_chunk: job={} agent={} sequence={} offset={} declared_size={} body_bytes={}",
        job_id,
        agent.name,
        params.sequence,
        params.offset,
        params.size,
        body_len,
    );

    if body_len == 0 {
        tracing::warn!(
            "upload_chunk: empty body for job {} sequence {}, skipping",
            job_id,
            params.sequence
        );
        return Ok(StatusCode::OK);
    }

    let data = match decode_chunk_body(body.as_ref()) {
        Ok(bytes) => bytes,
        Err(err) => {
            tracing::error!(
                "upload_chunk: failed to decode body for job {} sequence {}: {} (body_len={}, first16={:?})",
                job_id,
                params.sequence,
                err,
                body_len,
                body.iter().take(16).copied().collect::<Vec<_>>(),
            );
            return Err(AppError::Http(
                StatusCode::BAD_REQUEST,
                format!("Invalid chunk body: {err}"),
            ));
        }
    };

    let mut chunk = LogChunk {
        id: Uuid::new_v4(),
        job_id,
        sequence: params.sequence,
        offset: params.offset,
        size: params.size,
        data,
        created_at: Utc::now().naive_utc(),
    };

    state.db.create_log_chunk(&mut chunk).await?;

    Ok(StatusCode::OK)
}

fn decode_chunk_body(body: &[u8]) -> Result<Vec<u8>, String> {
    if looks_like_gzip(body) {
        let mut decoder = GzDecoder::new(body);
        let mut decompressed = Vec::with_capacity(body.len());
        decoder
            .read_to_end(&mut decompressed)
            .map_err(|e| format!("gzip decode failed: {e}"))?;
        Ok(decompressed)
    } else {
        // Some agents/proxies may already have decompressed the body, or send raw text.
        // Fall back to storing the raw bytes so logs are not silently dropped.
        Ok(body.to_vec())
    }
}

fn looks_like_gzip(body: &[u8]) -> bool {
    body.len() >= 2 && body[0] == 0x1f && body[1] == 0x8b
}

pub async fn metadata_exists(
    State(state): State<Arc<AppState>>,
    Path(job_id): Path<Uuid>,
    Extension(agent): Extension<Agent>,
    Json(payload): Json<MetadataExistsRequest>,
) -> Result<impl IntoResponse, AppError> {
    let job = load_job_for_agent(&state, job_id, agent.id).await?;
    let exists = state.db.metadata_exists(job.build_id, &payload.key).await?;
    tracing::info!(
        "metadata_exists: job={} agent={} build={} key={:?} exists={}",
        job_id,
        agent.name,
        job.build_id,
        payload.key,
        exists
    );
    Ok(Json(json!({ "exists": exists })))
}

pub async fn metadata_set(
    State(state): State<Arc<AppState>>,
    Path(job_id): Path<Uuid>,
    Extension(agent): Extension<Agent>,
    Json(payload): Json<MetadataSetRequest>,
) -> Result<impl IntoResponse, AppError> {
    let job = load_job_for_agent(&state, job_id, agent.id).await?;
    tracing::info!(
        "metadata_set: job={} agent={} build={} key={:?} value_len={}",
        job_id,
        agent.name,
        job.build_id,
        payload.key,
        payload.value.len()
    );

    state
        .db
        .set_metadata(job.build_id, &payload.key, &payload.value)
        .await?;
    Ok(StatusCode::OK)
}

pub async fn metadata_get(
    State(state): State<Arc<AppState>>,
    Path(job_id): Path<Uuid>,
    Extension(agent): Extension<Agent>,
    Json(payload): Json<MetadataGetRequest>,
) -> Result<impl IntoResponse, AppError> {
    let job = load_job_for_agent(&state, job_id, agent.id).await?;
    let value = state.db.get_metadata(job.build_id, &payload.key).await?;

    match value {
        Some(value) => {
            tracing::info!(
                "metadata_get: job={} agent={} build={} key={:?} value_len={}",
                job_id,
                agent.name,
                job.build_id,
                payload.key,
                value.len()
            );
            Ok((
                StatusCode::OK,
                Json(json!({ "key": payload.key, "value": value })),
            )
                .into_response())
        }
        None => {
            tracing::info!(
                "metadata_get: job={} agent={} build={} key={:?} -> not found",
                job_id,
                agent.name,
                job.build_id,
                payload.key
            );
            Ok((
                StatusCode::NOT_FOUND,
                Json(json!({
                    "key": payload.key,
                    "message": "Key not found",
                })),
            )
                .into_response())
        }
    }
}

pub async fn metadata_keys(
    State(state): State<Arc<AppState>>,
    Path(job_id): Path<Uuid>,
    Extension(agent): Extension<Agent>,
) -> Result<impl IntoResponse, AppError> {
    let job = load_job_for_agent(&state, job_id, agent.id).await?;
    let keys = state.db.get_metadata_keys(job.build_id).await?;
    tracing::info!(
        "metadata_keys: job={} agent={} build={} count={}",
        job_id,
        agent.name,
        job.build_id,
        keys.len()
    );
    Ok(Json(keys))
}

pub async fn upload_pipeline(
    State(state): State<Arc<AppState>>,
    Path(job_id): Path<Uuid>,
    Extension(agent): Extension<Agent>,
    Json(payload): Json<PipelineUploadRequest>,
) -> Result<impl IntoResponse, AppError> {
    tracing::info!(
        "Pipeline upload for job {} from agent {}",
        job_id,
        agent.name
    );
    let job = load_job_for_agent(&state, job_id, agent.id).await?;

    let (mut steps, pipeline_agents): (Vec<serde_json::Value>, Option<serde_json::Value>) =
        if let Some(pipeline_value) = &payload.pipeline {
            match pipeline_value {
                serde_json::Value::Object(obj) => {
                    let steps = if let Some(steps_value) = obj.get("steps") {
                        if let Some(steps_arr) = steps_value.as_array() {
                            steps_arr.clone()
                        } else {
                            return Err(AppError::Http(
                                StatusCode::BAD_REQUEST,
                                "pipeline.steps must be an array".into(),
                            ));
                        }
                    } else {
                        return Err(AppError::Http(
                            StatusCode::BAD_REQUEST,
                            "pipeline object must contain 'steps' array".into(),
                        ));
                    };
                    let agents = obj.get("agents").cloned();
                    if agents.is_some() {
                        tracing::info!("Pipeline has root-level agents config: {:?}", agents);
                    }
                    (steps, agents)
                }
                serde_json::Value::String(yaml_pipeline) => {
                    let pipeline_yaml: serde_yaml::Value = serde_yaml::from_str(yaml_pipeline)
                        .map_err(|e| {
                            AppError::Http(
                                StatusCode::BAD_REQUEST,
                                format!("invalid pipeline YAML: {}", e),
                            )
                        })?;
                    let pipeline_json = serde_json::to_value(pipeline_yaml).map_err(|e| {
                        AppError::Http(
                            StatusCode::BAD_REQUEST,
                            format!("invalid pipeline payload: {}", e),
                        )
                    })?;

                    let obj = pipeline_json.as_object().ok_or_else(|| {
                        AppError::Http(
                            StatusCode::BAD_REQUEST,
                            "pipeline YAML must decode to a JSON object".into(),
                        )
                    })?;

                    let steps = if let Some(steps_value) = obj.get("steps") {
                        if let Some(steps_arr) = steps_value.as_array() {
                            steps_arr.clone()
                        } else {
                            return Err(AppError::Http(
                                StatusCode::BAD_REQUEST,
                                "pipeline.steps must be an array".into(),
                            ));
                        }
                    } else {
                        return Err(AppError::Http(
                            StatusCode::BAD_REQUEST,
                            "pipeline object must contain 'steps' array".into(),
                        ));
                    };
                    let agents = obj.get("agents").cloned();
                    if agents.is_some() {
                        tracing::info!("Pipeline YAML has root-level agents config: {:?}", agents);
                    }
                    (steps, agents)
                }
                _ => {
                    return Err(AppError::Http(
                        StatusCode::BAD_REQUEST,
                        "pipeline must be a JSON object or YAML string".into(),
                    ));
                }
            }
        } else if let Some(steps) = payload.steps {
            (steps, None)
        } else {
            return Err(AppError::Http(
                StatusCode::BAD_REQUEST,
                "Either 'pipeline' or 'steps' must be provided".into(),
            ));
        };

    if let Some(ref default_agents) = pipeline_agents {
        for step in &mut steps {
            if let serde_json::Value::Object(ref mut step_obj) = step {
                if !step_obj.contains_key("agents") {
                    step_obj.insert("agents".to_string(), default_agents.clone());
                    tracing::debug!(
                        "Inherited pipeline agents into step: {:?}",
                        step_obj.get("label")
                    );
                }
            }
        }
    }

    if steps.is_empty() {
        tracing::info!("Empty pipeline uploaded, no jobs created");
        return Ok(StatusCode::OK);
    }

    let build = state.db.get_build_by_id(job.build_id).await?;

    let mut key_to_job_id: HashMap<String, Uuid> = HashMap::new();
    let mut created_jobs: Vec<(Uuid, serde_json::Value)> = Vec::new();
    let mut previous_job_ids: Vec<Uuid> = Vec::new();

    for step in &steps {
        if is_wait_step(step) {
            continue;
        }
        if is_block_step(step) {
            tracing::info!("Skipping block step (not implemented)");
            continue;
        }

        let parsed_step: PipelineStep = serde_json::from_value(step.clone()).unwrap_or_default();
        let new_job_id = Uuid::new_v4();

        if let Some(key) = &parsed_step.key {
            key_to_job_id.insert(key.clone(), new_job_id);
        }
        if let Some(label) = &parsed_step.label {
            key_to_job_id.entry(label.clone()).or_insert(new_job_id);
        }

        created_jobs.push((new_job_id, step.clone()));
        previous_job_ids.push(new_job_id);
    }

    let mut wait_dependencies: Vec<Uuid> = Vec::new();
    let mut jobs_before_wait: Vec<Uuid> = Vec::new();

    for step in &steps {
        if is_wait_step(step) {
            wait_dependencies.extend(jobs_before_wait.clone());
            jobs_before_wait.clear();
            continue;
        }
        if is_block_step(step) {
            continue;
        }

        let parsed_step: PipelineStep = serde_json::from_value(step.clone()).unwrap_or_default();

        let job_id = if let Some(key) = &parsed_step.key {
            key_to_job_id.get(key).copied()
        } else if let Some(label) = &parsed_step.label {
            key_to_job_id.get(label).copied()
        } else {
            created_jobs
                .iter()
                .find(|(_, s)| s == step)
                .map(|(id, _)| *id)
        };

        let Some(new_job_id) = job_id else {
            tracing::warn!("Could not find job_id for step, skipping");
            continue;
        };

        let mut depends_on_ids: Vec<Uuid> = Vec::new();
        depends_on_ids.extend(wait_dependencies.clone());

        if let Some(deps) = &parsed_step.depends_on {
            let dep_keys = parse_depends_on_field(deps);
            for dep_key in dep_keys {
                if let Some(dep_job_id) = key_to_job_id.get(&dep_key) {
                    depends_on_ids.push(*dep_job_id);
                } else {
                    tracing::warn!("Unknown dependency key: {}", dep_key);
                }
            }
        }

        depends_on_ids.sort();
        depends_on_ids.dedup();

        let runnable_at = if depends_on_ids.is_empty() {
            Some(Utc::now().naive_utc())
        } else {
            None
        };

        let state_str = if depends_on_ids.is_empty() {
            "scheduled"
        } else {
            "waiting"
        };

        let mut new_job = Job {
            id: new_job_id,
            build_id: build.id,
            step_config: step.clone(),
            state: state_str.to_string(),
            agent_id: None,
            job_token: None,
            env: Some(serde_json::json!({})),
            depends_on: if depends_on_ids.is_empty() {
                None
            } else {
                Some(depends_on_ids)
            },
            exit_status: None,
            signal: None,
            signal_reason: None,
            started_at: None,
            finished_at: None,
            runnable_at,
            chunks_failed_count: 0,
            trace_parent: None,
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        };

        state.db.create_job(&mut new_job).await?;
        tracing::info!(
            "Created job {} with label: {:?}",
            new_job.id,
            parsed_step.label
        );

        jobs_before_wait.push(new_job_id);
    }

    tracing::info!(
        "Pipeline upload complete: created {} jobs for build {}",
        created_jobs.len(),
        build.id
    );

    Ok(StatusCode::OK)
}

fn is_wait_step(step: &serde_json::Value) -> bool {
    if let Some(s) = step.as_str() {
        return s == "wait";
    }
    if let Some(obj) = step.as_object() {
        return obj.contains_key("wait");
    }
    false
}

fn is_block_step(step: &serde_json::Value) -> bool {
    if let Some(s) = step.as_str() {
        return s == "block";
    }
    if let Some(obj) = step.as_object() {
        return obj.contains_key("block");
    }
    false
}

fn parse_depends_on_field(deps: &serde_json::Value) -> Vec<String> {
    match deps {
        serde_json::Value::String(s) => vec![s.clone()],
        serde_json::Value::Array(arr) => arr
            .iter()
            .filter_map(|v| match v {
                serde_json::Value::String(s) => Some(s.clone()),
                serde_json::Value::Object(obj) => {
                    obj.get("step").and_then(|s| s.as_str()).map(String::from)
                }
                _ => None,
            })
            .collect(),
        _ => vec![],
    }
}
