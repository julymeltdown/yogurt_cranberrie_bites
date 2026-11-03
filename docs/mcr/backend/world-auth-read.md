# BE04 Auth-only World owner observation

Status: `CONTRACT_READY` for this bounded producer. Source-bound compilation,
focused transport tests and owned native qualification pass at
`30ca2a499edabb75720273a312d2a4196f18201d`. This does not accept BE04, BE37, IN01,
the whole World runtime, or release. Production composition remains separate.

## Scope and source

Initial base `215da400385c400f059913c826105ca4ec5f134b`, branch
`codex/mcr-be-world-auth-read`, isolated checkout
`/Users/lhs/Desktop/mcr-worktrees/be-world-auth-read`.
After the initial coherent source commit `97c07fa`, the task was cleanly rebased
onto published runtime base `8ceef5dc90c75198da84dd7f87614aa6d6da0fac`, as explicitly
authorized by delivery. The final ancestry is `8ceef5d` → implementation
`508929a5863fc48af2c07b3d42aadd551a266d65` → native fixture/evidence correction
`30ca2a499edabb75720273a312d2a4196f18201d` → the documentation-only handoff commit.
No uncommitted runtime work was imported.
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

## Validation and committed handoff

Source-only checks, exit 0:

- `python3 scripts/mcr/check-plan.py` (planning only).
- Rust 1.88.0 `rustfmt --edition 2021 --config skip_children=true` on new Rust files.
- `git diff --check`.
- `python3 -B -m unittest discover -s backend/services/concert-service/tests/world_auth_read -p test_native_smoke.py -v` — four receipt/summary refusal checks.

Focused peer source review found no actionable production finding. Its two
specific test gaps were addressed with independent monotonic/ambient deadline
cases and a stale observation whose time window remains otherwise valid.
That review was static; it did not certify compilation or runtime behavior.

All eight deterministic/guard cases pass in the privately copied library test
executable. Two native cases are ignored only in the ordinary deterministic run;
the owned harness then executes both explicitly with **2 passed, 0 failed,
0 ignored**. Tests live beside the adapter so they exercise the crate-private
constructors without exposing a public fixture constructor. The
native harness requires exact successful source-bound Cargo artifacts, privately
copies and hashes them, migrates isolated `mcr_world_auth_auth` and
`mcr_concert_world_auth` using their real migration CLIs, checks SQLx ledgers and
reapply, and refuses absent fixture ownership. Native cases cover real
issue/consume/current membership, signed production Auth owner reads, revocation,
expiry, account restrictions/deletion, bounded blocked SQL, cancellation, recovery
and outage. Restart checks preserve the exact owner revision/denial and consumed
ticket/current membership state. This does not run a multiuser WebSocket flow.

Final build parameters: Rust 1.88.0, `--locked --offline -j1`, both debug
profiles zero, incremental disabled, eight-GiB free-space floor. The allocated target was
`/Users/lhs/Desktop/mcr-build/media-combined-fa4b4b3/target` and is now released.
No third target was created. Native harness command:

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
Final qualification records are `build-4-combined.jsonl`,
`build-4-commands.json`, `pre-build-source-4.json`, `deterministic-final.json`,
`native-report-5.json` and `target-release.json` under that private root.
All final commands below exit 0. Both debug profiles are zero and incremental is
zero; the environment pins `RUSTUP_TOOLCHAIN=1.88.0` and the allocated target.

```sh
cargo test --locked --offline -j1 -p concert-service --lib \
  --test relationship_authority --no-run --message-format=json
cargo build --locked --offline -j1 -p auth-service -p concert-service \
  --bins --message-format=json
# Exact test paths come from successful Cargo JSON and verified private copies:
PRIVATE_LIBRARY_TEST world_auth::tests:: --test-threads=1
PRIVATE_RELATIONSHIP_TEST --test-threads=1
```

The current-combined no-run/build commands pass on the source above, which
includes the published runtime prerequisite. The final results are:

| Check | Executed result |
|---|---|
| New deterministic/guard cases | 8 passed; native cases excluded here |
| Retained relationship transport cases | 3 passed; its separate two-owner native case not rerun |
| New actual Auth/Concert native cases | 2 passed, none skipped |
| Native harness evidence guards | 4 passed |
| Auth migrations | Exact 14-file SQLx ledger, reapply and restart verify pass |
| Concert migrations | Exact source manifest SQLx ledger, reapply and restart verify pass |
| Native lifecycle | PostgreSQL 16.14 restart persistence and cleanup pass |

Native report SHA-256:
`57bdfa656ad81f6497e2cc184936b6bf4d5d11f4144c2deeadf610674b278c51`.
Combined Cargo JSON SHA-256:
`02c2570e9559900fe7e3a0ae15417c34973b30a100bbd878a016bac19d7621bd`.

| Exact copied executable | SHA-256 |
|---|---|
| Concert library test | `1c372bc7d0924ec19a21710c36eb73d808be63cec2dce531cd3600ce859fe42f` |
| Auth migration binary | `d71a4778d1169b4d448819a5697bcd8a1887741ced386067447474827f50e464` |
| Concert migration binary | `3924402d32bf10c094bd8fe07187cc6afc49b3e5ab0224a52562ce3812476168` |
| Retained relationship test | `929700a42b461cf56a64af1fea0719933d8460fcb4078cf06f857838636d1b8e` |

The first three frozen executables are in `native-binaries-5/`; the retained
executable is `retained-relationship-4`. Native report 5 records the clean source
revision and hashes of all backend Rust/SQL/manifests/protos, the retained lockfile
and harness sources. The later handoff changes only this document.

Earlier failure receipts and binaries remain preserved. One test setup incorrectly
unwrapped a malformed `SessionId` already rejected by the retained type; the fixture
was corrected. The first native attempt correctly refused a database outside
Concert's required `mcr_concert[_suffix]` namespace; the owned fixture was renamed.
A later expectation incorrectly assumed restoring account ACTIVE resurrected its
session. The retained Auth `0006_atomic_auth_and_delivery.sql` status-change
trigger revokes that family; the final native test requires it to remain denied.
The final harness also retains redacted native diagnostics before failing.
Production Auth behavior was not changed to satisfy these fixtures.

All 154 previous immutable artifact hashes still match. Every owned fixture root
from the attempts is absent, and the release audit found no owned Cargo, rustc,
serving, test or PostgreSQL process retaining the target. The target was explicitly
released to Main before this documentation-only handoff; about 14 GiB was free.

Remaining `NOT_RUN` outside this delivered scope: production key/TLS provisioning,
World runtime wiring of this new client, WebSocket/multiuser and device acceptance,
distributed revocation fencing, profile/privacy/block owner composition, whole-task
acceptance and release. The retained two-owner native suite is not presented as a
fresh combined run. The persistent producer goal may complete only after the clean
committed handoff; no broader acceptance is implied.
