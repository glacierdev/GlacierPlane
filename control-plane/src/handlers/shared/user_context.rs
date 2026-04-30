use std::sync::Arc;

use axum::http::{HeaderMap, StatusCode};
use uuid::Uuid;

use crate::{db::User, error::AppError, AppState};

pub(crate) fn extract_session_token(headers: &HeaderMap) -> Option<String> {
    if let Some(auth_header) = headers.get(axum::http::header::AUTHORIZATION) {
        if let Ok(auth_str) = auth_header.to_str() {
            if let Some(token) = auth_str.strip_prefix("Bearer ") {
                return Some(token.to_string());
            }
        }
    }
    if let Some(cookie_header) = headers.get(axum::http::header::COOKIE) {
        if let Ok(cookie_str) = cookie_header.to_str() {
            for cookie in cookie_str.split(';') {
                let cookie = cookie.trim();
                if let Some(token) = cookie.strip_prefix("session=") {
                    return Some(token.to_string());
                }
            }
        }
    }
    None
}

pub(crate) async fn get_authenticated_user(
    state: &Arc<AppState>,
    headers: &HeaderMap,
) -> Result<User, AppError> {
    let token = extract_session_token(headers)
        .ok_or_else(|| AppError::Http(StatusCode::UNAUTHORIZED, "Not authenticated".into()))?;
    state
        .db
        .get_user_by_session_token(&token)
        .await
        .map_err(|_| {
            AppError::Http(
                StatusCode::UNAUTHORIZED,
                "Invalid or expired session".into(),
            )
        })
}

pub(crate) async fn get_user_and_org_by_slug(
    state: &Arc<AppState>,
    headers: &HeaderMap,
    org_slug: &str,
) -> Result<(User, Uuid), AppError> {
    let user = get_authenticated_user(state, headers).await?;
    let org = state
        .db
        .get_organization_by_slug(org_slug)
        .await
        .map_err(|_| AppError::Http(StatusCode::NOT_FOUND, "Organization not found".into()))?;
    state
        .db
        .get_organization_member(org.id, user.id)
        .await
        .map_err(|_| {
            AppError::Http(
                StatusCode::FORBIDDEN,
                "Not a member of this organization".into(),
            )
        })?;
    Ok((user, org.id))
}

pub(crate) async fn get_user_and_org_admin_by_slug(
    state: &Arc<AppState>,
    headers: &HeaderMap,
    org_slug: &str,
) -> Result<(User, Uuid, String), AppError> {
    let user = get_authenticated_user(state, headers).await?;
    let org = state
        .db
        .get_organization_by_slug(org_slug)
        .await
        .map_err(|_| AppError::Http(StatusCode::NOT_FOUND, "Organization not found".into()))?;
    let member = state
        .db
        .get_organization_member(org.id, user.id)
        .await
        .map_err(|_| {
            AppError::Http(
                StatusCode::FORBIDDEN,
                "Not a member of this organization".into(),
            )
        })?;
    if member.role != "owner" && member.role != "admin" {
        return Err(AppError::Http(
            StatusCode::FORBIDDEN,
            "Admin or owner role required".into(),
        ));
    }
    Ok((user, org.id, member.role))
}
