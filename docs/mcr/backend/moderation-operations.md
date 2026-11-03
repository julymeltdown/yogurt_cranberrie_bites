# BE36 / BE38 moderation and event operations

Status: **IN_PROGRESS**. This bounded patch provides internal typed values and PostgreSQL producers. It does not establish qualified HTTP, real media/evidence review, operator authority, physical venue correctness, or complete-task acceptance.

## Ownership and ancestry

- Repository: `julymeltdown/mcr`; delivery branch: `master`; only Delivery integrates/pushes.
- Writer checkout: `/Users/lhs/Desktop/mcr-worktrees/be-moderation-operations`; branch: `codex/mcr-be-BE36`.
- Base: `51783cf5651b68ca98faede29d442c4dccc24f6f`.
- Registration-only commit: `1e05f90d905a40f81b2be51526b4fac43456927b`. Delivery explicitly delegated only `pub mod report` in domain/application roots, `pub mod admin` in the application root, and report/event_operations repository exports. Preserve other owners' exports during integration. `4bb2d7e` corrects only those declarations to follow the preserved crate documentation.
- Newly authored paths: domain `report/**`; application `report/**`, `admin/{mod,event,notices,places}.rs`; service repository `{report,event_operations}.rs`; service tests `{report,event_operations}.rs`; migration `0015_moderation_operations.sql`; scripts `{operations_smoke,test_operations_smoke}.py`; this document.
- No source pack, `docs/source/`, frontend, manifests, lockfiles, HTTP/proto contracts, router, or production role assignments changed.
- Place ownership is a caller-transaction port; this patch does not bypass immutable released place/map rows.
- BE39 report decisions, BE40 mural control, BE41 closure/archive, and BE42 account export/deletion are excluded. Phase mutation permits only DRAFT → OPEN and OPEN → LIVE.

## Provenance and dependencies

All product/domain/application logic is newly authored. The native test ownership preflight was adapted from `backend/services/concert-service/tests/command_receipts.rs` at the pinned base. The harness follows `backend/scripts/mcr/command_receipts_smoke.py` and reuses `migration_smoke.py` plus `concert_core_smoke.py` at that base for private-cluster ownership, subprocess redaction, and evidence capture. No legacy rendering/UI logic is reused.

Dependencies are retained workspace resolutions: async-trait, chrono, uuid, serde/serde_json, sha2, SQLx, and the existing BE15 command repository. The application adds no framework/SQL/serialization dependency. No dependency or lock resolution changed.

C04 candidate pins at the base (review candidate, not a claim of whole-contract freeze):

| Source | SHA-256 |
| --- | --- |
| `contracts/domain/v1/admin.schema.json` | `b8a4f8b91440d561bbcf23dc68bc196919a8893cf744ca09ac05ecaabff72cb8` |
| `contracts/domain/v1/feed.schema.json` | `9c8d991df0bb707d7a7a2d43d4b132e7fadb878a52633e1c1f338afc9407f8ed` |
| `contracts/domain/v1/operations.json` | `42ed48ee0cad846b94f9f7a5d39a38beeeb5f5b41e0129f157e51ff31c635223` |
| `contracts/domain/v1/policy.json` | `508d22af964b718cdcbd5a25b32b9a1e3f7b651a5b8c25c87d56f1f6b68d0ebe` |
| `contracts/assets/v1/map-manifest.schema.json` | `ed01d26b2f561c9885ad3fc5fe66912ee8175334393e49a85652756e301d6800` |

## Report behavior and privacy

`CreateReport` uses closed target/reason enums, text up to 2,000 Unicode scalar values, and at most four distinct non-nil evidence media UUIDs. The service resolves actor/event independently from wire content. `FencedOperationsAuthority` must be request-bound to canonical Auth and the current event-scoped role owner. `FencedReportEvidence` must verify target access or qualified recent-target evidence, and READY, actor-owned, event-scoped REPORT-purpose attachments. Their positive revision and fresh time window must remain fenced through the transaction. Input digest, actor/event, and expiry mismatches fail closed. Unavailable implementations never grant access.

A server-generated report ID is deterministic for actor/event/operation/idempotency key. BE15's HMAC canonical input fingerprint, receipt, domain effect, audit record and invalidation commit together. Same-key retries return the same safe receipt after current actor/report authorization; first-execution submission target/media checks are not repeated for an already committed receipt. Different input conflicts. A new key for the same actor/event/typed active target records only a safe `RESOURCE_UNAVAILABLE` failure. At most ten successfully created reports per actor/event in a rolling hour are admitted; the existing BE15 quotas also apply. SAFETY reporting remains allowed in READ_ONLY and is rejected in DRAFT/ARCHIVED.

Safe create/own/queue projections contain only `report_id,event_id,target_type,target_id,state,revision,created_at`. Reporter identity, private text, reason and evidence IDs are available only from the current MODERATOR/OPERATOR detail method. Every privileged queue/detail read and operator settings/place/notice read appends a minimal audit record before any content is returned. An audit failure rolls back and withholds the response. Ordinary own reads return only the reporter's safe status, never evidence. Target/public APIs receive no private projection. Domain private input/detail types deliberately do not implement Debug.

Evidence UUIDs are references, not public URLs. Actual evidence download needs an independently authorized media owner provider. No media readiness, target access, or role can be inferred from a participant/profile/bootstrap row or browser boolean.

## Event operations

Settings contain explicit input for all five feature flags and contract-bounded timing limits. Settings CAS uses the canonical event revision; each settings/phase mutation advances that event revision once. Initial settings are supplied by a qualified operator command, not fabricated defaults. `require_feature_in` retains event/settings SHARE locks within an already-authorized caller transaction and rejects missing settings, disabled features, non-writable phases or stale revisions. Root integration must wire this into each owning feature producer; exporting the helper alone does not establish global feature enforcement.

Notice storage is owned by BE38 in `concert_operation_notices`. The exact typed content/source/confirmation/state must be reviewed by a qualified owning `FencedNoticeReview` with an actor/event/notice/input-digest/revision/time binding. HTTPS syntax, bounded text and nonfuture confirmation time are validated again before mutation. Publication and withdrawal advance notice revision, audit and invalidate atomically. Public list/detail select only PUBLISHED rows; withdrawn/draft rows remain available only to the operator query. The durable BE15 outbox carries UUID/revision only; Delivery must route private report invalidations to the authorized reporter/operator audience and notice changes to authorized notice consumers. No external side effect is called inside a SQL producer.

The `FencedPlaceOperations` adapter requires local transactional updates by the place/map owner: event/current-map validation, exact CAS, reviewed provenance/confirmation, closure/accessibility handling, and immediate physical-selection fencing. A dependency error or deliberate failed outcome never records the operation applied. The default is unavailable. Existing released map provenance remains immutable. Event isolation of closure and source/map revision evolution require an owning implementation, not direct SQL from this patch.

Mutation lock order is event UPDATE, owner authority fences, BE15 global admission, aggregate. Reads retain event SHARE. This bounded design serializes writes per event; load/throughput qualification is NOT_RUN. Deferred database deadline guards cover receipts and all owned mutations/audited evidence reads, preserving the earliest role/evidence/review deadline through commit. Audit is append-only and does not store report text, media IDs, notice content or operator free-text reasons.

## Validation evidence

| Check | Status / exit |
| --- | --- |
| `python3 scripts/mcr/check-plan.py` | PASS / 0; planning/provenance only |
| Scoped Rust 1.88 rustfmt and `git diff --check` | PASS / 0 |
| `python3 -m unittest discover -s backend/scripts/mcr -p test_operations_smoke.py -v` | PASS / 0; 5 refusal/cleanup tests |
| All eight base + 0015 SQL files applied transactionally to a new private native PG16.14 cluster | PASS / 0; cluster removed; schema syntax only |
| Targeted Rust compile/unit tests | NOT_RUN; explicit shared Cargo lease pending |
| Sixteen authored native PG report/operation/place tests | NOT_RUN; explicit Cargo lease pending |
| Migration ledger, actual PostgreSQL restart persistence and harness cleanup | NOT_RUN in the full binary-bound harness |
| Current production Auth/role, media/target/source review, owning place mutation | NOT_COMPOSED / NOT_RUN |
| Qualified HTTP, real multiuser/operator flow, browser/device, load/release | NOT_RUN |

Run the full harness only with exact source-bound binaries and a new absolute evidence filename:

```sh
python3 backend/scripts/mcr/operations_smoke.py \
  --binary /absolute/validated/concert-service \
  --report-test /absolute/validated/report-tests \
  --operations-test /absolute/validated/event-operations-tests \
  --place-test /absolute/validated/place-operations-tests \
  --pg-bin /opt/homebrew/opt/postgresql@16/bin \
  --report /tmp/new-be36-be38-evidence.json
```

It requires the exact private database `mcr_concert_be_operations`, refuses foreign/replaced cluster ownership, checks test counts, restarts the actual server, verifies committed report/settings/notice/receipt/audit/outbox counts and removes its owned cluster. It never accepts a caller-supplied live database URL. Reports include source, binaries, contracts and migration digests. An incomplete test run or failed cleanup revokes success.

## Required integration and remaining qualification

1. Delivery integrates registration and source in order, preserving newer shared exports, migration ordering and owning-file changes.
2. Bind canonical request Auth and current event-scoped MODERATOR/OPERATOR authority with real revocation/revision fences. No new role endpoint is proposed.
3. Bind report target/READY-media evidence and exact sourced notice review providers. Supply real ownership/revocation/fault evidence; synthetic test leases do not qualify these providers.
4. Implement/qualify the existing place owner's transactional administrative port, preserving map immutability, event isolation, reviewed provenance and physical-selection safety.
5. Compose current C04 transport/closed-schema DTO validation, no-store responses, command receipt follow-up and opaque pagination in root-owned adapters. This patch's UUID read selectors are internal, not a newly invented wire cursor.
6. Adopt server feature gates in every owning feature mutation producer and invalidate/recheck subscribers on settings/phase changes. Integrate notice reads with the feed's authorized projection and private report routing with the outbox consumer.
7. Run combined ownership/import checks and current candidate-bound HTTP/multiuser/operator gates. Keep BE36/BE38 IN_PROGRESS until their remaining task criteria are met; IN05 acceptance remains separate.


## Delegated place-owner increment and source review

Delivery's exact additional grant names `backend/services/concert-service/src/adapters/out/repository/place/operations.rs`, `backend/services/concert-service/tests/place_operations.rs`, and only appended module/public-use exports in the existing `repository/place.rs`. No edits to migration 0003, root manifests, domain place values or existing selection code are granted by that increment. The core registration grant was: “pubmodreport domainlib+applicationlib, pubmodadmin applicationlib; pubmodreport/pubmodevent_operations repository.rs.” These are narrow explicit transfers, not general shared-file ownership.

The new `PgPlaceOperations` repeats current operator authority in the caller transaction, uses the same map publication advisory lock as the owner, locks/rechecks the event map binding then actual current place state, and CASes the place state's own revision. Closure changes preserve all immutable source/review/photo metadata. Concurrent current bindings to the same release cause a safe MAP_VERSION_MISMATCH instead of cross-event writes. Changes to immutable accessibility/label/provenance also require a newly reviewed release and return that safe failure. The three authored native tests cover concurrent CAS, selection exclusion, rollback, existing/future shared bindings, stale reopen after rebinding and immutable metadata; they are not yet executed.

This is a restricted closure adapter, not full place-operation acceptance. Current storage keys closure by global map/revision/place. A durable operated-release owner and binding trigger prevent future events inheriting operated state; writes after a catalog rebind fail closed to prevent local revision reuse matching stale requests. Full event-isolated closure with monotonic event/place revisions, reviewed new-release administrative publication and stale guidance invalidation still require the owning integration extension. No assertion of real physical arrival or reviewed venue facts follows from synthetic fixtures.

A targeted peer source review found five concrete issues, now corrected in source: receipt-only trigger field access, expired receipt retirement, cross-target key replay classification, completed-report replay after media loss, and missing operational-read audits. The reviewer rechecked all five and found no remaining issue within that fix scope. Added regressions remain NOT_RUN pending the explicit Cargo lease. Source review does not establish runtime or task acceptance.

The final feature-port signature returns `FeatureGuard<'_>` rather than a raw settings value. It retains the mutable caller connection borrow and exposes `connection()`, `settings()` and `revision()`; the caller retains the original transaction through the effect and BE15 commit. Owning producer adoption remains required.


SQL guard evidence: `/tmp/mcr-operations-ddl-20261104-02.json` records eleven native PostgreSQL 16.14 SQL-only checks passing with owned cleanup. It exercises deferred guards on report/settings/notice/audit rows, missing/expired authority rollback, append-only audit and expired retirement without a user lease. An initial fixture attempt (`...-01.json`) failed because it supplied `clock_timestamp()` later than the notice statement timestamp; the corrected fixture uses `statement_timestamp()`. These direct SQL checks are not compiled-producer qualification and predate the later durable place-ownership constraint increment.


Consumer comparison against FE38 base `072ea867`: the ten inspected admin definitions, five feed notice definitions and nine selected BE36/BE38 operation entries are structurally identical to the source base despite whole-file digest changes. This comparison does not repin uninspected source or policy behavior.


## Resume and forward migration at published base 1163b827

The sole writable checkout transferred to `/Users/lhs/Desktop/mcr-worktrees/be-operations-resume`, branch `codex/mcr-be-operations-resume`, at `1163b827f9e62390163788bb674d24855816e0ec`. The former checkout is read-only. The aligned persistent goal remains ACTIVE and unbudgeted; its historical checkout wording is superseded by this explicit ownership transfer, not by a claim that unfinished production work was completed.

Author commits were carried with `git cherry-pick -x`: `1e05f90d` → `08f8c4cf`, `14b8d598` → `bdcce967`, `4bb2d7e8` → `f943d47d`, `041dac90` → `6c647e78`, `30b91694` → `ce52dc14`, `8af6d7b` → `92ac5681`. Current unrelated Mural, context, Profile and root exports were preserved. No unrelated shared commits were imported.

The interrupted dirty guard was recovered from `/private/tmp/mcr-operations-recovery-1793741629163453000/0015_moderation_operations.sql`: exactly 8,690 bytes, SHA-256 `01622e9d134c6cb9e9950a450397e6c542af5289a8e372b331bf7ec52a786506`. Its bytes are retained in the newly reserved **0021_moderation_operations.sql**. The unpublished 0015 path is removed. Published migrations 1–12, 16, 18, 19 and 20 remain byte-for-byte unchanged.

Current C04 hashes at 1163b827: admin `953e78a482d998e35c9612c30467600adcead3c9e9d4fb6ab7679a2164aef640`; feed `590cce6afc00c7a869b5a9711104550ba280ffa5875c9dc49a0753d272320801`; operations `622c0e0eba6e1c835c5e953a3acd6825f97ed43aff9bc32b982c207fdfe31839`; policy `81b0da4c007543765f120c4ea58163cc1eadea41e10d24db54e1d177ef4e5fbb`. Ten required admin definitions and five notice definitions are structurally identical to the original source candidate. Whole-file drift is not silently ignored; the owned harness pins these current hashes. Operation/rule drift is inspected separately before final handoff.

Resume checks so far: Rust 1.88 locked/offline domain/application library tests PASS (6 application + 4 domain), and exact report/event_operations/place_operations service test executables plus the migrator compile PASS. Shared BACKEND lease limits are jobs ≤2, dev/test debug 0, incremental 0, ≥8 GiB available; no alternate target, dependency install, or MEDIA target is used. Binary artifact freezing precedes release of all Cargo handles and owned PostgreSQL work.

The owned harness now includes an explicitly selected ignored native helper under `tests/operations_support/` that applies the real published SQLx baseline through 0020 to a second private database. Its receipt rows are generated by SQLx, not manufactured. A deliberate late 0021 table collision must leave the baseline ledger/state intact and no partial operations tables; recovery then applies 0021, verifies all 17 current migration receipts, reapplies, and checks both fresh/forward databases across an actual PostgreSQL restart. Adapter qualification remains the 16 authored cases; the separately executed baseline helper adds one native ledger case. These resume native checks are NOT_RUN until the source-bound frozen binaries execute.
