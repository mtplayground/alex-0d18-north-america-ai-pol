#!/usr/bin/env bash
set -euo pipefail

repository_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repository_dir"

(cd frontend && npm ci && npm run build)
cargo build --release -p policy-backend -p policy-worker
