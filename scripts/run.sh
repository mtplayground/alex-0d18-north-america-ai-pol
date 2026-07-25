#!/usr/bin/env bash
set -euo pipefail

repository_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repository_dir"

if [[ -f .env.production ]]; then
  set -a
  # shellcheck disable=SC1091
  . ./.env.production
  set +a
fi

if [[ ! -x target/release/policy-backend ]]; then
  "$repository_dir/scripts/build.sh"
fi

exec "$repository_dir/target/release/policy-backend"
