# Browser refresh qualification for FE09

Status: ACCEPTED for BE03-BROWSER-REFRESH at producer
`0a8c422645458c9397af9289e15089ee3f9cb0ad`. This qualifies the backend
cookie/rotation input for original FE09. It does not claim full BE03, full FE09,
or whole-master acceptance.

The existing producer already implements refresh and atomic rotation. This task
adds reproducible browser qualification and an exact contract, with no redundant
production refresh module and no frontend source changes. The original native
HTTP receipt in `docs/mcr/evidence/browser-auth-integration.md` explicitly left
real browser cookies and multiple tabs NOT_RUN. This closes that specific gap.

The public contract is
[`browser-refresh-contract.md`](../../../backend/docs/api/browser-refresh-contract.md).
The initial receipt qualified `7c2f3af7b9fd7bab2ed2d863cc2262527cc8553e`.
The closing run rebuilt all six services from the pin above, including the newer
Gateway outer `RetainedScopeLayer` and Profile migration 0009, and reran the
complete suite using the byte-identical published harness. Its source and script
hashes match the final receipt exactly.

## Actual environment and assertions

The runner uses installed Chrome in a new Playwright context, the actual native
Gateway and Auth service, and generated private stores. Signup, private mail
verification, canonical identity, signing, rotation, and revocation are real.
No Auth response, database result, or clock is mocked. The ephemeral loopback
HTTPS terminator forwards only fixed Auth paths, holds the two race requests
after Chrome attaches cookies, and can drop one actual committed response.
Chrome trusts only its ephemeral certificate's SPKI; system trust is untouched.

The fixture enables the real Secure cookie issuer with a host-only domain. Auth
uses the accepted minimum 60-second access TTL. The suite waits past the actual
signed expiry plus the pinned verifier's 60-second leeway and the 30-second
reuse grace. It never rewrites timestamps or signed credentials to fake expiry.

| Scenario | Proven behavior at the actual public port |
| --- | --- |
| HTTPS issuance | Chrome stores Secure/HttpOnly/Lax host-only cookie; JavaScript cannot read it or Set-Cookie |
| Credential/CSRF denial | Omitted credentials, missing intent, and cross-site form are rejected without Auth snapshot changes |
| Two uncoordinated tabs | Same automatic predecessor cookie; one 200, one 409; no conflict Set-Cookie or extra successor |
| Real expiry and delayed 401 | Old bearer fails after signed expiry/leeway; renewed bearer passes protected authentication; stale GET does not rotate |
| Late predecessor reuse | Only its family ends; an independently logged-in family still renews |
| Native browser locking | Two actual tabs serialize cookie writers; both rotate, with peak upstream overlap one |
| Committed response loss | One application fetch, two Chromium transport attempts, final409, one committed rotation, no adopted cookie |
| Security generation | Legitimate rotation stays stable; late reuse and ancestor logout advance generation |
| Ancestor logout | Live descendant is revoked, actual browser cookie is removed, later refresh fails |

The safe protected read is `GET /api/auth/account`. After successful bearer
extraction and the real Auth lookup it returns 404 because this fixture has no
deletion saga; an expired bearer returns 401. The route is explicitly composed
in `auth_routes::browser_router`, so the 404 is not an unknown-route fallback.

There are 12 reported phases, including the real-expiry wait. Persistent browser
storage stays empty. Only the adversarial late-replay context is seeded with the
captured predecessor; ordinary issuance, racing tabs, and locked writers use
Chrome's automatic cookie jar.

A single fetch can cause a lower-level Chromium retry after pre-header EOF. This
was observed and asserted, not treated as an application retry or successful
renewal. The second request receives 409; the database rotates only once. The
application must fail closed and must not add a refresh or mutation replay.

The task-local readiness validator checks the exact current probe set, including
the required Auth/Friend `concert-owner-authority` probes, required flags, and
readiness/degradation/status consistency. The retained shared parser predates
those probes. No existing helper or service health policy is weakened.

## Reproduce and provenance

Assignment base: `c4ad561cd813f0f7f1e14f21cb2ee90592e09eea`.
Harness/contract publication: `066f2e94` (delivered through `c8fd98b1`).
Producer binaries: `0a8c422645458c9397af9289e15089ee3f9cb0ad`.

Build the six service bins from that source using Rust 1.88.0, locked/offline
dependencies, two jobs, debug information disabled, incremental compilation
disabled, and an explicitly allocated existing Cargo target. The actual closing
build exited 0 in 36.02 seconds. Copy/hash all six binaries outside the target
and release it before starting PostgreSQL. Do not attach arbitrary running
services or create another build target.

The runner installs nothing. Supply existing Chrome, Playwright, and Node:

```sh
python3 -B backend/scripts/mcr/browser_refresh.py \
  --bin-dir /absolute/owned/frozen/bin \
  --report /absolute/owned/new-browser-refresh-report.json \
  --chrome '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome' \
  --playwright /absolute/existing/node_modules/@playwright/test \
  --node /absolute/existing/node
```

Observed tooling: Node 26.7.0, Playwright 1.61.1, installed Chrome
156.0.8078.80, and PostgreSQL 16.14. The script installs none of them.

It reuses the guarded DevStack/migration helpers, creates a marker/inode-owned
PostgreSQL cluster and private mail sink, validates migrations before and after
physical PostgreSQL restart, and checks all six native services for readiness.
All database access is confined to that owned cluster. Cookie/token values,
passwords, mail payloads, browser profiles, and private store URLs are never
included in the published receipt. Failure termination is limited to the owned
process group; normal browser shutdown and native process/store cleanup pass.

Final Python AST, embedded JavaScript syntax, explicit-module CLI, and diff
checks pass. The complete native/browser command exits 0. All six Cargo artifact
records point to this checkout and are `fresh: false`; frozen executable mtimes
precede Cargo completion, copy hashes remain unchanged, and runtime hashes equal
the frozen manifest. No material artifact impact was found after a reported
shared-target access conflict. No target was accessed after release.

## Frozen execution receipt

```json
{
  "result": "PASS",
  "producer_revision": "0a8c422645458c9397af9289e15089ee3f9cb0ad",
  "started_at_utc": "2026-11-04T01:07:37.513122+00:00",
  "finished_at_utc": "2026-11-04T01:09:51.317777+00:00",
  "native_receipt_sha256": "e46cac5739809af1ead93d707efa699b2790a180d85346e771d82911fc439957",
  "script_sha256": "748c35416003b6ea05a5db88f67a365711bfde04073442c22586929e3e14d46a",
  "source_manifest_sha256": "7f2692afdcde4570856f52d2aff6b682861b981696f78d1ee28666f36ec2894f",
  "chrome": "156.0.8078.80",
  "phases": 12,
  "loss_probe": {
    "application_fetch_calls": 1,
    "transport_attempts": 2,
    "outcome": "conflict409",
    "committed_rotations": 1,
    "cookie_adopted": false
  },
  "proxy_observations": {
    "failure": null,
    "refresh_requests": 16,
    "peak_native_refresh_overlap": 1,
    "race_same_cookie": true,
    "conflict_set_cookie": false,
    "cross_site_cookie_absent": true
  },
  "migration_and_restart": "PASS for all six native service stores",
  "frozen_runtime_hash_agreement": "PASS",
  "cleanup": "PASS",
  "secrets_recorded": false,
  "production_deployment": "NOT_RUN",
  "frontend_production_wiring": "NOT_RUN",
  "Gateway_outer_response_layer": "PASS at producer_revision"
}
```

| Native service | SHA-256 | Migration count |
| --- | --- | --- |
| api-gateway | `67d07d1d212dfcfef4390011e46dffaca973b6a646746a3a162d6167831c7221` | 17 |
| auth-service | `3ddbc1caa04f5f528772bdf22e68d74509a70a374f704afb46c09bbcdd5a657c` | 17 |
| profile-service | `5e0428cd6684ff5b5afe9d32e8eda6e664d9baa51a89200aa65c26b2cf697b71` | 9 |
| friend-service | `36a4e9697b698c542947a5fc77a36735ed936d1feaeee5e9d8063362b90e7a1a` | 17 |
| post-service | `75b44e97f64f611776bb5a429f6249f61766ba2ceb6b7cdc5e5c287e042d0c47` | 31 |
| notification-service | `f8c53f7df0a571ab8a53bbd16fcc220c51e5d4d196d00def19e12a8a1e9623b3` | 4 |

The complete private receipt is retained as `current/browser-report.json`,
alongside `current/artifacts.json`, Cargo JSON, frozen copies, and
`current/resource-provenance.json` in the task-owned artifact directory. The
publishing handoff supplies its absolute location. The compact receipt above is
the durable repository evidence; no account identifiers or credentials appear.

## Remaining boundaries

Production deployment, production TLS/domain topology, Safari, OAuth/provider
callbacks, and expiry after the default 14-day refresh-cookie/signed-refresh
lifetime are NOT_RUN. The local host-only cookie does not certify the configured
production Domain, which production validation requires to be nonempty.

Native Web Locks over actual requests qualify the browser primitive and producer.
They do not certify the frontend cookie-policy wrapper, BroadcastChannel policy,
or production session controller. FE09's owner must enable the qualified producer
in production composition and prove concurrent 401 single-flight, delayed
old-token 401 behavior, logout races, private authority cleanup, and mutation
non-replay through its actual controller. Those remain frontend-owned checks;
this backend task does not relabel synthetic controller fixtures as live proof.
