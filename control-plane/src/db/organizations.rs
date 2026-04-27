use chrono::{NaiveDateTime, Utc};
use uuid::Uuid;

use super::{
    Agent, AgentToken, Database, Organization, OrganizationInvitation, OrganizationMember,
    OrganizationMemberWithUser, Pipeline, Queue,
};

impl Database {
    pub async fn create_organization(&self, name: &str, slug: &str) -> Result<Organization, sqlx::Error> {
        sqlx::query_as::<_, Organization>(
            r#"INSERT INTO organizations (name, slug)
               VALUES ($1, $2)
               RETURNING id, name, slug, created_at, updated_at"#,
        )
        .bind(name)
        .bind(slug)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn get_organization_by_id(&self, id: Uuid) -> Result<Organization, sqlx::Error> {
        sqlx::query_as::<_, Organization>(
            r#"SELECT id, name, slug, created_at, updated_at
               FROM organizations WHERE id = $1"#,
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn get_organization_by_slug(&self, slug: &str) -> Result<Organization, sqlx::Error> {
        sqlx::query_as::<_, Organization>(
            r#"SELECT id, name, slug, created_at, updated_at
               FROM organizations WHERE slug = $1"#,
        )
        .bind(slug)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn get_organizations_for_user(&self, user_id: Uuid) -> Result<Vec<(Organization, String)>, sqlx::Error> {
        let rows = sqlx::query_as::<_, (Uuid, String, String, NaiveDateTime, NaiveDateTime, String)>(
            r#"SELECT o.id, o.name, o.slug, o.created_at, o.updated_at, om.role
               FROM organizations o
               INNER JOIN organization_members om ON om.organization_id = o.id
               WHERE om.user_id = $1
               ORDER BY o.name ASC"#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(id, name, slug, created_at, updated_at, role)| {
                (Organization { id, name, slug, created_at, updated_at }, role)
            })
            .collect())
    }

    pub async fn add_organization_member(
        &self,
        organization_id: Uuid,
        user_id: Uuid,
        role: &str,
    ) -> Result<OrganizationMember, sqlx::Error> {
        sqlx::query_as::<_, OrganizationMember>(
            r#"INSERT INTO organization_members (organization_id, user_id, role)
               VALUES ($1, $2, $3)
               RETURNING id, organization_id, user_id, role, created_at"#,
        )
        .bind(organization_id)
        .bind(user_id)
        .bind(role)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn get_organization_member(
        &self,
        organization_id: Uuid,
        user_id: Uuid,
    ) -> Result<OrganizationMember, sqlx::Error> {
        sqlx::query_as::<_, OrganizationMember>(
            r#"SELECT id, organization_id, user_id, role, created_at
               FROM organization_members
               WHERE organization_id = $1 AND user_id = $2"#,
        )
        .bind(organization_id)
        .bind(user_id)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn get_organization_members(
        &self,
        organization_id: Uuid,
    ) -> Result<Vec<OrganizationMemberWithUser>, sqlx::Error> {
        sqlx::query_as::<_, OrganizationMemberWithUser>(
            r#"SELECT om.id, om.organization_id, om.user_id, om.role, om.created_at,
                      u.email as user_email, u.name as user_name
               FROM organization_members om
               INNER JOIN users u ON u.id = om.user_id
               WHERE om.organization_id = $1
               ORDER BY 
                   CASE om.role 
                       WHEN 'owner' THEN 0 
                       WHEN 'admin' THEN 1 
                       ELSE 2 
                   END,
                   om.created_at ASC"#,
        )
        .bind(organization_id)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn update_organization_member_role(
        &self,
        organization_id: Uuid,
        user_id: Uuid,
        new_role: &str,
    ) -> Result<OrganizationMember, sqlx::Error> {
        sqlx::query_as::<_, OrganizationMember>(
            r#"UPDATE organization_members 
               SET role = $3
               WHERE organization_id = $1 AND user_id = $2
               RETURNING id, organization_id, user_id, role, created_at"#,
        )
        .bind(organization_id)
        .bind(user_id)
        .bind(new_role)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn remove_organization_member(
        &self,
        organization_id: Uuid,
        user_id: Uuid,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            r#"DELETE FROM organization_members 
               WHERE organization_id = $1 AND user_id = $2"#,
        )
        .bind(organization_id)
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn create_organization_invitation(
        &self,
        organization_id: Uuid,
        token: &str,
        created_by: Uuid,
        expires_at: NaiveDateTime,
    ) -> Result<OrganizationInvitation, sqlx::Error> {
        sqlx::query_as::<_, OrganizationInvitation>(
            r#"INSERT INTO organization_invitations (organization_id, token, created_by, expires_at)
               VALUES ($1, $2, $3, $4)
               RETURNING id, organization_id, token, created_by, expires_at, used_by, used_at, created_at"#,
        )
        .bind(organization_id)
        .bind(token)
        .bind(created_by)
        .bind(expires_at)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn get_organization_invitation_by_token(
        &self,
        token: &str,
    ) -> Result<OrganizationInvitation, sqlx::Error> {
        sqlx::query_as::<_, OrganizationInvitation>(
            r#"SELECT id, organization_id, token, created_by, expires_at, used_by, used_at, created_at
               FROM organization_invitations
               WHERE token = $1"#,
        )
        .bind(token)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn use_organization_invitation(
        &self,
        invitation_id: Uuid,
        used_by: Uuid,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"UPDATE organization_invitations 
               SET used_by = $2, used_at = NOW()
               WHERE id = $1"#,
        )
        .bind(invitation_id)
        .bind(used_by)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_organization_invitations(
        &self,
        organization_id: Uuid,
    ) -> Result<Vec<OrganizationInvitation>, sqlx::Error> {
        sqlx::query_as::<_, OrganizationInvitation>(
            r#"SELECT id, organization_id, token, created_by, expires_at, used_by, used_at, created_at
               FROM organization_invitations
               WHERE organization_id = $1
               ORDER BY created_at DESC"#,
        )
        .bind(organization_id)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn get_pipelines_by_organization(&self, org_id: Uuid) -> Result<Vec<Pipeline>, sqlx::Error> {
        sqlx::query_as::<_, Pipeline>(
            r#"SELECT id, slug, repository_url, webhook_secret, config_cache,
                      user_id, organization_id, name, description, default_branch, created_at, updated_at
               FROM pipelines 
               WHERE organization_id = $1
               ORDER BY created_at DESC"#,
        )
        .bind(org_id)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn get_pipeline_by_id_and_org(&self, id: Uuid, org_id: Uuid) -> Result<Pipeline, sqlx::Error> {
        sqlx::query_as::<_, Pipeline>(
            r#"SELECT id, slug, repository_url, webhook_secret, config_cache,
                      user_id, organization_id, name, description, default_branch, created_at, updated_at
               FROM pipelines 
               WHERE id = $1 AND organization_id = $2"#,
        )
        .bind(id)
        .bind(org_id)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn delete_pipeline_by_org(&self, id: Uuid, org_id: Uuid) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(r#"DELETE FROM pipelines WHERE id = $1 AND organization_id = $2"#)
            .bind(id)
            .bind(org_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn update_pipeline_by_org(
        &self,
        id: Uuid,
        org_id: Uuid,
        name: &str,
        description: Option<&str>,
        repository_url: &str,
        default_branch: Option<&str>,
    ) -> Result<Pipeline, sqlx::Error> {
        let now = Utc::now();
        sqlx::query_as::<_, Pipeline>(
            r#"UPDATE pipelines 
               SET name = $3, description = $4, repository_url = $5, default_branch = $6, updated_at = $7
               WHERE id = $1 AND organization_id = $2
               RETURNING id, slug, repository_url, webhook_secret, config_cache,
                         user_id, organization_id, name, description, default_branch, created_at, updated_at"#,
        )
        .bind(id)
        .bind(org_id)
        .bind(name)
        .bind(description)
        .bind(repository_url)
        .bind(default_branch)
        .bind(now)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn get_queues_by_organization(&self, org_id: Uuid) -> Result<Vec<Queue>, sqlx::Error> {
        sqlx::query_as::<_, Queue>(
            r#"SELECT id, user_id, organization_id, pipeline_id, name, key, description, is_default, created_at, updated_at
               FROM queues WHERE organization_id = $1
               ORDER BY created_at DESC"#,
        )
        .bind(org_id)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn get_queue_by_id_and_org(&self, id: Uuid, org_id: Uuid) -> Result<Queue, sqlx::Error> {
        sqlx::query_as::<_, Queue>(
            r#"SELECT id, user_id, organization_id, pipeline_id, name, key, description, is_default, created_at, updated_at
               FROM queues WHERE id = $1 AND organization_id = $2"#,
        )
        .bind(id)
        .bind(org_id)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn get_queue_by_key_and_org(&self, key: &str, org_id: Uuid) -> Result<Queue, sqlx::Error> {
        sqlx::query_as::<_, Queue>(
            r#"SELECT id, user_id, organization_id, pipeline_id, name, key, description, is_default, created_at, updated_at
               FROM queues WHERE key = $1 AND organization_id = $2"#,
        )
        .bind(key)
        .bind(org_id)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn update_queue_by_org(
        &self,
        id: Uuid,
        org_id: Uuid,
        name: &str,
        description: Option<&str>,
    ) -> Result<Queue, sqlx::Error> {
        let now = Utc::now();
        sqlx::query_as::<_, Queue>(
            r#"UPDATE queues 
               SET name = $3, description = $4, updated_at = $5
               WHERE id = $1 AND organization_id = $2
               RETURNING id, user_id, organization_id, pipeline_id, name, key, description, is_default, created_at, updated_at"#,
        )
        .bind(id)
        .bind(org_id)
        .bind(name)
        .bind(description)
        .bind(now)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn delete_queue_by_org(&self, id: Uuid, org_id: Uuid) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            r#"DELETE FROM queues WHERE id = $1 AND organization_id = $2 AND is_default = false"#,
        )
        .bind(id)
        .bind(org_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn get_agent_tokens_by_organization(&self, org_id: Uuid) -> Result<Vec<AgentToken>, sqlx::Error> {
        sqlx::query_as::<_, AgentToken>(
            r#"SELECT id, token, name, description, user_id, organization_id, expires_at, created_at 
               FROM agent_tokens 
               WHERE organization_id = $1
               ORDER BY created_at DESC"#,
        )
        .bind(org_id)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn get_pipeline_by_slug_and_org(&self, slug: &str, org_id: Uuid) -> Result<Pipeline, sqlx::Error> {
        sqlx::query_as::<_, Pipeline>(
            r#"SELECT id, slug, repository_url, webhook_secret, config_cache,
                      user_id, organization_id, name, description, default_branch, created_at, updated_at
               FROM pipelines
               WHERE slug = $1 AND organization_id = $2"#,
        )
        .bind(slug)
        .bind(org_id)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn get_agents_by_organization(&self, org_id: Uuid) -> Result<Vec<Agent>, sqlx::Error> {
        sqlx::query_as::<_, Agent>(
            r#"SELECT id, uuid, name, hostname, os, arch, version, build, tags, priority,
                      status, registration_token_id, user_id, organization_id, queue_id, last_seen, last_heartbeat,
                      current_job_id, created_at, updated_at
               FROM agents 
               WHERE organization_id = $1
               ORDER BY created_at DESC"#,
        )
        .bind(org_id)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn delete_agent_token_by_org(&self, id: Uuid, org_id: Uuid) -> Result<(bool, u64), sqlx::Error> {
        self.delete_agent_token_scoped(id, None, Some(org_id)).await
    }

    pub async fn count_organizations_for_user(&self, user_id: Uuid) -> Result<i64, sqlx::Error> {
        sqlx::query_scalar::<_, i64>(
            r#"SELECT COUNT(*)::BIGINT FROM organization_members WHERE user_id = $1"#,
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn get_organizations_for_user_paginated(
        &self,
        user_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<(Organization, String)>, sqlx::Error> {
        let rows = sqlx::query_as::<_, (Uuid, String, String, NaiveDateTime, NaiveDateTime, String)>(
            r#"SELECT o.id, o.name, o.slug, o.created_at, o.updated_at, om.role
               FROM organizations o
               INNER JOIN organization_members om ON om.organization_id = o.id
               WHERE om.user_id = $1
               ORDER BY o.name ASC
               LIMIT $2 OFFSET $3"#,
        )
        .bind(user_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(id, name, slug, created_at, updated_at, role)| {
                (Organization { id, name, slug, created_at, updated_at }, role)
            })
            .collect())
    }

    pub async fn count_pipelines_by_organization(&self, org_id: Uuid) -> Result<i64, sqlx::Error> {
        sqlx::query_scalar::<_, i64>(
            r#"SELECT COUNT(*)::BIGINT FROM pipelines WHERE organization_id = $1"#,
        )
        .bind(org_id)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn get_pipelines_by_organization_paginated(
        &self,
        org_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Pipeline>, sqlx::Error> {
        sqlx::query_as::<_, Pipeline>(
            r#"SELECT id, slug, repository_url, webhook_secret, config_cache,
                      user_id, organization_id, name, description, default_branch, created_at, updated_at
               FROM pipelines
               WHERE organization_id = $1
               ORDER BY created_at DESC
               LIMIT $2 OFFSET $3"#,
        )
        .bind(org_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn count_queues_by_organization(&self, org_id: Uuid) -> Result<i64, sqlx::Error> {
        sqlx::query_scalar::<_, i64>(
            r#"SELECT COUNT(*)::BIGINT FROM queues WHERE organization_id = $1"#,
        )
        .bind(org_id)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn get_queues_by_organization_paginated(
        &self,
        org_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Queue>, sqlx::Error> {
        sqlx::query_as::<_, Queue>(
            r#"SELECT id, user_id, organization_id, pipeline_id, name, key, description, is_default, created_at, updated_at
               FROM queues WHERE organization_id = $1
               ORDER BY created_at DESC
               LIMIT $2 OFFSET $3"#,
        )
        .bind(org_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn count_agent_tokens_by_organization(&self, org_id: Uuid) -> Result<i64, sqlx::Error> {
        sqlx::query_scalar::<_, i64>(
            r#"SELECT COUNT(*)::BIGINT FROM agent_tokens WHERE organization_id = $1"#,
        )
        .bind(org_id)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn get_agent_tokens_by_organization_paginated(
        &self,
        org_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<AgentToken>, sqlx::Error> {
        sqlx::query_as::<_, AgentToken>(
            r#"SELECT id, token, name, description, user_id, organization_id, expires_at, created_at
               FROM agent_tokens
               WHERE organization_id = $1
               ORDER BY created_at DESC
               LIMIT $2 OFFSET $3"#,
        )
        .bind(org_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn count_agents_by_organization(&self, org_id: Uuid) -> Result<i64, sqlx::Error> {
        sqlx::query_scalar::<_, i64>(
            r#"SELECT COUNT(*)::BIGINT FROM agents WHERE organization_id = $1"#,
        )
        .bind(org_id)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn get_agents_by_organization_paginated(
        &self,
        org_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Agent>, sqlx::Error> {
        sqlx::query_as::<_, Agent>(
            r#"SELECT id, uuid, name, hostname, os, arch, version, build, tags, priority,
                      status, registration_token_id, user_id, organization_id, queue_id, last_seen, last_heartbeat,
                      current_job_id, created_at, updated_at
               FROM agents
               WHERE organization_id = $1
               ORDER BY created_at DESC
               LIMIT $2 OFFSET $3"#,
        )
        .bind(org_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
    }
}
