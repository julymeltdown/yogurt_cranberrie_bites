# Task 44 — deployment and production inputs runbook

Status: **DOCUMENTED**. Deliverable is a new runbook plus this evidence file.
No application, infrastructure or test source changed. Compose execution and a
restore drill remain **NOT_RUN** (no Docker daemon); QA08 remains
**DEFERRED_BY_USER / NOT_RUN**.

- Plan row: 44 "Document deployment and production inputs"
- Worktree: `/Users/lhs/Desktop/mcr-worktrees/exec-w8-deploy`
- Branch: `codex/mcr-exec-w8-deploy`
- Base: `b2676f68c`
- Deliverable: `docs/mcr/ops/deployment-runbook.md`
- Sparse checkout: docs, infra, scripts, backend/scripts, backend/docs,
  contracts, generated. Backend `src` is absent from the working tree, so the
  code cross-check reads committed objects with `git show HEAD:<path>`.

## What the document covers

1. Ten-entry Compose topology (PostgreSQL, Redis, one-shot migration runner,
   seven services) with exact internal/host ports and the health-only Concert
   mode.
2. Loopback-only native smoke profile and its prerequisites.
3. Host variables and the private env file, naming every secret without a value.
4. Per-workload mTLS (`<PREFIX>_GRPC_TLS_{CERT,KEY,CA}_PATH`), inbound caller
   credentials, public TLS termination, CORS/origin and trusted-proxy rules.
5. Auth email delivery, OAuth default-off behavior (including the production
   startup requirement for provider configuration), service endpoints, Post
   publisher, Concert release inputs, operator roles, retention and the
   `Asia/Seoul` event clock/end-of-day policy.
6. Start, migration apply/verify per service, backup/restore, restart ordering,
   graceful shutdown, health/readiness, rollback and known gaps.

## Variable cross-check

### Extraction command

```sh
python3 - <<'EOF'
import re, subprocess, pathlib
doc = pathlib.Path('docs/mcr/ops/deployment-runbook.md').read_text()
doc_names = set(re.findall(r'\b[A-Z][A-Z0-9_]{2,}\b', doc))
files = subprocess.check_output(['git','ls-tree','-r','--name-only','HEAD'], text=True).splitlines()
paths = [p for p in files if (('/src/config/' in p or p.endswith('/src/config/mod.rs'))
         and p.startswith('backend/services/'))
         or p in ('backend/crates/platform/src/config.rs',
                  'backend/crates/platform/src/actor_delegation.rs',
                  'backend/crates/platform/src/actor_delegation/exchange/resources.rs',
                  'backend/services/api-gateway/src/adapters/in/http/client_identity.rs',
                  'backend/services/auth-service/src/enrollment_policy/store.rs')]
names = set()
for p in paths:
    text = subprocess.check_output(['git','show',f'HEAD:{p}'], text=True)
    for m in re.findall(r'"([A-Z][A-Z0-9_]{2,})"', text):
        if re.match(r'^(GATEWAY|AUTH|PROFILE|FRIEND|POST|NOTIFICATION|CONCERT|ACTOR|RESOURCE|MCR)_', m):
            names.add(m)
print('missing:', sorted(n for n in names if n not in doc_names))
EOF
```

Result (superseded by the authoritative extraction below): **50 config files, 311 names, 0 missing.** Family counts:
`ACTOR 2, AUTH 73, CONCERT 61, FRIEND 28, GATEWAY 55, MCR 1, NOTIFICATION 14,
POST 61, PROFILE 14, RESOURCE 2`. The single `MCR_*` match is
`MCR_ENROLLMENT_SIGNUP_V1`, an enrollment-policy identifier in
`auth-service/src/enrollment_policy/store.rs`, not an environment variable; the
runbook states this explicitly.

Compose interpolation names were cross-checked separately:

```sh
grep -oE '\$\{?(MCR_[A-Z0-9_]+)' infra/mcr/compose.yml | sort -u
```

Result: 15 names (`MCR_ENV_FILE`, `MCR_DB_PASSWORD`, `MCR_REDIS_PASSWORD`,
`MCR_STACK_IMAGE`, `MCR_BUILD_JOBS`, and ten host-port overrides), **0 missing**
from the document.

### Startup-critical names verified present in the document

| Group | Names | Status |
| --- | --- | --- |
| Workload TLS | 7 prefixes x `{CERT,KEY,CA}_PATH` = 21 | present, section 4.1 |
| Service credentials | 7 `<PREFIX>_SERVICE_CREDENTIAL` + 7 optional `_PREVIOUS` | present, sections 3.2 and 4.1 |
| Databases | 7 `<PREFIX>_DATABASE_URL`, `POST_REDIS_URL` | present, section 3.2 |
| JWT | `AUTH_JWT_PRIVATE_KEY_PEM(_B64)`, `AUTH_JWT_KEY_ID`, `AUTH_ISSUER`, `AUTH_ACCESS_AUDIENCE`, `GATEWAY_JWT_ISSUER`, `GATEWAY_JWT_AUDIENCE`, `GATEWAY_JWT_JWKS_JSON` | present |
| Delegation/evidence | 4 names | present, section 3.2 |
| Gateway secrets | admission HMAC, search cursor HMAC, cookie domain/secure/max-age, CORS, trusted proxies | present, sections 3.2 and 4.2 |
| Mail | mode, endpoint, bearer, reset base, enrollment verification URL | present, section 5.1 |
| OAuth provider inputs | Google 7, Apple 6 + one of two secret forms, X 5 | present, section 5.2 |
| Retention purge actors | Profile, Post, Notification | present, section 5.7 |
| Concert release | `CONCERT_GRPC_ADDR` + 6 file settings | present, section 5.5 |
| Feature flags | all `CONCERT_*` feature/worker settings | present, Appendix A |

### Code facts that materially shaped the document

- `platform::config::resolve_tls_material` requires all three TLS files in
  production and rejects partial sets in every mode; no plaintext fallback.
- `build_caller_registry` requires the Gateway credential for every receiving
  service in production; missing credentials fail startup.
- Gateway production validation requires HTTPS CORS origins, a non-default
  search-cursor HMAC key, `GATEWAY_REFRESH_COOKIE_SECURE=true` and a non-empty
  cookie domain.
- Auth production rejects `AUTH_PASSWORD_RESET_MAILER_MODE=logging` and requires
  the HTTP mailer endpoint plus bearer token.
- `build_oauth_provider` constructs the HTTP provider client in production, so
  Google/Apple/X provider configuration is a startup requirement even though
  public OAuth is off. This is recorded as an absent production input.
- `GATEWAY_OAUTH_*` has no match in current Gateway source; those planning-era
  settings are excluded and called out as not part of this deployment.
- Gateway source defaults differ from Compose for Notification/Post gRPC ports,
  so the document requires explicit endpoints.
- Shutdown: SIGTERM requests drain; `platform::shutdown::supervise` allows 30 s.
  Docker's default 10 s stop grace would kill first, so the runbook uses
  `-t 40`.

## Validation commands and exits

| Command | Exit | Result |
| --- | --- | --- |
| `MCR_ENV_FILE=/tmp/mcr-doc-env MCR_DB_PASSWORD=x MCR_REDIS_PASSWORD=x docker compose -p mcr-doc -f infra/mcr/compose.yml config --services` | 0 | 10 services: postgres, redis, migrate + 7 |
| same command with `MCR_DB_PASSWORD` unset | 1 | `required variable MCR_DB_PASSWORD is missing a value` (named refusal) |
| `bash -n` on all 20 fenced `sh` blocks | 0 | no syntax errors |
| variable cross-check above | 0 | 311 source names, 0 missing (superseded) |
| Compose interpolation cross-check | 0 | 15 names, 0 missing |
| secret scan (base64 >= 40 chars, hex >= 32 chars) | 0 | no matches |
| forbidden-claim scan (`QA08 PASS`, `production ready`, `device PASS`, ...) | 0 | no matches |
| `git diff --check` | 0 | clean |

The compose daemon was not started. `docker compose config --services` and the
missing-variable probe are client-side validations and did not require a daemon.
No container was built or run.

## Dry read-through checklist

| # | Check | Result | Basis |
| --- | --- | --- | --- |
| 1 | Any operator can find the topology, ports and service names | PASS (read-through) | Section 1 matches `compose.yml`; `config --services` lists all 10 |
| 2 | Every secret is named without a value | PASS | Section 3 lists names only; secret scan clean |
| 3 | Each section names its exact command | PASS | 20 shell blocks; all parse with `bash -n` |
| 4 | Missing secret fails with a named error | PASS | exact Docker interpolation error captured above |
| 5 | Migration apply/verify is named per service | PASS | `migrate-all.sh` plus per-service `--migrate`/`--verify-migrations` commands |
| 6 | Backup/restore procedure is complete | PASS (procedure) / **NOT_RUN** (drill) | section 8 commands; no backup or restore executed |
| 7 | Restart order and graceful shutdown are explicit | PASS | section 9, reverse stop, `-t 40`, 30 s drain |
| 8 | Readiness is not confused with liveness | PASS | section 10 lists required checks per service |
| 9 | Public origin/CORS/cookie requirements are explicit | PASS | section 4.2 |
| 10 | Operator roles are described without inventing authority | PASS | section 5.6; role application marked NOT_RUN |
| 11 | Retention and `Asia/Seoul` end-of-day policy are explicit | PASS | section 5.7; venue policy numbers marked operator input |
| 12 | Known gaps are explicit and no false claim is made | PASS | section 12; QA08/capacity/device/release all disclaimed |

## Limitations

- The Runbook's staging start was not executed end to end: Docker is unavailable
  on this host. The image build and container readiness ordering remain the
  highest residual risk.
- No backup was taken and no restore was drilled.
- Production values (TLS material, provider registration, mail endpoint, venue
  policy, approved legal text, operator assignment application) are absent.
- The document does not claim production capacity, device validation or QA08.

## Cleanup

No processes, containers, volumes, databases or temp worktrees were created.
`/tmp/mcr-doc-env`, `/tmp/mcr-runbook-block-*.sh` and `/tmp/loc.json` are
disposable local files; none is evidence.

## First correction round after runbook verification (counts superseded)

Merged `origin/master` (127c7406f, BE38 0076) as `06ef5f8b8` and corrected
five reported errors in `d45f43313` (`docs(ops): correct runbook environment and
OAuth surfaces`). The branch now contains the original doc commit, the merge
commit and the correction commit.

| Reported issue | Source fact | Fix |
| --- | --- | --- |
| `AUTH_PASSWORD_QUEUE` does not exist | The real settings are `AUTH_PASSWORD_WORKERS`, `AUTH_PASSWORD_LOGIN_QUEUE`, `AUTH_PASSWORD_SIGNUP_WORKERS`, `AUTH_PASSWORD_SIGNUP_QUEUE`, `AUTH_PASSWORD_RESET_WORKERS`, `AUTH_PASSWORD_RESET_QUEUE` (auth config lines 1057-1067) | Removed the nonexistent name; the six real names remain in Appendix A |
| Wrong Post read key names | Actual pairs: `CONCERT_POST_READ_SIGNING_SEED` (Concert signs) + `CONCERT_POST_READ_VERIFYING_KEYS` (Post verifies); `POST_CONCERT_ROOT_READ_SIGNING_SEED` (Post signs) + `POST_CONCERT_ROOT_READ_VERIFYING_KEYS` (Concert verifies), all in `platform/src/actor_delegation/concert_read.rs:6-7,97-98` | Documented both real pairs in section 5.3 and the paragraph after it |
| OAuth section inverted | `browser_auth.rs:23-24` merges `oauth_routes` when controlled transport exists; `config/mod.rs:299-302` reads the four `GATEWAY_OAUTH_*` names; unknown provider is 400, known-but-disabled is 404 | Rewrote section 5.2 and the known-gaps row |
| Compose is not uniformly parity | `infra/mcr/compose.yml:225` sets `CONCERT_RUNTIME_MODE: production`; six other services are parity | Section 1 now states the service-specific modes |
| Appendix omitted OAuth names | `GATEWAY_OAUTH_ENABLED_PROVIDERS`, `GATEWAY_OAUTH_CONTROLLED_PROVIDERS`, `GATEWAY_OAUTH_HANDOFF_HMAC_KEY`, `GATEWAY_OAUTH_BROWSER_ORIGIN` | Added to Appendix A |

One requested replacement name, `CONCERT_POST_ROOT_READ_SIGNING_SEED`, does not
exist at merged HEAD or master. The source name is
`CONCERT_POST_READ_SIGNING_SEED`; the `ROOT_READ` pair is prefixed
`POST_CONCERT_ROOT_READ_*`. The runbook uses the source names and documents
both pairs, rather than introducing a second nonexistent variable.

Corrected runbook line references at the correction commit:

- service-specific runtime modes: line 38
- OAuth surface and four names: lines 262-290
- real Post read key pairs: lines 317-326
- OAuth known-gap row: line 663
- obsolete `AUTH_PASSWORD_QUEUE` removed; real worker names: line 736
- OAuth appendix names: line 752

Re-run results:

| Check | Result |
| --- | --- |
| `bash -n` on all fenced `sh` blocks | 20 blocks, exit 0 |
| Env cross-check against merged HEAD (config dirs + platform/identity files) | 315 names, 0 missing (superseded by the authoritative extraction below) |
| Compose interpolation names | 15 names, 0 missing |
| Secret scan | 0 base64/hex matches |
| Forbidden-claim scan | 0 matches; QA08 remains DEFERRED_BY_USER, no CI-green claim |
| Obsolete standalone names | 0 occurrences of `AUTH_PASSWORD_QUEUE`, `POST_READ_SIGNING_SEED`, `POST_READ_VERIFYING_KEYS` |
| `git diff --check` | exit 0 |

## Third correction round — OAuth order and superseded env count

Final correction commit: `243a1f2d1` (`docs(ops): finish the runbook environment and OAuth accuracy`).

Three residual defects were corrected:

1. **OAuth ordering and messages.** The runbook quoted a nonexistent
   `OAuth provider is invalid` message. Source facts:
   - `browser_auth.rs:23` merges `oauth_routes` only when controlled transport
     exists; with the default policy the whole router is absent and every OAuth
     path is 404.
   - `authorize` parses the provider first (`oauth_routes.rs:82-83`); unknown
     provider is 400 with the exact message `Unsupported identity provider`
     (`unit/http_auth_routes.rs:392`).
   - `callback` checks transport first (`oauth_routes.rs:129`), then parses
     (`oauth_routes.rs:140`), so an unknown provider without transport is 404.
     Apple form-post handlers also check transport first (`form_post.rs:31,83`).
   - Default-policy tests: `oauth_form_post_browser.rs:89` (404, Auth not called)
     and `unit/http_auth_routes.rs:400-410` (404 for google/apple/x).
2. **Nineteen missing runtime settings.** Added to section 5.8 with consumer and
   default, plus the three `PROFILE_SELF_*` provider endpoints found by the
   re-extraction. Two verification-note names are abbreviations, not source
   names: `CONCERT_CHILD_TIMEOUT_MS` -> `CONCERT_CHILD_RECOVERY_CHILD_TIMEOUT_MS`;
   `CONCERT_BATCH_BUDGET_MS` -> `CONCERT_CHILD_RECOVERY_BATCH_BUDGET_MS`.
3. **Credential rotation.** Section 4.1 lists all seven current and seven
   `_PREVIOUS` names, their actual readers, and the enforced semantics
   (current required, previous optional, previous-without-current fails,
   current != previous, both accepted during rotation, duplicate tokens rejected
   in Participant mode). `NOTIFICATION_SERVICE_CREDENTIAL_PREVIOUS` is not read
   by any current inbound registry.

### Authoritative env extraction

Method: for every tracked non-test backend Rust file (paths without `/tests/`),
collect the literal name in env accessor calls (`env_or`, `optional_env`,
`std::env::var`, `read`, `required`, `value`, `count`), every `*_ENV`
constant, and every `configured_destination` prefix expanded to
`_ENDPOINT`/`_BEARER_TOKEN`. Restrict to known env prefixes. Exclude the bare
destination prefixes and 33 test/fixture-only names (for example
`MCR_FEED_NATIVE_*`, `MCR_SELF_PROFILE_*`, `CONCERT_TEST_*`,
`POST_CONCERT_TEST_*`).

Result:

| Measure | Value |
| --- | ---: |
| Raw matched names | 332 |
| Excluded (2 bare prefixes + 33 test/fixture names) | 35 |
| Runtime literal names | **297** |
| Runtime names present in the runbook | **297** |
| Missing | **0** |

Dynamic families excluded from the literal count and documented as rules:
21 TLS paths, 14 current/previous workload credentials, four authority
settlement endpoints. The runbook states this scope explicitly; it no longer
claims "all env names" without qualification.

Re-run checks after the corrections: 20 shell blocks parsed; extraction 332 raw
/ 35 excluded / 297 runtime / 0 missing; compose 15 names / 0 missing; 0
secret-like strings; no forbidden claims; `git diff --check` exit 0.

## Fourth correction round — credential readers and superseded count

Final correction commit: `ffbc5f70b`
(`docs(ops): correct the credential table and scope the env count`).

- Section 4.1 now separates **accepted** from **required** credentials exactly as
  `build_caller_registry(is_production, accepted, required)` does
  (`platform/src/config.rs:264-289`): accepted callers are read if configured;
  only the required subset fails production startup when absent. Profile accepts
  five and requires only Gateway.
- Base and feature-gated registries are listed separately. Previous-value
  readers were corrected: `NOTIFICATION_SERVICE_CREDENTIAL_PREVIOUS` is read by
  Concert's notification target (`notification_target_runtime.rs:53-57,90-94`)
  and by Auth/Friend owner authority; `POST_SERVICE_CREDENTIAL_PREVIOUS` is read
  by Concert's feed callback (`feed_runtime.rs:96-104,134-138`) and owner
  authority; `PROFILE_SERVICE_CREDENTIAL_PREVIOUS` is read by Auth/Friend owner
  authority and Concert Participant.
- The 332/35/297 arithmetic was removed. Appendix A now embeds a runnable
  Python recipe scoped to `backend/services/**/src` and
  `backend/crates/**/src`, excluding `/tests/`, truncating at
  `#[cfg(test)]`, excluding build tools/tools paths and test-fixture names.
  Current output: `files=1081 raw=281 excluded=35 runtime=246 missing=0`.
- This 246 is lower than the verifier's 324 because the recipe counts only
  names used in env accessors/constants, not every uppercase literal (internal
  enums, error codes and event names are excluded). Dynamic TLS/credential/
  settlement-endpoint families are documented as rules and excluded from 246.

Post-correction checks: 21 shell blocks parsed; recipe reproduced 281/35/246/0;
secret scan 0; forbidden-claim scan 0; `git diff --check` exit 0.

## Fifth correction round — recipe stripping and production TLS claim

Final correction commit: `0c265916a`
(`docs(ops): correct the env recipe and production-mode claim`).

- The Appendix A recipe no longer truncates at the first `#[cfg(test)]` marker.
  It removes only the brace-matched test module/item, preserving production code
  later in the same file (for example `AppConfig::load()` after the inline test
  module in `api-gateway/src/config/mod.rs`).
- Re-run of the corrected recipe:
  `files=1081 raw=332 excluded=35 runtime=297 missing=0`. The 35 excluded names
  are all test/fixture-only; there are zero bare-prefix exclusions because
  destination prefixes never enter the candidate set. The prior 246 figure is
  superseded.
- The production TLS bullet now states the service-specific truth: Gateway,
  Auth, Profile, Friend, Post and Notification resolve TLS on their serving
  paths; health-only Concert validates only `CONCERT_METRICS_ADDR`,
  `CONCERT_RUNTIME_MODE` and `CONCERT_MIGRATION_MODE`; Concert resolves TLS
  only on an enabled bootstrap/World/Participant/feed path.

## Sixth correction round — health-only Concert validation

Final correction commit: `847c75b6b`
(`docs(ops): state the health-only Concert validation set precisely`).

The TLS bullet now states that health-only Concert still runs
`AppConfig::load()` and validates `CONCERT_DATABASE_URL` (explicit PostgreSQL
URL, host/database path, no server-option overrides, dedicated `mcr_concert`
database), `CONCERT_METRICS_ADDR`, `CONCERT_RUNTIME_MODE` and
`CONCERT_MIGRATION_MODE`; it also loads `CONCERT_ACCOUNT_DELETION_MODE`,
`CONCERT_PUBLIC_FEED_MODE`, `CONCERT_OWNER_MEDIA_MODE` and the eight
`CONCERT_WORLD_*` switches, so invalid modes/partial World settings fail even
when the feature is off. Only TLS material is deferred to enabled
bootstrap/World/Participant/feed serving paths.

Post-correction checks: 21 shell blocks parsed; recipe reproduced
`files=1081 raw=332 excluded=35 runtime=297 missing=0`; secret scan 0;
forbidden-claim scan 0; `git diff --check` exit 0.
