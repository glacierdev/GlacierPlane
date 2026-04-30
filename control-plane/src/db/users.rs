use chrono::NaiveDateTime;
use uuid::Uuid;

use super::{Database, User, UserSession};

impl Database {
    pub async fn create_user(
        &self,
        email: &str,
        name: &str,
        password_hash: &str,
    ) -> Result<User, sqlx::Error> {
        sqlx::query_as::<_, User>(
            r#"INSERT INTO users (email, name, password_hash)
               VALUES ($1, $2, $3)
               RETURNING id, email, name, password_hash, created_at, updated_at"#,
        )
        .bind(email)
        .bind(name)
        .bind(password_hash)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn get_user_by_email(&self, email: &str) -> Result<User, sqlx::Error> {
        sqlx::query_as::<_, User>(
            r#"SELECT id, email, name, password_hash, created_at, updated_at
               FROM users WHERE email = $1"#,
        )
        .bind(email)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn get_user_by_id(&self, id: Uuid) -> Result<User, sqlx::Error> {
        sqlx::query_as::<_, User>(
            r#"SELECT id, email, name, password_hash, created_at, updated_at
               FROM users WHERE id = $1"#,
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn user_exists(&self, email: &str) -> Result<bool, sqlx::Error> {
        let count = sqlx::query_scalar::<_, i64>(r#"SELECT COUNT(*) FROM users WHERE email = $1"#)
            .bind(email)
            .fetch_one(&self.pool)
            .await?;
        Ok(count > 0)
    }

    pub async fn create_user_session(
        &self,
        user_id: Uuid,
        token: &str,
        expires_at: NaiveDateTime,
    ) -> Result<UserSession, sqlx::Error> {
        sqlx::query_as::<_, UserSession>(
            r#"INSERT INTO user_sessions (user_id, token, expires_at)
               VALUES ($1, $2, $3)
               RETURNING id, user_id, token, expires_at, created_at"#,
        )
        .bind(user_id)
        .bind(token)
        .bind(expires_at)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn get_user_session_by_token(&self, token: &str) -> Result<UserSession, sqlx::Error> {
        sqlx::query_as::<_, UserSession>(
            r#"SELECT id, user_id, token, expires_at, created_at
               FROM user_sessions 
               WHERE token = $1 AND expires_at > NOW()"#,
        )
        .bind(token)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn get_user_by_session_token(&self, token: &str) -> Result<User, sqlx::Error> {
        sqlx::query_as::<_, User>(
            r#"SELECT u.id, u.email, u.name, u.password_hash, u.created_at, u.updated_at
               FROM users u
               INNER JOIN user_sessions s ON s.user_id = u.id
               WHERE s.token = $1 AND s.expires_at > NOW()"#,
        )
        .bind(token)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn delete_user_session(&self, token: &str) -> Result<(), sqlx::Error> {
        sqlx::query(r#"DELETE FROM user_sessions WHERE token = $1"#)
            .bind(token)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn delete_all_user_sessions(&self, user_id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query(r#"DELETE FROM user_sessions WHERE user_id = $1"#)
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn cleanup_expired_sessions(&self) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(r#"DELETE FROM user_sessions WHERE expires_at < NOW()"#)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }
}
