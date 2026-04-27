use std::sync::Arc;

use axum::{
    extract::{OriginalUri, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use chrono::{DateTime, Utc};

use crate::{
    db::Build,
    error::AppError,
    types::{BuildFilterParams, BuildWithJobsResponse, JobSummary},
    AppState,
};

use super::{
    get_authenticated_user, get_user_and_org_by_slug, paginate_params, paginated_response,
    update_build_status,
};

fn build_to_response(build: Build, job_summaries: Vec<JobSummary>) -> BuildWithJobsResponse {
    BuildWithJobsResponse {
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
        started_at: build
            .started_at
            .map(|t| DateTime::<Utc>::from_naive_utc_and_offset(t, Utc).to_rfc3339()),
        finished_at: build
            .finished_at
            .map(|t| DateTime::<Utc>::from_naive_utc_and_offset(t, Utc).to_rfc3339()),
        jobs: job_summaries,
    }
}

async fn builds_to_responses(
    state: &Arc<AppState>,
    builds: Vec<Build>,
) -> Result<Vec<BuildWithJobsResponse>, AppError> {
    let mut result = Vec::with_capacity(builds.len());
    for build in builds {
        for b in [&build] {
            if b.status == "running" || b.status == "scheduled" {
                update_build_status(&state.db, b.id, state.github.as_ref()).await?;
            }
        }
        let jobs = state.db.get_jobs_by_build_id(build.id).await?;
        let job_summaries: Vec<JobSummary> = jobs
            .into_iter()
            .map(|j| {
                let label = j
                    .step_config
                    .get("label")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                JobSummary {
                    id: j.id,
                    state: j.state,
                    exit_status: j.exit_status,
                    label,
                    started_at: j
                        .started_at
                        .map(|t| DateTime::<Utc>::from_naive_utc_and_offset(t, Utc).to_rfc3339()),
                    finished_at: j
                        .finished_at
                        .map(|t| DateTime::<Utc>::from_naive_utc_and_offset(t, Utc).to_rfc3339()),
                }
            })
            .collect();
        result.push(build_to_response(build, job_summaries));
    }
    Ok(result)
}

pub async fn list_org_builds(
    State(state): State<Arc<AppState>>,
    Path(org_slug): Path<String>,
    Query(params): Query<BuildFilterParams>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let (_user, org_id) = get_user_and_org_by_slug(&state, &headers, &org_slug).await?;

    let filter = params.to_filter();
    let pagination = params.to_pagination();
    let (page, per_page, limit, offset) = paginate_params(&pagination);
    let total = state
        .db
        .count_builds_for_org_filtered(org_id, &filter)
        .await?;
    let builds = state
        .db
        .get_builds_for_org_filtered(org_id, &filter, limit, offset)
        .await?;

    let responses = builds_to_responses(&state, builds).await?;
    Ok(paginated_response(
        responses,
        page,
        per_page,
        total,
        uri.path(),
    ))
}

pub async fn list_all_builds(
    State(state): State<Arc<AppState>>,
    Query(params): Query<BuildFilterParams>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let user = get_authenticated_user(&state, &headers)
        .await
        .map_err(|_| AppError::Http(StatusCode::UNAUTHORIZED, "Not authenticated".into()))?;

    let filter = params.to_filter();
    let pagination = params.to_pagination();
    let (page, per_page, limit, offset) = paginate_params(&pagination);
    let total = state
        .db
        .count_all_builds_filtered(user.id, &filter)
        .await?;
    let builds = state
        .db
        .get_all_builds_filtered(user.id, &filter, limit, offset)
        .await?;

    let responses = builds_to_responses(&state, builds).await?;
    Ok(paginated_response(
        responses,
        page,
        per_page,
        total,
        uri.path(),
    ))
}
