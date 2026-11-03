# BE03 browser authentication: bounded email handoff

Date: 2026-11-03. Task: BE03. Delivery status: **IN_PROGRESS — committed worker handoff; integrator review and acceptance pending**. The isolated email qualification passed. This does not mark all C03 providers, the frontend/browser journey, BE03 in its full specification scope, or the project ACCEPTED.

## Revision and ownership

- Worker branch: `codex/mcr-be-BE03`, isolated checkout `mcr-worktrees/be-auth`.
- Base: `69a5624cd153a216f2398cebabf062400dce23eb`.
- Implementation: `397e1d61e11677c90766163473016066d1088875`, directly descending from that base. This documentation is a separate follow-up commit. Only the integrator publishes reviewed work to `julymeltdown/mcr:master`; this worker has not pushed.
- Frozen input: C03 contract version `0.1.0`. No contract registry, schema, OpenAPI, proto, manifest, lockfile, frontend, migration, shared infrastructure, or `docs/source/` changes.
- Upstream foundation: port-rust `704f42149d5e8e35e1d578676d8ae2dac4f0f9c2`, imported before this task. Existing `boilerplate-grpc-rust/services/api-gateway/src/adapters/in/http/…`, Gateway configuration, and `boilerplate-grpc-rust/services/auth-service/src/config/mod.rs` map to the corresponding `backend/services/…` destinations. Existing tests map the same way. The email handlers, use cases, cookie functions, identity, persistence, and outbox remain the audited headless foundation; the browser policy and qualification harness are newly authored. See [source audit](source-audit.md) and its inventory for foundation provenance.
- The upstream pin above is taken from the existing source audit. `git cat-file -t` for that upstream SHA returned 128 in this worker object database; BE03 does not certify upstream history retention. The integrator owns that foundation-level provenance check. This task reuses the existing MCR files rather than importing more upstream code.
- The gateway HTTP router composition was explicitly delegated for BE03. Other shared ownership stays with the integrator. Existing `dev_stack.py`, `runtime_smoke.py`, and `migration_smoke.py` were read and invoked without edits or copied implementation.
- Dependencies: existing Axum/Tower HTTP, `chrono`, `serde_json`, `uuid`, `http`, and `jsonwebtoken`; Python standard library plus the unchanged owned-stack helpers. No dependency additions or resolution changes.

## Delivered behavior

The actual executable Gateway composes these qualified routes through the existing owning Auth/verification ports:

| Method | Public path |
| --- | --- |
| GET | `/api/auth/capabilities` |
| POST | `/api/auth/email/signup`, `/api/auth/email/verify`, `/api/auth/email/resend` |
| POST | `/api/auth/password/reset/request`, `/api/auth/password/reset/confirm` |
| POST | `/api/auth/login`, `/api/auth/refresh`, `/api/auth/logout` |
| GET, DELETE | `/api/auth/account` |
| GET | `/api/auth/account/deletion` |

Capabilities declares EMAIL enabled and GOOGLE/APPLE/X disabled with `HANDOFF_NOT_QUALIFIED`. It reports configured support, not a live dependency probe. The executable does not compose legacy bare `/auth/*`, email availability, OAuth authorize/callback/link, or parity/shadow dispatch. Unavailable surfaces are protected responses and do not dispatch; unsafe requests without browser headers are rejected before route lookup.

**Every public Gateway mutation** (methods other than GET, HEAD, OPTIONS), including non-Auth routes and admin routes, requires exactly one `Origin` equal to an explicit `GATEWAY_CORS_ALLOWED_ORIGINS` entry and exactly one `X-MCR-Intent: browser`. Missing, null, wildcard, suffix, trailing-slash, duplicate, and otherwise unequal origins are rejected; intent is exact and case sensitive. Configuration rejects wildcard origins, credentials, paths, queries, fragments, and serialized trailing slashes in all runtime modes; production HTTPS policy remains enforced. CORS and the guard share the configured list. Internal service traffic uses authenticated gRPC. Consumers of other public mutation routes must send the two browser headers too.

The boundary runs outside body parsing, quota/admission, credential/cookie handling, mutation journaling, telemetry, and downstream calls. Both executable transport errors and direct composed-router use receive the policy. All Auth responses, including capabilities, denial, preflight, malformed input, unavailable routes, and downstream errors, receive `Cache-Control: no-store`, `Pragma: no-cache`, `X-Content-Type-Options: nosniff`, `Referrer-Policy: no-referrer`, and bounded correlation headers. Guard denials use the frozen Origin error envelope. Capabilities failures use Core reasons and preserve Retry-After. CORS allows intent and exposes Retry-After.

The new path maps to inherited admission and high-risk session policies. Request-journal classification also maps Auth to its reserved authentication class, preventing durable recording of credential bodies and cookie rotation. Account deletion still requires a canonical live session; legacy tokens without a session ID and logged-out sessions cannot delete.

Login/refresh preserve camelCase, canonical Auth UUIDs, in-memory access-token delivery, an empty body `refreshToken`, and HttpOnly refresh cookies with inherited Path/Domain/SameSite/Secure configuration. Cookie clear attributes match issuance. Production Secure requirements were not weakened. Only the unchanged local launcher explicitly opts into plain loopback HTTP. Passwords retain exact UTF-8 bytes; no normalization was added. Reset URLs must be absolute configured HTTP(S) targets without userinfo/query/fragment/backslash; production still requires HTTPS. Request-supplied return targets cannot override that setting.

The producer now classifies malformed, expired, wrong-type, unknown-key, and invalid-subject decoded credentials as `CredentialRejected` rather than an internal failure. Signing/configuration failures retain their original handling. This fixes the real native malformed-refresh 500 observed during qualification.

## Verification and limitations

All commands below ran from the worker checkout. Rust commands used Rust 1.88.0, `--locked`, two jobs, and an external target. Logs and receipts are local evidence under `/Users/lhs/Desktop/mcr-build/be03/`; they are not committed source. No existing database was used. All six service binaries were launched from that directory's `bin/`; Gateway/Auth were built here and the other four binaries were copied unchanged from the qualified BE02 build. The receipt records all six binary and migration-manifest digests.

| Command (Rust commands prefixed with `CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/be03/target`) | Exit | Evidence/result |
| --- | --- | --- |
| `cargo +1.88.0 build --manifest-path backend/Cargo.toml --locked -j 2 -p api-gateway -p auth-service` | 0 | `build-05.log`; final production code |
| `cargo +1.88.0 test --manifest-path backend/Cargo.toml --locked -j 2 -p api-gateway --test browser_auth_http --test unit` | 0 | `gateway-tests-final.log`; 12 browser/harness tests and 208 inherited tests passed |
| `cargo +1.88.0 test --manifest-path backend/Cargo.toml --locked -j 2 -p api-gateway --test unit config::browser_origin_allowlist` | 0 | `origin-config-test.log`; later additional configuration negative test passed |
| `cargo +1.88.0 test --manifest-path backend/Cargo.toml --locked -j 2 -p auth-service --test unit` | 0 | `auth-unit-final.log`; 124 passed |
| `cargo +1.88.0 test --manifest-path backend/Cargo.toml --locked -j 2 -p auth-service --lib config::tests -- --skip external_migration_mode_rejects_schema_damage_without_owner_privileges --nocapture` | 0 | `auth-config-tests-04.log`; 10 passed, including actual signed expired JWTs and reset-target negatives |
| `python3 -B -m unittest discover -s backend/scripts/mcr -p test_browser_auth_smoke.py` | 0 | `python-tests-final.log`; 9 failure/ownership/privacy/cleanup tests passed |
| `python3 -B backend/scripts/mcr/browser_auth_smoke.py --bin-dir /Users/lhs/Desktop/mcr-build/be03/bin --report /Users/lhs/Desktop/mcr-build/be03/native-03.json` | 0 | 53 real HTTP checks, three local deliveries, all six services healthy, cleanup PASS |
| `python3 -B backend/scripts/mcr/dev_stack.py start --bin-dir /Users/lhs/Desktop/mcr-build/be03/bin --report /Users/lhs/Desktop/mcr-build/be03/cli-stack-01.json --ready-file /Users/lhs/Desktop/mcr-build/be03/cli-stack-01-ready.json --duration-seconds 120` | 0 | Unchanged foreground launcher/control path; healthy six-service stack, cleanup PASS |
| `python3 -B infra/mcr/backend/verify_local_auth.py --ready-file /Users/lhs/Desktop/mcr-build/be03/cli-stack-01-ready.json --report /Users/lhs/Desktop/mcr-build/be03/cli-auth-01.json` | 0 | Seven qualified email requests passed, two real local deliveries |
| `python3 -B backend/scripts/mcr/dev_stack.py stop --ready-file /Users/lhs/Desktop/mcr-build/be03/cli-stack-01-ready.json` | 0 | Owned stop/cleanup PASS |
| `python3 scripts/mcr/check-plan.py` | 0 | `plan-check.log`; planning/source preservation only |
| `git diff --check` | 0 | Whitespace verification |

`native-03.json` owns fresh PostgreSQL 16.14, Redis, six isolated MCR databases, six services, and a private local mail receiver. Migration, verification, PostgreSQL restart persistence, 84 ledger entries, and health checks passed. The harness validates the cluster marker/inode before state proof. Negative Origin/intent signup requests leave exact hashes of nine Auth tables and the mail count unchanged; denied refresh leaves state unchanged and the same cookie then refreshes successfully. Native verification covers real signup/verification/wrong and reused codes, login, neutral unknown resend/reset, refresh rotation, immediate reuse conflict without family destruction, reuse outside grace with family revocation, logout revocation, reset expiry/reuse, cooldown neutrality, password/session invalidation, and exact 8/128-byte Unicode password boundaries with rejected 6/130-byte inputs. Reset links come from actual private outbox delivery and bind the owned application target with a fragment token.

Time-based native refresh/reset tests change deadlines only in the disposable database. Actual JWT clock-expiration rejection is covered by signed-token unit tests; real-time native JWT expiration is NOT_RUN. Account routes are dispatch/session qualified; complete multi-service account erasure is NOT_RUN. Real browser cookie/SameSite behavior, simultaneous tabs, frontend shell/cache lifecycle, multiuser, physical devices, provider callbacks, production TLS/cookie domains, PostgreSQL 17, load, deployment, and release acceptance are NOT_RUN. No live OAuth or external mail was exercised. The Docker-only inherited external-schema damage test could not acquire its fixture because the container socket was unavailable; it remains NOT_RUN. Docker was not started or reconfigured.

The unchanged launcher's `auth_http` descriptor and generic `auth_acceptance`/OAuth notes describe its older inherited seam. They are not a route discovery API or this task's qualification verdict. The new top-level native receipt and actual requests above are the BE03 evidence. `verify_local_auth.py` changed only its route map/docstring and retains its deliberately narrow non-acceptance scope.

Failed receipts are preserved: `native-01.json` (17 observed statuses; malformed refresh returned 500, fixed in producer) and `native-02.json` (44 statuses; harness requested a second reset delivery inside the existing cooldown). The latter changed only the owned test clock/harness; production cooldown was preserved. Earlier route tests caught incorrect Auth request-journal classification; earlier inherited tests required new browser headers/routes. `auth-unit-01.log` caught an obsolete wrong-type token message assertion, now a typed credential-rejection assertion. None of those failures is presented as passing evidence.

Both successful runtime roots were absent after cleanup, and a fresh `ps` check found none of their recorded service/Redis/PostgreSQL PIDs. No cleanup targeted unrelated processes. Receipts omit email, password, body, tokens, cookies, and mail contents; local control descriptors contain an ownership secret and are not publication artifacts.

## Evidence binding

The runtime was exercised while HEAD was the base and the implementation was uncommitted. `native-03.json` therefore honestly records the base revision and hashes of every Gateway/Auth `.rs`/`.sql` source file, frozen contract inputs, the executed script, six binaries, and migration manifests. After the final test-only changes and implementation commit, a fresh comparison found **zero production-source or contract hash drift**. No production source changed after `build-05`/`native-03`. The last additions were tests and documentation; this is not a claim that a different binary was rerun after the commit.

Qualified Gateway SHA-256: `c1fa9838efe79a930725abaa68318cb5e710f2e997f9ed663da1ef3fb4de2af0`. Qualified Auth SHA-256: `518a5cc5229e8577d48be3f40562298917e0f13cc56e625a0eb5d3e6a0a2a9ca`. Smoke script SHA-256: `292dbd7bc36434ddb7ed2c615eb621e663c7b13d243c7649a996d22cf526bb4d`.

| Frozen input | SHA-256 |
| --- | --- |
| `docs/mcr/core-contract.md` | `97a907d3f215701c068bec717a20fdb62d01aa5d2f1d6552571fc098a74edf64` |
| `contracts/core/v1/operations.json` | `2f5b972dbccea09340e1519a1a2c522e3c42f63690fa26b9e58d721e9927ba62` |
| `contracts/core/v1/core.schema.json` | `9ddf3cd3c853ef6a8258a6215c546bd84eccde620ee73b636888d868cfca1c88` |
| `backend/docs/api/mcr-core.openapi.json` | `c8de71d5f3dfc125fd3a5206bc404a76df39367a45fa6c3f67ed6323de91c15d` |

| Local evidence file | SHA-256 |
| --- | --- |
| `native-03.json` | `8152069eaa3fb3d05e73e930732ceafcc60ce5df59bbbe3a10ca22681058fd2c` |
| `native-01.json` | `c943471eb856a7d336549b70a525b7ed33855e3de7deab65c144adb5c0e2b237` |
| `native-02.json` | `5f68b1f77b95574c3a5c67118bf164f4a354eeb5172c92117d8d3be8fc8aa4dc` |
| `cli-stack-01.json` | `cc6a29a2f10b6aab878a3cd15db88d226c123a6607a5646b6b2dd9dddbbbdfa9` |
| `cli-auth-01.json` | `ef5690768f425182a6954de63300eab40d3f548e15a304975c1e27b2fb32fc63` |
| `gateway-tests-final.log` | `441bbd4061a4a82621fc2fb62948161f07ce533c40a00168de02ade6dec18f18` |
| `origin-config-test.log` | `1429891dfef86bc8546abada444e416f36cc649736e748e85b89bfb94fb0e441` |
| `auth-unit-final.log` | `0cb6ba44b636b6a7fca8cb83413e39acaffbe105b63a42399061ba2451571396` |
| `auth-config-tests-04.log` | `261b887f67ec0cfd291b8eb1a8bd110560f702ffe9738cb7e83351f8282e9cb1` |
| `python-tests-final.log` | `b1e2df9be5b3e954c2d2d25efcf39cb02fe4e10735e67c33a5bb9488485289d7` |
| `build-05.log` | `9b3908514fab4445ae8112ff7ba35247a694353ed844475e5eda6f06562ddcad` |
| `plan-check.log` | `88606f33d640a29811a4cbdc9748fc0f1e16225b81ad7e697deaf3b20c9b86e1` |

Unchanged helper hashes: `dev_stack.py` = `dbfb13b6b9b8beef70c02b2c814590ecdbfcfa41dc8c2d3ea4597efe47672aa0`; `runtime_smoke.py` = `da2ac13ca1502e80a2528c982bf28b61c06707b4018c2a436e7f8bf41512e7a8`; `migration_smoke.py` = `a46f2dc3a6dcab9310230cd75997dfcd86571c88fd066a973a8a0c3b1a8c6eb8`.

## Implementation changed-path list

- `backend/scripts/mcr/browser_auth_smoke.py`
- `backend/scripts/mcr/test_browser_auth_smoke.py`
- `backend/services/api-gateway/src/adapters/in/http/auth_routes.rs`
- `backend/services/api-gateway/src/adapters/in/http/browser_auth.rs`
- `backend/services/api-gateway/src/adapters/in/http/mod.rs`
- `backend/services/api-gateway/src/adapters/in/http/refresh_routes.rs`
- `backend/services/api-gateway/src/adapters/in/http/request_journal.rs`
- `backend/services/api-gateway/src/adapters/in/http/server.rs`
- `backend/services/api-gateway/src/adapters/in/http/verification_routes.rs`
- `backend/services/api-gateway/src/config/mod.rs`
- `backend/services/api-gateway/tests/browser_auth_http.rs`
- `backend/services/api-gateway/tests/unit/admission.rs`
- `backend/services/api-gateway/tests/unit/config.rs`
- `backend/services/api-gateway/tests/unit/high_risk_sessions.rs`
- `backend/services/api-gateway/tests/unit/http_notification_privacy.rs`
- `backend/services/api-gateway/tests/unit/http_support.rs`
- `backend/services/api-gateway/tests/unit/http_support/auth_client.rs`
- `backend/services/api-gateway/tests/unit/oauth_form_post.rs`
- `backend/services/auth-service/src/config/mod.rs`
- `backend/services/auth-service/tests/unit/config_behavior.rs`
- `infra/mcr/backend/verify_local_auth.py`

The only documentation addition is `docs/mcr/backend/browser-auth.md`. Review should check this complete path list against the explicit assignment, inspect the global mutation-header impact and credential-body journal exclusion, and distinguish real native email proof from unrun browser/provider acceptance before integration.
