# Browser refresh qualification for FE09

Status: ACCEPTED for the exact qualified producer pin below; latest Gateway
transport rerun NOT_RUN. Backend owner: BE03-BROWSER-REFRESH. The existing producer
already implements refresh and atomic rotation. This task adds fresh browser
qualification rather than another refresh implementation. Production frontend
activation remains with the FE09 owner.

The public contract is
[`browser-refresh-contract.md`](../../../backend/docs/api/browser-refresh-contract.md).
The original native-HTTP receipt in `docs/mcr/evidence/browser-auth-integration.md`
explicitly left real browser cookie engines and multiple tabs NOT_RUN. The new
harness addresses that specific missing evidence using unchanged owning services.

## Reproduce

Build the six current native service binaries from one recorded source revision
using an explicitly allocated existing Cargo target, Rust 1.88.0, locked/offline
dependencies, two jobs, no incremental compilation, and debug information disabled.
Freeze/copy/hash `api-gateway`, `auth-service`, `profile-service`, `friend-service`,
`post-service`, and `notification-service` outside the shared target, and release
the target before starting PostgreSQL. Do not reuse arbitrary running services.

Run from the task checkout with a new absolute report filename:

```sh
python3 -B backend/scripts/mcr/browser_refresh.py \
  --bin-dir /absolute/owned/frozen/bin \
  --report /absolute/owned/new-browser-refresh-report.json \
  --chrome '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome' \
  --playwright /absolute/existing/node_modules/@playwright/test \
  --node /absolute/existing/node
```

The script installs nothing. It reuses the repository's guarded DevStack and
migration helpers, creates a marker/inode-owned PostgreSQL cluster and private
mail sink, and launches native services only against generated fixture stores.
Auth mail, identity, signing, token rotation, and family revocation are real.

The task-local readiness validator checks the exact current probe set, including
the new required Auth/Friend `concert-owner-authority` probe, its required flags,
and readiness/degradation/status consistency. The retained shared parser predates
that probe; the new harness does not omit it or weaken service health policy.

The fixture uses the accepted minimum 60-second access TTL and waits past actual
signed expiry plus the pinned verifier's 60-second leeway. It never rewrites the
database clock or signed credential to simulate expiration.

The owned loopback HTTPS terminator forwards only fixed Auth paths to the actual
Gateway. Chrome trusts only its ephemeral certificate's SPKI; no system trust
store changes or broad certificate-error bypass are used. The test enables the
real Secure cookie issuer and uses a host-only fixture domain. It holds two
requests only after Chrome has attached its automatic cookie, and can drop a
real committed rotation response. It never synthesizes an Auth response.

Private credentials and browser profiles are temporary and never included in
receipts. Reports contain source hashes, binary hashes, safe phase labels,
migration/readiness results, and owned cleanup results. Node/Chrome use an
ephemeral profile; failure termination is limited to the owned process group.

## Coverage

| Scenario | Assertion at the actual public port |
| --- | --- |
| HTTPS issuance | Browser stores Secure/HttpOnly/Lax host-only cookie; JavaScript cannot read it |
| Credential/CSRF denial | Omitted credentials, missing intent, and cross-site form are rejected without Auth snapshot changes |
| Two uncoordinated tabs | Same automatic predecessor cookie; one 200, one 409; no conflict Set-Cookie or extra successor |
| Real expiry and delayed 401 | Old bearer fails after signed expiry/leeway; fresh cookie renewal works; stale GET does not rotate |
| Late predecessor reuse | Only its family ends; independently logged-in family still renews |
| Native browser locking | Two actual tabs serialize cookie writers and each rotates successfully |
| Committed response loss | One application fetch cannot adopt credentials or rotate twice after the response is lost |
| Security generation | Legitimate rotation stays stable; late reuse and ancestor logout advance generation |
| Ancestor logout | Live descendant is revoked, actual browser cookie is removed, later refresh fails |

## Evidence and limits

Python AST and embedded JavaScript syntax checks pass. An isolated infrastructure
probe with installed Chrome 156.0.8078.80 confirms the pinned HTTPS origin is a
secure context, native Web Locks are present, persistent storage is empty, and
owned browser/listener cleanup succeeds. That probe does not exercise Auth.

The first complete native/browser run passed at producer revision
`7c2f3af7b9fd7bab2ed2d863cc2262527cc8553e`, with all 12 reported phases
(including the real-expiry wait) and cleanup PASS. A single application fetch after
pre-header response loss produced two Chromium transport attempts, a final 409,
exactly one committed rotation, and no adopted cookie. The backend prevented a
second rotation.

The native receipt qualifies the public Auth producer at that exact revision,
including Auth/Friend migration 0017. All six binaries were freshly built from
one source pin. No new production refresh implementation or frontend source was
needed. The original cookie/tab-race criterion is qualified at this pin; this is
not a claim of full BE03, full FE09, or whole-master acceptance.

The final script removes the machine-specific Playwright module default and
requires `--playwright`. Reconstructing precisely those two CLI-only edits
reproduces the native-run script SHA-256; the browser program is byte-identical.
Final Python AST, embedded Node syntax, and explicit-argument CLI checks pass.
The final CLI packaging itself was not rerun through the complete native suite.

An intervening product-delivery change adds `RetainedScopeLayer` in Gateway
`server.rs`. Static inspection confirms the layer delegates response frames
when no `ResponseScopeToken` is attached; public Auth handlers do not attach one.
A fresh native run through that newer outer HTTP layer remains NOT_RUN and is
queued for an existing build slot. Do not relabel the binary receipt as proof of
that newer transport. This does not change the documented Auth wire contract or
turn unrelated producer/root integration work into an FE09 contract barrier. Production deployment, production TLS/domain topology, Safari,
OAuth/provider callbacks, refresh-cookie/signed-refresh expiration after the default
14-day lifetime, and frontend production wiring are NOT_RUN. Native
Web Locks over real refresh requests do not certify the frontend cookie-policy
wrapper or its session/token-generation handling.

FE09's remaining implementation is to consume the published qualified producer,
enable its existing `qualifiedRefresh` path in production composition, and prove
concurrent 401 single-flight, delayed old-token 401 behavior, logout races, private
authority cleanup, and mutation non-replay through its actual controller. These
are frontend-owned checks, separate from backend cookie-engine evidence.

## Frozen execution receipt

Assignment base: `c4ad561cd813f0f7f1e14f21cb2ee90592e09eea`.
Fresh native compilation: Rust 1.88.0, locked/offline dependencies, two jobs,
debug information disabled, incremental compilation disabled, six service bins;
exit 0 in 36.14 seconds. The allocated existing target was released with zero
open handles before any private PostgreSQL or Chrome run.

```json
{
  "result": "PASS",
  "producer_revision": "7c2f3af7b9fd7bab2ed2d863cc2262527cc8553e",
  "started_at_utc": "2026-11-04T00:54:44.341332+00:00",
  "finished_at_utc": "2026-11-04T00:56:55.344346+00:00",
  "native_receipt_sha256": "e662d4e59cdc3e11c4018880be6eec7d4b5a444f5f6c3b492d9c1608366713f7",
  "native_run_script_sha256": "695208ab04d241d75aaab63a6c156730eaf6cc00b2653d805555968829c3a427",
  "final_script_sha256": "748c35416003b6ea05a5db88f67a365711bfde04073442c22586929e3e14d46a",
  "source_manifest_sha256": "717386a405c625414c880dcd6f001d706735f59834863f0d8b40c866ed332794",
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
  "cleanup": "PASS",
  "secrets_recorded": false,
  "production_deployment": "NOT_RUN",
  "frontend_production_wiring": "NOT_RUN",
  "latest_outer_HTTP_layer": "NOT_RUN"
}
```

| Native service | SHA-256 | Migration count |
| --- | --- | --- |
| api-gateway | `986f4cd6b2a9948cea436585388c1136bd5ed67b2f18b84ad426377f945c9f03` | 17 |
| auth-service | `e7c41cf956ba12a0e23359b6a9ec5b23c06ca144082f353c9d33bd0a518f0c95` | 17 |
| profile-service | `6d02f27a96b44c94241383c5f6e46dbe12a5b97e63e2597c4a442ada855c1b53` | 8 |
| friend-service | `43f651407453dcdba5c2b2bc11ed7177340944d0eb6879211e9637e5e59f3f1d` | 17 |
| post-service | `c82b066fae457ba1b83d17cc646de8557f6f2fbcf75c80bfeb96809b3314087e` | 31 |
| notification-service | `9e3f2120e0ae8f342b7dd8a86c5337453ef3c0e0ce1d00e2458e063dba72feca` | 4 |

The complete private receipt is retained as `browser-report-5.json` alongside
`artifacts.json` and frozen copies in the task-owned qualification artifact
directory; the publishing handoff supplies its absolute location. The compact
receipt above is the durable repository evidence. It contains no tokens, mail
payloads, passwords, signed cookies, account identifiers, or private store URLs.

Earlier setup attempts are not counted as acceptance: the invalid two-second
TTL was rejected by Auth; the old readiness parser rejected the newly added
probe; the strict proxy initially rejected Chrome's favicon; and the first loss
probe incorrectly assumed one fetch always meant one transport attempt. Each
attempt cleaned its owned resources. The full accepted run includes the actual
observed Chromium retry and all subsequent logout/generation checks.
