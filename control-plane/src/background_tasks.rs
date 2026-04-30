use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;

use crate::github::GitHubClient;
use crate::handlers::update_build_status;
use crate::AppState;

const STALLED_JOBS_INTERVAL_SECS: u64 = 60;
const LOST_AGENT_INTERVAL_SECS: u64 = 30;
const LOST_AGENT_TIMEOUT_SECS: i64 = 180;
const JOB_TIMEOUT_INTERVAL_SECS: u64 = 60;
const DEFAULT_JOB_TIMEOUT_MINUTES: i64 = 60;

pub fn spawn_all(state: &Arc<AppState>) {
    spawn_stalled_jobs_check(state);
    spawn_lost_agent_detection(state);
    spawn_job_timeout_check(state);
}

fn spawn_stalled_jobs_check(state: &Arc<AppState>) {
    let dispatcher = state.dispatcher.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(STALLED_JOBS_INTERVAL_SECS));
        loop {
            interval.tick().await;
            match dispatcher.check_stalled_jobs().await {
                Ok(0) => {}
                Ok(n) => tracing::info!("Stalled jobs check: failed {} stalled jobs", n),
                Err(e) => tracing::error!("Stalled jobs check error: {}", e),
            }
        }
    });
}

fn spawn_lost_agent_detection(state: &Arc<AppState>) {
    let db = state.db.clone();
    let github = state.github.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(LOST_AGENT_INTERVAL_SECS));
        loop {
            interval.tick().await;
            if let Err(e) = detect_lost_agents(&db, github.as_ref()).await {
                tracing::error!("Lost agent detection error: {}", e);
            }
        }
    });
}

async fn detect_lost_agents(
    db: &crate::db::Database,
    github: Option<&GitHubClient>,
) -> Result<(), crate::error::AppError> {
    let stale_agents = db.get_stale_agents(LOST_AGENT_TIMEOUT_SECS).await?;

    for mut agent in stale_agents {
        tracing::warn!(
            "Agent '{}' (id: {}) marked as lost — last seen {:?}",
            agent.name,
            agent.id,
            agent.last_seen
        );

        agent.status = "lost".to_string();

        if let Some(job_id) = agent.current_job_id.take() {
            match db.get_job_by_id(job_id).await {
                Ok(mut job) => {
                    if job.state == "running" || job.state == "accepted" {
                        finalize_job_with_reason(
                            db,
                            &mut job,
                            "failed",
                            "agent_lost",
                            true,
                            github,
                        )
                        .await?;

                        tracing::warn!("Job {} failed due to lost agent '{}'", job.id, agent.name);
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to fetch job {} for lost agent: {}", job_id, e);
                }
            }
        }

        db.update_agent(&agent).await?;
    }

    Ok(())
}

fn spawn_job_timeout_check(state: &Arc<AppState>) {
    let db = state.db.clone();
    let github = state.github.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(JOB_TIMEOUT_INTERVAL_SECS));
        loop {
            interval.tick().await;
            if let Err(e) = check_timed_out_jobs(&db, github.as_ref()).await {
                tracing::error!("Job timeout check error: {}", e);
            }
        }
    });
}

async fn check_timed_out_jobs(
    db: &crate::db::Database,
    github: Option<&GitHubClient>,
) -> Result<(), crate::error::AppError> {
    let running_jobs = db.get_running_jobs().await?;
    let now = Utc::now().naive_utc();

    for mut job in running_jobs {
        let started_at = match job.started_at {
            Some(ts) => ts,
            None => continue,
        };

        let timeout_minutes = job
            .step_config
            .get("timeout_in_minutes")
            .and_then(|v| v.as_i64())
            .unwrap_or(DEFAULT_JOB_TIMEOUT_MINUTES);

        let deadline = started_at + chrono::Duration::minutes(timeout_minutes);
        if now < deadline {
            continue;
        }

        tracing::warn!(
            "Job {} timed out after {} minutes (started at {:?})",
            job.id,
            timeout_minutes,
            started_at
        );

        job.finished_at = Some(now);
        let agent_id = job.agent_id.take();
        finalize_job_with_reason(db, &mut job, "timed_out", "timeout", false, github).await?;

        if let Some(aid) = agent_id {
            match db.get_agent_by_id(aid).await {
                Ok(mut agent) => {
                    agent.current_job_id = None;
                    db.update_agent(&agent).await?;
                }
                Err(e) => {
                    tracing::error!("Failed to fetch agent {} for timed-out job: {}", aid, e);
                }
            }
        }
    }

    Ok(())
}

async fn finalize_job_with_reason(
    db: &crate::db::Database,
    job: &mut crate::db::Job,
    state: &str,
    signal_reason: &str,
    clear_agent_link: bool,
    github: Option<&GitHubClient>,
) -> Result<(), crate::error::AppError> {
    job.state = state.to_string();
    job.signal_reason = Some(signal_reason.to_string());
    if job.finished_at.is_none() {
        job.finished_at = Some(Utc::now().naive_utc());
    }
    if clear_agent_link {
        job.agent_id = None;
    }
    db.update_job(job).await?;

    if let Err(e) = update_build_status(db, job.build_id, github).await {
        tracing::error!(
            "Failed to update build status for build {}: {}",
            job.build_id,
            e
        );
    }
    Ok(())
}
