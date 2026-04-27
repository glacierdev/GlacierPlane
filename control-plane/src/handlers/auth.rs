use std::sync::Arc;

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, Duration, Utc};
use serde_json::json;

use crate::{
    db::User,
    error::AppError,
    types::{UserLoginRequest, UserRegisterRequest, UserResponse},
    AppState,
};

use super::{extract_session_token, generate_secure_token, get_authenticated_user};

async fn create_session_response(
    state: &Arc<AppState>,
    user: &User,
    status: StatusCode,
) -> Result<
    (
        StatusCode,
        [(&'static str, String); 1],
        Json<serde_json::Value>,
    ),
    AppError,
> {
    let session_token = generate_secure_token(32);
    let expires_at = Utc::now().naive_utc() + Duration::days(7);
    state
        .db
        .create_user_session(user.id, &session_token, expires_at)
        .await?;

    let response = UserResponse {
        id: user.id,
        email: user.email.clone(),
        name: user.name.clone(),
        created_at: DateTime::<Utc>::from_naive_utc_and_offset(user.created_at, Utc).to_rfc3339(),
    };

    Ok((
        status,
        [(
            "Set-Cookie",
            format!(
                "session={}; HttpOnly; Path=/; Max-Age=604800; SameSite=Lax",
                session_token
            ),
        )],
        Json(json!({ "user": response, "token": session_token })),
    ))
}

pub async fn user_register(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<UserRegisterRequest>,
) -> Result<impl IntoResponse, AppError> {
    tracing::info!("User registration request for email: {}", payload.email);

    if payload.email.is_empty() || payload.name.is_empty() || payload.password.is_empty() {
        return Err(AppError::Http(StatusCode::BAD_REQUEST, "Email, name, and password are required".into()));
    }
    if !payload.email.contains('@') {
        return Err(AppError::Http(StatusCode::BAD_REQUEST, "Invalid email format".into()));
    }
    if payload.password.len() < 6 {
        return Err(AppError::Http(StatusCode::BAD_REQUEST, "Password must be at least 6 characters".into()));
    }
    if state.db.user_exists(&payload.email).await? {
        return Err(AppError::Http(StatusCode::CONFLICT, "User with this email already exists".into()));
    }

    let password_hash = bcrypt::hash(&payload.password, bcrypt::DEFAULT_COST)
        .map_err(|e| AppError::Message(format!("Failed to hash password: {}", e)))?;

    let user = state.db.create_user(&payload.email, &payload.name, &password_hash).await?;
    tracing::info!("User registered successfully: {} (id: {})", user.email, user.id);

    create_session_response(&state, &user, StatusCode::CREATED).await
}

pub async fn user_login(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<UserLoginRequest>,
) -> Result<impl IntoResponse, AppError> {
    tracing::info!("User login attempt for email: {}", payload.email);

    if payload.email.is_empty() || payload.password.is_empty() {
        return Err(AppError::Http(StatusCode::BAD_REQUEST, "Email and password are required".into()));
    }

    let user = state
        .db
        .get_user_by_email(&payload.email)
        .await
        .map_err(|_| AppError::Http(StatusCode::UNAUTHORIZED, "Invalid email or password".into()))?;

    let valid = bcrypt::verify(&payload.password, &user.password_hash)
        .map_err(|e| AppError::Message(format!("Failed to verify password: {}", e)))?;
    if !valid {
        return Err(AppError::Http(StatusCode::UNAUTHORIZED, "Invalid email or password".into()));
    }

    tracing::info!("User logged in: {} (id: {})", user.email, user.id);
    create_session_response(&state, &user, StatusCode::OK).await
}

pub async fn user_me(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let user = get_authenticated_user(&state, &headers).await?;

    let response = UserResponse {
        id: user.id,
        email: user.email,
        name: user.name,
        created_at: DateTime::<Utc>::from_naive_utc_and_offset(user.created_at, Utc).to_rfc3339(),
    };

    Ok(Json(response))
}

pub async fn user_logout(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    if let Some(token) = extract_session_token(&headers) {
        state.db.delete_user_session(&token).await?;
        tracing::info!("User logged out");
    }

    Ok((
        StatusCode::OK,
        [("Set-Cookie", "session=; HttpOnly; Path=/; Max-Age=0; SameSite=Lax".to_string())],
        Json(json!({ "message": "Logged out successfully" })),
    ))
}
