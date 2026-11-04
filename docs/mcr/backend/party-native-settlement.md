# Party native settlement

Status: IN_PROGRESS. Compilation and native execution: NOT_RUN at this source
checkpoint. Task PARTY-NATIVE-SETTLEMENT-01 starts from published master
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
state, and still accepts later metadata retirement without asserting success. READ and LOOKUP cannot
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
commit-time receipt snapshot. Neither state becomes a fabricated success flag. No raw request body or owner registration secrets are returned.

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

Rust1.88 rustfmt and changed-path whitespace check pass (exit0). Cargo and PG
remain NOT_RUN; no target or native lease is held at this checkpoint. A fresh
21d READY/OFFER/ACCEPT is required before either. No performance, load or
200-user work is included. Runtime results, failures, exact commands, artifact
hashes and final committed handoff will be appended after qualification.
