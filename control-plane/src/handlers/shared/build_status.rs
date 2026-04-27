use axum::http::StatusCode;
use chrono::Utc;
use uuid::Uuid;

use crate::{
    db::Database,
    error::{AppError, AppResult},
    github::GitHubClient,
};

pub(crate) async fn update_build_status(
    db: &Database,
    build_id: Uuid,
    github: Option<&GitHubClient>,
) -> AppResult<()> {
    let mut build = db
        .get_build_by_id(build_id)
        .await
        .map_err(|_| AppError::Http(StatusCode::NOT_FOUND, "Build not found".into()))?;

    let old_status = build.status.clone();

    let jobs = db.get_jobs_by_build_id(build_id).await?;

    let mut all_finished = true;
    let mut has_failed = false;
    let mut has_started = false;

    for job in jobs {
        if job.state == "running"
            || job.state == "accepted"
            || job.state == "scheduled"
            || job.state == "waiting"
        {
            all_finished = false;
        }
        if job.state == "running" || job.state == "accepted" {
            has_started = true;
        }
        if job.state == "failed" || job.state == "timed_out" {
            has_failed = true;
        }
    }

    if all_finished {
        build.status = if has_failed {
            "failed".into()
        } else {
            "passed".into()
        };
        build.finished_at = Some(Utc::now().naive_utc());
    } else if has_started && build.status == "scheduled" {
        build.status = "running".into();
        if build.started_at.is_none() {
            build.started_at = Some(Utc::now().naive_utc());
        }
    }

    db.update_build(&build).await?;

    if build.status != old_status {
        if let Some(gh) = github {
            crate::github::notify_build_status(gh, db, &build).await;
        }
    }

    Ok(())
}
