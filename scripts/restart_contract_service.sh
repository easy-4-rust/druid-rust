#!/usr/bin/env bash
set -euo pipefail

service_id="${1:?GitHub Actions service container ID is required}"
if [[ ! "$service_id" =~ ^[a-f0-9]{64}$ ]]; then
  echo "Invalid GitHub Actions service container ID" >&2
  exit 2
fi

docker restart "$service_id" >/dev/null
for attempt in {1..60}; do
  health="$(docker inspect --format '{{.State.Health.Status}}' "$service_id")"
  if [[ "$health" == healthy ]]; then
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
