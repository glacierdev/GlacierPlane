use chrono::Utc;
use uuid::Uuid;

use super::{Database, Queue};

impl Database {
    pub async fn create_queue(
        &self,
        user_id: Uuid,
        organization_id: Option<Uuid>,
        pipeline_id: Option<Uuid>,
        name: &str,
        key: &str,
        description: Option<&str>,
        is_default: bool,
    ) -> Result<Queue, sqlx::Error> {
        let id = Uuid::new_v4();
        let now = Utc::now();

        sqlx::query_as::<_, Queue>(
            r#"INSERT INTO queues (id, user_id, organization_id, pipeline_id, name, key, description, is_default, created_at, updated_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
               RETURNING id, user_id, organization_id, pipeline_id, name, key, description, is_default, created_at, updated_at"#,
        )
        .bind(id)
        .bind(user_id)
        .bind(organization_id)
        .bind(pipeline_id)
        .bind(name)
        .bind(key)
        .bind(description)
        .bind(is_default)
        .bind(now)
        .bind(now)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn get_queue_by_id(&self, id: Uuid) -> Result<Queue, sqlx::Error> {
        sqlx::query_as::<_, Queue>(
            r#"SELECT id, user_id, organization_id, pipeline_id, name, key, description, is_default, created_at, updated_at
               FROM queues WHERE id = $1"#,
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn get_queue_by_id_and_user(&self, id: Uuid, user_id: Uuid) -> Result<Queue, sqlx::Error> {
        sqlx::query_as::<_, Queue>(
            r#"SELECT id, user_id, organization_id, pipeline_id, name, key, description, is_default, created_at, updated_at
               FROM queues WHERE id = $1 AND user_id = $2"#,
        )
        .bind(id)
        .bind(user_id)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn get_queue_by_key_and_user(&self, key: &str, user_id: Uuid) -> Result<Queue, sqlx::Error> {
        sqlx::query_as::<_, Queue>(
            r#"SELECT id, user_id, organization_id, pipeline_id, name, key, description, is_default, created_at, updated_at
               FROM queues WHERE key = $1 AND user_id = $2"#,
        )
        .bind(key)
        .bind(user_id)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn get_queues_by_user(&self, user_id: Uuid) -> Result<Vec<Queue>, sqlx::Error> {
        sqlx::query_as::<_, Queue>(
            r#"SELECT id, user_id, organization_id, pipeline_id, name, key, description, is_default, created_at, updated_at
               FROM queues WHERE user_id = $1
               ORDER BY created_at DESC"#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn get_queues_by_pipeline(&self, pipeline_id: Uuid) -> Result<Vec<Queue>, sqlx::Error> {
        sqlx::query_as::<_, Queue>(
            r#"SELECT id, user_id, organization_id, pipeline_id, name, key, description, is_default, created_at, updated_at
               FROM queues WHERE pipeline_id = $1
               ORDER BY is_default DESC, created_at ASC"#,
        )
        .bind(pipeline_id)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn get_default_queue_for_pipeline(&self, pipeline_id: Uuid) -> Result<Queue, sqlx::Error> {
        sqlx::query_as::<_, Queue>(
            r#"SELECT id, user_id, organization_id, pipeline_id, name, key, description, is_default, created_at, updated_at
               FROM queues WHERE pipeline_id = $1 AND is_default = true"#,
        )
        .bind(pipeline_id)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn update_queue(
        &self,
        id: Uuid,
        user_id: Uuid,
        name: &str,
        description: Option<&str>,
    ) -> Result<Queue, sqlx::Error> {
        let now = Utc::now();

        sqlx::query_as::<_, Queue>(
            r#"UPDATE queues 
               SET name = $3, description = $4, updated_at = $5
               WHERE id = $1 AND user_id = $2
               RETURNING id, user_id, organization_id, pipeline_id, name, key, description, is_default, created_at, updated_at"#,
        )
        .bind(id)
        .bind(user_id)
        .bind(name)
        .bind(description)
        .bind(now)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn delete_queue(&self, id: Uuid, user_id: Uuid) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            r#"DELETE FROM queues WHERE id = $1 AND user_id = $2 AND is_default = false"#,
        )
        .bind(id)
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn delete_default_queue_for_pipeline(&self, id: Uuid) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(r#"DELETE FROM queues WHERE id = $1 AND is_default = true"#)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}
