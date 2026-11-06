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

Result: **50 config files, 311 names, 0 missing.** Family counts:
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
| variable cross-check above | 0 | 311 source names, 0 missing |
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
