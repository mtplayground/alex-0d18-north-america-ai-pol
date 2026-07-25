# Workspace layout

This repository is a monorepo with three independently runnable applications:

- `crates/backend`: Axum HTTP backend. It listens on `0.0.0.0:8080`.
- `crates/worker`: Rust ingestion-process entry point.
- `frontend`: React, TypeScript, Vite, and Tailwind browser application.
- `crates/shared`: infrastructure-independent types shared by backend and worker.

## Local development

Build the Rust workspace from the repository root:

```bash
cargo build
```

Format and lint every Rust crate:

```bash
cargo fmt-check
cargo lint
```

Use `.env.example` as the environment contract for the backend, object
storage, AI summarizer, and worker scheduler. The backend reads the managed
PostgreSQL connection string from `DATABASE_URL` and applies the checked-in
SQLx migrations at startup. Migration files live in `migrations/`; use
`sqlx migrate add <name>` from the repository root to create the next ordered
migration.

Run the backend:

```bash
cargo run -p policy-backend
```

## Bare deployment

Build the frontend and release backend from the checked-out repository:

```bash
./scripts/build.sh
```

Set the variables described in `.env.example` in the process environment, then
start the service with `./scripts/run.sh`. The backend serves the frontend from
`FRONTEND_DIST_DIR` (default: `frontend/dist`) and exposes `GET /health`, which
responds with `{"status":"ok"}`.

Run the worker in another terminal:

```bash
cargo run -p policy-worker
```

Build or serve the frontend:

```bash
cd frontend
npm install
npm run dev
```

The frontend toolchain includes type-checking, linting, formatting, and a
production build:

```bash
npm run typecheck
npm run lint
npm run format:check
npm run build
```

Database configuration, migrations, API routes, source ingestion, and
production static-file serving are deliberately added in their respective
issues.
