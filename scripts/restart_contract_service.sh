#!/usr/bin/env bash
set -euo pipefail

service_id="${1:?GitHub Actions service container ID is required}"
if [[ ! "$service_id" =~ ^[a-f0-9]{64}$ ]]; then
  echo "Invalid GitHub Actions service container ID" >&2
  exit 2
fi

echo "contract-restart: stopping and starting service" >&2
timeout --signal=TERM --kill-after=5s 90s docker restart "$service_id" >/dev/null
echo "contract-restart: service process started" >&2
for attempt in {1..60}; do
  health="$(docker inspect --format '{{.State.Health.Status}}' "$service_id")"
  if [[ "$health" == healthy ]]; then
    echo "contract-restart: service healthy" >&2
    exit 0
  fi
  if [[ "$health" == unhealthy ]]; then
    echo "Database service became unhealthy after restart" >&2
    exit 1
  fi
  sleep 1
done

echo "Database service did not become healthy after restart" >&2
exit 1
