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

Run the backend:

```bash
cargo run -p policy-backend
```

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

Database configuration, migrations, API routes, source ingestion, and
production static-file serving are deliberately added in their respective
issues.
