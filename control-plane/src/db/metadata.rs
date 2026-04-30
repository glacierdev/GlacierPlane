use uuid::Uuid;

use super::Database;

impl Database {
    pub async fn metadata_exists(&self, build_id: Uuid, key: &str) -> Result<bool, sqlx::Error> {
        let count = sqlx::query_scalar::<_, i64>(
            r#"SELECT COUNT(*) FROM metadata WHERE build_id = $1 AND key = $2"#,
        )
        .bind(build_id)
        .bind(key)
        .fetch_one(&self.pool)
        .await?;
        Ok(count > 0)
    }

    pub async fn get_metadata(
        &self,
        build_id: Uuid,
        key: &str,
    ) -> Result<Option<String>, sqlx::Error> {
        let result = sqlx::query_scalar::<_, String>(
            r#"SELECT value FROM metadata WHERE build_id = $1 AND key = $2"#,
        )
        .bind(build_id)
        .bind(key)
        .fetch_optional(&self.pool)
        .await?;
        Ok(result)
    }

    pub async fn set_metadata(
        &self,
        build_id: Uuid,
        key: &str,
        value: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"INSERT INTO metadata (build_id, key, value)
               VALUES ($1, $2, $3)
               ON CONFLICT (build_id, key) DO UPDATE SET
                    value = EXCLUDED.value,
                    updated_at = NOW()"#,
        )
        .bind(build_id)
        .bind(key)
        .bind(value)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_metadata_keys(&self, build_id: Uuid) -> Result<Vec<String>, sqlx::Error> {
        sqlx::query_scalar::<_, String>(
            r#"SELECT key FROM metadata WHERE build_id = $1 ORDER BY key"#,
        )
        .bind(build_id)
        .fetch_all(&self.pool)
        .await
    }
}
