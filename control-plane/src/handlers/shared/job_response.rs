use chrono::{DateTime, Utc};
use serde_json::json;

use crate::{
    db::{Agent, Build, Job, Pipeline},
    error::AppResult,
    types::{CommandStep, JobResponse},
};

pub(crate) fn convert_job_to_response(
    job: &Job,
    build: &Build,
    pipeline: Option<&Pipeline>,
    agent: &Agent,
    access_token: &str,
) -> AppResult<JobResponse> {
    let step: CommandStep =
        serde_json::from_value(job.step_config.clone()).unwrap_or_else(|_| CommandStep::default());

    let mut env = job
        .env
        .clone()
        .unwrap_or_else(|| json!({}))
        .as_object()
        .cloned()
        .unwrap_or_default();

    let command = match &step.command {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Array(arr)) => arr
            .iter()
            .filter_map(|v| v.as_str())
            .collect::<Vec<_>>()
            .join("\n"),
        _ => "true".to_string(),
    };

    if let Some(step_env) = &step.env {
        if let Some(env_obj) = step_env.as_object() {
            for (key, value) in env_obj {
                env.insert(key.clone(), value.clone());
            }
        }
    }

    env.insert("BUILDKITE".into(), json!("true"));
    env.insert("BUILDKITE_COMMAND".into(), json!(command));
    env.insert("BUILDKITE_COMMAND_EVAL".into(), json!("true"));
    env.insert("BUILDKITE_JOB_ID".into(), json!(job.id.to_string()));
    env.insert("BUILDKITE_BUILD_ID".into(), json!(build.id.to_string()));
    env.insert(
        "BUILDKITE_BUILD_NUMBER".into(),
        json!(build.number.to_string()),
    );
    env.insert(
        "BUILDKITE_PIPELINE_SLUG".into(),
        json!(&build.pipeline_slug),
    );
    env.insert("BUILDKITE_BRANCH".into(), json!(&build.branch));
    env.insert("BUILDKITE_COMMIT".into(), json!(&build.commit));
    env.insert(
        "BUILDKITE_MESSAGE".into(),
        json!(build.message.as_deref().unwrap_or("")),
    );
    env.insert(
        "BUILDKITE_BUILD_AUTHOR".into(),
        json!(build.author_name.as_deref().unwrap_or("")),
    );
    env.insert(
        "BUILDKITE_BUILD_AUTHOR_EMAIL".into(),
        json!(build.author_email.as_deref().unwrap_or("")),
    );
    env.insert("BUILDKITE_SOURCE".into(), json!("webhook"));
    env.insert(
        "BUILDKITE_LABEL".into(),
        json!(step.label.as_deref().unwrap_or("")),
    );
    env.insert("BUILDKITE_AGENT_ID".into(), json!(agent.uuid));
    env.insert("BUILDKITE_AGENT_NAME".into(), json!(&agent.name));
    env.insert("BUILDKITE_AGENT_ACCESS_TOKEN".into(), json!(access_token));
    env.insert("BUILDKITE_ORGANIZATION_SLUG".into(), json!("self-hosted"));

    if let Some(ref tag) = build.tag {
        env.insert("BUILDKITE_TAG".into(), json!(tag));
    } else {
        env.insert("BUILDKITE_TAG".into(), json!(""));
    }

    env.insert("BUILDKITE_PIPELINE_PROVIDER".into(), json!("github"));
    env.insert("BUILDKITE_ARTIFACT_PATHS".into(), json!(""));
    env.insert("BUILDKITE_PLUGINS".into(), json!(""));
    env.insert("BUILDKITE_RETRY_COUNT".into(), json!("0"));

    if let Some(pr_num) = build.pull_request_number {
        env.insert("BUILDKITE_PULL_REQUEST".into(), json!(pr_num.to_string()));
        if let Some(ref wp) = build.webhook_payload {
            if let Some(base) = wp
                .pointer("/pull_request/base/ref")
                .and_then(|v| v.as_str())
            {
                env.insert("BUILDKITE_PULL_REQUEST_BASE_BRANCH".into(), json!(base));
            }
            let pr_repo = wp
                .pointer("/pull_request/head/repo/ssh_url")
                .and_then(|v| v.as_str())
                .or_else(|| {
                    wp.pointer("/pull_request/head/repo/clone_url")
                        .and_then(|v| v.as_str())
                });
            if let Some(repo) = pr_repo {
                env.insert("BUILDKITE_PULL_REQUEST_REPO".into(), json!(repo));
            }
            if let Some(draft) = wp.pointer("/pull_request/draft").and_then(|v| v.as_bool()) {
                env.insert(
                    "BUILDKITE_PULL_REQUEST_DRAFT".into(),
                    json!(draft.to_string()),
                );
            }
        }
    } else {
        env.insert("BUILDKITE_PULL_REQUEST".into(), json!("false"));
    }

    if let Some(ref key) = step.key {
        env.insert("BUILDKITE_STEP_KEY".into(), json!(key));
    } else {
        env.insert("BUILDKITE_STEP_KEY".into(), json!(""));
    }

    env.insert(
        "BUILDKITE_BUILD_URL".into(),
        json!(format!(
            "https://glacierapi.glacierdev.sh/pipelines/{}/builds/{}",
            build.pipeline_slug, build.number
        )),
    );

    if let Some(p) = pipeline {
        env.insert("BUILDKITE_REPO".into(), json!(&p.repository_url));
        env.insert(
            "BUILDKITE_PIPELINE_DEFAULT_BRANCH".into(),
            json!(p.default_branch.as_deref().unwrap_or("main")),
        );
    } else {
        env.insert("BUILDKITE_REPO".into(), json!(""));
        env.insert("BUILDKITE_PIPELINE_DEFAULT_BRANCH".into(), json!("main"));
    }

    let resp = JobResponse {
        id: job.id,
        endpoint: format!("/v3/jobs/{}", job.id),
        state: Some(job.state.clone()),
        env,
        step,
        chunks_max_size_bytes: Some(1024 * 1024),
        chunks_interval_seconds: Some(1),
        log_max_size_bytes: Some(10 * 1024 * 1024),
        token: job.job_token.clone(),
        exit_status: job.exit_status.clone(),
        signal: job.signal.clone(),
        signal_reason: job.signal_reason.clone(),
        started_at: job.started_at.map(|t| {
            DateTime::<Utc>::from_naive_utc_and_offset(t, Utc)
                .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true)
        }),
        finished_at: job.finished_at.map(|t| {
            DateTime::<Utc>::from_naive_utc_and_offset(t, Utc)
                .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true)
        }),
        runnable_at: job.runnable_at.map(|t| {
            DateTime::<Utc>::from_naive_utc_and_offset(t, Utc)
                .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true)
        }),
        chunks_failed_count: Some(job.chunks_failed_count),
        trace_parent: job.trace_parent.clone(),
    };

    Ok(resp)
}
