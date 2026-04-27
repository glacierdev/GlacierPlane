#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/docker_helpers.sh"

start_all

log_step ":construction: E2E webhook setup — register user, org, pipeline, agent"

parse_response "$(api_call POST /api/auth/register \
    -H 'Content-Type: application/json' \
    -d '{"email":"webhook-e2e@example.com","name":"Webhook E2E","password":"testpass123"}')"
assert_status "register user" "201" "$STATUS"
USER_TOKEN=$(echo "$BODY" | jq -r '.token')
log_info "User registered"

parse_response "$(api_call POST /api/v2/organizations \
    -H 'Content-Type: application/json' \
    -H "Authorization: Bearer $USER_TOKEN" \
    -d '{"name":"Webhook E2E Org","slug":"webhook-e2e-org"}')"
assert_status "create organization" "201" "$STATUS"
ORG_ID=$(echo "$BODY" | jq -r '.id')
ORG_SLUG=$(echo "$BODY" | jq -r '.slug')
log_info "Organization created: $ORG_ID ($ORG_SLUG)"

parse_response "$(api_call POST "/api/v2/organizations/$ORG_SLUG/pipelines" \
    -H 'Content-Type: application/json' \
    -H "Authorization: Bearer $USER_TOKEN" \
    -d '{"name":"Webhook Pipeline","slug":"test-pipeline","repository_url":"https://github.com/test/repo.git"}')"
assert_status "create pipeline" "201" "$STATUS"
PIPELINE_SLUG=$(echo "$BODY" | jq -r '.slug')
log_info "Pipeline created: $PIPELINE_SLUG"

parse_response "$(api_call POST "/api/v2/organizations/$ORG_SLUG/agent-tokens" \
    -H 'Content-Type: application/json' \
    -H "Authorization: Bearer $USER_TOKEN" \
    -d '{"name":"Webhook Agent Token"}')"
assert_status "create agent token" "201" "$STATUS"
REG_TOKEN=$(echo "$BODY" | jq -r '.token')
log_info "Agent token created"

REG_PAYLOAD=$(cat <<'JSON'
{
  "name": "webhook-agent-1",
  "hostname": "webhook-host",
  "os": "linux",
  "arch": "amd64",
  "version": "3.80.0",
  "build": "1234",
  "meta_data": ["queue=ubuntu-1"]
}
JSON
)
parse_response "$(api_call POST /v3/register \
    -H "Authorization: Token $REG_TOKEN" \
    -H 'Content-Type: application/json' \
    -d "$REG_PAYLOAD")"
assert_status "agent register" "200" "$STATUS"
ACCESS_TOKEN=$(echo "$BODY" | jq -r '.access_token')
log_info "Agent registered"

parse_response "$(api_call POST /v3/connect \
    -H "Authorization: Token $ACCESS_TOKEN" \
    -H 'Content-Type: application/json' \
    -d '{"meta_data":["queue=ubuntu-1"]}')"
assert_status "agent connect" "200" "$STATUS"
log_info "Agent connected — setup complete"

get_build_count() {
    local raw
    raw="$(api_call GET "/api/v2/organizations/$ORG_SLUG/pipelines/$PIPELINE_SLUG/builds" \
        -H "Authorization: Bearer $USER_TOKEN")"
    parse_response "$raw"
    assert_status "GET /api/v2/organizations/:slug/pipelines/:slug/builds" "200" "$STATUS"
    echo "$BODY" | jq 'length'
}

log_step ":incoming_envelope: 3b.1 Push webhook"
PUSH_PAYLOAD=$(cat <<'JSON'
{
  "ref": "refs/heads/main",
  "after": "abc123def456",
  "before": "def456abc123",
  "deleted": false,
  "repository": {
    "full_name": "test/repo",
    "clone_url": "https://github.com/test/repo.git"
  },
  "commits": [{
    "id": "abc123def456",
    "message": "Test commit",
    "author": { "name": "Test", "email": "test@example.com" }
  }],
  "head_commit": {
    "id": "abc123def456",
    "message": "Test commit",
    "author": { "name": "Test", "email": "test@example.com" }
  }
}
JSON
)
parse_response "$(api_call POST /webhooks/github/test-secret \
    -H 'X-GitHub-Event: push' \
    -H 'Content-Type: application/json' \
    -d "$PUSH_PAYLOAD")"
assert_status "POST push webhook" "200" "$STATUS"
log_info "Push webhook accepted"

log_step ":mag: 3b.2 Verify build created"
parse_response "$(api_call GET "/api/v2/organizations/$ORG_SLUG/pipelines/$PIPELINE_SLUG/builds" \
    -H "Authorization: Bearer $USER_TOKEN")"
assert_status "list builds after push" "200" "$STATUS"
BUILD_COUNT=$(echo "$BODY" | jq 'length')
if [ "$BUILD_COUNT" -lt 1 ]; then
    log_error "Expected at least one build, got $BUILD_COUNT"
    log_error "Body: $BODY"
    exit 1
fi
BUILD_COMMIT=$(echo "$BODY" | jq -r '.[0].commit')
if [ "$BUILD_COMMIT" != "abc123def456" ]; then
    log_error "Expected build commit abc123def456, got $BUILD_COMMIT"
    exit 1
fi
JOB_LABEL=$(echo "$BODY" | jq -r '.[0].jobs[0].label')
if [ "$JOB_LABEL" != ":pipeline: Pipeline Upload" ]; then
    log_error "Expected first job label ':pipeline: Pipeline Upload', got '$JOB_LABEL'"
    exit 1
fi
log_info "Build created with correct commit and bootstrap job"

log_step ":satellite: 3b.3 Agent receives job"
parse_response "$(api_call GET /v3/ping \
    -H "Authorization: Token $ACCESS_TOKEN")"
assert_status "GET /v3/ping" "200" "$STATUS"
JOB_ID=$(echo "$BODY" | jq -r '.job.id')
if [ -z "$JOB_ID" ] || [ "$JOB_ID" = "null" ]; then
    log_error "Expected job in ping response"
    log_error "Body: $BODY"
    exit 1
fi
PING_COMMIT=$(echo "$BODY" | jq -r '.job.env.BUILDKITE_COMMIT')
if [ "$PING_COMMIT" != "abc123def456" ]; then
    log_error "Expected BUILDKITE_COMMIT abc123def456, got $PING_COMMIT"
    exit 1
fi
log_info "Agent received bootstrap job: $JOB_ID"

log_step ":white_check_mark: 3b.4 Accept and start"
parse_response "$(api_call PUT "/v3/jobs/$JOB_ID/accept" \
    -H "Authorization: Token $ACCESS_TOKEN")"
assert_status "accept job" "200" "$STATUS"

parse_response "$(api_call PUT "/v3/jobs/$JOB_ID/start" \
    -H "Authorization: Token $ACCESS_TOKEN" \
    -H 'Content-Type: application/json' \
    -d '{"started_at":"2026-03-06T12:00:00Z"}')"
assert_status "start job" "200" "$STATUS"
log_info "Job accepted and started"

log_step ":scroll: 3b.5 Upload log chunk"
TMP_GZ="/tmp/webhook-log-${BUILD_ID}.gz"
printf 'Hello from test build\n' | gzip -c > "$TMP_GZ"
CHUNK_SIZE=$(wc -c < "$TMP_GZ" | tr -d '[:space:]')
log_info "Compressed log chunk: $CHUNK_SIZE bytes"
parse_response "$(api_call POST "/v3/jobs/$JOB_ID/chunks?sequence=1&offset=0&size=$CHUNK_SIZE" \
    -H "Authorization: Token $ACCESS_TOKEN" \
    -H 'Content-Type: application/octet-stream' \
    --data-binary @"$TMP_GZ")"
assert_status "upload chunk" "200" "$STATUS"

log_step ":label: 3b.6 Metadata operations"
parse_response "$(api_call POST "/v3/jobs/$JOB_ID/data/set" \
    -H "Authorization: Token $ACCESS_TOKEN" \
    -H 'Content-Type: application/json' \
    -d '{"key":"test-key","value":"test-value"}')"
assert_status "metadata set" "200" "$STATUS"

parse_response "$(api_call POST "/v3/jobs/$JOB_ID/data/exists" \
    -H "Authorization: Token $ACCESS_TOKEN" \
    -H 'Content-Type: application/json' \
    -d '{"key":"test-key"}')"
assert_status "metadata exists" "200" "$STATUS"
assert_json_field "metadata exists true" "$BODY" ".exists" "true"

parse_response "$(api_call POST "/v3/jobs/$JOB_ID/data/get" \
    -H "Authorization: Token $ACCESS_TOKEN" \
    -H 'Content-Type: application/json' \
    -d '{"key":"test-key"}')"
assert_status "metadata get" "200" "$STATUS"
assert_json_field "metadata get value" "$BODY" ".value" "test-value"

parse_response "$(api_call POST "/v3/jobs/$JOB_ID/data/keys" \
    -H "Authorization: Token $ACCESS_TOKEN")"
assert_status "metadata keys" "200" "$STATUS"
KEY_FOUND=$(echo "$BODY" | jq '[.[] | select(. == "test-key")] | length')
if [ "$KEY_FOUND" -lt 1 ]; then
    log_error "test-key not found in metadata keys"
    log_error "Body: $BODY"
    exit 1
fi
log_info "Metadata CRUD works"

log_step ":checkered_flag: 3b.7 Finish job"
parse_response "$(api_call PUT "/v3/jobs/$JOB_ID/finish" \
    -H "Authorization: Token $ACCESS_TOKEN" \
    -H 'Content-Type: application/json' \
    -d '{"exit_status":"0","finished_at":"2026-03-06T12:00:05Z"}')"
assert_status "finish job" "200" "$STATUS"
log_info "Job finished with exit 0"

log_step ":bar_chart: 3b.8 Build status update"
parse_response "$(api_call GET "/api/v2/organizations/$ORG_SLUG/pipelines/$PIPELINE_SLUG/builds" \
    -H "Authorization: Bearer $USER_TOKEN")"
assert_status "list builds after finish" "200" "$STATUS"
LATEST_STATUS=$(echo "$BODY" | jq -r '.[0].state')
if [ "$LATEST_STATUS" != "passed" ]; then
    log_error "Expected latest build status 'passed', got '$LATEST_STATUS'"
    log_error "Body: $BODY"
    exit 1
fi
log_info "Build status updated to passed"

log_step ":git: 3b.9 Pull request webhook"
PR_PAYLOAD=$(cat <<'JSON'
{
  "action": "opened",
  "number": 42,
  "repository": {
    "full_name": "test/repo",
    "clone_url": "https://github.com/test/repo.git"
  },
  "pull_request": {
    "number": 42,
    "title": "Add new feature",
    "draft": false,
    "head": {
      "ref": "feature-branch",
      "sha": "pr-commit-sha-123",
      "repo": {
        "clone_url": "https://github.com/test/repo.git"
      }
    },
    "base": {
      "ref": "main",
      "sha": "base-sha-1"
    },
    "user": {
      "login": "testuser"
    }
  }
}
JSON
)
parse_response "$(api_call POST /webhooks/github/test-secret \
    -H 'X-GitHub-Event: pull_request' \
    -H 'Content-Type: application/json' \
    -d "$PR_PAYLOAD")"
assert_status "POST pull_request webhook" "200" "$STATUS"
log_info "PR webhook accepted"

parse_response "$(api_call GET /v3/ping \
    -H "Authorization: Token $ACCESS_TOKEN")"
assert_status "GET /v3/ping for PR job" "200" "$STATUS"
PR_JOB_ID=$(echo "$BODY" | jq -r '.job.id')
if [ -z "$PR_JOB_ID" ] || [ "$PR_JOB_ID" = "null" ]; then
    log_error "Expected PR job in ping response"
    log_error "Body: $BODY"
    exit 1
fi
assert_json_field "PR env number" "$BODY" ".job.env.BUILDKITE_PULL_REQUEST" "42"
assert_json_field "PR env base branch" "$BODY" ".job.env.BUILDKITE_PULL_REQUEST_BASE_BRANCH" "main"
assert_json_field "PR env draft" "$BODY" ".job.env.BUILDKITE_PULL_REQUEST_DRAFT" "false"
log_info "PR job received with correct env vars"

parse_response "$(api_call PUT "/v3/jobs/$PR_JOB_ID/accept" \
    -H "Authorization: Token $ACCESS_TOKEN")"
assert_status "accept PR job" "200" "$STATUS"
parse_response "$(api_call PUT "/v3/jobs/$PR_JOB_ID/start" \
    -H "Authorization: Token $ACCESS_TOKEN" \
    -H 'Content-Type: application/json' \
    -d '{"started_at":"2026-03-06T12:01:00Z"}')"
assert_status "start PR job" "200" "$STATUS"
parse_response "$(api_call PUT "/v3/jobs/$PR_JOB_ID/finish" \
    -H "Authorization: Token $ACCESS_TOKEN" \
    -H 'Content-Type: application/json' \
    -d '{"exit_status":"0","finished_at":"2026-03-06T12:01:05Z"}')"
assert_status "finish PR job" "200" "$STATUS"
log_info "PR job completed"

log_step ":fast_forward: 3b.10 Skip CI directive"
BEFORE_SKIP_COUNT=$(get_build_count)
SKIP_PAYLOAD=$(cat <<'JSON'
{
  "ref": "refs/heads/main",
  "after": "skip123",
  "before": "skip122",
  "deleted": false,
  "repository": {
    "full_name": "test/repo",
    "clone_url": "https://github.com/test/repo.git"
  },
  "commits": [{
    "id": "skip123",
    "message": "[skip ci] docs update",
    "author": { "name": "Docs", "email": "docs@example.com" }
  }],
  "head_commit": {
    "id": "skip123",
    "message": "[skip ci] docs update",
    "author": { "name": "Docs", "email": "docs@example.com" }
  }
}
JSON
)
parse_response "$(api_call POST /webhooks/github/test-secret \
    -H 'X-GitHub-Event: push' \
    -H 'Content-Type: application/json' \
    -d "$SKIP_PAYLOAD")"
assert_status "POST skip-ci webhook" "200" "$STATUS"
AFTER_SKIP_COUNT=$(get_build_count)
if [ "$BEFORE_SKIP_COUNT" != "$AFTER_SKIP_COUNT" ]; then
    log_error "Skip CI should not create build (before=$BEFORE_SKIP_COUNT after=$AFTER_SKIP_COUNT)"
    exit 1
fi
log_info "Skip CI prevented build creation"

log_step ":wastebasket: 3b.11 Branch deletion skip"
BEFORE_DELETE_COUNT=$(get_build_count)
DELETE_PAYLOAD=$(cat <<'JSON'
{
  "ref": "refs/heads/main",
  "after": "0000000000000000000000000000000000000000",
  "before": "abc123",
  "deleted": true,
  "repository": {
    "full_name": "test/repo",
    "clone_url": "https://github.com/test/repo.git"
  },
  "commits": []
}
JSON
)
parse_response "$(api_call POST /webhooks/github/test-secret \
    -H 'X-GitHub-Event: push' \
    -H 'Content-Type: application/json' \
    -d "$DELETE_PAYLOAD")"
assert_status "POST deleted-branch webhook" "200" "$STATUS"
AFTER_DELETE_COUNT=$(get_build_count)
if [ "$BEFORE_DELETE_COUNT" != "$AFTER_DELETE_COUNT" ]; then
    log_error "Deleted branch webhook should not create build (before=$BEFORE_DELETE_COUNT after=$AFTER_DELETE_COUNT)"
    exit 1
fi
log_info "Deleted branch push skipped"

log_step ":key: 3b.12 Invalid webhook secret"
parse_response "$(api_call POST /webhooks/github/wrong-secret \
    -H 'X-GitHub-Event: push' \
    -H 'Content-Type: application/json' \
    -d "$PUSH_PAYLOAD")"
assert_status "invalid webhook secret" "401" "$STATUS"

print_summary "E2E Webhook"
