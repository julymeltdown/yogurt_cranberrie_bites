# BE36 / BE38 report and operations handoff

BE36 and BE38 remain **IN_PROGRESS** at task/production level. The assigned internal adapter increment is implemented, committed and qualified against isolated PostgreSQL. This document does not certify production Auth/operator/media/source providers, full place operations, feature adoption in every producer, qualified HTTP, or IN05 acceptance.

## Delivery and provenance

- Delivery repository/branch: `julymeltdown/mcr`, `master`; Main integrates/pushes. No worker push occurred.
- Sole writable checkout: `/Users/lhs/Desktop/mcr-worktrees/be-operations-resume`, branch `codex/mcr-be-operations-resume`.
- Published parent: `1163b827f9e62390163788bb674d24855816e0ec`.
- Qualified implementation/test source: `f485420d94bb68ea5bca82a4f88fc239bb831010`. The later handoff commit changes this document only; all backend/contract source hashes must continue matching the artifact receipt.
- Historical checkout `/Users/lhs/Desktop/mcr-worktrees/be-moderation-operations` is read-only. Its aligned unbudgeted goal remains ACTIVE; unfinished broader requirements are not falsely completed.

| Original author commit | Recovered commit |
| --- | --- |
| `1e05f90d` | `08f8c4cf` |
| `14b8d598` | `bdcce967` |
| `4bb2d7e8` | `f943d47d` |
| `041dac90` | `6c647e78` |
| `30b91694` | `ce52dc14` |
| `8af6d7b` | `92ac5681` |

All six were carried with `cherry-pick -x`. `bca7a9a` recovers the interrupted guard into the forward migration and adds current-ledger qualification. `f485420` adds the direct shared-release acquisition assertion. No unrelated shared commits were imported. Integrate the complete recovered stack/final tree; do not deploy an intermediate historical 0015 tree.

The preserved recovery input is `/private/tmp/mcr-operations-recovery-1793741629163453000/0015_moderation_operations.sql`: 8,690 bytes, SHA-256 `01622e9d134c6cb9e9950a450397e6c542af5289a8e372b331bf7ec52a786506`. The same bytes now reside only at **`backend/services/concert-service/migrations/0021_moderation_operations.sql`**. The unpublished 0015 path is absent. All sixteen published migrations (1–12, 16, 18, 19, 20) match parent bytes exactly.

All report/admin product logic is newly authored. The private-cluster preflight derives from the pinned BE15 test/harness pattern; helpers `migration_smoke.py` and `concert_core_smoke.py` are reused without modification. Native place fixture assembly derives from the existing `tests/places.rs`, with geometry hash recomputed before content hash. Synthetic metadata is not production art or a reviewed venue fact. No `docs/source/`, frontend, manifest, lockfile, contract, proto or infrastructure file changed. Retained dependencies are async-trait, chrono, uuid, serde/serde_json, sha2, SQLx and BE15; application/domain boundaries remain free of SQL/transport imports.

## Exact path ownership

The 21 changed paths are limited to domain `report/**`, application `report/**` and `admin/{mod,event,notices,places}.rs`, service repositories `{report,event_operations}.rs` and `place/operations.rs`, tests `{report,event_operations,place_operations}.rs` and `operations_support/mod.rs`, owned smoke/guard scripts, migration 0021 and this document, plus four narrow shared registrations.

Explicit shared grants add only `pub mod report` to domain; `pub mod report` / `pub mod admin` to application; report/event_operations repository exports; and appended place operations module/public-use exports. Existing exports, crate documentation and all existing place functions are preserved. Current Mural, context and Profile implementations are untouched.

## Current contract pins and drift

| Source | SHA-256 at published parent |
| --- | --- |
| `contracts/domain/v1/admin.schema.json` | `953e78a482d998e35c9612c30467600adcead3c9e9d4fb6ab7679a2164aef640` |
| `contracts/domain/v1/feed.schema.json` | `590cce6afc00c7a869b5a9711104550ba280ffa5875c9dc49a0753d272320801` |
| `contracts/domain/v1/operations.json` | `622c0e0eba6e1c835c5e953a3acd6825f97ed43aff9bc32b982c207fdfe31839` |
| `contracts/domain/v1/policy.json` | `81b0da4c007543765f120c4ea58163cc1eadea41e10d24db54e1d177ef4e5fbb` |

Compared with author base `51783cf5651b68ca98faede29d442c4dccc24f6f`, ten required admin definitions and five notice definitions are structurally identical. All fourteen assigned report/settings/place/notice operation entries are unchanged. Their ten referenced rules (`event_phase`, `notice_create`, `notice_update`, `ops_read`, `place_update`, `public_read`, `report_create`, `report_own`, `report_review`, `settings`) and runtime/common guards/write policies/idempotency/pagination/transitions/error-status policies also match. Whole-file hashes changed outside these inspected surfaces; this does not repin uninspected behavior. The harness requires current hashes.

## Behavior and invariants

Report input has closed target/reason enums, ≤2,000 Unicode scalar values of private text and ≤4 distinct non-nil media IDs. Canonical actor/event come from a request-bound owner authority, never request role claims or participant/profile/bootstrap existence. Current MODERATOR/OPERATOR evidence access binds actor/event/action/target, owner revision, fresh checked time and expiry. The submission evidence owner must verify current target access or qualified recent-target evidence and READY actor-owned event-scoped REPORT attachments. Evidence/review response leases bind exact input digests. Missing, stale, misbound, unavailable or revoked authority fails closed.

Create/own/queue return only safe Report fields: report/event IDs, typed target, state, revision and creation time. Ordinary/target/public paths cannot serialize reporter identity, reason, text or evidence. Only privileged detail returns them, after a metadata-only audit commits. Operator settings/place/notice reads also audit in their transaction. Private input/detail types have no Debug implementation; audit rows contain no report text/media, notice content or free-text reasons. Evidence IDs never become public URLs.

Report IDs are deterministic by actor/event/operation/idempotency key. BE15 canonical HMAC receipts, local effects, immutable audit and UUID/revision invalidations are atomic. Same-key completed retries reauthorize current actor/report access, without requiring removed submission media. Different input/authorized target conflicts. Duplicate active actor/event/typed targets return a safe failure; successful creations are bounded to ten per actor/event/hour plus BE15 quotas. SAFETY reporting works in READ_ONLY and rejects DRAFT/ARCHIVED.

Settings accept every explicit flag/limit, CAS the event revision and increment it once. Phase only permits DRAFT→OPEN or OPEN→LIVE. `require_feature_in` returns a `FeatureGuard` retaining the caller connection borrow; event/settings row locks remain until the original transaction commits. Missing settings are unavailable, disabled/nonwritable features reject server-side and stale revisions conflict. This helper's qualification does not prove adoption by all feature producers.

BE38 owns `concert_operation_notices`. Exact content, HTTPS source, nonfuture confirmation and state require a fenced source-review lease. Publication/update/withdrawal use notice CAS and commit audit/outbox atomically. Public reads select only PUBLISHED rows; withdrawal removes them from that projection. Receipt SUCCEEDED proves a committed local resource revision; clients still need an authorized current reread before presenting the intended action as applied. It does not prove external notification delivery.

The restricted place owner locks the publication identity, event binding and actual current state, and CASes the place-state revision. It preserves immutable released provenance/verification/photo/label/accessibility fields. An operated release gains immutable event ownership; publication and acquisition guards prevent both future cross-event inheritance and acquisition of an already-shared release. Closure excludes concurrent physical selection. Rebound-catalog writes and immutable metadata changes fail closed with MAP_VERSION_MISMATCH until the full monotonic event/place/new-reviewed-release owner is integrated. Closing/reopening alone does not prove stale existing guidance has been invalidated. No physical arrival is inferred.

All command effects remain local SQL under BE15. Dependency errors, expired leases, failed audits, failed CAS or uncertain downstream results do not record applied effects. Deferred guards retain the earliest authority/review/evidence deadline through commit. Expired BE15 receipt retirement clears metadata without a user lease while preserving correlation and shape constraints. Writes serialize per event; load qualification is NOT_RUN.

## Executed qualification

| Check | Result / exit |
| --- | --- |
| Rust 1.88 locked/offline domain/application `--lib` | PASS / 0; 6 application + 4 domain tests |
| Exact three service tests + migrator compilation | PASS / 0 |
| Direct acquisition-guard assertion recompilation | PASS / 0 |
| Owned harness guard tests | PASS / 0; five tests |
| Native report / event operations / place cases | PASS / 0; 6 + 7 + 3 |
| Explicit published-0020 SQLx helper | PASS / 0; one additional native ledger case |
| Fresh migrations / verify / reapply | PASS / 0; all 17 current migration receipts |
| Forward published0020→0021 / verify / reapply | PASS / 0; 16 genuine baseline receipts → 17 |
| Deliberate late 0021 DDL fault | Expected CLI exit 1; no 0021 receipt or partial operations tables; published state unchanged |
| Real PostgreSQL restart | PASS; new postmaster, both ledgers and committed state preserved |
| Private cluster cleanup | PASS; owned directory removed |
| Plan checker / path scope / cross-directory imports / diff whitespace | PASS / 0 |

The ignored ledger helper is explicitly executed once before upgrade. The ordinary operations suite then reports 7 passed/1 known ignored; the exact-count harness verifies both invocations. Total native executions are **17 passed**, with no weakened expectations or replacement of failed checks. Test-only owner fixtures exercise real SQL row locks, denied/revoked/misbound leases and faults. They do not constitute real Auth/operator/media/source qualification.

After restart: 14 reports, 2 settings rows, 3 notices, 35 receipts, 26 outbox rows, 32 audit rows and 2 operated-release owners remained. Published forward event/receipt hashes also remained identical after failed migration, successful upgrade, reapply and restart.

Builds used only `/Users/lhs/Desktop/mcr-build/backend`, Rust 1.88, jobs 2, locked/offline, dev/test debug 0, incremental 0, with ≥8 GiB available. No install, alternate Cargo target or MEDIA target was used. Exact Cargo JSON artifacts were copied and hashed before all build handles were released; PostgreSQL ran only frozen binaries. The standing reservation covered one bounded place-test recompile. Target handles were released again before final native execution.

## Frozen evidence

- Qualified source: `f485420d94bb68ea5bca82a4f88fc239bb831010`.
- Artifact/source receipt: `/private/tmp/mcr-operations-native-final/artifacts.json`; SHA-256 `cdd336a5e3f967a5f210b08d9e6cbcc61cdf406af208899c7c3953277dc08ee5`.
- Final native report: `/private/tmp/mcr-operations-native-final/qualification-01.json`; SHA-256 `c028e086dc25f8a25be60afb96775f24b2ca1a19e5ae191481a452e47818a791`.
- Earlier successful run: `/private/tmp/mcr-operations-native-bca7a9a/qualification-01.json`; superseded for the direct acquisition assertion.
- These reports include exact source, contract, binary and migration digests and command exits. They are local artifacts; retain them with the handoff. Historical SQL-only evidence predates this complete qualification and is not used as post-fix native proof.

| Frozen executable | SHA-256 |
| --- | --- |
| `concert-service` | `c828deb328a82d26050e948c12832450b7e6a5017aebc2cad72287113ce7db01` |
| `place_operations` | `6db11c50c3a74b5ed9517cd8c41745b4e1222004282fc586f36866ccac608255` |
| `report` | `0d85863ffc72b993385ad45073aa84d752772caab183c430ef486e46d5dee9e4` |
| `event_operations` | `f33d57392ee0705e1eba45166a74c66c097e9adeaa03d340f49cc2d505585fb3` |

Commands, from the assigned checkout:

```sh
env RUSTUP_TOOLCHAIN=1.88.0 CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/backend \
  CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 \
  cargo test --manifest-path backend/Cargo.toml --locked --offline \
  -p concert-domain -p concert-application --lib --message-format=json
env RUSTUP_TOOLCHAIN=1.88.0 CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/backend \
  CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 \
  cargo test --manifest-path backend/Cargo.toml --locked --offline -p concert-service \
  --test report --test event_operations --test place_operations --no-run --message-format=json
env RUSTUP_TOOLCHAIN=1.88.0 CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/backend \
  CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 \
  cargo test --manifest-path backend/Cargo.toml --locked --offline -p concert-service \
  --test place_operations --no-run --message-format=json
python3 -m unittest discover -s backend/scripts/mcr -p test_operations_smoke.py -v
python3 backend/scripts/mcr/operations_smoke.py \
  --binary /private/tmp/mcr-operations-native-final/concert-service \
  --report-test /private/tmp/mcr-operations-native-final/report \
  --operations-test /private/tmp/mcr-operations-native-final/event_operations \
  --place-test /private/tmp/mcr-operations-native-final/place_operations \
  --pg-bin /opt/homebrew/opt/postgresql@16/bin \
  --report /private/tmp/mcr-operations-native-final/qualification-01.json
python3 scripts/mcr/check-plan.py
```

The evidence report filename must be new for any rerun; the harness never overwrites it or accepts a live caller-provided database URL. Both databases, socket and postmaster live under a new marker/inode-owned private cluster. Failed cleanup revokes success.

## Required integration and remaining gates

1. Main integrates the complete scoped stack, preserves newer shared exports, and publishes only the final 0021 tree. No additional general review gate is required.
2. Bind current canonical Auth/event roles, report target and READY-media evidence, and exact notice source review with real owner revision/revocation fences. Defaults remain unavailable.
3. Complete monotonic event/place revisions, reviewed replacement-release administrative mutation and existing-guidance invalidation. The restricted closure port is not full BE38 place acceptance.
4. Adopt the feature guard in every owning feature producer; invalidate and reauthorize capabilities/subscribers on changes. Exporting the helper alone does not close this requirement.
5. Compose the existing C04 HTTP/receipt/opaque-pagination adapters, notice feed read port and private report outbox audiences. No public role endpoint is invented. Frontend ordinary/report-detail boundaries remain distinct.
6. Run real provider/HTTP/multiuser/operator/venue and release gates against the integrated candidate. BE39 decisions, BE40 control, BE41 lifecycle, real assignments, deployment and live-data migration were not expanded by this patch.

The broader goal and BE36/BE38 task status remain unfinished where these required integrations are missing. Native adapter evidence cannot be promoted into production or whole-project acceptance.
