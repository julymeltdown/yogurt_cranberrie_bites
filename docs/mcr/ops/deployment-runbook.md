# MCR staging and production runbook

Status: authored from the current source revision for plan row 44. It documents
how to start the shipped staging stack and which inputs a production deployment
needs. It does not certify that stack: the Compose image build and container
run are **NOT_RUN** (no Docker daemon on the authoring host), QA08 capacity is
**DEFERRED_BY_USER / NOT_RUN**, and no production capacity, device, browser or
release claim follows from this document.

Every value below is a name or a non-secret default. Secrets are never written
into this document; read them from the deployment secret store.

## 1. Topology

`infra/mcr/compose.yml` defines one isolated project with ten service entries:
PostgreSQL, Redis, a one-shot migration runner, and seven application services.
Each application service is the same release image with a different command.

| Compose service | Command / image | Owned database | Internal listeners | Host port (loopback only) |
| --- | --- | --- | --- | --- |
| `postgres` | `postgres:16.14-bookworm` | creates all seven below | 5432 | `MCR_POSTGRES_HOST_PORT` (15432) |
| `redis` | `redis:7.4-bookworm`, `--appendonly yes`, `--requirepass` | - | 6379 | `MCR_REDIS_HOST_PORT` (16379) |
| `migrate` | stack image, `migrate-all.sh` | applies and verifies every set, then exits | - | - |
| `api-gateway` | stack image, `api-gateway` | `mcr_api_gateway` | HTTP 8080, gRPC 50050, metrics 9090 | `MCR_GATEWAY_HOST_PORT` (18080), `MCR_GATEWAY_METRICS_HOST_PORT` (19090) |
| `auth-service` | stack image, `auth-service` | `mcr_auth` | HTTP 8081, gRPC 50051, metrics 9091 | `MCR_AUTH_METRICS_HOST_PORT` (19091) |
| `profile-service` | stack image, `profile-service` | `mcr_profile` | HTTP 8082, gRPC 50052, metrics 9092 | `MCR_PROFILE_METRICS_HOST_PORT` (19092) |
| `friend-service` | stack image, `friend-service` | `mcr_friend` | HTTP 8083, gRPC 50053, metrics 9093 | `MCR_FRIEND_METRICS_HOST_PORT` (19093) |
| `post-service` | stack image, `post-service` | `mcr_post` | HTTP 8084, gRPC 50054, metrics 9094 | `MCR_POST_METRICS_HOST_PORT` (19094) |
| `notification-service` | stack image, `notification-service` | `mcr_notification` | HTTP 8085, gRPC 50055, metrics 9095 | `MCR_NOTIFICATION_METRICS_HOST_PORT` (19095) |
| `concert-service` | stack image, `concert-service` | `mcr_concert` | metrics 9096; **no gRPC listener** (health-only mode) | `MCR_CONCERT_METRICS_HOST_PORT` (19096) |

Databases are created by `infra/mcr/postgres-init.sql` on a fresh volume. All
host ports bind `127.0.0.1`; the internal network is project-scoped. Named
volumes are `postgres-data` and `redis-data`. The stack uses one database owner
(`mcr_owner`) for every database; per-service least-privilege runtime roles are a
production gap (section 12).

Runtime modes are service-specific. Six services run `parity`
(`GATEWAY_RUNTIME_MODE`, `AUTH_RUNTIME_MODE`, `PROFILE_RUNTIME_MODE`,
`FRIEND_RUNTIME_MODE`, `POST_RUNTIME_MODE`, `NOTIFICATION_RUNTIME_MODE`).
`concert-service` runs `CONCERT_RUNTIME_MODE=production` with
`CONCERT_MIGRATION_MODE=external` in its health-only mode. Internal gRPC is
plaintext because no TLS paths are set. The stack intentionally does not
exercise mTLS, does not mount a Concert product release, and points provider
egress at `127.0.0.1:9/disabled` in the smoke-generated environment. Concert
migrations, health and readiness are real; its product RPCs are absent and are
never simulated as success. Treat this stack as staging/reference only.

### Loopback-only native profile

`scripts/mcr/backend-smoke.sh` is the same lifecycle proof without Docker. It
builds the seven binaries, owns a private PostgreSQL cluster and Redis under
`/private/tmp`, reserves non-default loopback ports, and removes everything it
created. It refuses to attach to existing stores.

```sh
# Native profile, compile first, disposable stores, full lifecycle proof.
scripts/mcr/backend-smoke.sh --mode native --build \
  --pg-bin-dir /opt/homebrew/bin \
  --redis-binary /opt/homebrew/bin/redis-server \
  --openssl-binary /opt/homebrew/bin/openssl \
  --report /absolute/evidence/native-staging.json
```

Prerequisites: Rust 1.88.0, Python 3.11+, PostgreSQL 16 client/server binaries
(`initdb`, `pg_ctl`, `psql`, `createdb`), `redis-server`, `openssl`, and
`/usr/sbin/lsof` on macOS. `--mode auto` selects Compose when the daemon is
reachable and native otherwise. The native profile stops services in reverse
start order with SIGTERM and requires exit code 0; a forced kill fails the run.

## 2. Prerequisites

- Docker Engine with Compose v2 on the staging host, and sufficient disk for one
  Rust release build.
- A private directory for the env file and backups (mode 0700/0600).
- A secret store for the values in section 3.
- Production only: a TLS-terminating ingress/LB, managed PostgreSQL 16 and Redis
  7 with their own backups, a service-to-service PKI, and DNS for the public
  origin.
- The frontend build needs `VITE_MCR_EVENT_ID` (canonical event UUID) at build
  time. It is public configuration, not a credential; no default event exists.

## 3. Required variables and secrets

### 3.1 Compose invocation

```sh
# Host-side, read from the secret store; never commit or log them.
export MCR_ENV_FILE=/secure/mcr-staging/stack.env      # mode 0600
export MCR_DB_PASSWORD=...                            # PostgreSQL owner password
export MCR_REDIS_PASSWORD=...                         # Redis AUTH password
export MCR_STACK_IMAGE=mcr-stack:$(git rev-parse --short HEAD)
```

| Variable | Required | Purpose |
| --- | --- | --- |
| `MCR_ENV_FILE` | yes | Private env file consumed by every container and the migration runner |
| `MCR_DB_PASSWORD` | yes | PostgreSQL password for `mcr_owner`; substituted into every DSN |
| `MCR_REDIS_PASSWORD` | yes | Redis AUTH password; substituted into `POST_REDIS_URL` |
| `MCR_STACK_IMAGE` | no | Image tag to run; default `mcr-stack:local` |
| `MCR_BUILD_JOBS` | no | Cargo jobs for the image build; default 3 |
| `MCR_*_HOST_PORT` | no | Loopback host-port overrides listed in section 1 |

### 3.2 Env file contents (staging)

The env file must contain the following names for the shipped staging
template. Values come from the secret store and generated key material. Rows
marked optional may be omitted and fall back to a code default. The file is
mounted into every container; do not reuse it for another project.

| Name | Consumed by | Purpose |
| --- | --- | --- |
| `GATEWAY_DATABASE_URL`, `AUTH_DATABASE_URL`, `PROFILE_DATABASE_URL`, `FRIEND_DATABASE_URL`, `POST_DATABASE_URL`, `NOTIFICATION_DATABASE_URL`, `CONCERT_DATABASE_URL` | the service itself and the migration runner | PostgreSQL DSN per owned database |
| `POST_REDIS_URL` | post, migration runner | Redis DSN with password and database index |
| `AUTH_JWT_PRIVATE_KEY_PEM_B64` | auth entrypoint | Base64 of the RSA private key; the entrypoint decodes it to `AUTH_JWT_PRIVATE_KEY_PEM` |
| `AUTH_JWT_KEY_ID` (optional) | auth | JWT `kid` published in its JWKS; defaults to `auth-service-current` |
| `AUTH_ISSUER` | auth | Token issuer |
| `AUTH_ACCESS_AUDIENCE` | auth | Access-token audience |
| `AUTH_PUBLIC_APPLICATION_ROUTES_ENABLED` | auth | Must be `false`; the Gateway owns the public auth surface |
| `AUTH_PASSWORD_RESET_MAILER_MODE` | auth | `logging` (parity) or `http` (production) |
| `AUTH_PASSWORD_RESET_BASE_URL` | auth | Absolute reset-link base URL |
| `AUTH_GOOGLE_REDIRECT_URI`, `AUTH_APPLE_REDIRECT_URI`, `AUTH_X_REDIRECT_URI` | auth | Provider callback URLs; required when OAuth is enabled |
| `ACTOR_DELEGATION_SIGNING_SEED` | gateway | Base64 Ed25519 seed for actor delegation |
| `ACTOR_DELEGATION_VERIFYING_KEYS` | gateway and owners | Comma-separated Base64 Ed25519 public keys |
| `RESOURCE_EVIDENCE_SIGNING_SEED` | post | Base64 Ed25519 seed for resource evidence |
| `RESOURCE_EVIDENCE_VERIFYING_KEYS` | gateway | Comma-separated Base64 Ed25519 public keys |
| `GATEWAY_ADMISSION_HMAC_KEY` | gateway | 32-4096 byte HMAC key for admission pseudonyms; required when identity/principal budgets are configured |
| `GATEWAY_SEARCH_CURSOR_HMAC_KEY` | gateway | 32-4096 byte shared secret; production rejects the parity default |
| `GATEWAY_JWT_ISSUER`, `GATEWAY_JWT_AUDIENCE` | gateway | Must match Auth's issuer/audience |
| `GATEWAY_JWT_KEYS_REFRESH_SECONDS`, `GATEWAY_JWT_KEYS_MIN_REFRESH_SECONDS` | gateway | JWKS refresh cadence; invalid pairs fail startup |
| `GATEWAY_CORS_ALLOWED_ORIGINS` | gateway | Exact browser origins; also the mutation allowlist |
| `GATEWAY_REFRESH_COOKIE_SECURE` | gateway | `false` only for explicit local HTTP; `true` in production |
| `GATEWAY_SERVICE_CREDENTIAL`, `AUTH_SERVICE_CREDENTIAL`, `PROFILE_SERVICE_CREDENTIAL`, `FRIEND_SERVICE_CREDENTIAL`, `POST_SERVICE_CREDENTIAL`, `NOTIFICATION_SERVICE_CREDENTIAL`, `CONCERT_SERVICE_CREDENTIAL` | every service | Distinct per-workload caller credentials |
| `CONCERT_METRICS_ADDR` | concert service | Included by the smoke-generated file; the migration command itself reads only `CONCERT_DATABASE_URL` and the migration directory |

`RUST_LOG` and `TOKIO_WORKER_THREADS` are optional. Do not add provider
credentials to a staging file that does not need them; the smoke file points
OAuth endpoints at an unreachable loopback port and sets the mailer to
`logging`.

Generate the file the same way the smoke does, or provision it from your secret
manager. `scripts/mcr/backend_smoke.py` shows the exact shape (mode 0600,
sorted `KEY=value` lines, one value per line). Never run bare
`docker compose config` in CI or logs: it resolves the env file and prints
secrets.

## 4. mTLS material and TLS termination

### 4.1 Per-workload internal gRPC identity

`backend/crates/platform/src/config.rs` derives three names from each service
prefix:

| Workload | Certificate | Private key | Peer CA |
| --- | --- | --- | --- |
| Gateway | `GATEWAY_GRPC_TLS_CERT_PATH` | `GATEWAY_GRPC_TLS_KEY_PATH` | `GATEWAY_GRPC_TLS_CA_PATH` |
| Auth | `AUTH_GRPC_TLS_CERT_PATH` | `AUTH_GRPC_TLS_KEY_PATH` | `AUTH_GRPC_TLS_CA_PATH` |
| Profile | `PROFILE_GRPC_TLS_CERT_PATH` | `PROFILE_GRPC_TLS_KEY_PATH` | `PROFILE_GRPC_TLS_CA_PATH` |
| Friend | `FRIEND_GRPC_TLS_CERT_PATH` | `FRIEND_GRPC_TLS_KEY_PATH` | `FRIEND_GRPC_TLS_CA_PATH` |
| Post | `POST_GRPC_TLS_CERT_PATH` | `POST_GRPC_TLS_KEY_PATH` | `POST_GRPC_TLS_CA_PATH` |
| Notification | `NOTIFICATION_GRPC_TLS_CERT_PATH` | `NOTIFICATION_GRPC_TLS_KEY_PATH` | `NOTIFICATION_GRPC_TLS_CA_PATH` |
| Concert | `CONCERT_GRPC_TLS_CERT_PATH` | `CONCERT_GRPC_TLS_KEY_PATH` | `CONCERT_GRPC_TLS_CA_PATH` |

Rules enforced by the code:

- Production runtime mode requires all three files for every started service.
  A missing identity fails startup; there is no plaintext production fallback.
- Supplying only one or two of the three fails startup in every mode.
- Certificates carry the workload identity; the CA verifies peers. Keep one
  directory per workload at mode 0750, certificate/CA at 0644 and key at 0600,
  and mount them read-only at the paths you set.
- Workload credentials (`<PREFIX>_SERVICE_CREDENTIAL`) are a second, independent
  control. Production requires the Gateway credential for every receiving
  service. During rotation, set `<PREFIX>_SERVICE_CREDENTIAL_PREVIOUS`; a
  previous value without a current value fails startup, and overlapping
  workload credentials are rejected.

Inbound caller requirements from source (production):

| Receiving service | Must configure |
| --- | --- |
| Gateway | `POST_SERVICE_CREDENTIAL` |
| Auth | `GATEWAY_SERVICE_CREDENTIAL`; `CONCERT_SERVICE_CREDENTIAL` when Concert reads Auth |
| Profile | `GATEWAY_SERVICE_CREDENTIAL` |
| Friend | `GATEWAY_SERVICE_CREDENTIAL` |
| Post | `GATEWAY_SERVICE_CREDENTIAL` |
| Notification | `GATEWAY_SERVICE_CREDENTIAL` |
| Concert | `GATEWAY_SERVICE_CREDENTIAL`; `AUTH_SERVICE_CREDENTIAL`, `FRIEND_SERVICE_CREDENTIAL`, `PROFILE_SERVICE_CREDENTIAL` and its own credential when Participant mode is enabled |

Present the matching `<PREFIX>_SERVICE_CREDENTIAL` on outbound calls; each
service does so from its own prefix.

### 4.2 Public edge

The Gateway HTTP listener is plaintext. Terminate TLS at the ingress/LB and
forward to the Gateway HTTP port over a private network. The edge must:

- Preserve the `Host` and exact `Origin` headers; never rewrite or append to
  `Origin`. The Gateway rejects any mutation whose `Origin` is not an exact
  entry of `GATEWAY_CORS_ALLOWED_ORIGINS`.
- Forward `X-Forwarded-For`, and list the LB addresses in
  `GATEWAY_TRUSTED_PROXY_IPS`. When the peer is not trusted, forwarded
  identity is discarded and rate limiting uses the proxy address.
- Expose only the Gateway HTTP port. Database, Redis, metrics and internal gRPC
  ports are private.

Production Gateway values:

```text
GATEWAY_RUNTIME_MODE=production
GATEWAY_HTTP_ADDR=0.0.0.0:8080            # non-loopback is required in production
GATEWAY_GRPC_ADDR=0.0.0.0:50050
GATEWAY_CORS_ALLOWED_ORIGINS=https://<public-origin>
GATEWAY_REFRESH_COOKIE_SECURE=true
GATEWAY_REFRESH_COOKIE_DOMAIN=<public-cookie-domain>
GATEWAY_REFRESH_COOKIE_MAX_AGE_SECONDS=<match AUTH_REFRESH_TTL_SECONDS>
GATEWAY_TRUSTED_PROXY_IPS=<lb-ip>,<lb-ip-2>
```

`GATEWAY_CORS_ALLOWED_ORIGINS` accepts only exact `http(s)://host[:port]`
origins: no wildcard, path, query, fragment, userinfo or trailing slash.
Production requires HTTPS and a non-empty list. Every mutation (anything except
GET/HEAD/OPTIONS) additionally requires `X-MCR-Intent: browser`; both checks
run before routing. Refresh cookies are `Secure`, `HttpOnly`, `Path=/`,
`SameSite=Lax`; a non-empty `GATEWAY_REFRESH_COOKIE_DOMAIN` is required in
production. It is a cookie domain such as `app.example.com`, not a URL; the
inherited validator rejects an origin with a scheme.

## 5. Provider endpoints and operator inputs

### 5.1 Auth email delivery

This build has no SMTP client. Auth delivers verification and reset messages
through one of two modes:

| Variable | Staging | Production |
| --- | --- | --- |
| `AUTH_PASSWORD_RESET_MAILER_MODE` | `logging` | `http` (logging is rejected in production) |
| `AUTH_PASSWORD_RESET_MAILER_ENDPOINT` | not required | HTTPS delivery endpoint |
| `AUTH_PASSWORD_RESET_MAILER_BEARER_TOKEN` | not required | Bearer token for that endpoint |
| `AUTH_PASSWORD_RESET_BASE_URL` | loopback reset base | Absolute HTTPS URL without query/fragment |
| `AUTH_ENROLLMENT_VERIFICATION_URL` | optional | Absolute HTTPS URL whose path is exactly `/verify-email`; absent means the verification link is unavailable |

The HTTP mailer receives one delivery per outbox row and must be idempotent on
its `Idempotency-Key`. No mail destination, token or body belongs in logs.

### 5.2 OAuth (public OFF, controlled parity transport only)

`GET /api/auth/capabilities` always reports GOOGLE/APPLE/X disabled with
`HANDOFF_NOT_QUALIFIED`: `OAuthExposurePolicy::public_enabled` is hard-coded
false, and a non-empty `GATEWAY_OAUTH_ENABLED_PROVIDERS` fails startup with
`Public OAuth HANDOFF_NOT_QUALIFIED`. Leave it empty.

The Gateway does mount the OAuth surface, but only when controlled transport is
configured. `browser_auth.rs` merges `oauth_routes` when the exposure policy has
transport, exposing:

```text
GET  /auth/oauth/:provider/authorize
GET  /auth/oauth/:provider/callback
POST /auth/oauth/:provider/callback
POST /auth/oauth/:provider/callback/complete
```

Controlled transport settings (all read in
`api-gateway/src/config/mod.rs:299-302`):

| Variable | Rule |
| --- | --- |
| `GATEWAY_OAUTH_ENABLED_PROVIDERS` | Must be empty. Any value fails startup with `HANDOFF_NOT_QUALIFIED`. |
| `GATEWAY_OAUTH_CONTROLLED_PROVIDERS` | Comma list of `google`, `apple`, `x`; empty/absent means the OAuth router is not merged. Duplicates or unknown names fail startup. |
| `GATEWAY_OAUTH_HANDOFF_HMAC_KEY` | 32-4096 byte key, independent of `GATEWAY_ADMISSION_HMAC_KEY`; missing or the same key fails startup. |
| `GATEWAY_OAUTH_BROWSER_ORIGIN` | Exact HTTPS origin (HTTP only for loopback); no path, query, trailing slash or userinfo. It must also appear in `GATEWAY_CORS_ALLOWED_ORIGINS`. |

Policy rules enforced by the code:

- Controlled transport is parity-only. `GATEWAY_RUNTIME_MODE=production` with a
  non-empty controlled list fails startup.
- Controlled transport also requires `GATEWAY_REFRESH_COOKIE_SECURE=true` and an
  empty `GATEWAY_REFRESH_COOKIE_DOMAIN`; the OAuth handoff key must differ from
  the admission key.
- Unknown provider: HTTP 400 (`OAuth provider is invalid`). Known provider whose
  transport is not enabled: HTTP 404 (`OAuth provider unavailable`). With no
  controlled list, the whole router is absent and all OAuth paths return 404.
- Capabilities remain disabled even when controlled transport serves real
  handlers; this is a private qualification surface, not a public login toggle.

Production Auth separately constructs an HTTP OAuth provider client at startup,
so these provider names must be configured even while public OAuth stays off.
Missing values fail Auth startup:

| Provider | Required names |
| --- | --- |
| Google | `AUTH_GOOGLE_CLIENT_ID`, `AUTH_GOOGLE_CLIENT_SECRET`, `AUTH_GOOGLE_TOKEN_URI`, `AUTH_GOOGLE_USERINFO_URI`, `AUTH_GOOGLE_JWKS_JSON`, `AUTH_GOOGLE_JWKS_URI`, `AUTH_GOOGLE_REDIRECT_URI` |
| Apple | `AUTH_APPLE_CLIENT_ID`, `AUTH_APPLE_TOKEN_URI`, `AUTH_APPLE_USERINFO_URI`, `AUTH_APPLE_JWKS_JSON`, `AUTH_APPLE_JWKS_URI`, `AUTH_APPLE_REDIRECT_URI`, plus either `AUTH_APPLE_CLIENT_SECRET` or `AUTH_APPLE_TEAM_ID` + `AUTH_APPLE_KEY_ID` + `AUTH_APPLE_PRIVATE_KEY_PEM` |
| X | `AUTH_X_CLIENT_ID`, `AUTH_X_CLIENT_SECRET`, `AUTH_X_TOKEN_URI`, `AUTH_X_USERINFO_URI`, `AUTH_X_REDIRECT_URI` |

Optional: `AUTH_GOOGLE_ISSUER`, `AUTH_APPLE_ISSUER`,
`AUTH_{GOOGLE,APPLE,X}_AUTHORIZE_BASE_URI`,
`AUTH_APPLE_CLIENT_SECRET_TTL_SECONDS`, `AUTH_JWT_ROTATION_JWKS_JSON`.
Production validates provider URLs as HTTPS and rejects placeholder hosts.

This is an absent production input, not a documentation omission: real provider
registration is not supplied by this repository.

### 5.3 Service-to-service endpoints

Set these explicitly; some code defaults differ from the Compose port mapping
(`GATEWAY_NOTIFICATION_GRPC_ENDPOINT` defaults to 50054 in source while Compose
maps Notification to 50055).

| Caller | Endpoint names |
| --- | --- |
| Gateway | `GATEWAY_AUTH_GRPC_ENDPOINT`, `GATEWAY_PROFILE_GRPC_ENDPOINT`, `GATEWAY_FRIEND_GRPC_ENDPOINT`, `GATEWAY_POST_GRPC_ENDPOINT`, `GATEWAY_NOTIFICATION_GRPC_ENDPOINT`, `GATEWAY_CONCERT_GRPC_ENDPOINT` (optional; product routes stay unavailable when absent), `GATEWAY_CONCERT_RELEASE_DIGEST` (optional, exact 64-hex release digest) |
| Auth | `AUTH_PROFILE_GRPC_ENDPOINT`, `AUTH_FRIEND_GRPC_ENDPOINT`, `AUTH_POST_GRPC_ENDPOINT`, `AUTH_NOTIFICATION_GRPC_ENDPOINT`; `AUTH_CONCERT_GRPC_ENDPOINT` + `AUTH_CONCERT_DELETION_MODE=managed` + `AUTH_CONCERT_DELETION_DEADLINE_MS` when managed deletion is enabled |
| Friend | `FRIEND_PROFILE_GRPC_ENDPOINT` |
| Post | `POST_PROFILE_GRPC_ENDPOINT`, `POST_FRIEND_GRPC_ENDPOINT`, `POST_NOTIFICATION_GRPC_ENDPOINT`, `POST_GATEWAY_GRPC_ENDPOINT`; `POST_CONCERT_READ_MODE`, `POST_CONCERT_READ_AUTH_GRPC_ENDPOINT`, `POST_CONCERT_ROOT_GRPC_ENDPOINT`, `CONCERT_POST_READ_VERIFYING_KEYS`, `POST_CONCERT_ROOT_READ_SIGNING_SEED` when Concert feed reads are enabled |
| Notification | `NOTIFICATION_CONCERT_MODE=live` plus `NOTIFICATION_CONCERT_AUTH_GRPC_ENDPOINT`, `NOTIFICATION_CONCERT_FRIEND_GRPC_ENDPOINT`, `NOTIFICATION_CONCERT_ROOT_GRPC_ENDPOINT` |
| Concert | `CONCERT_AUTH_GRPC_ENDPOINT`, `CONCERT_FRIEND_GRPC_ENDPOINT`, `CONCERT_PROFILE_GRPC_ENDPOINT`, `CONCERT_POST_GRPC_ENDPOINT`, `CONCERT_NOTIFICATION_GRPC_ENDPOINT`; `CONCERT_POST_READ_SIGNING_SEED`, `POST_CONCERT_ROOT_READ_VERIFYING_KEYS` when feed reads are enabled; `CONCERT_PARTICIPANT_MODE=live` adds `CONCERT_PARTICIPANT_AUTH_GRPC_ENDPOINT`, `CONCERT_PARTICIPANT_FRIEND_GRPC_ENDPOINT`, `CONCERT_PARTICIPANT_PROFILE_GRPC_ENDPOINT`, `CONCERT_PRODUCER_INSTANCE_ID`, `CONCERT_PARTICIPANT_RELEASE_DIGEST`, `CONCERT_PARTICIPANT_RECOVERY_KEYS_JSON`, `CONCERT_RELATIONSHIP_SIGNING_SEED`, `CONCERT_PROFILE_SELF_SIGNING_SEED`, `PROFILE_PARTICIPANT_DECISION_VERIFYING_KEYS` |

The Concert-to-Post read edge uses two independent Ed25519 pairs. Concert signs
with `CONCERT_POST_READ_SIGNING_SEED`; Post verifies with
`CONCERT_POST_READ_VERIFYING_KEYS`. The Post-to-Concert root-read callback is
signed with `POST_CONCERT_ROOT_READ_SIGNING_SEED` and verified with
`POST_CONCERT_ROOT_READ_VERIFYING_KEYS`. These keys must be distinct from
Gateway and parity keys; a partial or shared-key configuration fails startup.

### 5.4 Post external publisher

`POST_EXTERNAL_PUBLISHER` is `disabled` in staging. Production accepts
`disabled` or `kafka`; `kafka` additionally requires
`POST_KAFKA_BOOTSTRAP_BROKERS`. `POST_REDIS_URL` is always required.
Broker URLs are validated as non-local in production.

### 5.5 Concert product serving

Compose runs Concert health-only: no `CONCERT_GRPC_ADDR`, so no product gRPC
listener and no release bundle. Enabling product serving requires all six files
and an explicit non-loopback address:

```text
CONCERT_RUNTIME_MODE=production
CONCERT_MIGRATION_MODE=external
CONCERT_GRPC_ADDR=<private-ip>:50056
CONCERT_BOOTSTRAP_FILE=<read-only path>
CONCERT_AVATAR_MANIFEST_FILE=<read-only path>
CONCERT_MAP_MANIFEST_FILE=<read-only path>
CONCERT_REVENGE_MANIFEST_FILE=<read-only path>
CONCERT_PARADE_MANIFEST_FILE=<read-only path>
CONCERT_DANGER_MANIFEST_FILE=<read-only path>
CONCERT_GRPC_TLS_CERT_PATH=<read-only path>
CONCERT_GRPC_TLS_KEY_PATH=<read-only path>
CONCERT_GRPC_TLS_CA_PATH=<read-only path>
```

The operator release bundle is produced by
`backend/scripts/mcr/build_operator_bundle.py` from reviewed sources; the
repository ships only the inert `quality-v6` candidate with every capability
false. Do not substitute fixture packs. Feature modes (feed, social, intro,
report, wall, owner media) are separate explicit settings and stay off unless
the corresponding producer and provider are composed.

### 5.6 Operator roles

Roles are event-scoped evidence, not JWT claims:

- `OPERATOR` is the authoritative producer of event revision and phase.
- `MODERATOR` handles reports, notices and content actions.
- Membership never grants an operator role. A missing or stale role owner denies
  the action; operator status alone never certifies source review.
- The managed issuer helper `backend/scripts/mcr/in04_wall_runtime/roles.py`
  signs an Ed25519 `MCR-OPERATIONS-ASSIGNMENT-V1` statement and publishes a
  mode-0600 JSON file for exactly one initial `OPERATOR` assignment at expected
  revision 0. The production `OperationsPublication` worker must apply that
  input with the same configured keyring and pool.
- No role table, self-grant route or JWT role claim exists. Real same-window role
  application, OwnAccess and Admin observation are **NOT_RUN**.

### 5.7 Retention values and event clock

Operational defaults (env-tunable unless noted):

| Item | Default / rule | Variable |
| --- | --- | --- |
| Access token | 900 s (60-86400) | `AUTH_ACCESS_TTL_SECONDS` |
| Refresh token | 1,209,600 s / 14 days (300-31,536,000) | `AUTH_REFRESH_TTL_SECONDS` |
| Refresh cookie | 1,209,600 s; must be set in production | `GATEWAY_REFRESH_COOKIE_MAX_AGE_SECONDS` |
| Gateway audit retention | 365 days (1-3650) | `GATEWAY_AUDIT_RETENTION_DAYS` |
| Feed event retention | 90 days (1-3650) | `POST_FEED_EVENT_RETENTION_DAYS` |
| Recommendation event retention | 90 days (1-3650) | `FRIEND_RECOMMENDATION_EVENT_RETENTION_DAYS` |
| Purge worker interval | 3,600,000 ms | `{PROFILE,POST,NOTIFICATION}_RETENTION_PURGE_WORKER_INTERVAL_MS` |
| Purge worker batch | 1,000 (Profile), 500 (Post) | `PROFILE_MUTATION_RECEIPT_RETENTION_BATCH_SIZE`, `POST_FEED_RETENTION_BATCH_SIZE` |
| Login lockout | 5 failures / 900 s | `AUTH_LOGIN_LOCKOUT_MAX_FAILURES`, `AUTH_LOGIN_LOCKOUT_WINDOW_SECONDS` |
| Gateway rate limit | 600 requests/minute | `GATEWAY_RATE_LIMIT_PER_MINUTE` |
| Search cursor TTL | 900 s (60-3600) | `GATEWAY_SEARCH_CURSOR_TTL_SECONDS` |
| Request timeout | 1000 ms; journal client timeout must be lower | `GATEWAY_REQUEST_TIMEOUT_MS`, `GATEWAY_JOURNAL_CLIENT_TIMEOUT_MS` |

Production requires a UUID in `PROFILE_RETENTION_PURGE_ACTOR_ID`,
`POST_RETENTION_PURGE_ACTOR_ID` and
`NOTIFICATION_RETENTION_PURGE_ACTOR_ID`. Use one dedicated, auditable system
principal per service.

Event time and venue expiry:

- `concert_events.timezone` is constrained to `Asia/Seoul`. Server and
  database time are authoritative; browser, device and server-local timezones
  are never used.
- `end_of_day` is the next 00:00:00 in the event timezone strictly after the
  database `server_time`, capped by an earlier approved event cutoff or the
  access deadline.
- Note expiry `ONE_HOUR` is exactly 3,600,000 ms from the database
  `created_at`; `VENUE_END_DAY` uses `end_of_day`. Expiry is absolute and a
  retry never extends it.
- Venue location policy has no code default. The operator policy provider must
  supply `fresh_ms`, `last_known_ms`, `fix_max_age_ms`, `future_skew_ms`,
  `sharing_lease_ms`, `request_ttl_ms`, `check_in_ttl_ms`,
  `coordinate_retention_ms`, `command_retention_ms`, `max_accuracy_m`,
  `max_viewers`, `timezone`, `valid_until`, `revision`, `venue_key`,
  `provider_id` and `source_revision`. Missing, stale or contradictory
  policy returns 503 before any disclosure.
- Invitation and message content TTLs and purge deadlines are operator policy
  inputs; the code has no fallback and the repository does not ship values.
  Automatic expiry/purge scheduling for venue content is **NOT_RUN**.

## 6. Start the staging stack

All commands run from the repository root. Define one helper for the project:

```sh
mcr() { docker compose -p mcr-staging -f infra/mcr/compose.yml "$@"; }
```

1. Preflight.

```sh
git rev-parse HEAD
docker compose version
docker info
```

2. Build the one release image (not executed by this repository; see section 12).

```sh
docker build -f infra/mcr/backend/stack.Dockerfile \
  --build-arg SOURCE_REVISION="$(git rev-parse HEAD)" \
  --build-arg CARGO_BUILD_JOBS=3 \
  -t "mcr-stack:$(git rev-parse --short HEAD)" .
```

3. Export `MCR_ENV_FILE`, `MCR_DB_PASSWORD`, `MCR_REDIS_PASSWORD` and
`MCR_STACK_IMAGE` as in section 3.1.

4. Validate composition without printing secrets.

```sh
mcr config --services
```

Expect ten lines: `postgres`, `redis`, `migrate`, and the seven services.

5. Start. `migrate` runs first; every application service waits for
   `service_completed_successfully`.

```sh
mcr up -d --no-build
mcr ps
```

6. Wait for liveness (each metrics listener).

```sh
for port in 19090 19091 19092 19093 19094 19095 19096; do
  printf '%s %s\n' "$port" "$(curl -s -o /dev/null -w '%{http_code}' "http://127.0.0.1:$port/livez")"
done
```

7. Require gateway readiness with every required dependency up.

```sh
curl -fsS http://127.0.0.1:19090/readyz
```

8. Check one public route without a session; expect HTTP 401 with code
   `GATEWAY_401`.

```sh
event=$(uuidgen | tr 'A-Z' 'a-z')
curl -s -o /dev/null -w '%{http_code}\n' \
  "http://127.0.0.1:18080/api/concert/events/$event/bootstrap"
```

9. Run the full lifecycle proof (fresh volumes, restart persistence, peer
   outage, teardown) instead of trusting a manual start.

```sh
scripts/mcr/backend-smoke.sh --mode compose --report /absolute/evidence/compose-staging.json
```

Compose mode exits non-zero with a `NOT_RUN` receipt when the Docker daemon is
unavailable. It never reports success without a daemon.

## 7. Migrations

Fresh start: the `migrate` service runs `migrate-all.sh`, which for each of the
seven services executes `<service> --migrate` followed by
`<service> --verify-migrations`. Any failure stops the stack before an
application service starts. Every service runs with
`<PREFIX>_DATABASE_MIGRATION_MODE=external` (Concert:
`CONCERT_MIGRATION_MODE=external`), so a running service verifies the schema
instead of changing it.

Re-run the whole set:

```sh
mcr run --rm --no-deps migrate
```

Apply and verify one service:

```sh
mcr run --rm --no-deps --entrypoint /usr/local/bin/api-gateway api-gateway --migrate
mcr run --rm --no-deps --entrypoint /usr/local/bin/api-gateway api-gateway --verify-migrations
```

Repeat with `auth-service`, `profile-service`, `friend-service`,
`post-service`, `notification-service` and `concert-service`.

Inspect the applied ledger (successful rows only):

```sh
for db in mcr_api_gateway mcr_auth mcr_profile mcr_friend mcr_post mcr_notification mcr_concert; do
  printf '%s ' "$db"
  mcr exec -T postgres sh -lc \
    "PGPASSWORD=\"\$MCR_DB_PASSWORD\" psql -U mcr_owner -d $db -Atc \
     \"select count(*), max(version) from public._sqlx_migrations where success\""
done
```

Migrations are forward-only; there are no down files. A failed migration must be
diagnosed and fixed forward or recovered from backup (section 8). Do not edit a
ledger row.

## 8. Backup and restore

Logical backup per database is the supported procedure. It preserves the
migration ledger, so `--verify-migrations` can check a restored database.

```sh
backup=/secure/mcr-backups/$(date -u +%Y%m%dT%H%M%SZ)
install -d -m 0700 "$backup"
for db in mcr_api_gateway mcr_auth mcr_profile mcr_friend mcr_post mcr_notification mcr_concert; do
  mcr exec -T postgres sh -lc \
    "PGPASSWORD=\"\$MCR_DB_PASSWORD\" pg_dump -U mcr_owner -Fc -d $db" > "$backup/$db.dump"
done
sha256sum "$backup"/*.dump > "$backup/SHA256SUMS"
chmod 0600 "$backup"/* "$backup/SHA256SUMS"
```

Redis uses AOF (`--appendonly yes`). Snapshot it after a save:

```sh
mcr exec -T redis sh -lc 'redis-cli --no-auth-warning -a "$MCR_REDIS_PASSWORD" SAVE'
mcr cp redis:/data/dump.rdb "$backup/redis-dump.rdb"
```

Restore (target stack running with applications stopped):

```sh
mcr stop -t 40 api-gateway concert-service auth-service post-service \
  friend-service notification-service profile-service
mcr exec -T postgres sh -lc \
  'PGPASSWORD="$MCR_DB_PASSWORD" pg_restore -U mcr_owner -d mcr_auth \
   --clean --if-exists --no-owner' < "$backup/mcr_auth.dump"
# repeat per database, then:
mcr run --rm --no-deps migrate
mcr up -d
```

Restore Redis only after Postgres is healthy and Post is stopped:

```sh
mcr stop -t 40 post-service redis
mcr cp "$backup/redis-dump.rdb" redis:/data/dump.rdb
mcr start redis post-service
```

Do not restore into a stack that is serving traffic, and never restore a
different project's dump. No automated backup schedule or restore drill exists
in this repository; both are **NOT_RUN** operator obligations. For production
use the managed database's PITR/backup facility instead of these container
commands.

## 9. Restart ordering and graceful shutdown

Start order (dependencies first):

```text
postgres -> redis -> migrate -> profile-service -> notification-service
  -> friend-service -> post-service -> auth-service -> concert-service
  -> api-gateway
```

Compose encodes the database/migration gates and peer start ordering. Stop in
the reverse order:

```sh
mcr stop -t 40 api-gateway concert-service auth-service post-service \
  friend-service notification-service profile-service
mcr stop -t 40 redis
mcr stop -t 40 postgres
```

Every service handles SIGTERM by requesting shutdown: readiness flips to
`ready:false`, workers finish their current cycle, and `supervise` allows up to
30 seconds for the listener/worker set to exit. Docker's default 10-second stop
grace would SIGKILL first, so always pass `-t 40` (or set
`stop_grace_period: 40s`) and treat a forced kill as a failed shutdown in
staging. Restart a single service with `mcr restart -t 40 <service>` only after
its dependencies are healthy; it is not a substitute for the ordered stop.

## 10. Health, readiness and logs

| Endpoint | Meaning |
| --- | --- |
| `GET /livez` on each metrics port | Process liveness only. Stays 200 during a database outage. |
| `GET /readyz` on each metrics port | Dependency readiness. 200 only when every required check is `up`; 503 otherwise, with a per-check body. |

Required readiness checks: gateway `database`, `gateway->auth`,
`gateway->profile`, `gateway->friend`, `gateway->post`,
`gateway->notification`, `signing-keys`; auth `database` plus optional peer
edges; profile `database`; friend `database` and `friend->profile`; post
`database`, `post->friend`, `post->gateway` (profile/notification edges are
optional); notification `database`; concert `database`.

```sh
mcr ps
mcr logs --tail=200 api-gateway
mcr logs --tail=200 auth-service
```

Do not treat `GET /livez` as readiness. A gateway that reports
`degraded:true` while still ready has an optional dependency down.

## 11. Rollback

- Pin the image by source revision (`MCR_STACK_IMAGE`) and keep the previous
  tag. Rollback the image and env file together; a config change is part of the
  release.
- Migrations are forward-only. If the previous image cannot read the new schema,
  restore the matching backup and its ledger, then start the previous image.
- Restore is the only supported schema rollback. Rehearse it in staging before
  any production schema change.

## 12. Known gaps and explicit non-claims

| Gap | State | Consequence |
| --- | --- | --- |
| Compose image build and container execution | **NOT_RUN** (Docker daemon unavailable) | `stack.Dockerfile` is statically validated only; a Linux build may still fail |
| 200-user / capacity qualification (QA08) | **DEFERRED_BY_USER / NOT_RUN** | No throughput, latency or concurrency claim |
| Device / browser acceptance | **NOT_RUN** | No iOS/Android or real-device claim |
| Backup schedule and restore drill | **NOT_RUN** | Procedures above are unexercised |
| Per-service least-privilege database roles | absent | All services share `mcr_owner` in this stack |
| Public OAuth | OFF; controlled parity transport only | Gateway mounts OAuth only with `GATEWAY_OAUTH_CONTROLLED_PROVIDERS` in parity mode; public capabilities stay disabled. Provider configuration is still a production Auth startup requirement (section 5.2) |
| Email delivery provider | absent (staging uses `logging`) | Production requires an HTTPS mailer endpoint and bearer token |
| Approved eligibility/legal/consent publication (FE10) | absent | Signup verification links have no approved policy text |
| Operator role application | source/helper only | Production role publication and Admin observation are **NOT_RUN** |
| Venue policy values and expiry scheduling | operator input absent | Location/meeting/notes return 503 or unavailable; scheduling **NOT_RUN** |
| Concert product release bundle | not shipped | Product RPCs are unavailable in Compose; fixtures are not substitutes |
| W04 virtual-room invitation policy | OPEN / NOT_IMPLEMENTED | No route or authority exists |

No statement in this document is evidence of production capacity, device
validation, QA08 completion, or release acceptance.

## 13. Dry read-through checklist

Use this when reviewing or executing the runbook:

- [ ] Section 1 ports and service names match `infra/mcr/compose.yml`.
- [ ] Every secret in section 3 comes from the secret store, not this document.
- [ ] `mcr config --services` prints ten services and no secret values.
- [ ] `mcr up -d --no-build` reaches all seven `/livez` 200.
- [ ] Gateway `/readyz` returns 200 with every required check `up`.
- [ ] The public bootstrap probe returns 401 `GATEWAY_401`.
- [ ] Each service reports `--verify-migrations` exit 0.
- [ ] `mcr stop -t 40` leaves no forced kill in the logs.
- [ ] A backup and a restore into a fresh stack both succeed (NOT_RUN until drilled).
- [ ] No step claims QA08, capacity, device or release acceptance.

## Appendix A. Complete environment surface

Names below are read by the current source. They have code defaults unless
marked; set them explicitly in production so the deployment does not inherit a
development default. Feature and worker rows apply only when that feature is
enabled.

**Per-service startup names** (set every one explicitly; the staging env file and
Compose project cover the values that differ):

- Gateway: `GATEWAY_RUNTIME_MODE`, `GATEWAY_DATABASE_MIGRATION_MODE`,
  `GATEWAY_SEED_ENABLED` (must be false), `GATEWAY_HTTP_ADDR`,
  `GATEWAY_GRPC_ADDR`, `GATEWAY_METRICS_ADDR`,
  `GATEWAY_DB_POOL_MAX_CONNECTIONS`, `GATEWAY_JOURNAL_DB_POOL_MAX_CONNECTIONS`,
  `GATEWAY_BACKGROUND_DB_POOL_MAX_CONNECTIONS`, `GATEWAY_JWT_JWKS_JSON`
  (required when `GATEWAY_JWT_KEYS_REFRESH_SECONDS=0`).
- Auth: `AUTH_RUNTIME_MODE`, `AUTH_DATABASE_MIGRATION_MODE`,
  `AUTH_SEED_ENABLED` (must be false), `AUTH_HTTP_ADDR`, `AUTH_GRPC_ADDR`,
  `AUTH_METRICS_ADDR`, `AUTH_DB_POOL_MAX_CONNECTIONS`,
  `AUTH_GOOGLE_AUTHORIZE_BASE_URI`, `AUTH_APPLE_AUTHORIZE_BASE_URI`,
  `AUTH_X_AUTHORIZE_BASE_URI`, `AUTH_CONCERT_DELETION_SOURCE_ENABLED`.
- Profile: `PROFILE_RUNTIME_MODE`, `PROFILE_DATABASE_MIGRATION_MODE`,
  `PROFILE_SEED_ENABLED`, `PROFILE_HTTP_ADDR`, `PROFILE_GRPC_ADDR`,
  `PROFILE_METRICS_ADDR`, `PROFILE_DB_POOL_MAX_CONNECTIONS`.
- Friend: `FRIEND_RUNTIME_MODE`, `FRIEND_DATABASE_MIGRATION_MODE`,
  `FRIEND_SEED_ENABLED`, `FRIEND_HTTP_ADDR`, `FRIEND_GRPC_ADDR`,
  `FRIEND_METRICS_ADDR`, `FRIEND_DB_POOL_MAX_CONNECTIONS`,
  `FRIEND_DB_POOL_ACQUIRE_TIMEOUT_MS`.
- Post: `POST_RUNTIME_MODE`, `POST_DATABASE_MIGRATION_MODE`,
  `POST_SEED_ENABLED`, `POST_HTTP_ADDR`, `POST_GRPC_ADDR`,
  `POST_METRICS_ADDR`, `POST_DB_POOL_MAX_CONNECTIONS`,
  `POST_DB_POOL_ACQUIRE_TIMEOUT_MS`.
- Notification: `NOTIFICATION_RUNTIME_MODE`,
  `NOTIFICATION_DATABASE_MIGRATION_MODE`, `NOTIFICATION_HTTP_ADDR`,
  `NOTIFICATION_GRPC_ADDR`, `NOTIFICATION_METRICS_ADDR`,
  `NOTIFICATION_DB_POOL_MAX_CONNECTIONS`.
- Concert: `CONCERT_MIGRATION_DIRECTORY` (defaults to the compiled migration
  path; set only to a reviewed operator package).

**Worker and queue tuning** (operational; defaults are safe for staging):

- Auth: `AUTH_LIFECYCLE_CLEANUP_WORKER_ENABLED`,
  `AUTH_LIFECYCLE_CLEANUP_WORKER_INTERVAL_MS`,
  `AUTH_USER_DELETION_WORKER_ENABLED`, `AUTH_USER_DELETION_WORKER_INTERVAL_MS`,
  `AUTH_USER_DELETION_WORKER_BATCH_SIZE`,
  `AUTH_USER_DELETION_WORKER_MAX_ATTEMPTS`,
  `AUTH_USER_DELETION_WORKER_LEASE_TIMEOUT_MS`,
  `AUTH_USER_DELETION_WORKER_BASE_RETRY_MS`, `AUTH_PASSWORD_WORKERS`,
  `AUTH_PASSWORD_LOGIN_QUEUE`,
  `AUTH_PASSWORD_RESET_QUEUE`, `AUTH_PASSWORD_RESET_WORKERS`,
  `AUTH_PASSWORD_SIGNUP_QUEUE`, `AUTH_PASSWORD_SIGNUP_WORKERS`.
- Gateway: `GATEWAY_ADMISSION_MODE`, `GATEWAY_ADMISSION_BUDGETS`,
  `GATEWAY_ADMISSION_IDENTITY_BUDGETS`, `GATEWAY_ADMISSION_PRINCIPAL_BUDGETS`,
  `GATEWAY_ADMISSION_MAX_IN_FLIGHT`, `GATEWAY_ADMISSION_FALLBACK_PER_SECOND`,
  `GATEWAY_ADMISSION_OVERFLOW_FACTOR`, `GATEWAY_AGGREGATION_CACHE_TTL_SECONDS`,
  `GATEWAY_ASYNC_OPERATION_KEYRING`,
  `GATEWAY_HIGH_RISK_SESSION_FRESHNESS_SECONDS`,
  `GATEWAY_RATE_LIMIT_V6_PREFIX_LENGTH`,
  `GATEWAY_SEARCH_CURSOR_LEGACY_ACCEPT_UNTIL`,
  `GATEWAY_TELEMETRY_QUEUE_CAPACITY`, `GATEWAY_TELEMETRY_RETENTION_DAYS`,
  `GATEWAY_DOCS_GRAFANA_URL`, `GATEWAY_DOCS_PROFILES_URL`,
  `GATEWAY_DOCS_PROMETHEUS_URL`, `GATEWAY_DOCS_PYROSCOPE_URL`,
  `GATEWAY_DOCS_SWAGGER_URL`, `GATEWAY_SERVICE_CREDENTIAL_PREVIOUS`,
  `GATEWAY_OAUTH_ENABLED_PROVIDERS`, `GATEWAY_OAUTH_CONTROLLED_PROVIDERS`,
  `GATEWAY_OAUTH_HANDOFF_HMAC_KEY`, `GATEWAY_OAUTH_BROWSER_ORIGIN`.
- Post: `POST_OUTBOX_WORKER_ENABLED`, `POST_OUTBOX_WORKER_INTERVAL_MS`,
  `POST_OUTBOX_WORKER_BATCH_SIZE`, `POST_OUTBOX_WORKER_MIN_EVENT_AGE_MS`,
  `POST_OUTBOX_DELIVERY_LEASE_MS`, `POST_OUTBOX_DELIVERY_MAX_ATTEMPTS`,
  `POST_OUTBOX_DELIVERY_RETRY_BASE_MS`, `POST_OUTBOX_DELIVERY_RETRY_CAP_MS`,
  `POST_OUTBOX_DELIVERY_EFFECT_TIMEOUT_MS`,
  `POST_OUTBOX_DELIVERY_COMPLETION_RESERVE_MS`,
  `POST_TIMELINE_FANOUT_WORKER_ENABLED`,
  `POST_TIMELINE_FANOUT_WORKER_INTERVAL_MS`,
  `POST_TIMELINE_FANOUT_BASE_RETRY_MS`, `POST_TIMELINE_FANOUT_CLAIM_LIMIT`,
  `POST_TIMELINE_FANOUT_LEASE_TIMEOUT_MS`,
  `POST_TIMELINE_FANOUT_MAX_ATTEMPTS`, `POST_TIMELINE_FOLLOWER_PAGE_SIZE`,
  `POST_TIMELINE_ENTRY_TTL_SECONDS`, `POST_HIGH_FANOUT_FOLLOWER_THRESHOLD`,
  `POST_FEED_CACHE_EPOCH`, `POST_FEED_CANDIDATE_PER_SOURCE_LIMIT`,
  `POST_FEED_CANDIDATE_SOURCE_TIMEOUT_MS`,
  `POST_FEED_EMBEDDING_SOURCE_ENABLED`, `POST_FEED_FOLLOWING_AUTHOR_LIMIT`,
  `POST_FEED_RANKER_ADAPTER`, `POST_FEED_RANK_VERSION`,
  `POST_FEED_RECENT_SEQUENCE_ENABLED`, `POST_FEED_RECENT_SEQUENCE_LIMIT`,
  `POST_FEED_RETENTION_WORKER_ENABLED`, `POST_FEED_RETENTION_WORKER_INTERVAL_MS`,
  `POST_FEED_SNAPSHOT_CANDIDATE_LIMIT`, `POST_FEED_SNAPSHOT_TTL_SECONDS`,
  `POST_RETENTION_PURGE_WORKER_ENABLED`,
  `POST_RETENTION_PURGE_WORKER_INTERVAL_MS`.
- Post Kafka (only when `POST_EXTERNAL_PUBLISHER=kafka`):
  `POST_KAFKA_TOPIC`, `POST_KAFKA_BOOTSTRAP_BROKERS`,
  `POST_KAFKA_CREATE_TOPIC`, `POST_KAFKA_PARTITION_COUNT`,
  `POST_KAFKA_CONNECT_TIMEOUT_MS`, `POST_KAFKA_PUBLISH_TIMEOUT_MS`.
- Friend: `FRIEND_MUTATION_RECEIPT_RETENTION_WORKER_ENABLED`,
  `FRIEND_MUTATION_RECEIPT_RETENTION_WORKER_INTERVAL_MS`,
  `FRIEND_MUTATION_RECEIPT_RETENTION_BATCH_SIZE`,
  `FRIEND_RECOMMENDATION_WORKER_ENABLED`,
  `FRIEND_RECOMMENDATION_WORKER_INTERVAL_MS`,
  `FRIEND_RECOMMENDATION_WORKER_BATCH_SIZE`,
  `FRIEND_RECOMMENDATION_WORKER_MAX_ATTEMPTS`,
  `FRIEND_RECOMMENDATION_WORKER_LEASE_TIMEOUT_MS`,
  `FRIEND_RECOMMENDATION_WORKER_BASE_RETRY_MS`,
  `FRIEND_RECOMMENDATION_RETENTION_WORKER_ENABLED`,
  `FRIEND_RECOMMENDATION_RETENTION_WORKER_INTERVAL_MS`,
  `FRIEND_RECOMMENDATION_RETENTION_BATCH_SIZE`,
  `FRIEND_PEOPLE_RANK_VERSION` (required non-empty),
  `FRIEND_PEOPLE_RANKER_ADAPTER`,
  `FRIEND_PEOPLE_EMBEDDING_SOURCE_ENABLED`,
  `FRIEND_PEOPLE_RECENT_SEQUENCE_ENABLED`,
  `FRIEND_PEOPLE_RECENT_SEQUENCE_LIMIT`.
- Profile: `PROFILE_RETENTION_PURGE_WORKER_ENABLED`,
  `PROFILE_RETENTION_PURGE_WORKER_INTERVAL_MS`,
  `PROFILE_MUTATION_RECEIPT_RETENTION_WORKER_ENABLED`,
  `PROFILE_MUTATION_RECEIPT_RETENTION_WORKER_INTERVAL_MS`.
- Notification: `NOTIFICATION_RETENTION_PURGE_WORKER_ENABLED`,
  `NOTIFICATION_RETENTION_PURGE_WORKER_INTERVAL_MS`.

**Concert feature settings** (required only when the matching feature mode is
enabled; all default to off/unavailable):

- `CONCERT_ACCOUNT_DELETION_MODE`, `CONCERT_PUBLIC_FEED_MODE`,
  `CONCERT_FEED_CURSOR_KEY_HEX`, `CONCERT_GIFT_MODE`,
  `CONCERT_GIFT_CURSOR_KEY_HEX`, `CONCERT_GIFT_RECEIPT_KEYS_JSON`,
  `CONCERT_INTRO_MODE`, `CONCERT_INTRO_CURSOR_KEY_HEX`,
  `CONCERT_INTRO_RECEIPT_KEYS_JSON`, `CONCERT_MEDIA_RECEIPT_KEYS_JSON`,
  `CONCERT_MEDIA_SANITIZER_QUEUE`, `CONCERT_MEDIA_SANITIZER_WORKERS`,
  `CONCERT_NOTIFICATION_INBOX_MODE`, `CONCERT_NOTIFICATION_TARGET_MODE`,
  `CONCERT_OWNER_MEDIA_MODE`, `CONCERT_PICKUP_KEYS_JSON`,
  `CONCERT_REPORT_MODE`, `CONCERT_REPORT_CURSOR_KEY_HEX`,
  `CONCERT_REPORT_RECEIPT_KEYS_JSON`, `CONCERT_SOCIAL_MODE`,
  `CONCERT_SOCIAL_REQUEST_TTL_SECONDS`, `CONCERT_SOCIAL_CURSOR_KEY_HEX`,
  `CONCERT_SOCIAL_HANDLE_KEYS_JSON`, `CONCERT_SOCIAL_COMMAND_KEYS_JSON`,
  `CONCERT_SOCIAL_ACCEPTED_CLAIM_ISSUER`,
  `CONCERT_SOCIAL_ACCEPTED_CLAIM_REVISION`, `CONCERT_WALL_TOOLS_B_MODE`,
  `CONCERT_WALL_TOOLS_B_RECEIPT_KEYS_JSON`, `CONCERT_WORLD_LISTEN_ADDR`,
  `CONCERT_WORLD_ALLOWED_ORIGINS`, `CONCERT_WORLD_OWNER_ID`,
  `CONCERT_WORLD_EVENT_ID`, `CONCERT_WORLD_ROOMS_JSON`,
  `CONCERT_WORLD_RESOURCE_ROOT`, `CONCERT_WORLD_SOURCE_BINDING_FILE`,
  `CONCERT_WORLD_INVALIDATION_CONSUMER_ID`.

`MCR_ENROLLMENT_SIGNUP_V1` is an enrollment-policy identifier, not an
environment variable.
