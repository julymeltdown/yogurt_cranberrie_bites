# BE04 Auth-only World owner observation

Status: `IN_PROGRESS`. This bounded producer reuses the integrated Auth owner
service. It does not accept BE04, BE37, IN01, the whole World runtime, or release.
Compilation/native qualification is `NOT_RUN` until the reserved target is released.

## Scope and source

Base `215da400385c400f059913c826105ca4ec5f134b`, branch
`codex/mcr-be-world-auth-read`, isolated checkout
`/Users/lhs/Desktop/mcr-worktrees/be-world-auth-read`.
The predecessor `2a9f326dc1208705fd03cc0b2a87cc69c5884013` is an abandoned separate
RPC WIP. It is stopped and excluded from this patch and its ancestry.

Existing owner provenance: [integration record](relationship-authority-integration.md)
and [qualified author handoff](relationship-authority.md). This patch uses the
retained `relationship.v1.AuthConcertRelationshipService.ReadCurrent`, Auth's
`PgCurrentRelationship`, migrations and dedicated Concert read signer/verifier.
No new RPC, identity database, key fallback, Gateway permission or provider lookup
is added. No manifest, lockfile, proto, runtime, Auth/Friend implementation or
historical source archive is changed.

New World source and tests are authored for MCR. The client reuses the existing
`grpc.rs` transport, edge budget and response validators; its only changes to that
file expose those helpers to the sibling module. Existing relationship client
behavior is preserved. The private native fixture guard/startup follows the
retained `tests/relationship_authority.rs` and `tests/world_admission.rs` patterns
at this base, with different database/environment names. Its Python harness imports
the retained `migration_smoke` lifecycle/ledger and `concert_core_smoke` receipt
helpers. It creates no persistent service and reads no live database.

## Sealed runtime API

All new types and constructors are crate-private in
`adapters::out::relationship_authority::world_auth`. There is no public raw-ID,
browser actor, `WorldSubject`, or expired Gateway-proof constructor.

```rust
WorldOwnerRequest::from_consumed(
    claim: &TicketClaim,
    request_id: Uuid,
    deadline: AuthorityRpcDeadline,
) -> Result<WorldOwnerRequest, WorldError>

WorldOwnerRequest::from_current(
    current: &CurrentOwnedMembership,
    expected_owner: &OwnerIdentity,
    expected_subject: &WorldSubject,
    expected_connection: ConnectionId,
    request_id: Uuid,
    deadline: AuthorityRpcDeadline,
) -> Result<WorldOwnerRequest, WorldError>

GrpcWorldAuthReader::connect(
    auth_endpoint: &str,
    service_token: &str,
    tls: Option<&TlsMaterial>,
    signer: RelationshipReadSigner,
) -> anyhow::Result<GrpcWorldAuthReader>

WorldAuthRead::observe(&self, request: &WorldOwnerRequest)
    -> Result<WorldAuthObservation, WorldError>
```

Admission runtime consumes the digest exactly once, checks the requested pin, then
binds the returned `TicketClaim` before fresh Auth and Concert admission. Initial
admission does not require membership. Binding validates internal event/pin/subject
consistency, canonical nonnil family/user/IDs, ticket lifetime and original access
expiry. The constructor neither consumes a ticket nor queries/mints membership.

Renewal runtime first reads `current_owned_membership` and supplies its expected
owner epoch/process, subject, canonical family and connection. The constructor
checks all of them, internal room/pin consistency and the live membership lease.
These public persistence records are not cryptographic capabilities: the runtime
must obtain them from its actual repository, rather than fabricate them from
browser inputs. Crate-private construction enforces the external API boundary;
it does not authenticate arbitrary code within the trusted server crate.

Both constructors preserve immutable record/membership `access_expires_at` in
`credential_expires_at()`. The effective read window is the minimum of that
expiry, the ticket/membership source expiry, the supplied original wall and
monotonic deadline, an earlier ambient call deadline, and two seconds. The
runtime must capture its outer deadline before earlier I/O; constructing one
after the I/O would misstate the original budget.

Requests are not cloneable and atomically burn their single transport attempt
before dispatch, including outage/cancellation. `cancel()` fails a request closed;
dropping the observe future drops the network future and cannot reset the attempt.
The runtime supplies a fresh request ID for every distinct read. This is not a
global durable request-ID registry; the existing signed guard also rejects
replayed transport proofs. No cache, retry or Friend call is used.

The signed query binds actor equal to subject, event, exact family, request ID,
`SAFETY_SELF` and effective expiry. Immutable access expiry remains local and
caps that signed expiry; the existing wire contract is unchanged. The client
requires exact full query echo, `no-store`, positive bounded timestamps, a
canonical nonnil owner epoch and positive safe revision. Observations predating
the binding, any inactive actor/subject or ended family, and late results fail
closed. Cancellation and deadline checks surround the network await and decoded
response validation. Diagnostics redact identity and owner details.

`WorldAuthObservation` exposes subject, original credential expiry, observation
time, bounded validity and the Auth `OwnerRevision`. Its `check(&request)` only
checks local scope and expiry. It does not reread current Auth state, certify
event admission, or fence revocation through a Concert publication/transaction.
An owner may revoke immediately after the snapshot. Runtime composition and its
process-local invalidation fence remain the runtime owner's separate work.

## Validation checkpoint

Source-only checks, exit 0:

- `python3 scripts/mcr/check-plan.py` (planning only).
- Rust 1.88.0 `rustfmt --edition 2021 --config skip_children=true` on new Rust files.
- `git diff --check`.
- `python3 -B -m unittest discover -s backend/services/concert-service/tests/world_auth_read -p test_native_smoke.py -v` — four receipt/summary refusal checks.

Focused peer source review found no actionable production finding. Its two
specific test gaps were addressed with independent monotonic/ambient deadline
cases and a stale observation whose time window remains otherwise valid.
That review was static; it did not certify compilation or runtime behavior.

Eight deterministic/guard cases and two ignored native cases are authored. The
native harness requires exact successful source-bound Cargo artifacts, privately
copies and hashes them, migrates isolated `mcr_world_auth_auth` and
`mcr_concert_world_auth` using their real migration CLIs, checks SQLx ledgers and
reapply, and refuses absent fixture ownership. Native cases cover real
issue/consume/current membership, signed production Auth owner reads, revocation,
expiry, account restrictions/deletion, bounded blocked SQL, cancellation, recovery
and outage. Restart checks preserve the exact owner revision/denial and consumed
ticket/current membership state. This does not run a multiuser WebSocket flow.

Reserved build parameters: Rust 1.88.0, `--locked --offline -j1`, both debug
profiles zero, incremental disabled, eight-GiB free-space floor. Do not use the
reserved target until Main confirms its actual release. Native harness command:

```sh
python3 -B backend/services/concert-service/tests/world_auth_read/native_smoke.py \
  --cargo-json ABSOLUTE_SUCCESSFUL_JSONL \
  --pg-bin /opt/homebrew/opt/postgresql@16/bin \
  --artifact-dir NEW_PRIVATE_ABSOLUTE_DIRECTORY \
  --report NEW_PRIVATE_ABSOLUTE_REPORT
```

Private evidence root:
`/private/tmp/mcr-world-auth-read-215da400-20261104/`.
The initial `frozen-before.json` pins 154 previous immutable evidence files.
Runtime commands, exit codes, source/binary hashes, cleanup and final delivered
commit ancestry must be recorded before claiming this bounded scope delivered.
