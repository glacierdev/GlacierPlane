use uuid::Uuid;

use super::{Database, Job, LogChunk};

impl Database {
    pub async fn get_runnable_jobs(&self) -> Result<Vec<Job>, sqlx::Error> {
        sqlx::query_as::<_, Job>(
            r#"SELECT id, build_id, step_config, state, agent_id, job_token, env,
                     depends_on, exit_status, signal, signal_reason, started_at, finished_at,
                     runnable_at, chunks_failed_count, traceparent, created_at, updated_at
               FROM jobs
               WHERE state IN ('scheduled', 'waiting')
                 AND (runnable_at IS NULL OR runnable_at <= NOW())
               ORDER BY created_at ASC"#,
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn get_jobs_by_dependency(&self, dep_job_id: Uuid) -> Result<Vec<Job>, sqlx::Error> {
        sqlx::query_as::<_, Job>(
            r#"SELECT id, build_id, step_config, state, agent_id, job_token, env,
                     depends_on, exit_status, signal, signal_reason, started_at, finished_at,
                     runnable_at, chunks_failed_count, traceparent, created_at, updated_at
               FROM jobs
               WHERE $1 = ANY(depends_on)"#,
        )
        .bind(dep_job_id)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn get_jobs_by_build_id(&self, build_id: Uuid) -> Result<Vec<Job>, sqlx::Error> {
        sqlx::query_as::<_, Job>(
            r#"SELECT id, build_id, step_config, state, agent_id, job_token, env,
                     depends_on, exit_status, signal, signal_reason, started_at, finished_at,
                     runnable_at, chunks_failed_count, traceparent, created_at, updated_at
               FROM jobs WHERE build_id = $1"#,
        )
        .bind(build_id)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn create_job(&self, job: &mut Job) -> Result<(), sqlx::Error> {
        let row = sqlx::query_as::<_, Job>(
            r#"INSERT INTO jobs (id, build_id, step_config, state, agent_id, job_token, env,
                                 depends_on, runnable_at, traceparent)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
               RETURNING id, build_id, step_config, state, agent_id, job_token, env,
                         depends_on, exit_status, signal, signal_reason, started_at, finished_at,
                         runnable_at, chunks_failed_count, traceparent, created_at, updated_at"#,
        )
        .bind(job.id)
        .bind(job.build_id)
        .bind(&job.step_config)
        .bind(&job.state)
        .bind(job.agent_id)
        .bind(&job.job_token)
        .bind(&job.env)
        .bind(&job.depends_on)
        .bind(job.runnable_at)
        .bind(&job.trace_parent)
        .fetch_one(&self.pool)
        .await?;

        *job = row;
        Ok(())
    }

    pub async fn get_job_by_id(&self, id: Uuid) -> Result<Job, sqlx::Error> {
        sqlx::query_as::<_, Job>(
            r#"SELECT id, build_id, step_config, state, agent_id, job_token, env,
                     depends_on, exit_status, signal, signal_reason, started_at, finished_at,
                     runnable_at, chunks_failed_count, traceparent, created_at, updated_at
               FROM jobs WHERE id = $1"#,
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn update_job(&self, job: &Job) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"UPDATE jobs SET 
                    state = $2, agent_id = $3, exit_status = $4, signal = $5, signal_reason = $6,
                    started_at = $7, finished_at = $8, runnable_at = $9, chunks_failed_count = $10,
                    updated_at = NOW()
               WHERE id = $1"#,
        )
        .bind(job.id)
        .bind(&job.state)
        .bind(job.agent_id)
        .bind(&job.exit_status)
        .bind(&job.signal)
        .bind(&job.signal_reason)
        .bind(job.started_at)
        .bind(job.finished_at)
        .bind(job.runnable_at)
        .bind(job.chunks_failed_count)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_jobs_for_token_agents(&self, token_id: Uuid, limit: i64) -> Result<Vec<Job>, sqlx::Error> {
        sqlx::query_as::<_, Job>(
            r#"SELECT j.id, j.build_id, j.step_config, j.state, j.agent_id, j.job_token, j.env,
                      j.depends_on, j.exit_status, j.signal, j.signal_reason, j.started_at, j.finished_at,
                      j.runnable_at, j.chunks_failed_count, j.traceparent, j.created_at, j.updated_at
               FROM jobs j
               INNER JOIN agents a ON j.agent_id = a.id
               WHERE a.registration_token_id = $1
               ORDER BY j.created_at DESC
               LIMIT $2"#,
        )
        .bind(token_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn get_running_jobs(&self) -> Result<Vec<Job>, sqlx::Error> {
        sqlx::query_as::<_, Job>(
            r#"SELECT id, build_id, step_config, state, agent_id, job_token, env,
                     depends_on, exit_status, signal, signal_reason, started_at, finished_at,
                     runnable_at, chunks_failed_count, traceparent, created_at, updated_at
               FROM jobs
               WHERE state = 'running' AND started_at IS NOT NULL"#,
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn create_log_chunk(&self, chunk: &mut LogChunk) -> Result<(), sqlx::Error> {
        let row = sqlx::query_as::<_, LogChunk>(
            r#"INSERT INTO log_chunks (job_id, sequence, byte_offset, size, data)
               VALUES ($1, $2, $3, $4, $5)
               ON CONFLICT (job_id, sequence) DO UPDATE SET
                    byte_offset = EXCLUDED.byte_offset,
                    size = EXCLUDED.size,
                    data = EXCLUDED.data
               RETURNING id, job_id, sequence, byte_offset, size, data, created_at"#,
        )
        .bind(chunk.job_id)
        .bind(chunk.sequence)
        .bind(chunk.offset)
        .bind(chunk.size)
        .bind(&chunk.data)
        .fetch_one(&self.pool)
        .await?;

        *chunk = row;
        Ok(())
    }

    pub async fn get_log_chunks_for_job(&self, job_id: Uuid) -> Result<Vec<LogChunk>, sqlx::Error> {
        sqlx::query_as::<_, LogChunk>(
            r#"SELECT id, job_id, sequence, byte_offset, size, data, created_at
               FROM log_chunks 
               WHERE job_id = $1
               ORDER BY sequence ASC"#,
        )
        .bind(job_id)
        .fetch_all(&self.pool)
        .await
    }
}
