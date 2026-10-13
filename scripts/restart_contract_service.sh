#!/usr/bin/env bash
set -euo pipefail

service_id="${1:?GitHub Actions service container ID is required}"
host_port="${2:?Published host port is required}"
if [[ ! "$service_id" =~ ^[a-f0-9]{64}$ ]]; then
  echo "Invalid GitHub Actions service container ID" >&2
  exit 2
fi
if [[ ! "$host_port" =~ ^[1-9][0-9]{0,4}$ ]] || (( host_port > 65535 )); then
  echo "Invalid published host port" >&2
  exit 2
fi

echo "contract-restart: stopping and starting service" >&2
timeout --signal=TERM --kill-after=5s 90s docker restart "$service_id" >/dev/null
echo "contract-restart: service process started" >&2
for attempt in {1..60}; do
  health="$(timeout --signal=TERM --kill-after=5s 10s docker inspect --format '{{.State.Health.Status}}' "$service_id")"
  if [[ "$health" == healthy ]]; then
    if timeout --signal=TERM --kill-after=1s 2s bash -c 'exec 3<>/dev/tcp/127.0.0.1/"$1"' _ "$host_port" 2>/dev/null; then
      echo "contract-restart: service healthy and host port reachable" >&2
      exit 0
    fi
  fi
  if [[ "$health" == unhealthy ]]; then
    echo "Database service became unhealthy after restart" >&2
    exit 1
  fi
  sleep 1
done

echo "Database service did not become healthy and reachable on its published host port after restart" >&2
docker port "$service_id" >&2 || true
exit 1
