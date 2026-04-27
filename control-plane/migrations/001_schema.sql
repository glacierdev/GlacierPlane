CREATE EXTENSION IF NOT EXISTS pgcrypto;

-- Users (must exist first for FKs)
CREATE TABLE IF NOT EXISTS users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email VARCHAR(255) UNIQUE NOT NULL,
    name VARCHAR(255) NOT NULL,
    password_hash VARCHAR(255) NOT NULL,
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_users_email ON users(email);

-- Organizations
CREATE TABLE IF NOT EXISTS organizations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    slug VARCHAR(255) UNIQUE NOT NULL,
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_organizations_slug ON organizations(slug);

-- User sessions
CREATE TABLE IF NOT EXISTS user_sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token VARCHAR(255) UNIQUE NOT NULL,
    expires_at TIMESTAMP NOT NULL,
    created_at TIMESTAMP DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_user_sessions_token ON user_sessions(token);
CREATE INDEX IF NOT EXISTS idx_user_sessions_user ON user_sessions(user_id);
CREATE INDEX IF NOT EXISTS idx_user_sessions_expires ON user_sessions(expires_at);

-- Pipelines
CREATE TABLE pipelines (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    slug VARCHAR(255) UNIQUE NOT NULL,
    repository_url VARCHAR(1024) NOT NULL,
    name VARCHAR(255),
    description TEXT,
    default_branch VARCHAR(255) DEFAULT 'main',
    webhook_secret VARCHAR(255),
    config_cache JSONB,
    user_id UUID REFERENCES users(id) ON DELETE CASCADE,
    organization_id UUID REFERENCES organizations(id) ON DELETE CASCADE,
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_pipelines_user ON pipelines(user_id);
CREATE INDEX IF NOT EXISTS idx_pipelines_org ON pipelines(organization_id);

-- Queues
CREATE TABLE IF NOT EXISTS queues (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    pipeline_id UUID REFERENCES pipelines(id) ON DELETE SET NULL,
    organization_id UUID REFERENCES organizations(id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    key VARCHAR(255) NOT NULL,
    description TEXT,
    is_default BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW(),
    UNIQUE(user_id, key)
);
CREATE INDEX IF NOT EXISTS idx_queues_user ON queues(user_id);
CREATE INDEX IF NOT EXISTS idx_queues_pipeline ON queues(pipeline_id);
CREATE INDEX IF NOT EXISTS idx_queues_key ON queues(user_id, key);
CREATE INDEX IF NOT EXISTS idx_queues_org ON queues(organization_id);

-- Registration tokens
CREATE TABLE agent_tokens (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    token VARCHAR(255) UNIQUE NOT NULL,
    name VARCHAR(255),
    description TEXT,
    expires_at TIMESTAMP,
    user_id UUID REFERENCES users(id) ON DELETE CASCADE,
    organization_id UUID REFERENCES organizations(id) ON DELETE CASCADE,
    created_at TIMESTAMP DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_agent_tokens_user ON agent_tokens(user_id);
CREATE INDEX IF NOT EXISTS idx_agent_tokens_org ON agent_tokens(organization_id);

-- Agents
CREATE TABLE IF NOT EXISTS agents (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    uuid TEXT UNIQUE NOT NULL,
    name TEXT NOT NULL,
    hostname TEXT NOT NULL,
    os TEXT NOT NULL,
    arch TEXT NOT NULL,
    version TEXT NOT NULL,
    build TEXT NOT NULL,
    tags TEXT[],
    priority INTEGER,
    status VARCHAR(50) NOT NULL DEFAULT 'disconnected',
    registration_token_id UUID REFERENCES agent_tokens(id),
    user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    queue_id UUID REFERENCES queues(id) ON DELETE SET NULL,
    organization_id UUID REFERENCES organizations(id) ON DELETE SET NULL,
    last_seen TIMESTAMP,
    last_heartbeat TIMESTAMP,
    current_job_id UUID,
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_agents_status ON agents(status);
CREATE INDEX IF NOT EXISTS idx_agents_tags ON agents USING GIN(tags);
CREATE INDEX IF NOT EXISTS idx_agents_last_seen ON agents(last_seen);
CREATE INDEX IF NOT EXISTS idx_agents_user ON agents(user_id);
CREATE INDEX IF NOT EXISTS idx_agents_queue ON agents(queue_id);
CREATE INDEX IF NOT EXISTS idx_agents_org ON agents(organization_id);

-- Access tokens
CREATE TABLE IF NOT EXISTS access_tokens (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    agent_id UUID NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    token TEXT UNIQUE NOT NULL,
    description TEXT,
    revoked_at TIMESTAMP,
    last_used_at TIMESTAMP,
    created_at TIMESTAMP DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_access_tokens_token ON access_tokens(token);
CREATE INDEX IF NOT EXISTS idx_access_tokens_agent ON access_tokens(agent_id);
CREATE INDEX IF NOT EXISTS idx_access_tokens_revoked ON access_tokens(revoked_at) WHERE revoked_at IS NULL;

-- Builds
CREATE TABLE builds (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    number INTEGER NOT NULL,
    pipeline_slug VARCHAR(255) NOT NULL,
    commit VARCHAR(255) NOT NULL,
    branch VARCHAR(255) NOT NULL,
    message TEXT,
    author_name VARCHAR(255),
    author_email VARCHAR(255),
    status VARCHAR(50) NOT NULL DEFAULT 'scheduled',
    webhook_payload JSONB,
    tag TEXT,
    pull_request_number INT,
    source VARCHAR(50) NOT NULL DEFAULT 'webhook',
    created_at TIMESTAMP DEFAULT NOW(),
    started_at TIMESTAMP,
    finished_at TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_builds_pipeline ON builds(pipeline_slug);
CREATE INDEX IF NOT EXISTS idx_builds_status ON builds(status);
CREATE INDEX IF NOT EXISTS idx_builds_created ON builds(created_at DESC);

-- Jobs
CREATE TABLE IF NOT EXISTS jobs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    build_id UUID NOT NULL REFERENCES builds(id) ON DELETE CASCADE,
    step_config JSONB NOT NULL,
    state VARCHAR(50) NOT NULL DEFAULT 'scheduled',
    agent_id UUID REFERENCES agents(id),
    job_token VARCHAR(255),
    env JSONB,
    depends_on UUID[],
    exit_status VARCHAR(10),
    signal VARCHAR(50),
    signal_reason VARCHAR(255),
    started_at TIMESTAMP,
    finished_at TIMESTAMP,
    runnable_at TIMESTAMP,
    chunks_failed_count INTEGER DEFAULT 0,
    traceparent VARCHAR(255),
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_jobs_build ON jobs(build_id);
CREATE INDEX IF NOT EXISTS idx_jobs_state ON jobs(state);
CREATE INDEX IF NOT EXISTS idx_jobs_agent ON jobs(agent_id);
CREATE INDEX IF NOT EXISTS idx_jobs_runnable ON jobs(runnable_at) WHERE state = 'scheduled';

-- Log chunks
CREATE TABLE IF NOT EXISTS log_chunks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    job_id UUID NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,
    sequence INTEGER NOT NULL,
    byte_offset BIGINT NOT NULL,
    size INTEGER NOT NULL,
    data BYTEA NOT NULL,
    created_at TIMESTAMP DEFAULT NOW(),
    UNIQUE(job_id, sequence)
);
CREATE INDEX IF NOT EXISTS idx_log_chunks_job ON log_chunks(job_id, sequence);

-- Artifacts
CREATE TABLE IF NOT EXISTS artifacts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    job_id UUID NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,
    path VARCHAR(1024) NOT NULL,
    size BIGINT,
    content_type VARCHAR(255),
    sha1sum VARCHAR(40),
    upload_url VARCHAR(2048),
    state VARCHAR(50) NOT NULL DEFAULT 'pending',
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_artifacts_job ON artifacts(job_id);

-- Build metadata
CREATE TABLE metadata (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    build_id UUID NOT NULL REFERENCES builds(id) ON DELETE CASCADE,
    key VARCHAR(255) NOT NULL,
    value TEXT NOT NULL,
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW(),
    UNIQUE(build_id, key)
);
CREATE INDEX IF NOT EXISTS idx_metadata_build ON metadata(build_id);
CREATE INDEX IF NOT EXISTS idx_metadata_key ON metadata(build_id, key);

-- Organization members
CREATE TABLE IF NOT EXISTS organization_members (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role VARCHAR(50) NOT NULL DEFAULT 'member',
    created_at TIMESTAMP DEFAULT NOW(),
    UNIQUE(organization_id, user_id)
);
CREATE INDEX IF NOT EXISTS idx_org_members_org ON organization_members(organization_id);
CREATE INDEX IF NOT EXISTS idx_org_members_user ON organization_members(user_id);
CREATE INDEX IF NOT EXISTS idx_org_members_role ON organization_members(organization_id, role);

-- Organization invitations
CREATE TABLE IF NOT EXISTS organization_invitations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    token VARCHAR(255) UNIQUE NOT NULL,
    created_by UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    expires_at TIMESTAMP NOT NULL,
    used_by UUID REFERENCES users(id) ON DELETE SET NULL,
    used_at TIMESTAMP,
    created_at TIMESTAMP DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_org_invitations_token ON organization_invitations(token);
CREATE INDEX IF NOT EXISTS idx_org_invitations_org ON organization_invitations(organization_id);
