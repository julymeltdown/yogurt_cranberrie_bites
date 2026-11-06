# Task 13 — BE44 reproducible real-stack startup

Status: compose candidate authored; native fallback executed PASS; compose execution NOT_RUN
(docker daemon refuses). Source revision `f75ec4238262b2345c8403137afda030fb6570e8`.
Branch `codex/mcr-exec-w2-compose` in `/Users/lhs/Desktop/mcr-worktrees/exec-w2-compose`.

## Deliverables

- `infra/mcr/compose.yml` — project `mcr`, PostgreSQL 16.14, Redis 7.4, a one-shot
  migration runner and the seven services (api-gateway, auth, profile, friend, post,
  notification, concert). Host ports are `127.0.0.1` only; project id, networks and
  named volumes are isolated per `-p` project.
- `infra/mcr/backend/stack.Dockerfile` and `stack-entrypoint.sh` / `migrate-all.sh` —
  one release image for all seven binaries, migration sets retained at their compiled
  manifest paths, JWT private key decoded from a single-line base64 env value.
- `infra/mcr/postgres-init.sql` — the seven owned databases on a fresh volume.
- `scripts/mcr/backend-smoke.sh` + `scripts/mcr/backend_smoke.py` — the lifecycle proof
  for both modes.
- Evidence receipts: `task-13-native.json` (PASS), `task-13-compose-notrun.json`.

## Compose contract

- Secrets come only from `MCR_ENV_FILE` (mode 0600, generated per run) and
  `MCR_DB_PASSWORD` / `MCR_REDIS_PASSWORD`; nothing is baked into the file.
- The shared env file also carries each owned `*_DATABASE_URL`, `POST_REDIS_URL` and
  `CONCERT_METRICS_ADDR` so the migration runner can apply all seven sets before any
  service starts.
- Gateway peer startup uses `service_started`, not `service_healthy`: post-service
  requires `post->gateway` for its own readiness, so a health gate would deadlock.
- Each service health-checks its own `/livez`; the smoke additionally validates every
  `/readyz` body against the expected required-probe set (auth and friend include
  `concert-owner-authority`, the gateway includes `signing-keys`).
- Concert runs the health-only operational mode (`CONCERT_RUNTIME_MODE=production`,
  `CONCERT_MIGRATION_MODE=external`, metrics on `0.0.0.0:9096`). No product release
  bundle is mounted, its gRPC surface stays off, and no RPC is simulated.
- Providers are absent, not faked: OAuth endpoints point at the unreachable
  `127.0.0.1:9/disabled`, the reset mailer is `logging`, Kafka is empty, and the post
  external publisher is `disabled`.

## Changes in this session

- `scripts/mcr/backend_smoke.py` now validates readiness itself: the shared
  `runtime_smoke.PEERS` map predates the auth/friend `concert-owner-authority` probe and
  rejected every honest report as malformed. The local validator keeps the strict
  shape check, adds that probe, and runs in both modes.
- Restart rows now target `mcr_api_gateway` (hyphen mapping), not the non-existent
  `mcr_api-gateway` database.
- Concert liveness is probed and recorded alongside the other six services.
- `docker_available()` honours the active Docker context instead of one hard-coded
  socket path, and the NOT_RUN receipt records the daemon diagnostic.
- `infra/mcr/compose.yml`: concert migration variable corrected to
  `CONCERT_MIGRATION_MODE`; gateway peer dependencies changed to `service_started`.

## Validation

| Command | Exit |
| --- | --- |
| `python3 -m py_compile scripts/mcr/backend_smoke.py` | 0 |
| `bash -n scripts/mcr/backend-smoke.sh` | 0 |
| `docker compose -p mcr-validate -f infra/mcr/compose.yml config --services` (with a temp `MCR_ENV_FILE`) | 0, ten services listed |
| `docker info` | 1 — `Cannot connect to the Docker daemon at unix:///Users/lhs/.docker/run/docker.sock. Is the docker daemon running?` |
| `scripts/mcr/backend-smoke.sh --mode native --bin-dir /Users/lhs/Desktop/mcr-build/exec-w2-compose/debug --report .../task-13-native.json` | 0, `{"cleanup": "PASS", "failure": null, "mode": "native", "result": "PASS"}` |
| `scripts/mcr/backend-smoke.sh --mode compose --report .../task-13-compose-notrun.json` | 1, `{"mode": "compose", "result": "NOT_RUN", "cleanup": "NOT_RUN"}` |

Native run (`task-13-native.json`, started 2026-11-06T05:06:14Z, owned directory
`/private/tmp/mcr-pg-*`, removed on cleanup):

- Seven databases migrated and verified against their manifests (17, 19, 10, 19, 33, 7,
  52 applied migrations); the ledger was re-verified after a PostgreSQL restart.
- All seven `/livez` returned 200 before and after the restart.
- Gateway `/readyz` 200 with every required peer `up`; auth and friend reported the
  `concert-owner-authority` probe `up`; concert `/readyz` 200 with `database` up.
- One public route: `GET /api/concert/events/{event}/bootstrap` returned 401
  `GATEWAY_401` (`AUTHENTICATION_REQUIRED`) — a real gateway route, not a stub.
- Peer outage: stopping friend-service made gateway `/readyz` return 503 with
  `gateway->friend` down, so readiness is honest.
- Restart: one row per owned database survived a full stack stop, a PostgreSQL stop/start
  with a changed postmaster PID, and a fresh service topology.
- Cleanup PASS: 16 owned process stops and the private cluster directory removed.

## NOT_RUN

- `docker compose up --build`, all real container `/livez`/`/readyz`, the container
  restart row proof, `docker compose down -v` and the label-based leftover audit of
  containers, networks and volumes. Reason: the Docker daemon is not running
  (exact diagnostic above); this was the authorized fallback after the provider outage.
- The stack image has therefore not been built or executed.

## Risks

- The compose image build and container wiring are validated statically
  (`docker compose config`) only. Highest residual risk: the Rust release build inside
  `stack.Dockerfile` and the in-container readiness ordering.
- Concert's production health-only startup is exercised natively only in `local` mode;
  the compose variant (`production`, non-loopback metrics) is config-valid but unrun.
- `backend/scripts/mcr/runtime_smoke.py` still rejects the `concert-owner-authority`
  probe, so `dev_stack.py` and `runtime_smoke.run` fail their health stage against this
  revision. That helper is outside this task's writable paths and was not edited.
