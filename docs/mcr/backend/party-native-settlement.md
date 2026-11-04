# Party native settlement

Status at committed handoff: IN_PROGRESS / awaiting named Root publication.
Implementation and developer-owned component qualification passed; the following
evidence supersedes the earlier NOT_RUN checkpoints. Task PARTY-NATIVE-SETTLEMENT-01 starts from published master
`7d5d91bd1121b1a95b2302ca02e9224019a5deaa`, in the exclusive
`codex/mcr-be-party-native-settlement` worktree. The completed Sessions and Spatial
branches and their evidence are preserved.

## Behavior and boundaries

`PartyNativeResolver` resolves the actual CONCERT native transaction for all
eight Party operations: six commands and two reads. The existing scoped guard
now captures and registers the real transaction on its caller's connection,
awaits both canonical Auth/Friend registration confirmations, and inserts an
immutable `PARTY_NATIVE_V1` witness. The witness binds configured instance,
system identifier, database OID, full XID, backend PID/start, exact published
Product parent, request/body/target digest, command key and explicit role.
The old generic `PARTY_REQUEST_V1` correlation is never positive proof.

Migration 0057 validates the current connection's identity and exact confirmed
parent at insertion and deferred commit, recomputes the Party binding digest,
and enforces the original deadline. It holds parent/tracking shared locks through
commit. COMMAND requires its actual matching receipt. The deferred trigger installs a
second immutable command-receipt witness containing the exact receipt ID and
commit-time snapshot. A restrictive foreign key retains that original receipt;
key reuse cannot substitute a replacement row. Resolution verifies the original
immutable receipt correlation, reports both commit-time and current business
state, and still accepts later metadata retirement without asserting success.
READ and LOOKUP cannot
commit changes to Party aggregates, memberships, applications, receipts or
outbox, including effects written before the witness was installed. Retained
policy pair reservations and child bookkeeping remain outside that business
effect prohibition. Witness update, deletion and truncation fail closed.

The application-to-party prelookup used by decide/cancel application is explicitly
LOOKUP, since it commits before a command receipt exists. Root transferred the
exact default `PartyLiveOwnerGuard::track_lookup_in` method and the one
`application_party()` call replacement in addition to the originally assigned
module/export hunks. Other guard implementations retain a compatible default;
the actual scoped guard overrides it with a native LOOKUP witness.

Resolution requires the exact ROOT_FINISHED parent, published delivery secret
and confirmed identity before reporting any native outcome. A foreign real
aborted XID is not rollback proof for this request. Active work remains
IN_PROGRESS; absent, malformed or mismatched proof is UNKNOWN. Committed work
requires the immutable witness and, for commands, its current actual receipt.
COMMITTED describes the transaction; FAILED and PENDING receipts are not
business success. Private correlation evidence exposes the actual receipt state
at resolution, including later RETIRED metadata, alongside the actual immutable
commit-time receipt snapshot. Neither state becomes a fabricated success flag.
Receipt instants are compared as timestamps, independent of the resolver session
time zone; a separate Asia/Seoul pool is included in the native regression.
No raw request body or owner registration secrets are returned.

Root retains config/native resolver routing, runtime/main, actual Profile and
community resource factory composition, manifests, contracts and infrastructure.
This assignment does not qualify Root's Party factory or HTTP/user journey.
It preserves the existing Party authorization, resources, original deadlines,
idempotency and single acquired owner scope. The exported registration helper
confers no policy/resource authority.

## Provenance and dependencies

All paths below are relative to `backend/services/concert-service` and their
source revision is the full published base above. Original source remains
unchanged. No dependency, manifest, lockfile, source archive or other migration
is edited. Existing sqlx, platform, tonic, serde, serde_json, sha2, uuid,
async-trait and Tokio resolutions are retained.

| Read-only source | SHA256 | Destination |
| --- | --- | --- |
| src/operations_live/settlement.rs | 93417c7a09e32c7ea0e3d27aeffb91f2573e0a9785f379871a1914fee697f9cf | src/party_live/settlement.rs, adapted transaction/witness pattern |
| migrations/0047_operations_native_witness.sql | 8757982a0f42ad9248565bb760e6a92f254ef8eb8debc2fcf1ee5758b0e9d43e | migrations/0057_party_native_witness.sql, new Party checks |
| tests/operations_live_support/capture.rs | dc1ff202e50fa12eaa0f381135132b5688fa0f1fed9ffdebbfeb591d08b3f901 | tests/party_native_settlement/capture.rs |
| tests/operations_live_support/environment.rs | f6db7378a6f1420f0f3074676750e4ea1ef69488a8995fdd0c0571c6387376d0 | tests/party_native_settlement/environment.rs |
| tests/operations_live_support/owners.rs | 04bcb958b6f2816f2ec99bf8ab745bfc0e62e0cd4e8cb42003dc13fc54a49327 | tests/party_native_settlement/owners.rs |

Each fixture preserves its upstream provenance comments. The private compile,
guardian and native controller under `/tmp/mcr-party-native-settlement-01` adapt
the retained Sessions controller under `/tmp/mcr-world-sessions-journal-be42`.
The native controller uses the unchanged repository migration cluster and
invalidation PKI helpers, pinned with the complete backend/contracts read set.

## Validation scope

The new test artifact has two pure cases and six ignored native cases. Native
fixtures use real signed Gateway capture, actual Auth/Friend PostgreSQL owner
stores, mTLS RPC, Root ledger, confirmed registrations and native PostgreSQL16.
Fixture Auth users/families are seeded; signup is not exercised. A deliberately
receipt-only producer always returns a business failure. It grants no Party
policy or resource authority and creates no successful Party aggregate. Actual
BE15 fingerprinting, failure receipt, exact replay and changed-body conflict
paths are exercised. Direct retirement of an actual fixture receipt tests old
commit resolution after business-outcome fields are cleared; this is not an
elapsed 24-hour expiry-worker test. Synthetic invalid receipt metadata is used only for
negative READ/LOOKUP effect cases, never positive product acceptance.

Planned native coverage includes all eight real native commits, failed command
receipts, explicit prelookup, before/after effect refusal, wrong scopes and
native identities, immutable witness, receiptless command refusal, deferred
original deadline, actual rollback, active/drop-worker drain, and physical
postmaster restart with unchanged witnesses/receipts/outbox and migration ledgers.
The controller requires an owned inode/marker/system-bound three-database
Unix-socket-only cluster, releases shared build target before copied execution,
and tears down only that owned cluster. Cleanup is not settlement evidence.

## Qualified source and commands — 2026-11-05 KST

The actual frozen functional source is
`ae6ad3ce30eb634204f373d8f410ef87efa71e4a` on the retained exclusive branch.
The complete 2,410-file backend/contracts inventory is
`/tmp/mcr-party-native-settlement-01/source.json`, SHA256
`fa1e4994951e46536a334e0731bec3b6d6cb5c11d35e87163e2f0941ada65df4`.
The documentation-only handoff does not change those compiled functional files.
No source archives, dependency resolutions, shared configuration or other
implementation owner's files changed. The two extra trait/lookup hunks are the
explicit Root transfer documented above.

The single selected build, with installed Rust 1.88, locked offline resolutions,
two jobs, dev/test debug zero, incremental off, retained contracts build opt2
and both Rust flag environment variables absent, was:

```sh
cargo test --manifest-path backend/Cargo.toml --locked --offline -j 2 \
  -p concert-service --test party_native_settlement --no-run \
  --message-format=json-render-diagnostics
```

It passed, exit 0, in Cargo's 44.70 seconds (45.767 seconds including controller
observation and copying). The exact debug-zero test artifact was selected from
Cargo JSON by target kind, manifest, test profile and executable; one regular
copy was frozen outside MEDIA. The executable has 31,662,864 bytes and SHA256
`c9434fa1359235b9880e5ddec5a5485b708d94797d341ca1ef0b06b89aeeeead`.
The independent guardian started before each child. No Cargo/PG workload ran
before actual allocation acceptance. Root explicitly transferred metadata-only
refresh of the four own-worktree Auth/Concert/Notification/api-gateway build.rs
files. Both complete before/after receipts preserve bytes, size, mode, uid/gid,
inode and atime; only mtime advanced. The admission receipt is
`build-script-mtime-admission.json`, SHA256
`7f76116d4c7e7c0e73e07ea7988101a50bcd415647da410eaea78f1dfd739e10`.
No cache was deleted and no source rebase was performed.

Copied pure execution used
`party_native_settlement --skip native_ --test-threads=1 --nocapture`:
2 passed, 0 failed, exit 0. Each native case used the same frozen executable
with `--ignored --exact NAME --test-threads=1 --nocapture`:

| Exact native case | Final result |
| --- | --- |
| native_party_all_eight_operations_bind_real_commit_and_failed_receipt | 1 passed, exit 0 |
| native_party_lookup_and_read_cannot_commit_command_effects | 1 passed, exit 0 |
| native_party_exact_scope_identity_and_immutable_witness_fail_closed | 1 passed, exit 0 |
| native_party_failed_receipt_replay_and_changed_body_remain_truthful | 1 passed, exit 0 |
| native_party_active_worker_drop_waits_for_actual_rollback_drain | 1 passed, exit 0 |
| native_party_physical_restart_reconstructs_exact_resolver_proof | 1 passed, exit 0 |

The native controller used PostgreSQL 16.14 with one private inode/marker/system-
bound, Unix-socket-only cluster and exactly the Auth/Friend/Concert databases.
Actual SQLx ledgers matched the source: Auth18, Friend18, Concert44 including
0057. It exercised production Auth/Friend stores and authenticated mTLS RPC,
signed Gateway parent capture, real Root registrations and native transactions.
All eight operation bindings committed real transactions; the six command
business receipts were FAILED, never reported as business success. Explicit
lookup/read effect refusals, original deferred deadline, missing receipt,
foreign scope/identity, immutable receipt/witness, replacement refusal, actual
replay/conflict, retirement and Asia/Seoul resolver regression passed.

The active-worker case refused owner settlement while native work remained
in progress. After cancellation it polled actual PostgreSQL rollback status;
Drop/join completion was not accepted as rollback proof. Both production owner
services then settled their prepared exclusions. The controller verified no
prepared work before physically stopping/restarting PostgreSQL, required a new
postmaster PID, checked unchanged native witnesses, exact command receipt
snapshots, receipts, outbox and all three migration ledgers, then reconstructed
resolvers from the stored exact identities. This is actual component recovery,
not a claim of full Party HTTP transport fault injection or Root factory startup.

## Preserved observer failure and resource disposition

The first native run passed all six functional cases and owned cleanup, but
its independent guardian exited 1 with RuntimeError while observing allocation.
The original failure did not retain the precise transient path or a complete
final observed-PID history; those limitations are preserved, not reconstructed.
All original reports/logs/controls remain under `native-attempt1/`.

Before a second native child, the developer added bounded allocation-observation
retries, diagnostic detail and unconditional final PID-list preservation to the
private guardian. A passive native-controller change also retains redacted
PostgreSQL diagnostics before owned teardown. Original and successor controller
hashes, reasons and timestamps are in lease/controller revision receipts. These
changes did not alter functional source, executable, test scope, resource caps
or original deadlines. The unchanged copied artifact reran all six native cases:
all passed, final guardian exit 0, no stop reason. No resource success is inferred
from the first failed guardian. No extra target access or compilation occurred.

The actual named allocation was PARTY-SETTLEMENT-MEDIA-NEXT-202611041655Z.
Acceptance was 01:55:37.657654 KST; MEDIA target release was 01:57:05.402687 KST;
FULL was 01:59:29.796868 KST on 2026-11-05. Machine receipts preserve their
original UTC values. Target release required compiler and handle counts zero
and preceded every copied test/PG action. The usable slot was handed directly
to Wall202, without waiting for copied execution, documentation or coordinator
acknowledgment. No target access occurred afterward.

FULL was 232.139 seconds after acceptance and 144.394 seconds after target
release, within the original 600-second freeze / 720-second work / 780-second
FULL and 180-second release-to-FULL limits. Both native runs totaled 22.487
seconds, within cumulative120. Maximum measured whole usage was 4,727,250,944
bytes, new usage 156,844,032 bytes, and minimum free space 26,590,482,432 bytes.
Observed maximum process counts were 7 build and 16 native. The unchanged whole
8GiB / new total1GiB / executable96MiB / 40build / 48native limits held. All
recorded PIDs and current owned workload matches were absent, the private PG
root was absent, and the final independent guardian passed. Prior historical
shared-target counters remain UNKNOWN and centrally charged; no historical zero
or complete historical PID/counter proof is invented.

The private evidence directory contains source/tool/controller receipts,
original allocation and revisions, Cargo JSON, executable hash, copied logs,
actual ledger/restart hashes, retained failure and final resource audit. Key
files are `compile.json`, `binary.json`, `pure-guardian.stdout`,
`native-report.json`, `native-guardian.json`, `target-release.json`, `full.json`
and `native-attempt1/`. Rustfmt, changed-path checks and the planning provenance
checker pass (exit0). Planning evidence does not certify product acceptance.

## Handoff scope and remaining product boundaries

This developer delivers the five task commits after published7d5, including the
final documentation-only handoff, through the named Root publisher. Root owns
publication/reconciliation and relevant combined checks; no manager or SL review,
readback acknowledgment or proof-only commit is a prerequisite. The canonical
publisher receipt completes this component's delivery evidence.

No successful Party aggregate, positive Party policy/resource provider, actual
Profile/community factory, full browser/HTTP/multiuser flow, live-data migration,
performance/load/200-user result or original BE task acceptance is claimed by
this native-settlement component. The fixture and metadata limits above remain
explicit. Root separately owns actual runtime composition and broader journeys.
