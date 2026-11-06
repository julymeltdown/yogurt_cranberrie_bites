#!/usr/bin/env bash
# BE44 real-stack smoke entrypoint. Compose is authoritative; when the Docker
# daemon is unavailable the same semantics run on the native process stack.
#
#   scripts/mcr/backend-smoke.sh                     # compose if reachable, else native
#   scripts/mcr/backend-smoke.sh --mode compose      # docker compose only; NOT_RUN without a daemon
#   scripts/mcr/backend-smoke.sh --mode native       # native stack; add --build to compile first
#   scripts/mcr/backend-smoke.sh --report /abs/path.json
#
# Compose mode builds infra/mcr/backend/stack.Dockerfile, starts PostgreSQL,
# Redis, the migration runner and all seven services on loopback-only ports,
# inserts a real row, restarts PostgreSQL and the gateway, then removes the
# project with its volumes and verifies no labeled container/network/volume
# remains. Native mode proves the same contract with owned local processes.
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
exec python3 "$root/scripts/mcr/backend_smoke.py" "$@"
