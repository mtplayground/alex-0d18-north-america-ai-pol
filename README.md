# Self-hosted operation

This repository contains three applications:

- `crates/backend`: Axum HTTP server that applies database migrations and serves the built frontend.
- `crates/worker`: scheduled source-ingestion process.
- `frontend`: React/Vite browser application, built into `frontend/dist`.

## Prerequisites

- Rust toolchain with Cargo
- Node.js and npm
- PostgreSQL reachable from the backend and worker
- An S3-compatible bucket for raw source snapshots
- An AI summarizer endpoint and credential
- `curl` for the post-deployment verification command

## Configuration

Copy `.env.example` to `.env.production` and replace every example secret. Do not commit
`.env.production`; it is ignored by Git. The run scripts load that file when it is present,
while a service manager or deployment platform may provide the same variables directly.

| Variable | Used by | Purpose |
| --- | --- | --- |
| `DATABASE_URL` | backend, worker | PostgreSQL connection string. The backend applies checked-in migrations at startup. |
| `LISTEN_ADDRESS` | backend | Bind address; defaults to `0.0.0.0:8080`. |
| `FRONTEND_DIST_DIR` | backend | Built frontend directory; defaults to `frontend/dist`. |
| `OBJECT_STORAGE_ACCESS_KEY_ID` | worker | S3-compatible object storage credential. |
| `OBJECT_STORAGE_SECRET_ACCESS_KEY` | worker | S3-compatible object storage secret. |
| `OBJECT_STORAGE_BUCKET` | worker | Bucket for raw source snapshots. |
| `OBJECT_STORAGE_PREFIX` | worker | Object key prefix for snapshots. |
| `OBJECT_STORAGE_ENDPOINT` | worker | S3-compatible endpoint URL. |
| `OBJECT_STORAGE_REGION` | worker | Object storage region. |
| `OBJECT_STORAGE_FORCE_PATH_STYLE` | worker | `true` for path-style S3 endpoints, otherwise `false`. |
| `AI_SUMMARIZER_API_KEY` | worker | Server-side summarizer credential. |
| `AI_SUMMARIZER_BASE_URL` | worker | Summarizer API base URL. |
| `AI_SUMMARIZER_MODEL` | worker | Summarizer model identifier. |
| `AI_SUMMARIZER_TIMEOUT_SECONDS` | worker | Per-summary timeout; defaults to 20 seconds. |
| `SCHEDULER_CADENCE_SECONDS` | worker | Delay between runs; defaults to `3600` for hourly ingestion. |

The worker makes one ingestion pass immediately when it starts, then waits for the configured
cadence. Keep exactly one worker instance active unless duplicate ingestion is intentionally
managed by the operator. A source failure, including summarizer downtime, is recorded and does
not prevent the worker from continuing with the remaining sources.

## Build and run

From the repository root, create production artifacts for the frontend, backend, and worker:

```bash
./scripts/build.sh
```

Start the backend in one process:

```bash
./scripts/run.sh
```

Start the worker in a separate long-running process:

```bash
./scripts/run-worker.sh
```

The backend serves the frontend and exposes `GET /health`, which returns `{"status":"ok"}`.
After starting both processes, verify the deployed backend and frontend shell:

```bash
./scripts/verify-deploy.sh
# or against a remote deployment
DEPLOY_BASE_URL=https://your-host.example ./scripts/verify-deploy.sh
```

## Service manager setup

Use your platform's process manager to supervise the backend and worker separately, with the
repository root as the working directory and the production environment injected into both.
For a systemd-style deployment, the command portions are:

```ini
# backend service
ExecStart=/path/to/repository/scripts/run.sh
Restart=always

# worker service
ExecStart=/path/to/repository/scripts/run-worker.sh
Restart=always
```

Run the worker continuously rather than from a separate daily cron job: it performs the startup
pass, observes `SCHEDULER_CADENCE_SECONDS`, and retains per-source outcomes for operator review.
If a platform requires scheduled jobs instead, start the worker once per schedule and set its
cadence longer than the job timeout so it completes only the initial pass.

## Local development and validation

```bash
cargo test --workspace
cd frontend
npm ci
npm run typecheck
npm run lint
npm run format:check
npm run test:e2e
npm run build
```

The optional database end-to-end test requires an isolated `E2E_DATABASE_URL`; see
[`tests/e2e/README.md`](tests/e2e/README.md) for the command.
