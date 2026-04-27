#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

BUILD_ID="${BUILDKITE_BUILD_ID:-local-$(date +%s)}"

DB_CONTAINER="test-db-${BUILD_ID}"
CP_CONTAINER="test-cp-${BUILD_ID}"
NETWORK="test-net-${BUILD_ID}"
CP_IMAGE="control-plane-test:${BUILD_ID}"

export DB_CONTAINER CP_CONTAINER NETWORK CP_IMAGE BUILD_ID

TESTS_PASSED=0
TESTS_FAILED=0
TEST_START_TIME=""

log_info()  { echo "[INFO]  $(date '+%H:%M:%S') $*"; }
log_error() { echo "[ERROR] $(date '+%H:%M:%S') $*" >&2; }
log_step()  { echo ""; echo "--- $*"; }

install_tool() {
    local tool="$1"
    log_info "Attempting to install '$tool'..."

    if command -v apt-get >/dev/null 2>&1; then
        case "$tool" in
            docker)
                log_info "Installing Docker via get.docker.com..."
                curl -fsSL https://get.docker.com | sh
                systemctl start docker 2>/dev/null || true
                ;;
            *)
                apt-get update -qq && apt-get install -y -qq "$tool"
                ;;
        esac
    elif command -v yum >/dev/null 2>&1; then
        case "$tool" in
            docker)
                log_info "Installing Docker via get.docker.com..."
                curl -fsSL https://get.docker.com | sh
                systemctl start docker 2>/dev/null || true
                ;;
            *)
                yum install -y "$tool"
                ;;
        esac
    else
        log_error "No supported package manager found (need apt-get or yum)"
        return 1
    fi

    if command -v "$tool" >/dev/null 2>&1; then
        log_info "Successfully installed '$tool'"
    else
        log_error "Failed to install '$tool'"
        return 1
    fi
}

check_dependencies() {
    log_step ":mag: Pre-flight dependency check"
    local missing=()

    log_info "Bash version: ${BASH_VERSION:-unknown}"
    log_info "User: $(whoami)"
    log_info "Hostname: $(hostname)"
    log_info "Working directory: $(pwd)"
    log_info "Kernel: $(uname -srm)"
    log_info "PATH: $PATH"

    for cmd in docker curl jq gzip; do
        if command -v "$cmd" >/dev/null 2>&1; then
            local ver
            ver="$(${cmd} --version 2>&1 | head -1 || true)"
            log_info "  $cmd: $(command -v "$cmd") ($ver)"
        else
            log_error "  $cmd: NOT FOUND"
            missing+=("$cmd")
        fi
    done

    if [ ${#missing[@]} -gt 0 ]; then
        log_info "Missing tools: ${missing[*]} — attempting auto-install..."
        local failed=()
        for tool in "${missing[@]}"; do
            if ! install_tool "$tool"; then
                failed+=("$tool")
            fi
        done
        if [ ${#failed[@]} -gt 0 ]; then
            log_error "Could not install: ${failed[*]}"
            exit 1
        fi
        log_info "All missing tools installed. Re-checking..."
        for cmd in docker curl jq gzip; do
            local ver
            ver="$(${cmd} --version 2>&1 | head -1 || true)"
            log_info "  $cmd: $(command -v "$cmd") ($ver)"
        done
    fi

    if ! docker info >/dev/null 2>&1; then
        log_error "Docker daemon is not running or current user cannot access it."
        log_error "Try: sudo systemctl start docker && sudo usermod -aG docker \$(whoami)"
        exit 1
    fi
    log_info "Docker daemon is accessible"

    if [ -n "${BUILDKITE_BUILD_ID:-}" ]; then
        log_info "Buildkite environment:"
        log_info "  BUILD_ID:     $BUILDKITE_BUILD_ID"
        log_info "  BUILD_NUMBER: ${BUILDKITE_BUILD_NUMBER:-n/a}"
        log_info "  PIPELINE:     ${BUILDKITE_PIPELINE_SLUG:-n/a}"
        log_info "  BRANCH:       ${BUILDKITE_BRANCH:-n/a}"
        log_info "  COMMIT:       ${BUILDKITE_COMMIT:-n/a}"
        log_info "  STEP_KEY:     ${BUILDKITE_STEP_KEY:-n/a}"
    else
        log_info "Running outside Buildkite (local mode)"
    fi

    log_info "Pre-flight checks passed"
}

cleanup() {
    local exit_code=$?
    log_step ":broom: Cleanup"

    if [ $exit_code -ne 0 ]; then
        log_error "Script exiting with code $exit_code — dumping container logs for debugging"
        if docker ps -a --format '{{.Names}}' | grep -q "^${CP_CONTAINER}$" 2>/dev/null; then
            log_info "=== Control-plane container logs (last 80 lines) ==="
            docker logs "$CP_CONTAINER" 2>&1 | tail -80 || true
            log_info "=== End control-plane logs ==="
        fi
        if docker ps -a --format '{{.Names}}' | grep -q "^${DB_CONTAINER}$" 2>/dev/null; then
            log_info "=== Postgres container logs (last 30 lines) ==="
            docker logs "$DB_CONTAINER" 2>&1 | tail -30 || true
            log_info "=== End Postgres logs ==="
        fi
    fi

    docker rm -f "$DB_CONTAINER" "$CP_CONTAINER" 2>/dev/null || true
    docker network rm "$NETWORK" 2>/dev/null || true
    log_info "Containers and network removed"
}

build_cp_image() {
    log_step ":docker: Building control-plane image"
    log_info "Image tag: $CP_IMAGE"
    log_info "Build context: $REPO_ROOT/control-plane/"

    if ! docker build -t "$CP_IMAGE" "$REPO_ROOT/control-plane/"; then
        log_error "Docker build failed"
        return 1
    fi
    log_info "Image built successfully"
}

start_postgres() {
    log_step ":postgres: Starting Postgres"
    docker network create "$NETWORK" 2>/dev/null || true
    log_info "Network: $NETWORK"

    docker run -d --name "$DB_CONTAINER" \
        --network "$NETWORK" \
        -e POSTGRES_USER=glacier \
        -e POSTGRES_PASSWORD=glacier123 \
        -e POSTGRES_DB=glacier_test \
        postgres:16-alpine
    log_info "Container started: $DB_CONTAINER"

    log_info "Waiting for Postgres readiness..."
    for i in $(seq 1 30); do
        if docker exec "$DB_CONTAINER" \
            psql -U glacier -d glacier_test -c "SELECT 1" >/dev/null 2>&1; then
            log_info "Postgres ready after ${i}s"
            return 0
        fi
        sleep 1
    done
    log_error "Postgres failed to become ready after 30s"
    log_error "Container logs:"
    docker logs "$DB_CONTAINER" 2>&1 | tail -20
    return 1
}

run_migrations() {
    log_step ":database: Running migrations"
    log_info "Copying migration file into container..."
    docker cp "$REPO_ROOT/control-plane/migrations/001_schema.sql" "$DB_CONTAINER:/tmp/001_schema.sql"
    log_info "Executing migration..."
    docker exec "$DB_CONTAINER" \
        psql -U glacier -d glacier_test -f /tmp/001_schema.sql
    log_info "Migrations applied"
}

start_control_plane() {
    log_step ":gear: Starting control-plane"
    docker run -d --name "$CP_CONTAINER" \
        --network "$NETWORK" \
        -e DATABASE_URL="postgres://glacier:glacier123@${DB_CONTAINER}/glacier_test?sslmode=disable" \
        -e WEBHOOK_SECRET=test-secret \
        -e PORT=8080 \
        -e RUST_LOG=control_plane=debug \
        -p 0:8080 \
        "$CP_IMAGE"
    log_info "Container started: $CP_CONTAINER"

    CP_PORT=$(docker port "$CP_CONTAINER" 8080 | head -1 | cut -d: -f2 || true)
    if [ -z "$CP_PORT" ]; then
        log_error "Failed to get mapped port for control-plane container"
        return 1
    fi
    export CP_PORT
    log_info "Mapped port: $CP_PORT"

    log_info "Waiting for health check on localhost:$CP_PORT..."
    for i in $(seq 1 30); do
        if curl -sf "http://localhost:${CP_PORT}/api/health" >/dev/null 2>&1; then
            log_info "Control-plane ready after ${i}s"
            return 0
        fi
        sleep 1
    done
    log_error "Control-plane failed to become ready after 30s"
    log_error "Container status: $(docker inspect --format='{{.State.Status}}' "$CP_CONTAINER" 2>/dev/null || echo 'unknown')"
    log_error "Container logs:"
    docker logs "$CP_CONTAINER" 2>&1 | tail -50
    return 1
}

start_all() {
    trap cleanup EXIT
    check_dependencies
    build_cp_image
    start_postgres
    run_migrations
    start_control_plane
}

begin_test() {
    TEST_START_TIME=$(date +%s%N 2>/dev/null || date +%s)
}

assert_status() {
    local description="$1"
    local expected_status="$2"
    local actual_status="$3"
    local body="${4:-}"

    if [ "$actual_status" != "$expected_status" ]; then
        TESTS_FAILED=$((TESTS_FAILED + 1))
        log_error "FAIL: $description"
        log_error "  Expected status: $expected_status"
        log_error "  Actual status:   $actual_status"
        if [ -n "$body" ]; then
            log_error "  Response body:   $body"
        fi
        exit 1
    fi
    TESTS_PASSED=$((TESTS_PASSED + 1))
    echo "  PASS: $description (HTTP $actual_status)"
}

assert_json_field() {
    local description="$1"
    local json="$2"
    local field="$3"
    local expected="$4"

    local actual
    actual=$(echo "$json" | jq -r "$field")

    if [ "$actual" != "$expected" ]; then
        TESTS_FAILED=$((TESTS_FAILED + 1))
        log_error "FAIL: $description"
        log_error "  Field:    $field"
        log_error "  Expected: $expected"
        log_error "  Actual:   $actual"
        log_error "  Body:     $json"
        exit 1
    fi
    TESTS_PASSED=$((TESTS_PASSED + 1))
    echo "  PASS: $description ($field == $expected)"
}

assert_json_not_empty() {
    local description="$1"
    local json="$2"
    local field="$3"

    local actual
    actual=$(echo "$json" | jq -r "$field")

    if [ -z "$actual" ] || [ "$actual" = "null" ]; then
        TESTS_FAILED=$((TESTS_FAILED + 1))
        log_error "FAIL: $description"
        log_error "  Field $field is empty or null"
        log_error "  Body: $json"
        exit 1
    fi
    TESTS_PASSED=$((TESTS_PASSED + 1))
    echo "  PASS: $description ($field is set)"
}

api_call() {
    local method="$1"
    local path="$2"
    shift 2

    curl -s -w "\n%{http_code}" --max-time 15 -X "$method" "http://localhost:${CP_PORT}${path}" "$@"
}

parse_response() {
    local raw="$1"
    BODY=$(echo "$raw" | sed '$d')
    STATUS=$(echo "$raw" | tail -1)
    export BODY STATUS
}

print_summary() {
    local suite_name="${1:-Tests}"
    local total=$((TESTS_PASSED + TESTS_FAILED))
    echo ""
    echo "========================================"
    echo " $suite_name Summary"
    echo "========================================"
    echo " Total:  $total"
    echo " Passed: $TESTS_PASSED"
    echo " Failed: $TESTS_FAILED"
    echo "========================================"
    if [ "$TESTS_FAILED" -gt 0 ]; then
        echo " RESULT: FAILED"
    else
        echo " RESULT: ALL PASSED"
    fi
    echo "========================================"
}
