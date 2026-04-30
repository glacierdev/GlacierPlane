# GlacierPlane

Glacier is a self-hosted CI/CD control plane for running builds on your own infrastructure. It receives GitHub webhooks, creates builds and jobs, dispatches them to connected agents, stores logs in PostgreSQL, exposes a management UI, and can report commit statuses back to GitHub.

The control plane implements the Buildkite Agent V3 protocol, so the standard `buildkite-agent` binary can be used as a worker process.

## Navigation

- 📷 [Video Overview](#video-overview)
- 🔎 [Repository Layout](#repository-layout)

📄 Instructions:

- [1. Deploy The Control Plane On Ubuntu](#1-deploy-the-control-plane-on-ubuntu)
- [2. Run The UI](#2-run-the-ui)
- [3. Prepare A Repository Pipeline](#3-prepare-a-repository-pipeline)
- [4. Configure GitHub Webhooks](#4-configure-github-webhooks)
- [5. Set Up An Agent](#5-set-up-an-agent)
- [6. Run A Build](#6-run-a-build)
- [7. API Overview, Development](#7-api-overview)

## Video Overview

[Glacier video overview](https://www.youtube.com/watch?v=<id>)

## Repository Layout

```text
.
├── control-plane/          # Rust API server and agent protocol implementation
│   ├── src/
│   ├── migrations/
│   ├── Dockerfile
│   └── .env.example
├── ui/                     # React/Vite UI dashboard
├── docker-compose.yml      # control-plane + PostgreSQL
├── glacier.yml             # project CI pipeline example
└── buildkite.yml           # compatible pipeline file name
```

## Features

- GitHub webhook handling for `push`, `pull_request`, and `ping` events.
- Build creation from webhooks or the API.
- GitHub commit status reporting: `pending`, `success`, `failure`.
- Pipeline files named `glacier.yml` or `buildkite.yml`.
- Pipeline steps with `agents`, `queue`, `key`, `depends_on`, `wait`, and `timeout_in_minutes`.
- Skip directives: `[skip ci]` and `[ci skip]`.
- Agent registration, connection, ping, heartbeat, disconnect, job lifecycle, log upload, and build metadata endpoints.
- Organization-based UI with users, roles, invitations, pipelines, queues, agent tokens, agents, builds, and job logs.
- Queue-based dispatch, tag matching, dependency checks, agent priority, lost-agent detection, stalled-job detection, and job timeouts.

## Requirements

Control-plane server:

- Ubuntu 22.04+ recommended
- Docker Engine 20.10+
- Docker Compose v2
- A public HTTP endpoint reachable by GitHub webhooks

Local UI machine:

- Node.js 20+ recommended
- npm

Agent machine:

- Linux host
- `buildkite-agent` binary
- Git access to the repositories it will clone
- Runtime tools required by your pipeline, for example `python3`, `pip`, `pytest`, `docker`, `curl`, or `jq`

## 1. Deploy The Control Plane On Ubuntu

Install Docker:

```bash
curl -fsSL https://get.docker.com | sh
sudo systemctl enable --now docker
sudo usermod -aG docker "$USER"
```

Log out and back in so the Docker group change takes effect.

Clone the repository:

```bash
git clone https://github.com/glacierdev/GlacierPlane glacier
cd glacier
```

Create the environment file used by `docker-compose.yml`:

```bash
cp control-plane/.env.example .env
```

Edit `.env`:

```bash
nano .env
```

Set at least:

```env
WEBHOOK_SECRET=<random-hex-secret>
HOST_PORT=80
GITHUB_TOKEN=
POSTGRES_USER=glacier
POSTGRES_PASSWORD=glacier123
POSTGRES_DB=glacier
RUST_LOG=control_plane=info,tower_http=info
```

Generate a webhook secret if needed:

```bash
openssl rand -hex 32
```

Environment variables:


| Variable            | Description                                                                                                    |
| ------------------- | -------------------------------------------------------------------------------------------------------------- |
| `WEBHOOK_SECRET`    | Required. Secret placed in the GitHub webhook URL path: `/webhooks/github/<secret>`.                           |
| `HOST_PORT`         | Host port mapped to the control plane. Defaults to `80`.                                                       |
| `GITHUB_TOKEN`      | GitHub PAT for commit statuses. Use `repo:status` for public repositories, or `repo` for private repositories. |
| `POSTGRES_USER`     | PostgreSQL user. Defaults to `glacier`.                                                                        |
| `POSTGRES_PASSWORD` | PostgreSQL password. Defaults to `glacier123`.                                                                 |
| `POSTGRES_DB`       | PostgreSQL database. Defaults to `glacier`.                                                                    |
| `RUST_LOG`          | Rust log filter. Use `control_plane=debug,tower_http=info` while debugging.                                    |


Start the stack:

```bash
docker compose up -d --build
```

Check it:

```bash
docker compose ps
curl http://localhost:${HOST_PORT:-80}/api/health
```

Expected health response:

```text
OK
```

On first start, PostgreSQL runs `control-plane/migrations/001_schema.sql` automatically. The schema is applied only when the database volume is created for the first time.

Useful operations:

```bash
docker compose logs -f control-plane
docker compose logs -f postgres
docker compose restart control-plane
docker compose down
docker compose exec postgres psql -U glacier -d glacier
```

To wipe all database data and re-run the initial schema:

```bash
docker compose down -v
docker compose up -d --build
```

## 2. Run The UI

The UI is a React/Vite app in `ui/`. It talks to the control-plane API through `VITE_CONTROL_PLANE_URL`.

```bash
cd ui
cp .env.example .env
npm install
```

Edit `ui/.env` and point the UI to your control-plane API. This variable is required.

```env
VITE_CONTROL_PLANE_URL=http://<control-plane-host-or-domain>
```

Start the UI:

```bash
npm run dev
```

Open:

```text
http://localhost:5173
```

In the UI:

1. Register a user.
2. Create an organization from the organization selector.
3. Create a pipeline in **Pipelines**.
4. Set the pipeline repository URL to the GitHub repository you want to build.
5. Create an agent registration token in **Agents**.
6. Copy the full token immediately. It is shown only once.

Organization roles:

- `owner`: full access, including settings and member management.
- `admin`: full access except owner-only actions.
- `member`: resource access without organization settings.

## 3. Prepare A Repository Pipeline

In the repository you want to build, add `glacier.yml`:

```yaml
agents:
  queue: "ubuntu-1"

steps:
  - label: "syntax check"
    key: "check"
    command: |
      python3 -m py_compile main.py
    timeout_in_minutes: 5

  - wait

  - label: "unit tests"
    key: "tests"
    depends_on: "check"
    command: |
      python3 -m pip install -r requirements.txt
      python3 -m pytest
    timeout_in_minutes: 10
```

Supported pipeline behavior:

- `glacier.yml` is used as the project pipeline file.
- `buildkite.yml` is accepted as a compatible file name.
- `agents.queue` selects the queue that should run the job.
- Step `agents` can also include custom tag requirements such as `os: linux`.
- `key` names a step for dependencies.
- `depends_on` can be a string or an array.
- `wait` creates a dependency barrier.
- `timeout_in_minutes` fails a running job when the timeout is exceeded.

Skip CI by including either directive in a commit message or pull request title:

```text
[skip ci]
[ci skip]
```

Push events check the commit message. Pull request events check the PR title.

## 4. Configure GitHub Webhooks

In GitHub, open the repository settings and create a webhook:


| Setting      | Value                                                                    |
| ------------ | ------------------------------------------------------------------------ |
| Payload URL  | `http://<control-plane-host-or-domain>/webhooks/github/<WEBHOOK_SECRET>` |
| Content type | `application/json`                                                       |
| Secret       | Leave empty                                                              |
| Events       | Push events and Pull request events                                      |


The secret is validated from the URL path. The GitHub webhook `Secret` field is intentionally not used.

Supported events:

- `ping`: returns `200 OK`.
- `push`: creates a build unless the push deletes a branch or includes `[skip ci]` / `[ci skip]`.
- `pull_request`: creates a build for `opened`, `synchronize`, and `reopened` unless the PR title includes a skip directive.

If `GITHUB_TOKEN` is set, commit statuses are posted to GitHub with context:

```text
ci/<pipeline_slug>
```

## 5. Set Up An Agent

Install the agent binary on the machine that will execute jobs:

```bash
sudo wget https://github.com/buildkite/agent/releases/latest/download/buildkite-agent-linux-amd64 -O /usr/local/bin/buildkite-agent
sudo chmod +x /usr/local/bin/buildkite-agent
```

Create an agent user and workspace:

```bash
sudo useradd -m -s /bin/bash buildkite-agent || true
sudo mkdir -p /var/lib/buildkite-agent
sudo chown buildkite-agent:buildkite-agent /var/lib/buildkite-agent
```

Run the agent manually:

```bash
export BUILDKITE_AGENT_TOKEN="<registration-token-from-ui>"
export BUILDKITE_AGENT_ENDPOINT="http://<control-plane-host-or-domain>/v3"
buildkite-agent start --tags "queue=ubuntu-1,os=linux" --name "%hostname-%spawn"
```

Or use a config file:

```bash
sudo mkdir -p /etc/buildkite-agent
sudo tee /etc/buildkite-agent/buildkite-agent.cfg >/dev/null <<'EOF'
token="<registration-token-from-ui>"
tags="queue=ubuntu-1,os=linux"
name="%hostname-%spawn"
priority=1
EOF
```

Then start:

```bash
buildkite-agent start
```

What happens:

1. The agent registers with `POST /v3/register` using the registration token.
2. The control plane issues an access token.
3. The agent connects with `POST /v3/connect`.
4. The agent polls `GET /v3/ping` for work.
5. Jobs move through accept, start, log upload, and finish.

Queue behavior:

- `queue=ubuntu-1` assigns the agent to the `ubuntu-1` queue.
- If the queue does not exist, the control plane creates it automatically.
- Queue assignment is refreshed when the agent reconnects.
- Higher `priority` values are preferred when multiple agents in the same queue are available.

For private repositories, configure SSH keys or HTTPS credentials for the user that runs the agent.

## 6. Run A Build

Make a change in the configured GitHub repository:

```bash
git add .
git commit -m "trigger ci"
git push origin main
```

Watch control-plane logs:

```bash
docker compose logs -f control-plane
```

In the UI:

1. Open **Pipelines**.
2. Open the pipeline.
3. Watch the new build move through scheduled/running/passed or failed states.
4. Expand jobs to view logs.

Logs are uploaded as chunks and rendered in the UI with ANSI color support.

On GitHub, the commit status should move from `pending` to `success` or `failure` when `GITHUB_TOKEN` is configured.

To test skip behavior:

```bash
git commit --allow-empty -m "docs update [skip ci]"
git push origin main
```

No new build should be created for that commit.

## 7. API Overview

Public endpoints:

- `GET /api/health`
- `POST /v3/register`
- `POST /webhooks/github/:secret`

Agent protocol endpoints:

- `POST /v3/connect`
- `GET /v3/ping`
- `POST /v3/heartbeat`
- `POST /v3/disconnect`
- `GET /v3/jobs/:job_id`
- `PUT /v3/jobs/:job_id/accept`
- `PUT /v3/jobs/:job_id/start`
- `PUT /v3/jobs/:job_id/finish`
- `POST /v3/jobs/:job_id/chunks`
- `POST /v3/jobs/:job_id/pipelines`
- `POST /v3/jobs/:job_id/data/exists`
- `POST /v3/jobs/:job_id/data/set`
- `POST /v3/jobs/:job_id/data/get`
- `POST /v3/jobs/:job_id/data/keys`

UI/API endpoints include:

- `/api/auth/*`
- `/api/v2/organizations`
- `/api/v2/organizations/:org_slug/pipelines`
- `/api/v2/organizations/:org_slug/queues`
- `/api/v2/organizations/:org_slug/agent-tokens`
- `/api/v2/organizations/:org_slug/agents`
- `/api/v2/organizations/:org_slug/builds`
- `/api/v2/builds`

List endpoints support pagination with `page` and `per_page`. Responses expose `Link` and `X-Total-Count` headers.

Build lists support filters for state, branch, commit, date ranges, and creator.

## Development

The main extension points are:

- `control-plane/src/main.rs`: route wiring.
- `control-plane/src/handlers/`: API handlers.
- `control-plane/src/db/`: database queries.
- `control-plane/src/dispatcher.rs`: job matching and dispatch rules.
- `control-plane/src/webhooks.rs`: GitHub webhook processing.
- `control-plane/src/github.rs`: commit status reporting.
- `control-plane/src/background_tasks.rs`: lost agents, stalled jobs, and timeouts.
- `ui/src/components/`: dashboard UI.

