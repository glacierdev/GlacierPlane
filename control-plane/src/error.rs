use axum::{http::StatusCode, response::IntoResponse, Json};
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("{0}")]
    Http(StatusCode, String),
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Yaml(#[from] serde_yaml::Error),
    #[error("{0}")]
    Message(String),
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match &self {
            AppError::Http(code, msg) => (*code, msg.clone()),
            AppError::Sqlx(err) => {
                tracing::error!("database error: {err:?}");
                let msg = match err {
                    sqlx::Error::RowNotFound => "Not found".to_string(),
                    sqlx::Error::Database(db_err) => {
                        if let Some(code) = db_err.code() {
                            match code.as_ref() {
                                "23505" => "A record with this value already exists".to_string(),
                                "42P01" => "Database table not found. Please run migrations.".to_string(),
                                _ => format!("Database error: {}", db_err.message())
                            }
                        } else {
                            format!("Database error: {}", db_err.message())
                        }
                    }
                    _ => format!("Database error: {}", err)
                };
                (StatusCode::INTERNAL_SERVER_ERROR, msg)
            }
            AppError::Json(err) => {
                tracing::warn!("json error: {err:?}");
                (StatusCode::BAD_REQUEST, format!("Invalid JSON: {}", err))
            }
            AppError::Yaml(err) => {
                tracing::warn!("yaml error: {err:?}");
                (StatusCode::BAD_REQUEST, format!("Invalid YAML: {}", err))
            }
            AppError::Message(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
        };

        (status, Json(ErrorBody { error: message })).into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;
