# BE-PARTICIPANT-LIVE

Status: IN_PROGRESS. This extends Phase A at `0313afa5`; it does not replace its
bounded CONTRACT_READY assessment or accept BE07/IN01.

## Ownership and source

The participant owner writes `src/participant/**`, participant RPC definitions,
dedicated participant tests/harnesses, and this evidence. Resource SL assigned
only `migrations/0027_participant_profile_coordinator.sql` as an additional shared
path; World retains 0026. Product startup loads the migration directory through
migration-runtime, without a numbered compile-time registry. The root composer
retains startup, routing, manifests and readiness probes. Canonical avatar
application/repository transaction hooks remain assigned to their original owner.

The current Auth adapter calls the existing Auth-owned
`AuthConcertRelationshipService.ReadCurrent` with a dedicated Concert read key,
explicit workload credentials, exact self actor/event/session/request binding,
SAFETY_SELF purpose, and the original two-second wall/monotonic deadline. It
requires a fresh exact no-store owner response and active account/session facts.
It implements no mutation capability. Network fixtures exercise the real guards
and client transport, but deliberately synthetic owner replies are not native
Auth or public-route acceptance.

The participant context retains the platform-sealed original parent when the
platform captures it. It checks the exact RPC literal, actor and complete unary
protobuf. Access for live guard acquisition refuses a missing parent. The guard
owner must add the three exact Gateway participant RPC literals to parent capture;
this participant patch does not modify platform allowlists.

The full C03 parser preserves absent versus explicit-null bio, permits selected
profile fields and/or preferences, and rejects duplicate and unknown properties.
The verified envelope now recognizes the full syntax. Until canonical SELF and
coordinator orchestration are composed, profile edits explicitly return dependency
unavailable before any owner call or preference write. No public header or
expected_revision key is changed. Canonical Profile retains its established
nonblank, no-control-character content rules; DTO parsing does not fabricate a
successful canonical value that its owner refuses.

## Cross-store protocol under implementation

The Profile SELF owner supplies canonical content plus epoch/revision, prepares
only selected nickname/bio fields, and retains the actual prepared transaction.
Its deterministic GID is `mcr_sp_` plus the request UUID's lowercase simple form.
The planned effect digest is SHA256 over ASCII `mcr profile self effect v1\0`,
the 32-byte guard-binding digest, the 16-byte operation UUID, the 16-byte Profile
epoch, its big-endian i64 revision, and SHA256 of the exact raw PATCH body.

Migration 0027 is a durable operation registry under implementation. It fixes the original
binding, expected participant revision, Profile version/GID/effect, and actual
Concert transaction identity before either store's protected effect. Only the
actual Concert participant CAS transaction may write its COMMIT decision.
Profile reconciliation must independently query the authoritative committed
decision. A registered aborted Concert transaction can establish ABORT; timeout,
absence, an unknown transaction, or a caller boolean cannot. Unknowns retain
prepared effects and owner exclusion. The SQL trigger additionally requires the
COMMIT writer's full XID, system identifier, database OID, PID and backend start
to match the registered native transaction.

The registry now also records an immutable prepare-attempt marker durably before
the actual Profile RPC, after locking and checking the participant revision.
Attempted/unsettled operations and committed decisions without an authenticated
Profile terminal receipt gate subsequent participant access. An aborted stale CAS
without a prepare attempt cannot have changed Profile. Visibility checks reject a
snapshot older than a later coordinated committed revision. Live reads still need
the local-snapshot, fresh-Profile, final-gate ordering when composed. The real
Profile client alone will construct the internal terminal receipt after validating
the canonical response; an HTTP boolean cannot clear this gate.
The exact original owner-binding protobuf is also persisted without the token or
raw Profile text. Recovery checks its canonical fingerprint against the immutable
registry and uses it only to query an existing operation with fresh bounded system
credentials. It cannot acquire a renewed read/write grant from the expired binding.
The journal's product constructor now requires the actual validated deployed
release; begin rejects a different request release before effects.

The new Rust journal captures that identity from its own actual transaction,
registers it before protected effects, durably inserts PENDING on a separate
connection, and locks event/admission/participant before the stale-revision check.
It writes the participant CAS and decision in that actual transaction. The
dedicated Profile-only ResolveDecision RPC reads the exact operation binding and
this store's actual pg_xact_status; a new recovery query cannot create an operation
or choose an outcome. The independent Rust Concert half is qualified below; its real cross-store composition remains unfinished.
The actual held-owner adapter, real Profile prepare/reconcile, cross-store crash
and visibility proofs, and public route remain unfinished.

## Validation

Independent checkpoint `4c84ca8f82737b9476888a3bb01446f83be82678`:
Rust 1.88.0, locked/offline, jobs=2, debug=0, incremental=0. The exact bounded
`cargo test --manifest-path backend/Cargo.toml --locked --offline -j2 -p
concert-service --lib participant::live --no-run --message-format=json` exited 0.
The exact Cargo-emitted test executable was copied and SHA256 checked as
`d284cf849d7e8d6fcda79ee17bd547078de2823606041fc136ac6fff1cabfc2b`.
The MEDIA build target was explicitly released before running that private copy.

Four focused tests PASS: two actual-network Auth client tests (14 exact/fault
owner replies, wrong credential and original deadline) and two C03 parser tests.
The stale Auth fixture was corrected to three seconds old after the earlier
one-microsecond case failed to establish staleness. The coordinator test's response
body mapping was corrected after its initial compile failure. No receiving
security check was relaxed. Focused output and source/artifact pins are in
`participant-live-evidence/focused-4c84ca8f.txt` and
`participant-live-evidence/coordinator-binary-4c84ca8f.json`.

Command `python3 backend/scripts/mcr/participant_coordinator_sql_smoke.py
--pg-bin /opt/homebrew/opt/postgresql@16/bin --binary <private-copy>
--receipt <exact-cargo-receipt>` exited 0 against private PostgreSQL 16.14.
The actual guarded Rust Concert coordinator native test PASS: original signed
C03 context, same-connection identity registration, event/admission/participant
locks, stale CAS refusal before prepare, profile-only preference preservation,
actual UPDATE followed by revocation rollback, COMMIT/ABORT decision queries and
foreign-body UNRESOLVED. A Concert commit without a canonical Profile terminal
receipt remains invisible. Owner eligibility and Profile prepared replies in this
test are explicitly synthetic fixtures; it does not qualify actual owner stores,
Profile prepare/reconcile, or a public HTTP route.

The same run passed eight SQL checks, restart and owned cleanup. Every current
Concert migration applied. Another connection could not decide; the registered
transaction committed effect and decision together; rollback retained PENDING
without either effect or COMMIT; original binding and prepare/terminal records
were immutable; restart retained committed effect, decision and aborted status.
The SQL-only portion uses an explicit effect probe and synthetic Profile terminal
metadata. Full report: `participant-live-evidence/coordinator-native-4c84ca8f.json`.
Its legacy scope string describes only the SQL portion; the separate `native_rust`
field identifies the additional exact Rust test. The earlier SQL-only report is
retained separately. Four Python artifact-boundary tests also PASS.

The compile emits inherited warnings and unused recovery/terminal seams, because
the staged SELF client has not been exported. Warnings were not suppressed.
The real held-owner adapter compiles against published SDK `888cb6bd`; actual
Auth/Friend native qualification and its use in a live participant flow remain
separate obligations. Qualified canonical avatar hooks `60991c53` are consumed
without changing their implementation. The typed SELF client in `live/profile.rs`
is unexported and NOT_RUN until coherent producer/proto-generation inputs arrive.

The additive decision IDL alone is published as `43cb9e9f`, CONTRACT_READY.
Vendored protoc exit 0; source SHA256
`8f5df61f1cdce1ed7e386da17a7ed8e3ea914eb24a117a39a85ec17cae26d6b9`, descriptor
SHA256 `639b449a2f27eee0d221cef74435afd14cc1389f6b77a1e91a445130cf7ae2e0`.
The root-owned response delivery extension is agreed but not imported here.

Actual Profile client/terminal/recovery, full live producer orchestration,
root/Gateway composition, real save/reload and browser/multiuser acceptance remain
NOT_RUN. The persistent full Live goal remains active. Phase A validation does
not certify these new paths. Required completion still includes implementation,
meaningful native/public-route validation, committed non-force delivery,
documentation and concrete fixes.

## Next source checkpoint (not yet qualified)

`live/service.rs` stages the live producer with concrete Auth/SELF clients, the
actual CanonicalOwnerFence, Journal and a root-supplied delivery publisher. It
acquires against the sealed original C03 context, publishes delivery before
protected work, uses the canonical avatar effect fence, coordinates full Profile
PATCH, and joins local snapshot -> fresh Profile -> final visibility gate. It
attaches only the internal typed delivery capability and response-scope extension;
it never certifies serialization or owner completion on return/error/Drop.
`live/recovery.rs` stages a restartable worker over persisted original operations,
using bounded terminal queries and keyset pagination so old unknown operations
cannot starve newer settlements. These files remain unexported and NOT_RUN.

The external decision endpoint fixture now uses the real guarded handler with
explicit production-format dedicated keys, private PG ownership checks, loopback
readiness, an owned stop marker and a bounded lifetime. The native harness will
run it and its signed generated client in separate copied-binary processes.
Additional native journal assertions cover recovery of the original binding,
terminal receipt agreement/idempotency, settled visibility and stale snapshot
refusal. These additions are NOT_RUN until the next allocated compile/native
turn. Neither synthetic terminal metadata nor an endpoint fixture qualifies the
actual canonical Profile producer. The SELF generated include now uses Prost's
escaped filename `profile.self_.v1`, while the wire package stays `profile.self.v1`.
