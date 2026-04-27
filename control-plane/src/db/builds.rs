use chrono::Utc;
use sqlx::Postgres;
use uuid::Uuid;

use super::{Build, Database, Pipeline, PipelineStats};
use crate::types::BuildFilter;

fn append_build_filters(qb: &mut sqlx::QueryBuilder<'_, Postgres>, filter: &BuildFilter) {
    if let Some(ref states) = filter.states {
        if !states.is_empty() {
            qb.push(" AND builds.status IN (");
            let mut sep = qb.separated(", ");
            for state in states {
                sep.push_bind(state.clone());
            }
            sep.push_unseparated(")");
        }
    }

    if let Some(ref branch) = filter.branch {
        if branch.contains('*') {
            qb.push(" AND builds.branch LIKE ");
            qb.push_bind(branch.replace('*', "%"));
        } else {
            qb.push(" AND builds.branch = ");
            qb.push_bind(branch.clone());
        }
    }

    if let Some(ref commit) = filter.commit {
        if commit.len() < 40 {
            qb.push(" AND builds.commit LIKE ");
            qb.push_bind(format!("{}%", commit));
        } else {
            qb.push(" AND builds.commit = ");
            qb.push_bind(commit.clone());
        }
    }

    if let Some(created_from) = filter.created_from {
        qb.push(" AND builds.created_at >= ");
        qb.push_bind(created_from);
    }

    if let Some(created_to) = filter.created_to {
        qb.push(" AND builds.created_at <= ");
        qb.push_bind(created_to);
    }

    if let Some(finished_from) = filter.finished_from {
        qb.push(" AND builds.finished_at >= ");
        qb.push_bind(finished_from);
    }

    if let Some(ref creator) = filter.creator {
        qb.push(" AND builds.author_name ILIKE ");
        qb.push_bind(format!("%{}%", creator));
    }
}

impl Database {
    pub async fn create_build(&self, build: &mut Build) -> Result<(), sqlx::Error> {
        let row = sqlx::query_as::<_, Build>(
            r#"INSERT INTO builds (number, pipeline_slug, commit, branch, tag, message,
                                   author_name, author_email, status, webhook_payload, started_at, finished_at,
                                   pull_request_number, source)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
               RETURNING id, number, pipeline_slug, commit, branch, tag, message, author_name, author_email,
                         status, webhook_payload, created_at, started_at, finished_at, pull_request_number, source"#,
        )
        .bind(build.number)
        .bind(&build.pipeline_slug)
        .bind(&build.commit)
        .bind(&build.branch)
        .bind(&build.tag)
        .bind(&build.message)
        .bind(&build.author_name)
        .bind(&build.author_email)
        .bind(&build.status)
        .bind(&build.webhook_payload)
        .bind(build.started_at)
        .bind(build.finished_at)
        .bind(build.pull_request_number)
        .bind(&build.source)
        .fetch_one(&self.pool)
        .await?;

        *build = row;
        Ok(())
    }

    pub async fn get_build_by_id(&self, id: Uuid) -> Result<Build, sqlx::Error> {
        sqlx::query_as::<_, Build>(
            r#"SELECT id, number, pipeline_slug, commit, branch, tag, message, author_name, author_email,
                     status, webhook_payload, created_at, started_at, finished_at, pull_request_number, source
               FROM builds WHERE id = $1"#,
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn update_build(&self, build: &Build) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"UPDATE builds SET 
                    status = $2, started_at = $3, finished_at = $4
               WHERE id = $1"#,
        )
        .bind(build.id)
        .bind(&build.status)
        .bind(build.started_at)
        .bind(build.finished_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_next_build_number(&self, pipeline_slug: &str) -> Result<i32, sqlx::Error> {
        let row = sqlx::query_scalar::<_, i32>(
            r#"SELECT COALESCE(MAX(number), 0) + 1 FROM builds WHERE pipeline_slug = $1"#,
        )
        .bind(pipeline_slug)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn get_builds_for_pipeline(&self, pipeline_slug: &str, limit: i64) -> Result<Vec<Build>, sqlx::Error> {
        sqlx::query_as::<_, Build>(
            r#"SELECT id, number, pipeline_slug, commit, branch, tag, message, author_name, author_email,
                      status, webhook_payload, created_at, started_at, finished_at, pull_request_number, source
               FROM builds 
               WHERE pipeline_slug = $1
               ORDER BY created_at DESC
               LIMIT $2"#,
        )
        .bind(pipeline_slug)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn get_build_by_pipeline_slug_and_number(&self, pipeline_slug: &str, number: i32) -> Result<Build, sqlx::Error> {
        sqlx::query_as::<_, Build>(
            r#"SELECT id, number, pipeline_slug, commit, branch, tag, message, author_name, author_email,
                     status, webhook_payload, created_at, started_at, finished_at, pull_request_number, source
               FROM builds WHERE pipeline_slug = $1 AND number = $2"#,
        )
        .bind(pipeline_slug)
        .bind(number)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn get_pipeline_stats(&self, pipeline_slug: &str) -> Result<PipelineStats, sqlx::Error> {
        sqlx::query_as::<_, PipelineStats>(
            r#"SELECT 
                   COUNT(*)::BIGINT as total_builds,
                   COUNT(*) FILTER (WHERE status = 'passed')::BIGINT as passed_builds,
                   COUNT(*) FILTER (WHERE status = 'failed')::BIGINT as failed_builds,
                   COUNT(*) FILTER (WHERE status = 'running')::BIGINT as running_builds,
                   COALESCE(AVG(EXTRACT(EPOCH FROM (finished_at - started_at))) FILTER (WHERE finished_at IS NOT NULL AND started_at IS NOT NULL), 0)::FLOAT8 as avg_duration_seconds
               FROM builds 
               WHERE pipeline_slug = $1"#,
        )
        .bind(pipeline_slug)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn get_or_create_pipeline(
        &self,
        slug: &str,
        repo_url: &str,
    ) -> Result<Pipeline, sqlx::Error> {
        if !repo_url.is_empty() {
            let normalized_repo = normalize_repo_url(repo_url);
            tracing::debug!("Looking for pipeline by repo URL: {} (normalized: {})", repo_url, normalized_repo);

            if let Ok(mut p) = self.get_pipeline_by_repo_url(&normalized_repo).await {
                tracing::info!(
                    "Found existing pipeline '{}' (id: {}, user_id: {:?}) by repository URL match",
                    p.slug, p.id, p.user_id
                );
                if p.repository_url != repo_url {
                    let now = Utc::now();
                    sqlx::query(
                        r#"UPDATE pipelines SET repository_url = $2, updated_at = $3 WHERE id = $1"#,
                    )
                    .bind(p.id)
                    .bind(repo_url)
                    .bind(now)
                    .execute(&self.pool)
                    .await?;
                    p.repository_url = repo_url.to_string();
                }
                return Ok(p);
            }
        }

        match self.get_pipeline_by_slug(slug).await {
            Ok(mut p) => {
                tracing::debug!("Found pipeline by slug '{}' (id: {}, user_id: {:?})", slug, p.id, p.user_id);
                if !repo_url.is_empty() && p.repository_url != repo_url {
                    let now = Utc::now();
                    sqlx::query(
                        r#"UPDATE pipelines SET repository_url = $2, updated_at = $3 WHERE id = $1"#,
                    )
                    .bind(p.id)
                    .bind(repo_url)
                    .bind(now)
                    .execute(&self.pool)
                    .await?;
                    p.repository_url = repo_url.to_string();
                }
                return Ok(p);
            }
            Err(sqlx::Error::RowNotFound) => {
                tracing::debug!("Pipeline not found by slug '{}'", slug);
            }
            Err(e) => return Err(e),
        }

        tracing::info!("Creating new pipeline with slug '{}' and repo '{}'", slug, repo_url);
        let id = Uuid::new_v4();
        let now = Utc::now();
        let pipeline = sqlx::query_as::<_, Pipeline>(
            r#"INSERT INTO pipelines (id, slug, repository_url, name, default_branch, created_at, updated_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7)
               RETURNING id, slug, repository_url, webhook_secret, config_cache, 
                         user_id, organization_id, name, description, default_branch, created_at, updated_at"#,
        )
        .bind(id)
        .bind(slug)
        .bind(repo_url)
        .bind(slug)
        .bind("main")
        .bind(now)
        .bind(now)
        .fetch_one(&self.pool)
        .await?;

        Ok(pipeline)
    }

    pub async fn get_pipeline_by_repo_url(&self, normalized_url: &str) -> Result<Pipeline, sqlx::Error> {
        let pattern = format!("%{}%", normalized_url);

        sqlx::query_as::<_, Pipeline>(
            r#"SELECT id, slug, repository_url, webhook_secret, config_cache, 
                      user_id, organization_id, name, description, default_branch, created_at, updated_at
               FROM pipelines 
               WHERE repository_url LIKE $1
               ORDER BY user_id IS NOT NULL DESC, created_at DESC
               LIMIT 1"#,
        )
        .bind(pattern)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn find_pipeline_by_repo_url(&self, repo_url: &str) -> Result<Pipeline, sqlx::Error> {
        let owner_repo = extract_owner_repo(repo_url);
        let pattern = format!("%{}%", owner_repo);

        tracing::debug!("Finding pipeline by repo URL: {} (owner/repo: {}, pattern: {})", repo_url, owner_repo, pattern);

        sqlx::query_as::<_, Pipeline>(
            r#"SELECT id, slug, repository_url, webhook_secret, config_cache, 
                      user_id, organization_id, name, description, default_branch, created_at, updated_at
               FROM pipelines 
               WHERE repository_url LIKE $1 AND user_id IS NOT NULL
               ORDER BY created_at DESC
               LIMIT 1"#,
        )
        .bind(pattern)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn get_pipeline_by_slug(&self, slug: &str) -> Result<Pipeline, sqlx::Error> {
        sqlx::query_as::<_, Pipeline>(
            r#"SELECT id, slug, repository_url, webhook_secret, config_cache, 
                      user_id, organization_id, name, description, default_branch, created_at, updated_at
               FROM pipelines WHERE slug = $1"#,
        )
        .bind(slug)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn get_pipeline_by_id_and_user(&self, id: Uuid, user_id: Uuid) -> Result<Pipeline, sqlx::Error> {
        sqlx::query_as::<_, Pipeline>(
            r#"SELECT id, slug, repository_url, webhook_secret, config_cache,
                      user_id, organization_id, name, description, default_branch, created_at, updated_at
               FROM pipelines 
               WHERE id = $1 AND user_id = $2"#,
        )
        .bind(id)
        .bind(user_id)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn get_pipeline_by_id(&self, id: Uuid) -> Result<Pipeline, sqlx::Error> {
        sqlx::query_as::<_, Pipeline>(
            r#"SELECT id, slug, repository_url, webhook_secret, config_cache,
                      user_id, organization_id, name, description, default_branch, created_at, updated_at
               FROM pipelines 
               WHERE id = $1"#,
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn create_pipeline_for_user(
        &self,
        user_id: Uuid,
        organization_id: Option<Uuid>,
        slug: &str,
        name: &str,
        repository_url: &str,
        description: Option<&str>,
        default_branch: Option<&str>,
    ) -> Result<Pipeline, sqlx::Error> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let branch = default_branch.unwrap_or("main");

        sqlx::query_as::<_, Pipeline>(
            r#"INSERT INTO pipelines (id, slug, repository_url, user_id, organization_id, name, description, default_branch, created_at, updated_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
               RETURNING id, slug, repository_url, webhook_secret, config_cache,
                         user_id, organization_id, name, description, default_branch, created_at, updated_at"#,
        )
        .bind(id)
        .bind(slug)
        .bind(repository_url)
        .bind(user_id)
        .bind(organization_id)
        .bind(name)
        .bind(description)
        .bind(branch)
        .bind(now)
        .bind(now)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn update_pipeline(
        &self,
        id: Uuid,
        user_id: Uuid,
        name: &str,
        description: Option<&str>,
        repository_url: &str,
        default_branch: Option<&str>,
    ) -> Result<Pipeline, sqlx::Error> {
        let now = Utc::now();

        sqlx::query_as::<_, Pipeline>(
            r#"UPDATE pipelines 
               SET name = $3, description = $4, repository_url = $5, default_branch = $6, updated_at = $7
               WHERE id = $1 AND user_id = $2
               RETURNING id, slug, repository_url, webhook_secret, config_cache,
                         user_id, organization_id, name, description, default_branch, created_at, updated_at"#,
        )
        .bind(id)
        .bind(user_id)
        .bind(name)
        .bind(description)
        .bind(repository_url)
        .bind(default_branch)
        .bind(now)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn delete_pipeline(&self, id: Uuid, user_id: Uuid) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(r#"DELETE FROM pipelines WHERE id = $1 AND user_id = $2"#)
            .bind(id)
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn get_pipelines_by_user(&self, user_id: Uuid) -> Result<Vec<Pipeline>, sqlx::Error> {
        sqlx::query_as::<_, Pipeline>(
            r#"SELECT id, slug, repository_url, webhook_secret, config_cache,
                      user_id, organization_id, name, description, default_branch, created_at, updated_at
               FROM pipelines 
               WHERE user_id = $1
               ORDER BY created_at DESC"#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn count_builds_for_pipeline(&self, pipeline_slug: &str) -> Result<i64, sqlx::Error> {
        sqlx::query_scalar::<_, i64>(
            r#"SELECT COUNT(*)::BIGINT FROM builds WHERE pipeline_slug = $1"#,
        )
        .bind(pipeline_slug)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn get_builds_for_pipeline_paginated(
        &self,
        pipeline_slug: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Build>, sqlx::Error> {
        sqlx::query_as::<_, Build>(
            r#"SELECT id, number, pipeline_slug, commit, branch, tag, message, author_name, author_email,
                      status, webhook_payload, created_at, started_at, finished_at, pull_request_number, source
               FROM builds
               WHERE pipeline_slug = $1
               ORDER BY created_at DESC
               LIMIT $2 OFFSET $3"#,
        )
        .bind(pipeline_slug)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn count_builds_filtered(
        &self,
        pipeline_slug: &str,
        filter: &BuildFilter,
    ) -> Result<i64, sqlx::Error> {
        let mut qb: sqlx::QueryBuilder<Postgres> =
            sqlx::QueryBuilder::new("SELECT COUNT(*)::BIGINT FROM builds WHERE builds.pipeline_slug = ");
        qb.push_bind(pipeline_slug.to_string());
        append_build_filters(&mut qb, filter);
        let (count,): (i64,) = qb.build_query_as().fetch_one(&self.pool).await?;
        Ok(count)
    }

    pub async fn get_builds_filtered(
        &self,
        pipeline_slug: &str,
        filter: &BuildFilter,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Build>, sqlx::Error> {
        let mut qb: sqlx::QueryBuilder<Postgres> = sqlx::QueryBuilder::new(
            "SELECT builds.id, builds.number, builds.pipeline_slug, builds.commit, builds.branch, builds.tag, \
             builds.message, builds.author_name, builds.author_email, builds.status, builds.webhook_payload, \
             builds.created_at, builds.started_at, builds.finished_at, builds.pull_request_number, builds.source \
             FROM builds WHERE builds.pipeline_slug = ",
        );
        qb.push_bind(pipeline_slug.to_string());
        append_build_filters(&mut qb, filter);
        qb.push(" ORDER BY builds.created_at DESC LIMIT ");
        qb.push_bind(limit);
        qb.push(" OFFSET ");
        qb.push_bind(offset);
        qb.build_query_as::<Build>().fetch_all(&self.pool).await
    }

    pub async fn count_builds_for_org_filtered(
        &self,
        org_id: Uuid,
        filter: &BuildFilter,
    ) -> Result<i64, sqlx::Error> {
        let mut qb: sqlx::QueryBuilder<Postgres> = sqlx::QueryBuilder::new(
            "SELECT COUNT(*)::BIGINT FROM builds \
             JOIN pipelines ON builds.pipeline_slug = pipelines.slug \
             WHERE pipelines.organization_id = ",
        );
        qb.push_bind(org_id);
        append_build_filters(&mut qb, filter);
        let (count,): (i64,) = qb.build_query_as().fetch_one(&self.pool).await?;
        Ok(count)
    }

    pub async fn get_builds_for_org_filtered(
        &self,
        org_id: Uuid,
        filter: &BuildFilter,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Build>, sqlx::Error> {
        let mut qb: sqlx::QueryBuilder<Postgres> = sqlx::QueryBuilder::new(
            "SELECT builds.id, builds.number, builds.pipeline_slug, builds.commit, builds.branch, builds.tag, \
             builds.message, builds.author_name, builds.author_email, builds.status, builds.webhook_payload, \
             builds.created_at, builds.started_at, builds.finished_at, builds.pull_request_number, builds.source \
             FROM builds \
             JOIN pipelines ON builds.pipeline_slug = pipelines.slug \
             WHERE pipelines.organization_id = ",
        );
        qb.push_bind(org_id);
        append_build_filters(&mut qb, filter);
        qb.push(" ORDER BY builds.created_at DESC LIMIT ");
        qb.push_bind(limit);
        qb.push(" OFFSET ");
        qb.push_bind(offset);
        qb.build_query_as::<Build>().fetch_all(&self.pool).await
    }

    pub async fn count_all_builds_filtered(
        &self,
        user_id: Uuid,
        filter: &BuildFilter,
    ) -> Result<i64, sqlx::Error> {
        let mut qb: sqlx::QueryBuilder<Postgres> = sqlx::QueryBuilder::new(
            "SELECT COUNT(*)::BIGINT FROM builds \
             JOIN pipelines ON builds.pipeline_slug = pipelines.slug \
             JOIN organization_members ON pipelines.organization_id = organization_members.organization_id \
             WHERE organization_members.user_id = ",
        );
        qb.push_bind(user_id);
        append_build_filters(&mut qb, filter);
        let (count,): (i64,) = qb.build_query_as().fetch_one(&self.pool).await?;
        Ok(count)
    }

    pub async fn get_all_builds_filtered(
        &self,
        user_id: Uuid,
        filter: &BuildFilter,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Build>, sqlx::Error> {
        let mut qb: sqlx::QueryBuilder<Postgres> = sqlx::QueryBuilder::new(
            "SELECT builds.id, builds.number, builds.pipeline_slug, builds.commit, builds.branch, builds.tag, \
             builds.message, builds.author_name, builds.author_email, builds.status, builds.webhook_payload, \
             builds.created_at, builds.started_at, builds.finished_at, builds.pull_request_number, builds.source \
             FROM builds \
             JOIN pipelines ON builds.pipeline_slug = pipelines.slug \
             JOIN organization_members ON pipelines.organization_id = organization_members.organization_id \
             WHERE organization_members.user_id = ",
        );
        qb.push_bind(user_id);
        append_build_filters(&mut qb, filter);
        qb.push(" ORDER BY builds.created_at DESC LIMIT ");
        qb.push_bind(limit);
        qb.push(" OFFSET ");
        qb.push_bind(offset);
        qb.build_query_as::<Build>().fetch_all(&self.pool).await
    }
}

fn normalize_repo_url(url: &str) -> String {
    let mut result = url.to_string();
    if result.ends_with(".git") {
        result = result[..result.len() - 4].to_string();
    }
    if result.starts_with("git@") {
        result = result[4..].to_string();
        result = result.replace(':', "/");
    }
    if result.starts_with("https://") {
        result = result[8..].to_string();
    }
    if result.starts_with("http://") {
        result = result[7..].to_string();
    }
    if result.ends_with('/') {
        result = result[..result.len() - 1].to_string();
    }
    result
}

fn extract_owner_repo(url: &str) -> String {
    let mut result = url.to_string();
    if result.ends_with(".git") {
        result = result[..result.len() - 4].to_string();
    }
    if result.starts_with("git@") {
        if let Some(pos) = result.find(':') {
            result = result[pos + 1..].to_string();
        }
    } else {
        if result.starts_with("https://") {
            result = result[8..].to_string();
        } else if result.starts_with("http://") {
            result = result[7..].to_string();
        }
        if let Some(pos) = result.find('/') {
            result = result[pos + 1..].to_string();
        }
    }
    if result.ends_with('/') {
        result = result[..result.len() - 1].to_string();
    }
    result
}