#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/docker_helpers.sh"

start_all

log_step ":construction: E2E agent setup — register user, org, agent token"

parse_response "$(api_call POST /api/auth/register \
    -H 'Content-Type: application/json' \
    -d '{"email":"agent-e2e@example.com","name":"Agent E2E","password":"testpass123"}')"
assert_status "register user" "201" "$STATUS"
USER_TOKEN=$(echo "$BODY" | jq -r '.token')
log_info "User registered, token obtained"

parse_response "$(api_call POST /api/v2/organizations \
    -H 'Content-Type: application/json' \
    -H "Authorization: Bearer $USER_TOKEN" \
    -d '{"name":"Agent E2E Org","slug":"agent-e2e-org"}')"
assert_status "create organization" "201" "$STATUS"
ORG_ID=$(echo "$BODY" | jq -r '.id')
ORG_SLUG=$(echo "$BODY" | jq -r '.slug')
log_info "Organization created: $ORG_ID ($ORG_SLUG)"

parse_response "$(api_call POST "/api/v2/organizations/$ORG_SLUG/agent-tokens" \
    -H 'Content-Type: application/json' \
    -H "Authorization: Bearer $USER_TOKEN" \
    -d '{"name":"Agent E2E Token"}')"
assert_status "create agent token" "201" "$STATUS"
REG_TOKEN=$(echo "$BODY" | jq -r '.token')
log_info "Agent registration token created"

log_step ":satellite: 3a.1 Agent registration"
REG_PAYLOAD=$(cat <<'JSON'
{
  "name": "test-agent-1",
  "hostname": "test-host",
  "os": "linux",
  "arch": "amd64",
  "version": "3.80.0",
  "build": "1234",
  "meta_data": ["queue=test-queue-1"]
}
JSON
)
parse_response "$(api_call POST /v3/register \
    -H "Authorization: Token $REG_TOKEN" \
    -H 'Content-Type: application/json' \
    -d "$REG_PAYLOAD")"
assert_status "POST /v3/register" "200" "$STATUS"
assert_json_not_empty "register returns access token" "$BODY" ".access_token"
ACCESS_TOKEN=$(echo "$BODY" | jq -r '.access_token')
log_info "Agent registered, access token obtained"

log_step ":railway_track: 3a.2 Queue auto-creation"
parse_response "$(api_call GET "/api/v2/organizations/$ORG_SLUG/queues" \
    -H "Authorization: Bearer $USER_TOKEN")"
assert_status "GET /api/v2/organizations/:slug/queues" "200" "$STATUS"
QUEUE_FOUND=$(echo "$BODY" | jq '[.[] | select(.key == "test-queue-1")] | length')
if [ "$QUEUE_FOUND" -lt 1 ]; then
    log_error "Expected auto-created queue test-queue-1"
    log_error "Body: $BODY"
    exit 1
fi
log_info "Queue auto-created and listed ($QUEUE_FOUND match)"

log_step ":electric_plug: 3a.3 Agent connect"
parse_response "$(api_call POST /v3/connect \
    -H "Authorization: Token $ACCESS_TOKEN" \
    -H 'Content-Type: application/json' \
    -d '{"meta_data":["queue=test-queue-1"]}')"
assert_status "POST /v3/connect" "200" "$STATUS"
log_info "Agent connected"

log_step ":heartbeat: 3a.4 Heartbeat"
parse_response "$(api_call POST /v3/heartbeat \
    -H "Authorization: Token $ACCESS_TOKEN")"
assert_status "POST /v3/heartbeat" "200" "$STATUS"
assert_json_field "heartbeat status" "$BODY" ".status" "ok"

log_step ":satellite_antenna: 3a.5 Ping without job"
parse_response "$(api_call GET /v3/ping \
    -H "Authorization: Token $ACCESS_TOKEN")"
assert_status "GET /v3/ping" "200" "$STATUS"
HAS_JOB=$(echo "$BODY" | jq '.job != null')
if [ "$HAS_JOB" = "true" ]; then
    log_error "Expected no job on ping, but got one"
    log_error "Body: $BODY"
    exit 1
fi
log_info "Ping returned no job as expected"

log_step ":mag: 3a.6 Agent appears in listing"
parse_response "$(api_call GET "/api/v2/organizations/$ORG_SLUG/agents" \
    -H "Authorization: Bearer $USER_TOKEN")"
assert_status "GET /api/v2/organizations/:slug/agents" "200" "$STATUS"
AGENT_COUNT=$(echo "$BODY" | jq '[.[] | select(.name == "test-agent-1" and .connection_state == "connected")] | length')
if [ "$AGENT_COUNT" -lt 1 ]; then
    log_error "Expected connected agent in listing"
    log_error "Body: $BODY"
    exit 1
fi
log_info "Agent listed as connected"

log_step ":electric_plug: 3a.7 Agent disconnect"
parse_response "$(api_call POST /v3/disconnect \
    -H "Authorization: Token $ACCESS_TOKEN")"
assert_status "POST /v3/disconnect" "200" "$STATUS"
log_info "Agent disconnected"

log_step ":no_entry_sign: 3a.8 Invalid token rejected"
parse_response "$(api_call GET /v3/ping \
    -H "Authorization: Token invalid-token-value")"
assert_status "GET /v3/ping invalid token" "401" "$STATUS"

log_step ":lock: 3a.9 Revoked token rejected"
log_info "Revoking access token via direct DB update..."
docker exec "$DB_CONTAINER" psql -U glacier -d glacier_test -c \
    "UPDATE access_tokens SET revoked_at = NOW() WHERE token = '$ACCESS_TOKEN';" >/dev/null
log_info "Token revoked in database"

parse_response "$(api_call GET /v3/ping \
    -H "Authorization: Token $ACCESS_TOKEN")"
assert_status "GET /v3/ping revoked token" "401" "$STATUS"

print_summary "E2E Agent Lifecycle"
