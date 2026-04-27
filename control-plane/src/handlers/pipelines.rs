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
    db::{Build, Job},
    error::AppError,
    types::{
        BuildCreateRequest, BuildFilterParams, BuildSummary, BuildWithJobsResponse,
        JobLogResponse, JobSummary, PaginationParams, PipelineCreateRequest,
        PipelineDetailResponse, PipelineQueueInfo, PipelineResponse, PipelineStatsResponse,
        PipelineUpdateRequest, PipelineWithStatsResponse,
    },
    AppState,
};

use super::{get_user_and_org_by_slug, paginate_params, paginated_response, update_build_status};

async fn reconcile_pipeline_build_statuses(
    state: &Arc<AppState>,
    pipeline_slug: &str,
) -> Result<(), AppError> {
    let recent_builds = state.db.get_builds_for_pipeline(pipeline_slug, 100).await?;
    for build in recent_builds
        .iter()
        .filter(|b| b.status == "running" || b.status == "scheduled")
    {
        update_build_status(&state.db, build.id, state.github.as_ref()).await?;
    }
    Ok(())
}

pub async fn list_user_pipelines(
    State(state): State<Arc<AppState>>,
    Path(org_slug): Path<String>,
    Query(pagination): Query<PaginationParams>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let (_user, org_id) = get_user_and_org_by_slug(&state, &headers, &org_slug).await?;
    let (page, per_page, limit, offset) = paginate_params(&pagination);
    let total = state.db.count_pipelines_by_organization(org_id).await?;
    let pipelines = state.db.get_pipelines_by_organization_paginated(org_id, limit, offset).await?;

    let mut response: Vec<PipelineWithStatsResponse> = Vec::new();

    for pipeline in pipelines {
        reconcile_pipeline_build_statuses(&state, &pipeline.slug).await?;
        let stats = state.db.get_pipeline_stats(&pipeline.slug).await?;
        let recent_builds = state.db.get_builds_for_pipeline(&pipeline.slug, 10).await?;

        let recent_builds_summary: Vec<BuildSummary> = recent_builds
            .into_iter()
            .map(|b| BuildSummary {
                id: b.id,
                number: b.number,
                commit: b.commit,
                branch: b.branch,
                message: b.message,
                author_name: b.author_name,
                state: b.status,
                source: b.source,
                created_at: DateTime::<Utc>::from_naive_utc_and_offset(b.created_at, Utc).to_rfc3339(),
                started_at: b.started_at.map(|t| DateTime::<Utc>::from_naive_utc_and_offset(t, Utc).to_rfc3339()),
                finished_at: b.finished_at.map(|t| DateTime::<Utc>::from_naive_utc_and_offset(t, Utc).to_rfc3339()),
            })
            .collect();

        let queues = state.db.get_queues_by_pipeline(pipeline.id).await.unwrap_or_default();
        let queues_info: Vec<PipelineQueueInfo> = queues
            .into_iter()
            .map(|q| PipelineQueueInfo { id: q.id, name: q.name, key: q.key })
            .collect();

        response.push(PipelineWithStatsResponse {
            id: pipeline.id,
            slug: pipeline.slug,
            name: pipeline.name.unwrap_or_default(),
            description: pipeline.description,
            repository_url: pipeline.repository_url,
            default_branch: pipeline.default_branch,
            created_at: DateTime::<Utc>::from_naive_utc_and_offset(pipeline.created_at, Utc).to_rfc3339(),
            updated_at: DateTime::<Utc>::from_naive_utc_and_offset(pipeline.updated_at, Utc).to_rfc3339(),
            total_builds: stats.total_builds,
            passed_builds: stats.passed_builds,
            failed_builds: stats.failed_builds,
            running_builds: stats.running_builds,
            avg_duration_seconds: stats.avg_duration_seconds,
            recent_builds: recent_builds_summary,
            queues: queues_info,
        });
    }

    Ok(paginated_response(response, page, per_page, total, uri.path()))
}

pub async fn get_user_pipeline(
    State(state): State<Arc<AppState>>,
    Path((org_slug, pipeline_slug)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let (_user, org_id) = get_user_and_org_by_slug(&state, &headers, &org_slug).await?;
    let pipeline = state.db.get_pipeline_by_slug_and_org(&pipeline_slug, org_id).await
        .map_err(|_| AppError::Http(StatusCode::NOT_FOUND, "Pipeline not found".into()))?;

    reconcile_pipeline_build_statuses(&state, &pipeline.slug).await?;
    let stats = state.db.get_pipeline_stats(&pipeline.slug).await?;
    let builds = state.db.get_builds_for_pipeline(&pipeline.slug, 50).await?;

    let mut builds_with_jobs: Vec<BuildWithJobsResponse> = Vec::new();
    for build in builds {
        let jobs = state.db.get_jobs_by_build_id(build.id).await?;
        let job_summaries: Vec<JobSummary> = jobs
            .into_iter()
            .map(|j| {
                let label = j.step_config.get("label").and_then(|v| v.as_str()).map(String::from);
                JobSummary {
                    id: j.id,
                    state: j.state,
                    exit_status: j.exit_status,
                    label,
                    started_at: j.started_at.map(|t| DateTime::<Utc>::from_naive_utc_and_offset(t, Utc).to_rfc3339()),
                    finished_at: j.finished_at.map(|t| DateTime::<Utc>::from_naive_utc_and_offset(t, Utc).to_rfc3339()),
                }
            })
            .collect();

        builds_with_jobs.push(BuildWithJobsResponse {
            id: build.id,
            number: build.number,
            pipeline_slug: build.pipeline_slug,
            commit: build.commit,
            branch: build.branch,
            message: build.message,
            author_name: build.author_name,
            author_email: build.author_email,
            state: build.status,
            source: build.source,
            created_at: DateTime::<Utc>::from_naive_utc_and_offset(build.created_at, Utc).to_rfc3339(),
            started_at: build.started_at.map(|t| DateTime::<Utc>::from_naive_utc_and_offset(t, Utc).to_rfc3339()),
            finished_at: build.finished_at.map(|t| DateTime::<Utc>::from_naive_utc_and_offset(t, Utc).to_rfc3339()),
            jobs: job_summaries,
        });
    }

    let response = PipelineDetailResponse {
        pipeline: PipelineResponse {
            id: pipeline.id,
            slug: pipeline.slug,
            name: pipeline.name.unwrap_or_default(),
            description: pipeline.description,
            repository_url: pipeline.repository_url,
            default_branch: pipeline.default_branch,
            created_at: DateTime::<Utc>::from_naive_utc_and_offset(pipeline.created_at, Utc).to_rfc3339(),
            updated_at: DateTime::<Utc>::from_naive_utc_and_offset(pipeline.updated_at, Utc).to_rfc3339(),
        },
        stats: PipelineStatsResponse {
            total_builds: stats.total_builds,
            passed_builds: stats.passed_builds,
            failed_builds: stats.failed_builds,
            running_builds: stats.running_builds,
            avg_duration_seconds: stats.avg_duration_seconds,
        },
        builds: builds_with_jobs,
    };

    Ok(Json(response))
}

pub async fn create_user_pipeline(
    State(state): State<Arc<AppState>>,
    Path(org_slug): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<PipelineCreateRequest>,
) -> Result<impl IntoResponse, AppError> {
    let (user, org_id) = get_user_and_org_by_slug(&state, &headers, &org_slug).await?;

    if payload.name.is_empty() || payload.slug.is_empty() || payload.repository_url.is_empty() {
        return Err(AppError::Http(StatusCode::BAD_REQUEST, "Name, slug, and repository URL are required".into()));
    }
    if !payload.slug.chars().all(|c| c.is_alphanumeric() || c == '-') {
        return Err(AppError::Http(StatusCode::BAD_REQUEST, "Slug must contain only alphanumeric characters and hyphens".into()));
    }
    if state.db.get_pipeline_by_slug(&payload.slug).await.is_ok() {
        return Err(AppError::Http(StatusCode::CONFLICT, "A pipeline with this slug already exists".into()));
    }

    let pipeline = state.db
        .create_pipeline_for_user(user.id, Some(org_id), &payload.slug, &payload.name, &payload.repository_url, payload.description.as_deref(), payload.default_branch.as_deref())
        .await?;

    tracing::info!("Pipeline created: {} (id: {})", pipeline.slug, pipeline.id);

    let response = PipelineResponse {
        id: pipeline.id,
        slug: pipeline.slug,
        name: pipeline.name.unwrap_or_default(),
        description: pipeline.description,
        repository_url: pipeline.repository_url,
        default_branch: pipeline.default_branch,
        created_at: DateTime::<Utc>::from_naive_utc_and_offset(pipeline.created_at, Utc).to_rfc3339(),
        updated_at: DateTime::<Utc>::from_naive_utc_and_offset(pipeline.updated_at, Utc).to_rfc3339(),
    };

    Ok((StatusCode::CREATED, Json(response)))
}

pub async fn update_user_pipeline(
    State(state): State<Arc<AppState>>,
    Path((org_slug, pipeline_slug)): Path<(String, String)>,
    headers: HeaderMap,
    Json(payload): Json<PipelineUpdateRequest>,
) -> Result<impl IntoResponse, AppError> {
    let (_user, org_id) = get_user_and_org_by_slug(&state, &headers, &org_slug).await?;

    if payload.name.is_empty() || payload.repository_url.is_empty() {
        return Err(AppError::Http(StatusCode::BAD_REQUEST, "Name and repository URL are required".into()));
    }

    let pipeline = state.db.get_pipeline_by_slug_and_org(&pipeline_slug, org_id).await
        .map_err(|_| AppError::Http(StatusCode::NOT_FOUND, "Pipeline not found".into()))?;
    let pipeline = state.db
        .update_pipeline_by_org(pipeline.id, org_id, &payload.name, payload.description.as_deref(), &payload.repository_url, payload.default_branch.as_deref())
        .await?;

    tracing::info!("Pipeline updated: {} (id: {})", pipeline.slug, pipeline.id);

    let response = PipelineResponse {
        id: pipeline.id,
        slug: pipeline.slug,
        name: pipeline.name.unwrap_or_default(),
        description: pipeline.description,
        repository_url: pipeline.repository_url,
        default_branch: pipeline.default_branch,
        created_at: DateTime::<Utc>::from_naive_utc_and_offset(pipeline.created_at, Utc).to_rfc3339(),
        updated_at: DateTime::<Utc>::from_naive_utc_and_offset(pipeline.updated_at, Utc).to_rfc3339(),
    };

    Ok(Json(response))
}

pub async fn delete_user_pipeline(
    State(state): State<Arc<AppState>>,
    Path((org_slug, pipeline_slug)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let (_user, org_id) = get_user_and_org_by_slug(&state, &headers, &org_slug).await?;
    let pipeline = state.db.get_pipeline_by_slug_and_org(&pipeline_slug, org_id).await
        .map_err(|_| AppError::Http(StatusCode::NOT_FOUND, "Pipeline not found".into()))?;

    let queues = state.db.get_queues_by_pipeline(pipeline.id).await?;
    for queue in queues {
        if queue.is_default {
            state.db.delete_default_queue_for_pipeline(queue.id).await?;
            tracing::info!("Deleted default queue {} for pipeline {}", queue.id, pipeline.id);
        }
    }

    let deleted = state.db.delete_pipeline_by_org(pipeline.id, org_id).await?;
    if !deleted {
        return Err(AppError::Http(StatusCode::NOT_FOUND, "Pipeline not found".into()));
    }

    tracing::info!("Pipeline deleted: {} (slug: {})", pipeline.id, pipeline_slug);
    Ok(Json(json!({ "message": "Pipeline deleted successfully" })))
}

pub async fn get_pipeline_builds(
    State(state): State<Arc<AppState>>,
    Path((org_slug, pipeline_slug)): Path<(String, String)>,
    Query(params): Query<BuildFilterParams>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let (_user, org_id) = get_user_and_org_by_slug(&state, &headers, &org_slug).await?;
    let pipeline = state.db.get_pipeline_by_slug_and_org(&pipeline_slug, org_id).await
        .map_err(|_| AppError::Http(StatusCode::NOT_FOUND, "Pipeline not found".into()))?;

    reconcile_pipeline_build_statuses(&state, &pipeline.slug).await?;
    let filter = params.to_filter();
    let pagination = params.to_pagination();
    let (page, per_page, limit, offset) = paginate_params(&pagination);
    let total = state.db.count_builds_filtered(&pipeline.slug, &filter).await?;
    let builds = state.db.get_builds_filtered(&pipeline.slug, &filter, limit, offset).await?;

    let mut builds_with_jobs: Vec<BuildWithJobsResponse> = Vec::new();
    for build in builds {
        let jobs = state.db.get_jobs_by_build_id(build.id).await?;
        let job_summaries: Vec<JobSummary> = jobs
            .into_iter()
            .map(|j| {
                let label = j.step_config.get("label").and_then(|v| v.as_str()).map(String::from);
                JobSummary {
                    id: j.id,
                    state: j.state,
                    exit_status: j.exit_status,
                    label,
                    started_at: j.started_at.map(|t| DateTime::<Utc>::from_naive_utc_and_offset(t, Utc).to_rfc3339()),
                    finished_at: j.finished_at.map(|t| DateTime::<Utc>::from_naive_utc_and_offset(t, Utc).to_rfc3339()),
                }
            })
            .collect();

        builds_with_jobs.push(BuildWithJobsResponse {
            id: build.id,
            number: build.number,
            pipeline_slug: build.pipeline_slug,
            commit: build.commit,
            branch: build.branch,
            message: build.message,
            author_name: build.author_name,
            author_email: build.author_email,
            state: build.status,
            source: build.source,
            created_at: DateTime::<Utc>::from_naive_utc_and_offset(build.created_at, Utc).to_rfc3339(),
            started_at: build.started_at.map(|t| DateTime::<Utc>::from_naive_utc_and_offset(t, Utc).to_rfc3339()),
            finished_at: build.finished_at.map(|t| DateTime::<Utc>::from_naive_utc_and_offset(t, Utc).to_rfc3339()),
            jobs: job_summaries,
        });
    }

    Ok(paginated_response(builds_with_jobs, page, per_page, total, uri.path()))
}

pub async fn get_build(
    State(state): State<Arc<AppState>>,
    Path((org_slug, pipeline_slug, number)): Path<(String, String, i32)>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let (_user, org_id) = get_user_and_org_by_slug(&state, &headers, &org_slug).await?;
    state.db.get_pipeline_by_slug_and_org(&pipeline_slug, org_id).await
        .map_err(|_| AppError::Http(StatusCode::NOT_FOUND, "Pipeline not found".into()))?;

    let build = state.db.get_build_by_pipeline_slug_and_number(&pipeline_slug, number).await
        .map_err(|_| AppError::Http(StatusCode::NOT_FOUND, "Build not found".into()))?;

    update_build_status(&state.db, build.id, state.github.as_ref()).await?;
    let build = state.db.get_build_by_id(build.id).await?;

    let jobs = state.db.get_jobs_by_build_id(build.id).await?;
    let job_summaries: Vec<JobSummary> = jobs
        .into_iter()
        .map(|j| {
            let label = j.step_config.get("label").and_then(|v| v.as_str()).map(String::from);
            JobSummary {
                id: j.id,
                state: j.state,
                exit_status: j.exit_status,
                label,
                started_at: j.started_at.map(|t| DateTime::<Utc>::from_naive_utc_and_offset(t, Utc).to_rfc3339()),
                finished_at: j.finished_at.map(|t| DateTime::<Utc>::from_naive_utc_and_offset(t, Utc).to_rfc3339()),
            }
        })
        .collect();

    let response = BuildWithJobsResponse {
        id: build.id,
        number: build.number,
        pipeline_slug: build.pipeline_slug,
        commit: build.commit,
        branch: build.branch,
        message: build.message,
        author_name: build.author_name,
        author_email: build.author_email,
        state: build.status,
        source: build.source,
        created_at: DateTime::<Utc>::from_naive_utc_and_offset(build.created_at, Utc).to_rfc3339(),
        started_at: build.started_at.map(|t| DateTime::<Utc>::from_naive_utc_and_offset(t, Utc).to_rfc3339()),
        finished_at: build.finished_at.map(|t| DateTime::<Utc>::from_naive_utc_and_offset(t, Utc).to_rfc3339()),
        jobs: job_summaries,
    };

    Ok(Json(response))
}

pub async fn create_build(
    State(state): State<Arc<AppState>>,
    Path((org_slug, pipeline_slug)): Path<(String, String)>,
    headers: HeaderMap,
    Json(payload): Json<BuildCreateRequest>,
) -> Result<impl IntoResponse, AppError> {
    let (_user, org_id) = get_user_and_org_by_slug(&state, &headers, &org_slug).await?;
    let pipeline = state
        .db
        .get_pipeline_by_slug_and_org(&pipeline_slug, org_id)
        .await
        .map_err(|_| AppError::Http(StatusCode::NOT_FOUND, "Pipeline not found".into()))?;

    if payload.commit.is_empty() {
        return Err(AppError::Http(
            StatusCode::BAD_REQUEST,
            "commit is required".into(),
        ));
    }
    if payload.branch.is_empty() {
        return Err(AppError::Http(
            StatusCode::BAD_REQUEST,
            "branch is required".into(),
        ));
    }

    let build_number = state.db.get_next_build_number(&pipeline.slug).await?;

    let now = Utc::now().naive_utc();
    let mut build = Build {
        id: uuid::Uuid::new_v4(),
        number: build_number,
        pipeline_slug: pipeline.slug.clone(),
        commit: payload.commit,
        branch: payload.branch,
        tag: None,
        message: payload.message,
        author_name: payload.author.as_ref().and_then(|a| a.name.clone()),
        author_email: payload.author.as_ref().and_then(|a| a.email.clone()),
        status: "scheduled".to_string(),
        webhook_payload: None,
        created_at: now,
        started_at: None,
        finished_at: None,
        pull_request_number: None,
        source: "api".to_string(),
    };

    state.db.create_build(&mut build).await?;

    if let Some(ref meta) = payload.meta_data {
        for (key, value) in meta {
            state.db.set_metadata(build.id, key, value).await?;
        }
    }

    let pipeline_upload_step = serde_json::json!({
        "label": ":pipeline: Pipeline Upload",
        "command": "buildkite-agent pipeline upload",
        "key": "pipeline-upload"
    });

    let job_env = match payload.env {
        Some(ref env) => serde_json::to_value(env).unwrap_or_else(|_| serde_json::json!({})),
        None => serde_json::json!({}),
    };

    let mut job = Job {
        id: uuid::Uuid::new_v4(),
        build_id: build.id,
        step_config: pipeline_upload_step,
        state: "scheduled".to_string(),
        agent_id: None,
        job_token: None,
        env: Some(job_env),
        depends_on: None,
        exit_status: None,
        signal: None,
        signal_reason: None,
        started_at: None,
        finished_at: None,
        runnable_at: Some(now),
        chunks_failed_count: 0,
        trace_parent: None,
        created_at: now,
        updated_at: now,
    };

    state.db.create_job(&mut job).await?;

    if let Some(ref gh) = state.github {
        crate::github::notify_build_status(gh, &state.db, &build).await;
    }

    tracing::info!(
        "Build #{} created via API for pipeline '{}' (commit: {}, branch: {})",
        build.number,
        pipeline.slug,
        build.commit,
        build.branch
    );

    let job_summaries = vec![JobSummary {
        id: job.id,
        state: job.state,
        exit_status: job.exit_status,
        label: job
            .step_config
            .get("label")
            .and_then(|v| v.as_str())
            .map(String::from),
        started_at: None,
        finished_at: None,
    }];

    let response = BuildWithJobsResponse {
        id: build.id,
        number: build.number,
        pipeline_slug: build.pipeline_slug,
        commit: build.commit,
        branch: build.branch,
        message: build.message,
        author_name: build.author_name,
        author_email: build.author_email,
        state: build.status,
        source: build.source,
        created_at: DateTime::<Utc>::from_naive_utc_and_offset(build.created_at, Utc).to_rfc3339(),
        started_at: None,
        finished_at: None,
        jobs: job_summaries,
    };

    Ok((StatusCode::CREATED, Json(response)))
}

pub async fn get_job_log(
    State(state): State<Arc<AppState>>,
    Path((org_slug, pipeline_slug, number, job_id)): Path<(String, String, i32, Uuid)>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let (_user, org_id) = get_user_and_org_by_slug(&state, &headers, &org_slug).await?;
    state.db.get_pipeline_by_slug_and_org(&pipeline_slug, org_id).await
        .map_err(|_| AppError::Http(StatusCode::NOT_FOUND, "Pipeline not found".into()))?;

    let build = state.db.get_build_by_pipeline_slug_and_number(&pipeline_slug, number).await
        .map_err(|_| AppError::Http(StatusCode::NOT_FOUND, "Build not found".into()))?;

    let job = state.db.get_job_by_id(job_id).await
        .map_err(|_| AppError::Http(StatusCode::NOT_FOUND, "Job not found".into()))?;

    if job.build_id != build.id {
        return Err(AppError::Http(StatusCode::NOT_FOUND, "Job not found in this build".into()));
    }

    let chunks = state.db.get_log_chunks_for_job(job.id).await?;
    let content: String = chunks
        .into_iter()
        .map(|chunk| String::from_utf8_lossy(&chunk.data).to_string())
        .collect::<Vec<_>>()
        .join("");
    let size = content.len();

    Ok(Json(JobLogResponse {
        content,
        size,
        header_times: vec![],
    }))
}
