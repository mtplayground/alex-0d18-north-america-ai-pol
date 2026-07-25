#!/usr/bin/env bash
set -euo pipefail

# Override this for a remote host, for example: DEPLOY_BASE_URL=https://app.example.com
deploy_base_url=${DEPLOY_BASE_URL:-http://127.0.0.1:8080}
deploy_base_url=${deploy_base_url%/}

health_response=$(curl --fail --silent --show-error "$deploy_base_url/health")
if [[ "$health_response" != *'"status":"ok"'* ]]; then
  echo "unexpected health response from $deploy_base_url: $health_response" >&2
  exit 1
fi

homepage=$(curl --fail --silent --show-error "$deploy_base_url/")
if [[ "$homepage" != *'<div id="root"></div>'* ]]; then
  echo "frontend shell was not served by $deploy_base_url" >&2
  exit 1
fi

echo "deployment verified at $deploy_base_url"
