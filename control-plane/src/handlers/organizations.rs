use std::sync::Arc;

use axum::{
    extract::{OriginalUri, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, Duration, Utc};
use serde_json::json;
use uuid::Uuid;

use crate::{
    error::AppError,
    types::{
        OrganizationCreateRequest, OrganizationDetailResponse, OrganizationInvitationResponse,
        OrganizationMemberResponse, OrganizationResponse, PaginationParams,
        UpdateMemberRoleRequest,
    },
    AppState,
};

use super::{
    generate_secure_token, get_authenticated_user, get_user_and_org_admin_by_slug, paginate_params,
    paginated_response,
};

pub async fn list_organizations(
    State(state): State<Arc<AppState>>,
    Query(pagination): Query<PaginationParams>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let user = get_authenticated_user(&state, &headers).await?;
    let (page, per_page, limit, offset) = paginate_params(&pagination);
    let total = state.db.count_organizations_for_user(user.id).await?;
    let orgs = state
        .db
        .get_organizations_for_user_paginated(user.id, limit, offset)
        .await?;

    let response: Vec<OrganizationResponse> = orgs
        .into_iter()
        .map(|(org, role)| OrganizationResponse {
            id: org.id,
            name: org.name,
            slug: org.slug,
            role,
            created_at: DateTime::<Utc>::from_naive_utc_and_offset(org.created_at, Utc)
                .to_rfc3339(),
            updated_at: DateTime::<Utc>::from_naive_utc_and_offset(org.updated_at, Utc)
                .to_rfc3339(),
        })
        .collect();

    Ok(paginated_response(
        response,
        page,
        per_page,
        total,
        uri.path(),
    ))
}

pub async fn create_organization(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<OrganizationCreateRequest>,
) -> Result<impl IntoResponse, AppError> {
    let user = get_authenticated_user(&state, &headers).await?;

    if payload.name.is_empty() || payload.slug.is_empty() {
        return Err(AppError::Http(
            StatusCode::BAD_REQUEST,
            "Name and slug are required".into(),
        ));
    }
    if !payload
        .slug
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-')
    {
        return Err(AppError::Http(
            StatusCode::BAD_REQUEST,
            "Slug must contain only alphanumeric characters and hyphens".into(),
        ));
    }
    if state
        .db
        .get_organization_by_slug(&payload.slug)
        .await
        .is_ok()
    {
        return Err(AppError::Http(
            StatusCode::CONFLICT,
            "An organization with this slug already exists".into(),
        ));
    }

    let org = state
        .db
        .create_organization(&payload.name, &payload.slug)
        .await?;
    state
        .db
        .add_organization_member(org.id, user.id, "owner")
        .await?;

    tracing::info!(
        "Organization created: {} (id: {}) by user {}",
        org.slug,
        org.id,
        user.id
    );

    let response = OrganizationResponse {
        id: org.id,
        name: org.name,
        slug: org.slug,
        role: "owner".to_string(),
        created_at: DateTime::<Utc>::from_naive_utc_and_offset(org.created_at, Utc).to_rfc3339(),
        updated_at: DateTime::<Utc>::from_naive_utc_and_offset(org.updated_at, Utc).to_rfc3339(),
    };

    Ok((StatusCode::CREATED, Json(response)))
}

pub async fn get_organization(
    State(state): State<Arc<AppState>>,
    Path(org_slug): Path<String>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let (_user, org_id, role) = get_user_and_org_admin_by_slug(&state, &headers, &org_slug).await?;

    let org = state
        .db
        .get_organization_by_id(org_id)
        .await
        .map_err(|_| AppError::Http(StatusCode::NOT_FOUND, "Organization not found".into()))?;

    let members = state.db.get_organization_members(org_id).await?;
    let invitations = state.db.get_organization_invitations(org_id).await?;

    let members_resp: Vec<OrganizationMemberResponse> = members
        .into_iter()
        .map(|m| OrganizationMemberResponse {
            id: m.id,
            user_id: m.user_id,
            email: m.user_email,
            name: m.user_name,
            role: m.role,
            created_at: DateTime::<Utc>::from_naive_utc_and_offset(m.created_at, Utc).to_rfc3339(),
        })
        .collect();

    let invitations_resp: Vec<OrganizationInvitationResponse> = invitations
        .into_iter()
        .map(|inv| OrganizationInvitationResponse {
            id: inv.id,
            token: inv.token.clone(),
            invite_url: format!("/join/{}", inv.token),
            expires_at: DateTime::<Utc>::from_naive_utc_and_offset(inv.expires_at, Utc)
                .to_rfc3339(),
            created_at: DateTime::<Utc>::from_naive_utc_and_offset(inv.created_at, Utc)
                .to_rfc3339(),
            used: inv.used_by.is_some(),
        })
        .collect();

    let response = OrganizationDetailResponse {
        organization: OrganizationResponse {
            id: org.id,
            name: org.name,
            slug: org.slug,
            role,
            created_at: DateTime::<Utc>::from_naive_utc_and_offset(org.created_at, Utc)
                .to_rfc3339(),
            updated_at: DateTime::<Utc>::from_naive_utc_and_offset(org.updated_at, Utc)
                .to_rfc3339(),
        },
        members: members_resp,
        invitations: invitations_resp,
    };

    Ok(Json(response))
}

pub async fn create_organization_invitation(
    State(state): State<Arc<AppState>>,
    Path(org_slug): Path<String>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let (user, org_id, _role) = get_user_and_org_admin_by_slug(&state, &headers, &org_slug).await?;

    let token = generate_secure_token(32);
    let expires_at = (Utc::now() + Duration::days(7)).naive_utc();

    let invitation = state
        .db
        .create_organization_invitation(org_id, &token, user.id, expires_at)
        .await?;

    tracing::info!(
        "Organization invitation created for org {} by user {}",
        org_id,
        user.id
    );

    let response = OrganizationInvitationResponse {
        id: invitation.id,
        token: invitation.token.clone(),
        invite_url: format!("/join/{}", invitation.token),
        expires_at: DateTime::<Utc>::from_naive_utc_and_offset(invitation.expires_at, Utc)
            .to_rfc3339(),
        created_at: DateTime::<Utc>::from_naive_utc_and_offset(invitation.created_at, Utc)
            .to_rfc3339(),
        used: false,
    };

    Ok((StatusCode::CREATED, Json(response)))
}

pub async fn join_organization(
    State(state): State<Arc<AppState>>,
    Path(token): Path<String>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let user = get_authenticated_user(&state, &headers).await?;

    let invitation = state
        .db
        .get_organization_invitation_by_token(&token)
        .await
        .map_err(|_| AppError::Http(StatusCode::NOT_FOUND, "Invalid invitation link".into()))?;

    if invitation.expires_at < Utc::now().naive_utc() {
        return Err(AppError::Http(
            StatusCode::BAD_REQUEST,
            "Invitation has expired".into(),
        ));
    }
    if invitation.used_by.is_some() {
        return Err(AppError::Http(
            StatusCode::BAD_REQUEST,
            "Invitation has already been used".into(),
        ));
    }
    if state
        .db
        .get_organization_member(invitation.organization_id, user.id)
        .await
        .is_ok()
    {
        return Err(AppError::Http(
            StatusCode::CONFLICT,
            "Already a member of this organization".into(),
        ));
    }

    state
        .db
        .add_organization_member(invitation.organization_id, user.id, "member")
        .await?;
    state
        .db
        .use_organization_invitation(invitation.id, user.id)
        .await?;

    let org = state
        .db
        .get_organization_by_id(invitation.organization_id)
        .await?;
    tracing::info!(
        "User {} joined organization {} via invitation",
        user.id,
        org.slug
    );

    let response = OrganizationResponse {
        id: org.id,
        name: org.name,
        slug: org.slug,
        role: "member".to_string(),
        created_at: DateTime::<Utc>::from_naive_utc_and_offset(org.created_at, Utc).to_rfc3339(),
        updated_at: DateTime::<Utc>::from_naive_utc_and_offset(org.updated_at, Utc).to_rfc3339(),
    };

    Ok(Json(response))
}

pub async fn update_member_role(
    State(state): State<Arc<AppState>>,
    Path((org_slug, target_user_id)): Path<(String, Uuid)>,
    headers: HeaderMap,
    Json(payload): Json<UpdateMemberRoleRequest>,
) -> Result<impl IntoResponse, AppError> {
    let (user, org_id, caller_role) =
        get_user_and_org_admin_by_slug(&state, &headers, &org_slug).await?;

    let target_member = state
        .db
        .get_organization_member(org_id, target_user_id)
        .await
        .map_err(|_| AppError::Http(StatusCode::NOT_FOUND, "Member not found".into()))?;
    if target_member.role == "owner" {
        return Err(AppError::Http(
            StatusCode::FORBIDDEN,
            "Cannot change the owner's role".into(),
        ));
    }
    if payload.role == "owner" {
        return Err(AppError::Http(
            StatusCode::FORBIDDEN,
            "Cannot assign owner role".into(),
        ));
    }
    if payload.role != "admin" && payload.role != "member" {
        return Err(AppError::Http(
            StatusCode::BAD_REQUEST,
            "Role must be 'admin' or 'member'".into(),
        ));
    }
    if payload.role == "admin" && caller_role != "owner" {
        return Err(AppError::Http(
            StatusCode::FORBIDDEN,
            "Only the owner can promote members to admin".into(),
        ));
    }

    let updated = state
        .db
        .update_organization_member_role(org_id, target_user_id, &payload.role)
        .await?;
    tracing::info!(
        "Member {} role updated to {} in org {} by {}",
        target_user_id,
        payload.role,
        org_id,
        user.id
    );

    Ok(Json(json!({
        "id": updated.id,
        "user_id": updated.user_id,
        "role": updated.role
    })))
}

pub async fn remove_member(
    State(state): State<Arc<AppState>>,
    Path((org_slug, target_user_id)): Path<(String, Uuid)>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let (user, org_id, _role) = get_user_and_org_admin_by_slug(&state, &headers, &org_slug).await?;

    let target_member = state
        .db
        .get_organization_member(org_id, target_user_id)
        .await
        .map_err(|_| AppError::Http(StatusCode::NOT_FOUND, "Member not found".into()))?;
    if target_member.role == "owner" {
        return Err(AppError::Http(
            StatusCode::FORBIDDEN,
            "Cannot remove the owner".into(),
        ));
    }

    state
        .db
        .remove_organization_member(org_id, target_user_id)
        .await?;
    tracing::info!(
        "Member {} removed from org {} by {}",
        target_user_id,
        org_id,
        user.id
    );

    Ok(Json(json!({ "message": "Member removed successfully" })))
}
