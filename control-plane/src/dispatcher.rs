use chrono::Utc;
use std::collections::HashMap;

use crate::{
    db::{Agent, Database, Job, Queue},
    error::AppError,
    github::GitHubClient,
    handlers::update_build_status,
};

const PRIORITY_RECENT_SECS: f64 = 60.0;

#[derive(Clone)]
pub struct Dispatcher {
    db: Database,
    github: Option<GitHubClient>,
}

impl Dispatcher {
    pub fn new(db: Database, github: Option<GitHubClient>) -> Self {
        Self { db, github }
    }

    pub async fn match_job_to_agent(&self, agent: &Agent) -> Result<Option<Job>, AppError> {
        if agent.current_job_id.is_some() {
            tracing::info!(
                "Agent {} (id: {}) already has job {:?} assigned, skipping match",
                agent.name,
                agent.id,
                agent.current_job_id
            );
            return Ok(None);
        }

        let jobs = self.db.get_runnable_jobs().await?;
        tracing::debug!(
            "Found {} runnable jobs for agent {} (queue_id: {:?})",
            jobs.len(),
            agent.name,
            agent.queue_id
        );

        for job in jobs {
            tracing::debug!(
                "Checking job {} | label: {:?} | key: {:?}",
                job.id,
                job.step_config.get("label"),
                job.step_config.get("key")
            );

            let is_pipeline_upload = is_pipeline_upload_job(&job);

            if is_pipeline_upload {
                tracing::info!(
                    "Job {} is a pipeline-upload bootstrap job, skipping queue check",
                    job.id
                );

                let build = self.db.get_build_by_id(job.build_id).await?;
                let pipeline = self.db.get_pipeline_by_slug(&build.pipeline_slug).await?;
                if let Some(pipeline_org_id) = pipeline.organization_id {
                    if agent.organization_id != Some(pipeline_org_id) {
                        tracing::info!(
                            "Skipping job {}: pipeline-upload belongs to org {}, but agent {} belongs to org {:?}",
                            job.id, pipeline_org_id, agent.name, agent.organization_id
                        );
                        continue;
                    }
                }
            } else {
                let queue_result = self.resolve_job_queue(&job).await?;
                if self.fail_if_queue_unrunnable(&job, &queue_result).await? {
                    continue;
                }

                if let JobQueueResult::Queue(queue) = queue_result {
                    let Some(agent_queue_id) = agent.queue_id else {
                        tracing::info!(
                            "Skipping job {}: job requires queue '{}' (id: {}), but agent {} (id: {}) has no queue assigned",
                            job.id, queue.key, queue.id, agent.name, agent.id
                        );
                        continue;
                    };

                    if agent_queue_id != queue.id {
                        tracing::info!(
                            "Skipping job {}: job requires queue '{}' (id: {}), but agent {} (id: {}) is in queue id {}",
                            job.id, queue.key, queue.id, agent.name, agent.id, agent_queue_id
                        );
                        continue;
                    }
                }
            }

            if !tags_match(&job, agent) {
                tracing::info!(
                    "Skipping job {}: agent {} (tags: {:?}) does not satisfy required tags {:?}",
                    job.id,
                    agent.name,
                    agent.tags,
                    job.step_config.get("agents")
                );
                continue;
            }

            if !self.dependencies_met(&job).await? {
                tracing::info!(
                    "Skipping job {}: dependencies not yet satisfied (depends_on: {:?})",
                    job.id,
                    job.depends_on
                );
                continue;
            }

            if !is_pipeline_upload {
                if let Some(queue_id) = agent.queue_id {
                    match self
                        .db
                        .has_higher_priority_agent_in_queue(
                            queue_id,
                            agent.id,
                            agent.priority,
                            PRIORITY_RECENT_SECS,
                        )
                        .await
                    {
                        Ok(true) => {
                            tracing::info!(
                                "Holding job {}: a higher-priority agent is available in queue {} (current agent {} priority: {:?})",
                                job.id, queue_id, agent.name, agent.priority
                            );
                            continue;
                        }
                        Ok(false) => {}
                        Err(e) => {
                            tracing::error!("Priority check failed: {}", e);
                        }
                    }
                }
            }

            return Ok(Some(job));
        }

        Ok(None)
    }

    async fn resolve_job_queue(&self, job: &Job) -> Result<JobQueueResult, AppError> {
        tracing::debug!(
            "Resolving queue for job {} | step_config: {:?}",
            job.id,
            job.step_config
        );

        let agents_config = job.step_config.get("agents");
        tracing::debug!("Job {} agents config: {:?}", job.id, agents_config);

        let target_queue_key = match agents_config {
            Some(serde_json::Value::Object(map)) => {
                let queue = map.get("queue").and_then(|v| v.as_str()).map(String::from);
                tracing::debug!("Job {} has agents.queue: {:?}", job.id, queue);
                queue
            }
            Some(other) => {
                tracing::debug!("Job {} agents is not an object: {:?}", job.id, other);
                None
            }
            None => {
                tracing::debug!("Job {} has no agents config in step_config", job.id);
                None
            }
        };

        if let Some(queue_key) = target_queue_key {
            tracing::info!("Job {} requests queue key: '{}'", job.id, queue_key);

            let build = self.db.get_build_by_id(job.build_id).await?;
            tracing::debug!(
                "Job {} belongs to build {} (pipeline: {})",
                job.id,
                build.id,
                build.pipeline_slug
            );

            let pipeline = self.db.get_pipeline_by_slug(&build.pipeline_slug).await?;
            tracing::debug!(
                "Pipeline '{}' (id: {}) has user_id: {:?}",
                pipeline.slug,
                pipeline.id,
                pipeline.user_id
            );

            if let Some(org_id) = pipeline.organization_id {
                tracing::debug!(
                    "Looking for queue '{}' in organization {}",
                    queue_key,
                    org_id
                );
                match self.db.get_queue_by_key_and_org(&queue_key, org_id).await {
                    Ok(queue) => {
                        tracing::info!(
                            "Found queue '{}' (id: {}) for job {}",
                            queue.key,
                            queue.id,
                            job.id
                        );
                        return Ok(JobQueueResult::Queue(queue));
                    }
                    Err(sqlx::Error::RowNotFound) => {
                        tracing::debug!(
                            "Queue '{}' not found for org {}, trying user lookup",
                            queue_key,
                            org_id
                        );
                    }
                    Err(e) => {
                        tracing::error!("Database error looking up queue by org: {}", e);
                        return Err(e.into());
                    }
                }
            }

            if let Some(user_id) = pipeline.user_id {
                tracing::debug!(
                    "Looking for queue '{}' owned by user {}",
                    queue_key,
                    user_id
                );
                match self.db.get_queue_by_key_and_user(&queue_key, user_id).await {
                    Ok(queue) => {
                        tracing::info!(
                            "Found queue '{}' (id: {}) for job {}",
                            queue.key,
                            queue.id,
                            job.id
                        );
                        return Ok(JobQueueResult::Queue(queue));
                    }
                    Err(sqlx::Error::RowNotFound) => {
                        tracing::warn!(
                            "Queue '{}' not found for user {} (job {})",
                            queue_key,
                            user_id,
                            job.id
                        );
                        return Ok(JobQueueResult::QueueNotFound(queue_key));
                    }
                    Err(e) => {
                        tracing::error!("Database error looking up queue: {}", e);
                        return Err(e.into());
                    }
                }
            }

            tracing::warn!(
                "Pipeline '{}' has no owner (user_id and organization_id are NULL), cannot find queue '{}'",
                pipeline.slug, queue_key
            );
            return Ok(JobQueueResult::QueueNotFound(queue_key));
        }

        tracing::debug!(
            "Job {} has no queue specified, checking pipeline default",
            job.id
        );

        let build = self.db.get_build_by_id(job.build_id).await?;
        let pipeline = self.db.get_pipeline_by_slug(&build.pipeline_slug).await?;

        tracing::debug!(
            "Checking default queue for pipeline '{}' (id: {})",
            pipeline.slug,
            pipeline.id
        );

        match self.db.get_default_queue_for_pipeline(pipeline.id).await {
            Ok(queue) => {
                tracing::info!(
                    "Using default queue '{}' (id: {}) for job {}",
                    queue.key,
                    queue.id,
                    job.id
                );
                Ok(JobQueueResult::Queue(queue))
            }
            Err(sqlx::Error::RowNotFound) => {
                tracing::warn!(
                    "No default queue found for pipeline '{}' (id: {})",
                    pipeline.slug,
                    pipeline.id
                );
                Ok(JobQueueResult::NoQueue)
            }
            Err(e) => {
                tracing::error!("Database error looking up default queue: {}", e);
                Err(e.into())
            }
        }
    }

    async fn fail_job(&self, job: &Job, reason: &str) -> Result<(), AppError> {
        let mut failed_job = job.clone();
        failed_job.state = "failed".to_string();
        failed_job.signal_reason = Some(reason.to_string());
        failed_job.runnable_at = None;
        failed_job.finished_at = Some(Utc::now().naive_utc());
        self.db.update_job(&failed_job).await?;
        update_build_status(&self.db, failed_job.build_id, self.github.as_ref()).await?;
        Ok(())
    }

    async fn fail_if_queue_unrunnable(
        &self,
        job: &Job,
        queue_result: &JobQueueResult,
    ) -> Result<bool, AppError> {
        match queue_result {
            JobQueueResult::Queue(queue) => {
                let agents_in_queue = self.db.get_agents_by_queue(queue.id).await?;
                if agents_in_queue.is_empty() {
                    tracing::warn!(
                        "Job {} failed: No agents in queue '{}' (id: {})",
                        job.id,
                        queue.key,
                        queue.id
                    );
                    self.fail_job(job, "No agents in queue").await?;
                    return Ok(true);
                }
            }
            JobQueueResult::NoQueue => {
                tracing::warn!("Job {} failed: Queue not specified", job.id);
                self.fail_job(job, "Queue not specified").await?;
                return Ok(true);
            }
            JobQueueResult::QueueNotFound(key) => {
                tracing::warn!("Job {} failed: Queue '{}' not found", job.id, key);
                self.fail_job(job, &format!("Queue '{}' not found", key))
                    .await?;
                return Ok(true);
            }
        }
        Ok(false)
    }

    async fn dependencies_met(&self, job: &Job) -> Result<bool, AppError> {
        let Some(deps) = job.depends_on.clone() else {
            return Ok(true);
        };

        if deps.is_empty() {
            return Ok(true);
        }

        for dep_id in deps {
            match self.db.get_job_by_id(dep_id).await {
                Ok(dep_job) => match classify_dependency_state(&dep_job.state) {
                    DependencyStatus::Satisfied => {}
                    DependencyStatus::Pending => return Ok(false),
                    DependencyStatus::Failed => {
                        tracing::warn!(
                                "Dependency job {} is in terminal failure state '{}' for job {}, marking dependent as failed",
                                dep_id,
                                dep_job.state,
                                job.id
                            );
                        self.fail_job(job, "dependency_failed").await?;
                        return Ok(false);
                    }
                },
                Err(sqlx::Error::RowNotFound) => {
                    tracing::warn!(
                        "Dependency job {} not found for job {}, marking job as failed",
                        dep_id,
                        job.id
                    );
                    self.fail_job(job, "missing_dependency").await?;
                    return Ok(false);
                }
                Err(err) => return Err(err.into()),
            }
        }

        Ok(true)
    }

    pub async fn check_dependent_jobs(&self, finished_job: &Job) -> Result<(), AppError> {
        let dependent_jobs = self.db.get_jobs_by_dependency(finished_job.id).await?;

        for mut job in dependent_jobs {
            if self.dependencies_met(&job).await? {
                let now = Utc::now().naive_utc();
                job.runnable_at = Some(now);
                job.state = "scheduled".to_string();
                self.db.update_job(&job).await?;
            }
        }

        Ok(())
    }

    pub async fn check_stalled_jobs(&self) -> Result<u32, AppError> {
        let jobs = self.db.get_runnable_jobs().await?;
        let mut failed_count = 0;

        for job in jobs {
            let queue_result = self.resolve_job_queue(&job).await?;
            if self.fail_if_queue_unrunnable(&job, &queue_result).await? {
                failed_count += 1;
            }
        }

        Ok(failed_count)
    }
}

enum JobQueueResult {
    Queue(Queue),
    NoQueue,
    QueueNotFound(String),
}

#[derive(Debug, PartialEq, Eq)]
enum DependencyStatus {
    Satisfied,
    Pending,
    Failed,
}

fn classify_dependency_state(state: &str) -> DependencyStatus {
    match state {
        "finished" => DependencyStatus::Satisfied,
        "failed" | "timed_out" | "canceled" | "cancelled" => DependencyStatus::Failed,
        _ => DependencyStatus::Pending,
    }
}

fn is_pipeline_upload_job(job: &Job) -> bool {
    if let Some(key) = job.step_config.get("key") {
        if key.as_str() == Some("pipeline-upload") {
            return true;
        }
    }
    if let Some(command) = job.step_config.get("command") {
        if let Some(cmd_str) = command.as_str() {
            if cmd_str.contains("buildkite-agent pipeline upload") {
                return true;
            }
        }
    }
    false
}

fn tags_match(job: &Job, agent: &Agent) -> bool {
    let required_agents = match job.step_config.get("agents") {
        Some(serde_json::Value::Object(map)) => map.clone(),
        _ => return true,
    };

    if required_agents.is_empty() {
        return true;
    }

    let agent_tags: HashMap<String, String> = agent
        .tags
        .as_ref()
        .map(|tags| {
            tags.iter()
                .map(|t| {
                    let parts: Vec<&str> = t.splitn(2, '=').collect();
                    if parts.len() == 2 {
                        (parts[0].to_string(), parts[1].to_string())
                    } else {
                        (t.clone(), String::new())
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    for (key, required_value) in required_agents.iter() {
        if key == "queue" || key == "priority" {
            continue;
        }

        let required_str = match required_value {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Bool(b) => b.to_string(),
            serde_json::Value::Number(n) => n.to_string(),
            _ => continue,
        };

        match agent_tags.get(key) {
            Some(agent_value) if agent_value == &required_str => continue,
            Some(_) => return false,
            None => return false,
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use serde_json::json;
    use uuid::Uuid;

    fn make_job(step_config: serde_json::Value) -> Job {
        Job {
            id: Uuid::new_v4(),
            build_id: Uuid::new_v4(),
            step_config,
            state: "scheduled".to_string(),
            agent_id: None,
            job_token: None,
            env: None,
            depends_on: None,
            exit_status: None,
            signal: None,
            signal_reason: None,
            started_at: None,
            finished_at: None,
            runnable_at: None,
            chunks_failed_count: 0,
            trace_parent: None,
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        }
    }

    fn make_agent(tags: Option<Vec<String>>) -> Agent {
        Agent {
            id: Uuid::new_v4(),
            uuid: Uuid::new_v4().to_string(),
            name: "test-agent".to_string(),
            hostname: "test-host".to_string(),
            os: "linux".to_string(),
            arch: "amd64".to_string(),
            version: "3.0".to_string(),
            build: "1".to_string(),
            tags,
            priority: None,
            status: "connected".to_string(),
            registration_token_id: None,
            user_id: None,
            organization_id: None,
            queue_id: None,
            last_seen: None,
            last_heartbeat: None,
            current_job_id: None,
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        }
    }

    #[test]
    fn pipeline_upload_by_key() {
        let job = make_job(json!({
            "key": "pipeline-upload",
            "command": "buildkite-agent pipeline upload"
        }));
        assert!(is_pipeline_upload_job(&job));
    }

    #[test]
    fn pipeline_upload_by_command_substring() {
        let job = make_job(json!({
            "command": "buildkite-agent pipeline upload .buildkite/pipeline.yml"
        }));
        assert!(is_pipeline_upload_job(&job));
    }

    #[test]
    fn pipeline_upload_key_only() {
        let job = make_job(json!({"key": "pipeline-upload"}));
        assert!(is_pipeline_upload_job(&job));
    }

    #[test]
    fn regular_job_not_pipeline_upload() {
        let job = make_job(json!({"key": "build", "command": "cargo test"}));
        assert!(!is_pipeline_upload_job(&job));
    }

    #[test]
    fn empty_step_config_not_pipeline_upload() {
        let job = make_job(json!({}));
        assert!(!is_pipeline_upload_job(&job));
    }

    #[test]
    fn no_agent_requirements_matches_any() {
        let job = make_job(json!({"command": "echo hello"}));
        let agent = make_agent(Some(vec!["os=linux".to_string()]));
        assert!(tags_match(&job, &agent));
    }

    #[test]
    fn matching_single_tag() {
        let job = make_job(json!({"agents": {"os": "linux"}}));
        let agent = make_agent(Some(vec!["os=linux".to_string()]));
        assert!(tags_match(&job, &agent));
    }

    #[test]
    fn mismatching_single_tag() {
        let job = make_job(json!({"agents": {"os": "linux"}}));
        let agent = make_agent(Some(vec!["os=macos".to_string()]));
        assert!(!tags_match(&job, &agent));
    }

    #[test]
    fn matching_multiple_tags() {
        let job = make_job(json!({"agents": {"os": "linux", "arch": "amd64"}}));
        let agent = make_agent(Some(vec!["os=linux".to_string(), "arch=amd64".to_string()]));
        assert!(tags_match(&job, &agent));
    }

    #[test]
    fn missing_required_tag() {
        let job = make_job(json!({"agents": {"os": "linux", "arch": "amd64"}}));
        let agent = make_agent(Some(vec!["os=linux".to_string()]));
        assert!(!tags_match(&job, &agent));
    }

    #[test]
    fn queue_tag_is_skipped() {
        let job = make_job(json!({"agents": {"queue": "default", "os": "linux"}}));
        let agent = make_agent(Some(vec!["os=linux".to_string()]));
        assert!(tags_match(&job, &agent));
    }

    #[test]
    fn priority_tag_is_skipped() {
        let job = make_job(json!({"agents": {"priority": "5", "os": "linux"}}));
        let agent = make_agent(Some(vec!["os=linux".to_string()]));
        assert!(tags_match(&job, &agent));
    }

    #[test]
    fn empty_agents_object_matches_any() {
        let job = make_job(json!({"agents": {}}));
        let agent = make_agent(Some(vec!["os=linux".to_string()]));
        assert!(tags_match(&job, &agent));
    }

    #[test]
    fn agent_with_no_tags_fails_requirements() {
        let job = make_job(json!({"agents": {"os": "linux"}}));
        let agent = make_agent(None);
        assert!(!tags_match(&job, &agent));
    }

    #[test]
    fn queue_only_requirement_matches_any_agent() {
        let job = make_job(json!({"agents": {"queue": "ubuntu-1"}}));
        let agent = make_agent(None);
        assert!(tags_match(&job, &agent));
    }

    #[test]
    fn boolean_tag_value_match() {
        let job = make_job(json!({"agents": {"docker": true}}));
        let agent = make_agent(Some(vec!["docker=true".to_string()]));
        assert!(tags_match(&job, &agent));
    }

    #[test]
    fn dependency_finished_is_satisfied() {
        assert_eq!(
            classify_dependency_state("finished"),
            DependencyStatus::Satisfied
        );
    }

    #[test]
    fn dependency_running_is_pending() {
        assert_eq!(
            classify_dependency_state("running"),
            DependencyStatus::Pending
        );
    }

    #[test]
    fn dependency_waiting_is_pending() {
        assert_eq!(
            classify_dependency_state("waiting"),
            DependencyStatus::Pending
        );
    }

    #[test]
    fn dependency_failed_is_failed() {
        assert_eq!(
            classify_dependency_state("failed"),
            DependencyStatus::Failed
        );
    }

    #[test]
    fn dependency_timed_out_is_failed() {
        assert_eq!(
            classify_dependency_state("timed_out"),
            DependencyStatus::Failed
        );
    }

    #[test]
    fn dependency_canceled_is_failed() {
        assert_eq!(
            classify_dependency_state("canceled"),
            DependencyStatus::Failed
        );
    }
}
