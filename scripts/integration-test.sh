#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# Integration test runner for otvi-server.
#
# Automatically:
#   1. Starts the httpbin Docker container
#   2. Waits for it to be healthy
#   3. Sets HTTPBIN_URL to point at the local container
#   4. Runs all integration tests (including those marked #[ignore])
#   5. Stops the container on exit
#
# Usage:
#   ./scripts/integration-test.sh
# ─────────────────────────────────────────────────────────────────────────────
set -euo pipefail

COMPOSE_FILE="docker-compose.test.yml"
HTTPBIN_PORT=8888
HTTPBIN_URL="http://localhost:${HTTPBIN_PORT}"

# cd to repository root (parent of scripts/)
cd "$(dirname "$0")/.."

cleanup() {
    echo "── Stopping httpbin container ──"
    docker compose -f "$COMPOSE_FILE" down --timeout 5 2>/dev/null || true
}
trap cleanup EXIT

echo "── Starting httpbin container ──"
docker compose -f "$COMPOSE_FILE" up -d

echo "── Waiting for httpbin to be ready ──"
for i in $(seq 1 30); do
    if curl -sf "${HTTPBIN_URL}/get" > /dev/null 2>&1; then
        echo "httpbin is ready"
        break
    fi
    if [ "$i" -eq 30 ]; then
        echo "ERROR: httpbin did not become ready in 30 seconds" >&2
        exit 1
    fi
    sleep 1
done

echo "── Running integration tests ──"
HTTPBIN_URL="${HTTPBIN_URL}" cargo test -p otvi-server --test integration -- --include-ignored "$@"
