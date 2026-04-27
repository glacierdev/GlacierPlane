#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/docker_helpers.sh"

start_all

log_step ":heartbeat: 2b.1 Health check"
parse_response "$(api_call GET /api/health)"
assert_status "GET /api/health" "200" "$STATUS"

log_step ":bust_in_silhouette: 2b.2 User registration"
parse_response "$(api_call POST /api/auth/register \
    -H 'Content-Type: application/json' \
    -d '{"email":"test@example.com","name":"Test User","password":"testpass123"}')"
assert_status "POST /api/auth/register" "201" "$STATUS"
assert_json_not_empty "register returns user id" "$BODY" ".user.id"
assert_json_field "register returns email" "$BODY" ".user.email" "test@example.com"
assert_json_not_empty "register returns token" "$BODY" ".token"

USER_TOKEN=$(echo "$BODY" | jq -r '.token')
USER_ID=$(echo "$BODY" | jq -r '.user.id')

log_step ":key: 2b.3 User login"
parse_response "$(api_call POST /api/auth/login \
    -H 'Content-Type: application/json' \
    -d '{"email":"test@example.com","password":"testpass123"}')"
assert_status "POST /api/auth/login" "200" "$STATUS"
assert_json_not_empty "login returns token" "$BODY" ".token"

LOGIN_TOKEN=$(echo "$BODY" | jq -r '.token')

log_step ":no_entry: 2b.4 Login with wrong password"
parse_response "$(api_call POST /api/auth/login \
    -H 'Content-Type: application/json' \
    -d '{"email":"test@example.com","password":"wrongpassword"}')"
assert_status "POST /api/auth/login wrong password" "401" "$STATUS"

log_step ":white_check_mark: 2b.5 Session validation"
parse_response "$(api_call GET /api/auth/me \
    -H "Authorization: Bearer $USER_TOKEN")"
assert_status "GET /api/auth/me" "200" "$STATUS"
assert_json_field "me returns correct id" "$BODY" ".id" "$USER_ID"

log_step ":lock: 2b.6 No token → 401"
parse_response "$(api_call GET /api/auth/me)"
assert_status "GET /api/auth/me no token" "401" "$STATUS"

log_step ":lock: 2b.7 Invalid token → 401"
parse_response "$(api_call GET /api/auth/me \
    -H 'Authorization: Bearer invalid-garbage-token')"
assert_status "GET /api/auth/me invalid token" "401" "$STATUS"

log_step ":office: 2b.8 Organization creation"
parse_response "$(api_call POST /api/v2/organizations \
    -H 'Content-Type: application/json' \
    -H "Authorization: Bearer $USER_TOKEN" \
    -d '{"name":"Test Org","slug":"test-org"}')"
assert_status "POST /api/v2/organizations" "201" "$STATUS"
assert_json_not_empty "org returns id" "$BODY" ".id"
assert_json_not_empty "org returns slug" "$BODY" ".slug"

ORG_ID=$(echo "$BODY" | jq -r '.id')
ORG_SLUG=$(echo "$BODY" | jq -r '.slug')

log_step ":pipeline: 2b.9 Pipeline creation"
parse_response "$(api_call POST "/api/v2/organizations/$ORG_SLUG/pipelines" \
    -H 'Content-Type: application/json' \
    -H "Authorization: Bearer $USER_TOKEN" \
    -d '{"name":"Test Pipeline","slug":"test-pipeline","repository_url":"https://github.com/test/repo.git"}')"
assert_status "POST /api/v2/organizations/:slug/pipelines" "201" "$STATUS"
assert_json_field "pipeline slug" "$BODY" ".slug" "test-pipeline"

PIPELINE_SLUG=$(echo "$BODY" | jq -r '.slug')

log_step ":pencil: 2b.10 Pipeline update"
parse_response "$(api_call PATCH "/api/v2/organizations/$ORG_SLUG/pipelines/$PIPELINE_SLUG" \
    -H 'Content-Type: application/json' \
    -H "Authorization: Bearer $USER_TOKEN" \
    -d '{"name":"Updated Pipeline","description":"New description","repository_url":"https://github.com/test/repo.git"}')"
assert_status "PATCH /api/v2/organizations/:slug/pipelines/:slug" "200" "$STATUS"
assert_json_field "pipeline updated name" "$BODY" ".name" "Updated Pipeline"

log_step ":railway_track: 2b.11 Queue creation"
parse_response "$(api_call POST "/api/v2/organizations/$ORG_SLUG/queues" \
    -H 'Content-Type: application/json' \
    -H "Authorization: Bearer $USER_TOKEN" \
    -d '{"name":"Test Queue","key":"test-queue-1"}')"
assert_status "POST /api/v2/organizations/:slug/queues" "201" "$STATUS"
assert_json_field "queue key" "$BODY" ".key" "test-queue-1"

QUEUE_ID=$(echo "$BODY" | jq -r '.id')

log_step ":pencil: 2b.12 Queue update"
parse_response "$(api_call PUT "/api/v2/organizations/$ORG_SLUG/queues/$QUEUE_ID" \
    -H 'Content-Type: application/json' \
    -H "Authorization: Bearer $USER_TOKEN" \
    -d '{"name":"Updated Queue","description":"New description"}')"
assert_status "PUT /api/v2/organizations/:slug/queues/:id" "200" "$STATUS"
assert_json_field "queue updated name" "$BODY" ".name" "Updated Queue"

log_step ":ticket: 2b.13 Agent token creation"
parse_response "$(api_call POST "/api/v2/organizations/$ORG_SLUG/agent-tokens" \
    -H 'Content-Type: application/json' \
    -H "Authorization: Bearer $USER_TOKEN" \
    -d '{"name":"Test Token"}')"
assert_status "POST /api/v2/organizations/:slug/agent-tokens" "201" "$STATUS"
assert_json_not_empty "agent token id" "$BODY" ".id"
assert_json_not_empty "agent token value" "$BODY" ".token"

AGENT_TOKEN_ID=$(echo "$BODY" | jq -r '.id')

log_step ":clipboard: 2b.14 List resources"

parse_response "$(api_call GET "/api/v2/organizations/$ORG_SLUG/pipelines" \
    -H "Authorization: Bearer $USER_TOKEN")"
assert_status "GET /api/v2/organizations/:slug/pipelines" "200" "$STATUS"
PIPELINE_COUNT=$(echo "$BODY" | jq 'length')
if [ "$PIPELINE_COUNT" -lt 1 ]; then
    log_error "Expected at least 1 pipeline, got $PIPELINE_COUNT"
    exit 1
fi
log_info "GET /api/v2/organizations/:slug/pipelines returns $PIPELINE_COUNT pipeline(s)"

parse_response "$(api_call GET "/api/v2/organizations/$ORG_SLUG/queues" \
    -H "Authorization: Bearer $USER_TOKEN")"
assert_status "GET /api/v2/organizations/:slug/queues" "200" "$STATUS"

parse_response "$(api_call GET "/api/v2/organizations/$ORG_SLUG/agent-tokens" \
    -H "Authorization: Bearer $USER_TOKEN")"
assert_status "GET /api/v2/organizations/:slug/agent-tokens" "200" "$STATUS"

parse_response "$(api_call GET "/api/v2/organizations/$ORG_SLUG/agents" \
    -H "Authorization: Bearer $USER_TOKEN")"
assert_status "GET /api/v2/organizations/:slug/agents" "200" "$STATUS"

parse_response "$(api_call GET /api/v2/organizations \
    -H "Authorization: Bearer $USER_TOKEN")"
assert_status "GET /api/v2/organizations" "200" "$STATUS"

log_step ":no_entry_sign: 2b.15 Duplicate rejection"

parse_response "$(api_call POST /api/auth/register \
    -H 'Content-Type: application/json' \
    -d '{"email":"test@example.com","name":"Dup User","password":"testpass123"}')"
assert_status "duplicate user registration" "409" "$STATUS"

parse_response "$(api_call POST /api/v2/organizations \
    -H 'Content-Type: application/json' \
    -H "Authorization: Bearer $USER_TOKEN" \
    -d '{"name":"Test Org Dup","slug":"test-org"}')"
assert_status "duplicate org slug" "409" "$STATUS"

parse_response "$(api_call POST "/api/v2/organizations/$ORG_SLUG/pipelines" \
    -H 'Content-Type: application/json' \
    -H "Authorization: Bearer $USER_TOKEN" \
    -d '{"name":"Dup Pipeline","slug":"test-pipeline","repository_url":"https://github.com/dup/repo.git"}')"
assert_status "duplicate pipeline slug" "409" "$STATUS"

parse_response "$(api_call POST "/api/v2/organizations/$ORG_SLUG/queues" \
    -H 'Content-Type: application/json' \
    -H "Authorization: Bearer $USER_TOKEN" \
    -d '{"name":"Dup Queue","key":"test-queue-1"}')"
assert_status "duplicate queue key" "409" "$STATUS"

log_step ":warning: 2b.16 Validation errors"

parse_response "$(api_call POST /api/auth/register \
    -H 'Content-Type: application/json' \
    -d '{"email":"","name":"No Email","password":"testpass123"}')"
assert_status "empty email" "400" "$STATUS"

parse_response "$(api_call POST /api/auth/register \
    -H 'Content-Type: application/json' \
    -d '{"email":"valid@email.com","name":"Short Pass","password":"abc"}')"
assert_status "short password" "400" "$STATUS"

parse_response "$(api_call POST "/api/v2/organizations/$ORG_SLUG/pipelines" \
    -H 'Content-Type: application/json' \
    -H "Authorization: Bearer $USER_TOKEN" \
    -d '{"name":"Bad Slug","slug":"","repository_url":"https://github.com/x/y.git"}')"
assert_status "empty slug" "400" "$STATUS"

parse_response "$(api_call POST "/api/v2/organizations/$ORG_SLUG/pipelines" \
    -H 'Content-Type: application/json' \
    -H "Authorization: Bearer $USER_TOKEN" \
    -d '{"name":"Bad Slug","slug":"invalid slug!","repository_url":"https://github.com/x/y.git"}')"
assert_status "invalid slug chars" "400" "$STATUS"

log_step ":envelope: 2b.17 Organization invitation flow"

parse_response "$(api_call POST /api/auth/register \
    -H 'Content-Type: application/json' \
    -d '{"email":"invited@example.com","name":"Invited User","password":"testpass123"}')"
assert_status "register second user" "201" "$STATUS"
USER2_TOKEN=$(echo "$BODY" | jq -r '.token')

parse_response "$(api_call POST "/api/v2/organizations/$ORG_SLUG/invitations" \
    -H 'Content-Type: application/json' \
    -H "Authorization: Bearer $USER_TOKEN")"
assert_status "create invitation" "201" "$STATUS"
assert_json_not_empty "invitation token" "$BODY" ".token"
INVITE_TOKEN=$(echo "$BODY" | jq -r '.token')

parse_response "$(api_call POST "/api/v2/organizations/join/$INVITE_TOKEN" \
    -H 'Content-Type: application/json' \
    -H "Authorization: Bearer $USER2_TOKEN")"
assert_status "join organization" "200" "$STATUS"

parse_response "$(api_call GET /api/v2/organizations \
    -H "Authorization: Bearer $USER2_TOKEN")"
assert_status "second user sees org" "200" "$STATUS"
ORG_COUNT=$(echo "$BODY" | jq 'length')
if [ "$ORG_COUNT" -lt 1 ]; then
    log_error "Invited user should see at least 1 organization, got $ORG_COUNT"
    exit 1
fi
log_info "Invited user sees $ORG_COUNT organization(s)"

log_step ":shield: 2b.18 Role-based access control"

parse_response "$(api_call GET "/api/v2/organizations/$ORG_SLUG" \
    -H "Authorization: Bearer $USER2_TOKEN")"
assert_status "member cannot access org detail" "403" "$STATUS"

parse_response "$(api_call GET "/api/v2/organizations/$ORG_SLUG" \
    -H "Authorization: Bearer $USER_TOKEN")"
assert_status "owner can access org detail" "200" "$STATUS"

log_step ":wastebasket: 2b.19 Resource deletion"

parse_response "$(api_call DELETE "/api/v2/organizations/$ORG_SLUG/agent-tokens/$AGENT_TOKEN_ID" \
    -H "Authorization: Bearer $USER_TOKEN")"
assert_status "DELETE /api/v2/organizations/:slug/agent-tokens/:id" "200" "$STATUS"

parse_response "$(api_call DELETE "/api/v2/organizations/$ORG_SLUG/queues/$QUEUE_ID" \
    -H "Authorization: Bearer $USER_TOKEN")"
assert_status "DELETE /api/v2/organizations/:slug/queues/:id" "200" "$STATUS"

parse_response "$(api_call DELETE "/api/v2/organizations/$ORG_SLUG/pipelines/$PIPELINE_SLUG" \
    -H "Authorization: Bearer $USER_TOKEN")"
assert_status "DELETE /api/v2/organizations/:slug/pipelines/:slug" "200" "$STATUS"

parse_response "$(api_call GET "/api/v2/organizations/$ORG_SLUG/pipelines" \
    -H "Authorization: Bearer $USER_TOKEN")"
REMAINING=$(echo "$BODY" | jq 'length')
if [ "$REMAINING" -ne 0 ]; then
    log_error "Expected 0 pipelines after deletion, got $REMAINING"
    exit 1
fi
log_info "Pipeline deleted successfully"

log_step ":door: 2b.20 Logout"
parse_response "$(api_call POST /api/auth/logout \
    -H "Authorization: Bearer $LOGIN_TOKEN")"
assert_status "POST /api/auth/logout" "200" "$STATUS"

parse_response "$(api_call GET /api/auth/me \
    -H "Authorization: Bearer $LOGIN_TOKEN")"
assert_status "session invalidated after logout" "401" "$STATUS"

print_summary "API Smoke Tests"
