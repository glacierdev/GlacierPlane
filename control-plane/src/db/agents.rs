use uuid::Uuid;

use super::{AccessToken, Agent, AgentToken, Database};

impl Database {
    pub async fn get_agent_token_by_token(&self, token: &str) -> Result<AgentToken, sqlx::Error> {
        sqlx::query_as::<_, AgentToken>(
            r#"SELECT id, token, name, description, user_id, organization_id, expires_at, created_at 
               FROM agent_tokens WHERE token = $1"#,
        )
        .bind(token)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn create_agent(&self, agent: &mut Agent) -> Result<(), sqlx::Error> {
        let row = sqlx::query_as::<_, Agent>(
            r#"INSERT INTO agents 
                (uuid, name, hostname, os, arch, version, build, tags, priority, status,
                 registration_token_id, user_id, organization_id, queue_id, last_seen, last_heartbeat)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
               RETURNING id, uuid, name, hostname, os, arch, version, build, tags, priority,
                 status, registration_token_id, user_id, organization_id, queue_id, last_seen, last_heartbeat,
                 current_job_id, created_at, updated_at"#,
        )
        .bind(&agent.uuid)
        .bind(&agent.name)
        .bind(&agent.hostname)
        .bind(&agent.os)
        .bind(&agent.arch)
        .bind(&agent.version)
        .bind(&agent.build)
        .bind(&agent.tags)
        .bind(agent.priority)
        .bind(&agent.status)
        .bind(agent.registration_token_id)
        .bind(agent.user_id)
        .bind(agent.organization_id)
        .bind(agent.queue_id)
        .bind(agent.last_seen)
        .bind(agent.last_heartbeat)
        .fetch_one(&self.pool)
        .await?;

        *agent = row;
        Ok(())
    }

    pub async fn get_agent_by_access_token(&self, token: &str) -> Result<Agent, sqlx::Error> {
        sqlx::query(
            r#"UPDATE access_tokens SET last_used_at = NOW() 
               WHERE token = $1 AND revoked_at IS NULL"#,
        )
        .bind(token)
        .execute(&self.pool)
        .await?;

        sqlx::query_as::<_, Agent>(
            r#"SELECT a.id, a.uuid, a.name, a.hostname, a.os, a.arch, a.version, a.build, 
                      a.tags, a.priority, a.status, a.registration_token_id, a.user_id, a.organization_id, a.queue_id,
                      a.last_seen, a.last_heartbeat, a.current_job_id, a.created_at, a.updated_at
               FROM agents a
               INNER JOIN access_tokens t ON t.agent_id = a.id
               WHERE t.token = $1 
                 AND t.revoked_at IS NULL"#,
        )
        .bind(token)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn get_agent_by_id(&self, id: Uuid) -> Result<Agent, sqlx::Error> {
        sqlx::query_as::<_, Agent>(
            r#"SELECT id, uuid, name, hostname, os, arch, version, build, tags, priority,
                      status, registration_token_id, user_id, organization_id, queue_id, last_seen, last_heartbeat,
                      current_job_id, created_at, updated_at
               FROM agents WHERE id = $1"#,
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn update_agent(&self, agent: &Agent) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"UPDATE agents SET 
                    status = $2, last_seen = $3, last_heartbeat = $4, current_job_id = $5,
                    queue_id = $6, priority = $7, updated_at = NOW()
               WHERE id = $1"#,
        )
        .bind(agent.id)
        .bind(&agent.status)
        .bind(agent.last_seen)
        .bind(agent.last_heartbeat)
        .bind(agent.current_job_id)
        .bind(agent.queue_id)
        .bind(agent.priority)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn create_access_token(
        &self,
        access_token: &mut AccessToken,
    ) -> Result<(), sqlx::Error> {
        let row = sqlx::query_as::<_, AccessToken>(
            r#"INSERT INTO access_tokens (agent_id, token, description)
               VALUES ($1, $2, $3)
               RETURNING id, agent_id, token, description, revoked_at, last_used_at, created_at"#,
        )
        .bind(access_token.agent_id)
        .bind(&access_token.token)
        .bind(&access_token.description)
        .fetch_one(&self.pool)
        .await?;

        *access_token = row;
        Ok(())
    }

    pub async fn get_access_token_by_token(&self, token: &str) -> Result<AccessToken, sqlx::Error> {
        sqlx::query_as::<_, AccessToken>(
            r#"SELECT id, agent_id, token, description, revoked_at, last_used_at, created_at
               FROM access_tokens 
               WHERE token = $1 AND revoked_at IS NULL"#,
        )
        .bind(token)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn get_access_tokens_for_agent(
        &self,
        agent_id: Uuid,
    ) -> Result<Vec<AccessToken>, sqlx::Error> {
        sqlx::query_as::<_, AccessToken>(
            r#"SELECT id, agent_id, token, description, revoked_at, last_used_at, created_at
               FROM access_tokens 
               WHERE agent_id = $1
               ORDER BY created_at DESC"#,
        )
        .bind(agent_id)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn revoke_access_token(&self, token_id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query(r#"UPDATE access_tokens SET revoked_at = NOW() WHERE id = $1"#)
            .bind(token_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn revoke_all_access_tokens_for_agent(
        &self,
        agent_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"UPDATE access_tokens SET revoked_at = NOW() 
               WHERE agent_id = $1 AND revoked_at IS NULL"#,
        )
        .bind(agent_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_all_agent_tokens(&self) -> Result<Vec<AgentToken>, sqlx::Error> {
        sqlx::query_as::<_, AgentToken>(
            r#"SELECT id, token, name, description, user_id, organization_id, expires_at, created_at 
               FROM agent_tokens 
               ORDER BY created_at DESC"#,
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn get_agent_token_by_id(&self, id: Uuid) -> Result<AgentToken, sqlx::Error> {
        sqlx::query_as::<_, AgentToken>(
            r#"SELECT id, token, name, description, user_id, organization_id, expires_at, created_at 
               FROM agent_tokens WHERE id = $1"#,
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn get_agents_by_registration_token(
        &self,
        token_id: Uuid,
    ) -> Result<Vec<Agent>, sqlx::Error> {
        sqlx::query_as::<_, Agent>(
            r#"SELECT id, uuid, name, hostname, os, arch, version, build, tags, priority,
                      status, registration_token_id, user_id, organization_id, queue_id, last_seen, last_heartbeat,
                      current_job_id, created_at, updated_at
               FROM agents 
               WHERE registration_token_id = $1
               ORDER BY created_at DESC"#,
        )
        .bind(token_id)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn get_stale_agents(&self, timeout_seconds: i64) -> Result<Vec<Agent>, sqlx::Error> {
        sqlx::query_as::<_, Agent>(
            r#"SELECT id, uuid, name, hostname, os, arch, version, build, tags, priority,
                      status, registration_token_id, user_id, organization_id, queue_id, last_seen, last_heartbeat,
                      current_job_id, created_at, updated_at
               FROM agents
               WHERE status = 'connected'
                 AND last_seen < NOW() - make_interval(secs => $1::double precision)"#,
        )
        .bind(timeout_seconds as f64)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn get_agent_by_id_and_user(
        &self,
        id: Uuid,
        user_id: Uuid,
    ) -> Result<Agent, sqlx::Error> {
        sqlx::query_as::<_, Agent>(
            r#"SELECT id, uuid, name, hostname, os, arch, version, build, tags, priority,
                      status, registration_token_id, user_id, organization_id, queue_id, last_seen, last_heartbeat,
                      current_job_id, created_at, updated_at
               FROM agents WHERE id = $1 AND user_id = $2"#,
        )
        .bind(id)
        .bind(user_id)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn get_agents_by_user(&self, user_id: Uuid) -> Result<Vec<Agent>, sqlx::Error> {
        sqlx::query_as::<_, Agent>(
            r#"SELECT id, uuid, name, hostname, os, arch, version, build, tags, priority,
                      status, registration_token_id, user_id, organization_id, queue_id, last_seen, last_heartbeat,
                      current_job_id, created_at, updated_at
               FROM agents 
               WHERE user_id = $1
               ORDER BY created_at DESC"#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn get_agents_by_queue(&self, queue_id: Uuid) -> Result<Vec<Agent>, sqlx::Error> {
        sqlx::query_as::<_, Agent>(
            r#"SELECT id, uuid, name, hostname, os, arch, version, build, tags, priority,
                      status, registration_token_id, user_id, organization_id, queue_id, last_seen, last_heartbeat,
                      current_job_id, created_at, updated_at
               FROM agents 
               WHERE queue_id = $1
               ORDER BY created_at DESC"#,
        )
        .bind(queue_id)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn has_higher_priority_agent_in_queue(
        &self,
        queue_id: Uuid,
        current_agent_id: Uuid,
        current_priority: Option<i32>,
        recent_seconds: f64,
    ) -> Result<bool, sqlx::Error> {
        let count = sqlx::query_scalar::<_, i64>(
            r#"SELECT COUNT(*) FROM agents
               WHERE queue_id = $1
                 AND id != $2
                 AND status = 'connected'
                 AND current_job_id IS NULL
                 AND last_seen > NOW() - make_interval(secs => $3::double precision)
                 AND COALESCE(priority, 0) > COALESCE($4::INT, 0)"#,
        )
        .bind(queue_id)
        .bind(current_agent_id)
        .bind(recent_seconds)
        .bind(current_priority)
        .fetch_one(&self.pool)
        .await?;
        Ok(count > 0)
    }

    pub async fn create_agent_token_for_user(
        &self,
        user_id: Uuid,
        organization_id: Option<Uuid>,
        token: &str,
        name: &str,
        description: Option<&str>,
    ) -> Result<AgentToken, sqlx::Error> {
        sqlx::query_as::<_, AgentToken>(
            r#"INSERT INTO agent_tokens (token, name, description, user_id, organization_id)
               VALUES ($1, $2, $3, $4, $5)
               RETURNING id, token, name, description, user_id, organization_id, expires_at, created_at"#,
        )
        .bind(token)
        .bind(name)
        .bind(description)
        .bind(user_id)
        .bind(organization_id)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn get_agent_tokens_by_user(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<AgentToken>, sqlx::Error> {
        sqlx::query_as::<_, AgentToken>(
            r#"SELECT id, token, name, description, user_id, organization_id, expires_at, created_at 
               FROM agent_tokens 
               WHERE user_id = $1
               ORDER BY created_at DESC"#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
    }

    pub(crate) async fn delete_agent_token_scoped(
        &self,
        id: Uuid,
        user_id: Option<Uuid>,
        org_id: Option<Uuid>,
    ) -> Result<(bool, u64), sqlx::Error> {
        sqlx::query(
            r#"UPDATE jobs SET agent_id = NULL 
               WHERE agent_id IN (SELECT id FROM agents WHERE registration_token_id = $1)"#,
        )
        .bind(id)
        .execute(&self.pool)
        .await?;

        let agents_result = sqlx::query(r#"DELETE FROM agents WHERE registration_token_id = $1"#)
            .bind(id)
            .execute(&self.pool)
            .await?;

        let agents_deleted = agents_result.rows_affected();

        let result = match (user_id, org_id) {
            (Some(uid), None) => {
                sqlx::query(r#"DELETE FROM agent_tokens WHERE id = $1 AND user_id = $2"#)
                    .bind(id)
                    .bind(uid)
                    .execute(&self.pool)
                    .await?
            }
            (None, Some(oid)) => {
                sqlx::query(r#"DELETE FROM agent_tokens WHERE id = $1 AND organization_id = $2"#)
                    .bind(id)
                    .bind(oid)
                    .execute(&self.pool)
                    .await?
            }
            _ => {
                sqlx::query(r#"DELETE FROM agent_tokens WHERE id = $1"#)
                    .bind(id)
                    .execute(&self.pool)
                    .await?
            }
        };

        Ok((result.rows_affected() > 0, agents_deleted))
    }

    pub async fn delete_agent_token(
        &self,
        id: Uuid,
        user_id: Uuid,
    ) -> Result<(bool, u64), sqlx::Error> {
        self.delete_agent_token_scoped(id, Some(user_id), None)
            .await
    }
}
