use std::sync::Arc;

use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
};

use crate::handlers::parse_authorization_token;
use crate::{db::Agent, error::AppError, AppState};

#[derive(Clone, Debug)]
pub struct AuthenticatedAgent {
    pub agent: Agent,
    pub access_token: String,
}

pub async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, AppError> {
    let token = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(parse_authorization_token)
        .ok_or_else(|| {
            AppError::Http(
                StatusCode::UNAUTHORIZED,
                "Missing authorization header".into(),
            )
        })?;

    let agent = state
        .db
        .get_agent_by_access_token(&token)
        .await
        .map_err(|_| AppError::Http(StatusCode::UNAUTHORIZED, "Invalid or revoked token".into()))?;

    let auth_agent = AuthenticatedAgent {
        agent: agent.clone(),
        access_token: token,
    };

    req.extensions_mut().insert(agent);
    req.extensions_mut().insert(auth_agent);

    Ok(next.run(req).await)
}
