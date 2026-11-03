# IN01 current six-service runtime qualification — 2026-11-04

Status: **current six-service Auth/runtime qualification PASS; World and IN01 NOT_RUN**.
This is a bounded execution handoff, not full product acceptance. The corrected
run used current source and exact immutable binaries without a historical override.
Two independent accounts completed real signup, captured email verification and
login. Actual bootstrap returned 503. Owned cleanup passed; no live accounts or
gate configuration remain.

## Assignment and source binding

- Worktree: `/Users/lhs/Desktop/mcr-worktrees/be-in01-current-runtime`.
- Branch: `codex/mcr-be-IN01-current-runtime`.
- Base: `5ef2df7f9fa01868b0f734233c87a00e37195b61`.
- Corrected execution source: `6e157d1439be831735c3d1e37cd5590c1fdcf33d`.
- Binary build source: `683484baaf78792292337ad0c197a5d574536db3`.
- Bundle: `/Users/lhs/Desktop/mcr-build/current-service-683484b/bundle`.
- Build receipt: `/Users/lhs/Desktop/mcr-build/current-service-683484b/build_receipt.json`.
- Build receipt SHA256: `beed9cfaad633a9c9e00cd85cbf4af21b9572a1eae03c94680cd067969ac27dc`.

The published validator matched all 1,286 receipt source hashes and all six
binary hashes before launch: `SOURCE_HASHES_MATCH_BUILD_RECEIPT`, zero drift.
The build predates integration of these provisioning-tool files, which are not
entries in that source manifest; no hash was bypassed for the read correction.
This worker used the supplied artifacts and did not run Cargo, copy targets,
modify binaries, edit services/contracts/routers, or start another build.

| Binary | SHA256 |
| --- | --- |
| api-gateway | `eed2fb4cc06a2f4cf88b7b790e00bd83cca8e68ab74ff5f696f536603739a9ea` |
| auth-service | `a7ca963053b6734a0db34b89125dac7a25b182703844fefad1c5492bb033dcbb` |
| profile-service | `f278d58621c4922e12a8f3ac84f7d9268c73a004fbfa5697c7cb75fbf5f86ec6` |
| friend-service | `9250eccdf74664c1dda3b2cda77b04eed6d953a392256cd5a3bb0be817d70ef9` |
| post-service | `47e5b34092ba2b6112006b31dfb434925ecb6dab2aca8f2ca136e5383692d805` |
| notification-service | `6e5b9949d0fde226398bb5b9bc03022d25acfcc39a57a5e1cc2f310fd588e957` |

Exact writable tracked paths are this new document and, only for the exposed
read defect, `backend/scripts/mcr/in01_provision.py`,
`backend/scripts/mcr/test_in01_provision.py`, and
`docs/mcr/backend/in01-provisioning.md`. The earlier provisioning worktree remains
stopped. Only Delivery publishes master; this checkpoint does not wait for an
additional main-chat or scrum-lead review.

## Actual corrected execution

```sh
PYTHONDONTWRITEBYTECODE=1 python3 backend/scripts/mcr/in01_provision.py owned \
  --bin-dir /Users/lhs/Desktop/mcr-build/current-service-683484b/bundle \
  --binary-receipt /Users/lhs/Desktop/mcr-build/current-service-683484b/build_receipt.json \
  --release-revision 683484baaf78792292337ad0c197a5d574536db3 \
  --output-dir /private/tmp/mcr-in01-current-proof-bodyless-20261104 \
  --hold-seconds 0
```

Exit 0. Actual runtime interval: **2026-11-04 02:48:50–02:48:59 KST**.
Sanitized receipt:
`/private/tmp/mcr-in01-current-proof-bodyless-20261104/receipt.json`, SHA256
`2ccfb179a9e1371851782e45df811dd0cd4b3fde51241ef7b713554e297f2ba6`.
Its source/helper/contract and migration hashes are retained with exact observed
HTTP statuses, readiness and process-stop results.

| Executed check | Actual result |
| --- | --- |
| Isolated PostgreSQL | 16.14 Homebrew; new marker/inode-owned cluster, no imported live data |
| Migration ledgers | Gateway 17; Auth 13; Profile 7; Friend 14; Post 31; Notification 4; exact source checksums matched |
| Migration lifecycle | All six migrate/verify, PostgreSQL restart persistence and verify-after-restart PASS |
| Service startup | All six reached non-degraded readiness and live HTTP 200 |
| Account 0 | Actual signup 200 → actual private outbox email → verify 200 → login 200 |
| Account 1 | Actual signup 200 → actual private outbox email → verify 200 → login 200 |
| Canonical identities | Signup/verification/login UUID equal within each journey; two final UUIDs distinct; both AUTHENTICATED |
| Event | New owned `mcr_in_concert`, exact bounded migrations 0001–0005, generated OPEN event revision 1 |
| Default room | NOT_RUN: reviewed selected-source-pack runtime pins absent; no room inserted |
| Actual bootstrap GET | Account 0 returned 503; subsequent rooms/account-1 World reads NOT_RUN after this missing prerequisite |
| World/full IN01 | NOT_RUN; no tickets, admission, sockets, snapshots, movement or visual acceptance inferred |
| Cleanup | PASS; all child stops clean, no forced/unexpected exit, owned root removed |

No synthetic user, verification, role, admission, participant, ticket, membership
or owner lease was inserted. The direct event initialization is schema-fixture
execution only, not a Concert SQLx-ledger or serving-runtime qualification.

Output directory is 0700 and every output file 0600. A scan against both generated
emails/passwords/access tokens/refresh cookies found none in the receipt.
No `config.json` was emitted. There was no prepared live consumer, so the stack
was not held; credentials remain private evidence and their accounts/event were
removed with the cluster. They must not be supplied as live frontend gate inputs.

## Preserved first attempt and concrete correction

The first current run at the exact assignment base exited 0 under the earlier
bounded accounting, with both real Auth journeys and cleanup PASS, but bootstrap
returned **413**. That receipt is preserved unchanged at
`/private/tmp/mcr-in01-current-proof-5ef2df7-20261104/receipt.json`, SHA256
`1354e04275e95f4a5ac9afbab239aae9371f10bd8de39c6ec5e8e27fdd3436c8`.
Its PASS field does not qualify the failed bootstrap request.

The unchanged `browser_auth_smoke.Browser.request` sends `{}` for GET. Current
`concert_bootstrap.rs:20` imposes a zero-byte body limit and its handler also
requires an empty body. Commit `6e157d1` added a bodyless `WorldReader`, adapting
the existing helper's bounded response, no-store, correlation, no-referrer and
no-redirect checks. Auth mutation requests still use the unchanged helper.
Only actual 404/503 reads are unavailable prerequisites; unexpected rejections
such as 413 or authentication failures now fail the run. Four focused loopback
HTTP regressions cover both-account bodyless reads, 413 failure, 503 NOT_RUN and
HTTP-200 privacy-policy failure. The corrected native run used a new namespace.

The first command also transcribed an incorrect unused declared release value,
`683484b761e74508f9fdbaf9228aca7886071acf`; actual validated binary/source bindings
were the correct receipt revision throughout. No config was emitted. The
corrected command above declares the exact build revision as well.

## Exact remaining owner and configuration requirements

The **observed** current result is bootstrap 503. Source inspection explains the
missing composition: unchanged `dev_stack.DevStack` launches exactly six retained
services and does not set `GATEWAY_CONCERT_GRPC_ENDPOINT`. Gateway config reads
that optional endpoint at `config/mod.rs:488`; absent endpoint leaves
`state.concert_client` empty at lines 1653–1664, and the current bootstrap handler
returns ServiceBusy at `concert_bootstrap.rs:39–42`. Auth/Profile live owner
projection, event participation and successful Concert bootstrap were not
executed through this missing client.

A separately assigned runtime/composition owner must supply:

1. A qualified immutable **seventh `concert-service` binary** and source/build
   receipt, plus a fresh fully migrated/verified SQLx ledger. Current Concert
   `config/mod.rs:155–165` accepts only `mcr_concert` or `mcr_concert_<suffix>`.
   The tool's bounded `mcr_in_concert` fixture database is not usable by that
   current serving/CLI guard. The optional Concert initialization seam therefore
   needs a coordinated namespace correction before it can be qualified; no
   successful Concert CLI run is claimed here.
2. Explicit owned `CONCERT_DATABASE_URL`, `CONCERT_METRICS_ADDR`,
   `CONCERT_RUNTIME_MODE=local`, verified external migration mode,
   `CONCERT_GRPC_ADDR` on an owned loopback reservation and the Gateway's matching
   `GATEWAY_CONCERT_GRPC_ENDPOINT`. No endpoint is silently enabled by this patch.
3. Validated product-release files for `CONCERT_BOOTSTRAP_FILE`,
   `CONCERT_AVATAR_MANIFEST_FILE`, `CONCERT_MAP_MANIFEST_FILE`,
   `CONCERT_REVENGE_MANIFEST_FILE`, `CONCERT_PARADE_MANIFEST_FILE`, and
   `CONCERT_DANGER_MANIFEST_FILE`. `BootstrapRuntime::load` requires these when
   gRPC serving is enabled; the main executable loads `ProductReleaseBundle`
   before opening listeners. Metadata test fixtures cannot approve product art.
4. The real Gateway caller credential/trust registry, actor-delegation signing
   and verifying-key relationship for the Concert audience, and the selected
   TLS/local runtime policy. Readiness of six other services does not establish
   this authenticated seventh-service edge.
5. Actual available default-room/resource pins, current room/ticket/World socket
   composition, selected-source-pack runtime artwork and the real browser gate.
   The pinned character source pack still says NOT_PREPARED with null
   `production_atlas.runtime_manifest_id`; no fallback artwork is substituted.

These are owner seams, not an authorization to modify service composition,
shared configuration/contracts or assets from this checkout. Full current
World/IN01 remains NOT_RUN independently of this successful current Auth/runtime
qualification.

## Verification and delivery

- `PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s backend/scripts/mcr -p test_in01_provision.py -v`: exit 0, 27 tests, including four new real loopback transport regressions. Their simulated replies do not certify product World behavior.
- Corrected strict native command above: exit 0; exact receipt and limitations recorded above.
- `git diff --check`: exit 0.
- `python3 scripts/mcr/check-plan.py`: exit 0; all 13 source archive checksums intact. Planning integrity is separate from runtime results.

Ancestry: assignment base `5ef2df7` → read correction `6e157d1` → this evidence
handoff commit. The four changed paths are the complete assigned scope; no
frontend, service, central/shared or archived-source edits are included.
