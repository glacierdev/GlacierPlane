use std::sync::Arc;

use axum::{
    body::Bytes,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use chrono::Utc;
use subtle::ConstantTimeEq;
use uuid::Uuid;

use crate::{
    db::{Build, Job},
    error::AppError,
    types::GitHubWebhookPayload,
    AppState,
};

const PR_TRIGGER_ACTIONS: &[&str] = &["opened", "synchronize", "reopened"];

struct BuildContext {
    pipeline_slug: String,
    build_number: i32,
    webhook_payload: Option<serde_json::Value>,
}

struct BuildSeed {
    commit: String,
    branch: String,
    tag: Option<String>,
    message: Option<String>,
    author_name: Option<String>,
    author_email: Option<String>,
    pull_request_number: Option<i32>,
}

pub async fn handle_github(
    State(state): State<Arc<AppState>>,
    Path(url_secret): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<impl IntoResponse, AppError> {
    if !state.webhook_secret.is_empty()
        && !bool::from(
            url_secret
                .as_bytes()
                .ct_eq(state.webhook_secret.as_bytes()),
        )
    {
        tracing::warn!("Invalid webhook URL secret provided");
        return Err(AppError::Http(
            StatusCode::UNAUTHORIZED,
            "Invalid secret".into(),
        ));
    }

    let event_type = headers
        .get("X-GitHub-Event")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown");

    tracing::info!("Received GitHub webhook event: {}", event_type);

    if event_type == "ping" {
        tracing::info!("Responding to GitHub ping event");
        return Ok(StatusCode::OK);
    }

    if event_type != "push" && event_type != "pull_request" {
        tracing::info!("Ignoring GitHub event type: {}", event_type);
        return Ok(StatusCode::OK);
    }

    let payload: GitHubWebhookPayload = serde_json::from_slice(&body)?;

    if event_type == "pull_request" {
        return handle_pull_request_event(&state, &payload, &body).await;
    }

    handle_push_event(&state, &payload, &body).await
}

async fn handle_push_event(
    state: &Arc<AppState>,
    payload: &GitHubWebhookPayload,
    body: &[u8],
) -> Result<StatusCode, AppError> {
    let null_commit = "0000000000000000000000000000000000000000";
    if payload.deleted || payload.after.as_deref() == Some(null_commit) {
        tracing::info!(
            "Ignoring branch deletion push event (ref: {:?})",
            payload.r#ref
        );
        return Ok(StatusCode::OK);
    }

    let head_commit_message = payload
        .head_commit
        .as_ref()
        .map(|c| &c.message)
        .or_else(|| payload.commits.last().map(|c| &c.message));

    if skip_ci_requested(head_commit_message.map(|s| s.as_str())) {
        return Ok(StatusCode::OK);
    }

    let commit = payload
        .after
        .clone()
        .ok_or_else(|| AppError::Http(StatusCode::BAD_REQUEST, "No commit found".into()))?;

    let ref_str = payload.r#ref.as_deref().unwrap_or("");
    let (branch, tag) = if let Some(tag_name) = ref_str.strip_prefix("refs/tags/") {
        tracing::info!("Tag push detected: {}", tag_name);
        ("main".to_string(), Some(tag_name.to_string()))
    } else if let Some(branch_name) = ref_str.strip_prefix("refs/heads/") {
        (branch_name.to_string(), None)
    } else {
        ("main".to_string(), None)
    };

    let context = prepare_build_context(state, payload, body, "Push").await?;
    let seed = BuildSeed {
        commit,
        branch,
        tag,
        message: payload.commits.first().map(|c| c.message.clone()),
        author_name: payload.commits.first().map(|c| c.author.name.clone()),
        author_email: payload.commits.first().map(|c| c.author.email.clone()),
        pull_request_number: None,
    };
    let mut build = build_from_seed(context, seed);

    create_build_with_upload_job(state, &mut build).await?;
    Ok(StatusCode::OK)
}

async fn handle_pull_request_event(
    state: &Arc<AppState>,
    payload: &GitHubWebhookPayload,
    body: &[u8],
) -> Result<StatusCode, AppError> {
    let action = payload.action.as_deref().unwrap_or("unknown");
    if !PR_TRIGGER_ACTIONS.contains(&action) {
        tracing::info!("Ignoring pull_request action: {}", action);
        return Ok(StatusCode::OK);
    }

    let pr = payload.pull_request.as_ref().ok_or_else(|| {
        AppError::Http(StatusCode::BAD_REQUEST, "Missing pull_request object".into())
    })?;

    if skip_ci_requested(pr.title.as_deref()) {
        return Ok(StatusCode::OK);
    }

    let commit = pr.head.sha.clone();
    let branch = pr.head.r#ref.clone();
    let pr_number = payload.number.or(pr.number).unwrap_or(0) as i32;

    tracing::info!(
        "Pull request #{} (action: {}) — branch: {}, commit: {}",
        pr_number, action, branch, commit
    );

    let message = pr
        .title
        .clone()
        .unwrap_or_else(|| format!("Pull request #{}", pr_number));

    let context = prepare_build_context(state, payload, body, "PR").await?;
    let seed = BuildSeed {
        commit,
        branch,
        tag: None,
        message: Some(message),
        author_name: pr.user.as_ref().map(|u| u.login.clone()),
        author_email: None,
        pull_request_number: Some(pr_number),
    };
    let mut build = build_from_seed(context, seed);

    create_build_with_upload_job(state, &mut build).await?;
    Ok(StatusCode::OK)
}

fn skip_ci_requested(text: Option<&str>) -> bool {
    if let Some(msg) = text {
        let lower = msg.to_lowercase();
        if lower.contains("[skip ci]") || lower.contains("[ci skip]") {
            tracing::info!("Skipping build creation: skip CI directive detected");
            return true;
        }
    }
    false
}

fn repository_url(payload: &GitHubWebhookPayload) -> String {
    payload
        .repository
        .ssh_url
        .clone()
        .unwrap_or_else(|| payload.repository.clone_url.clone())
}

async fn prepare_build_context(
    state: &Arc<AppState>,
    payload: &GitHubWebhookPayload,
    body: &[u8],
    event_name: &str,
) -> Result<BuildContext, AppError> {
    let repo_url = repository_url(payload);
    let pipeline = find_pipeline(state, &repo_url).await?;
    tracing::info!(
        "{} webhook matched pipeline '{}' (id: {})",
        event_name,
        pipeline.slug,
        pipeline.id
    );

    let build_number = state.db.get_next_build_number(&pipeline.slug).await?;
    let webhook_payload = serde_json::from_slice::<serde_json::Value>(body).ok();

    Ok(BuildContext {
        pipeline_slug: pipeline.slug,
        build_number,
        webhook_payload,
    })
}

fn build_from_seed(context: BuildContext, seed: BuildSeed) -> Build {
    Build {
        id: Uuid::new_v4(),
        number: context.build_number,
        pipeline_slug: context.pipeline_slug,
        commit: seed.commit,
        branch: seed.branch,
        tag: seed.tag,
        message: seed.message,
        author_name: seed.author_name,
        author_email: seed.author_email,
        status: "scheduled".to_string(),
        webhook_payload: context.webhook_payload,
        created_at: Utc::now().naive_utc(),
        started_at: None,
        finished_at: None,
        pull_request_number: seed.pull_request_number,
        source: "webhook".to_string(),
    }
}

async fn find_pipeline(
    state: &Arc<AppState>,
    repo_url: &str,
) -> Result<crate::db::Pipeline, AppError> {
    state
        .db
        .find_pipeline_by_repo_url(repo_url)
        .await
        .map_err(|_| {
            tracing::warn!("No pipeline found for repository: {}", repo_url);
            AppError::Http(
                StatusCode::NOT_FOUND,
                format!(
                    "No pipeline configured for repository: {}. Please create a pipeline in the UI first.",
                    repo_url
                ),
            )
        })
}

async fn create_build_with_upload_job(
    state: &Arc<AppState>,
    build: &mut Build,
) -> Result<(), AppError> {
    state.db.create_build(build).await?;

    let pipeline_upload_step = serde_json::json!({
        "label": ":pipeline: Pipeline Upload",
        "command": "if [ -f glacier.yml ]; then buildkite-agent pipeline upload glacier.yml; elif [ -f buildkite.yml ]; then buildkite-agent pipeline upload buildkite.yml; else buildkite-agent pipeline upload; fi",
        "key": "pipeline-upload"
    });

    let mut job = Job {
        id: Uuid::new_v4(),
        build_id: build.id,
        step_config: pipeline_upload_step,
        state: "scheduled".to_string(),
        agent_id: None,
        job_token: None,
        env: Some(serde_json::json!({})),
        depends_on: None,
        exit_status: None,
        signal: None,
        signal_reason: None,
        started_at: None,
        finished_at: None,
        runnable_at: Some(Utc::now().naive_utc()),
        chunks_failed_count: 0,
        trace_parent: None,
        created_at: Utc::now().naive_utc(),
        updated_at: Utc::now().naive_utc(),
    };

    state.db.create_job(&mut job).await?;

    if let Some(ref gh) = state.github {
        crate::github::notify_build_status(gh, &state.db, build).await;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skip_ci_lowercase() {
        assert!(skip_ci_requested(Some("fix: [skip ci] typo")));
    }

    #[test]
    fn ci_skip_alternate_form() {
        assert!(skip_ci_requested(Some("[ci skip] docs only")));
    }

    #[test]
    fn skip_ci_case_insensitive() {
        assert!(skip_ci_requested(Some("[Skip CI] refactor")));
    }

    #[test]
    fn skip_ci_uppercase() {
        assert!(skip_ci_requested(Some("[SKIP CI] force")));
    }

    #[test]
    fn ci_skip_mixed_case() {
        assert!(skip_ci_requested(Some("[CI Skip] minor")));
    }

    #[test]
    fn skip_ci_at_end_of_message() {
        assert!(skip_ci_requested(Some("docs: update readme [skip ci]")));
    }

    #[test]
    fn no_skip_ci_in_normal_message() {
        assert!(!skip_ci_requested(Some("fix: improve CI pipeline")));
    }

    #[test]
    fn none_text_returns_false() {
        assert!(!skip_ci_requested(None));
    }

    #[test]
    fn empty_string_returns_false() {
        assert!(!skip_ci_requested(Some("")));
    }

    #[test]
    fn partial_skip_ci_not_matched() {
        assert!(!skip_ci_requested(Some("skip ci without brackets")));
    }

    #[test]
    fn build_from_seed_sets_scheduled_status() {
        let context = BuildContext {
            pipeline_slug: "test-pipeline".to_string(),
            build_number: 42,
            webhook_payload: None,
        };
        let seed = BuildSeed {
            commit: "abc123".to_string(),
            branch: "main".to_string(),
            tag: None,
            message: Some("test commit".to_string()),
            author_name: Some("Test".to_string()),
            author_email: Some("test@example.com".to_string()),
            pull_request_number: None,
        };
        let build = build_from_seed(context, seed);
        assert_eq!(build.status, "scheduled");
        assert_eq!(build.pipeline_slug, "test-pipeline");
        assert_eq!(build.number, 42);
        assert_eq!(build.commit, "abc123");
        assert_eq!(build.branch, "main");
        assert!(build.tag.is_none());
        assert_eq!(build.message.as_deref(), Some("test commit"));
        assert!(build.pull_request_number.is_none());
    }

    #[test]
    fn build_from_seed_with_pr_number() {
        let context = BuildContext {
            pipeline_slug: "pr-pipeline".to_string(),
            build_number: 7,
            webhook_payload: Some(serde_json::json!({"action": "opened"})),
        };
        let seed = BuildSeed {
            commit: "pr-sha".to_string(),
            branch: "feature-branch".to_string(),
            tag: None,
            message: Some("Add feature".to_string()),
            author_name: Some("dev".to_string()),
            author_email: None,
            pull_request_number: Some(42),
        };
        let build = build_from_seed(context, seed);
        assert_eq!(build.pull_request_number, Some(42));
        assert_eq!(build.branch, "feature-branch");
        assert!(build.webhook_payload.is_some());
    }

    #[test]
    fn build_from_seed_with_tag() {
        let context = BuildContext {
            pipeline_slug: "release".to_string(),
            build_number: 1,
            webhook_payload: None,
        };
        let seed = BuildSeed {
            commit: "tag-sha".to_string(),
            branch: "main".to_string(),
            tag: Some("v1.0.0".to_string()),
            message: None,
            author_name: None,
            author_email: None,
            pull_request_number: None,
        };
        let build = build_from_seed(context, seed);
        assert_eq!(build.tag.as_deref(), Some("v1.0.0"));
    }
}
