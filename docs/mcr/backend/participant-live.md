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

The new Rust journal captures that identity from its own actual transaction,
registers it before protected effects, durably inserts PENDING on a separate
connection, and locks event/admission/participant before the stale-revision check.
It writes the participant CAS and decision in that actual transaction. The
dedicated Profile-only ResolveDecision RPC reads the exact operation binding and
this store's actual pg_xact_status; a new recovery query cannot create an operation
or choose an outcome. These new Rust paths remain unqualified at this checkpoint.
The actual held-owner adapter, real Profile prepare/reconcile, cross-store crash
and visibility proofs, and public route remain unfinished.

## Validation

Auth source `ab01b9dc`: Rust 1.88 locked/offline jobs=2 debug=0 incremental=0
compile exit 0, exact Cargo-emitted library-test copy SHA256
`5b0427dbb72034bd15c7ba6253bc800abc5be243325cb09c87354941b3a1c715`.
An initial cached contracts-build path pointed to a different checkout; selective
`cargo clean -p contracts-build` repaired only that package. The subsequent compile
caught a fixture helper shadowing its function; that was fixed before the passing
build. From the frozen private copy: deadline/credential Auth test PASS; full Auth
reply-case test FAIL because the synthetic "stale" observation was only one
microsecond old and could still be fresh. Its fixture now uses an actual three
second-old observation; rerun pending. No receiving authority check was relaxed.
Both C03 parser tests PASS, including frozen-schema comparisons and duplicate keys.

SQL-only command `python3 backend/scripts/mcr/participant_coordinator_sql_smoke.py
--pg-bin /opt/homebrew/opt/postgresql@16/bin`: exit 0 on actual private PG16.14,
eight checks PASS and owned cluster cleanup PASS. All current Concert SQL applied;
another connection could not decide; the exact registered transaction committed
effect and decision together; rollback retained PENDING but neither effect nor
decision; binding/Profile receipt/terminal decision were immutable; restart retained
the decision/effect and actual aborted status. The prepare marker and terminal
receipt tables also refused UPDATE. Their Profile identity metadata are explicitly
synthetic structural fixtures. This exercises SQL using an explicit effect-probe
table, not the Rust participant implementation or actual Profile. Exact migration
and driver hashes are in `participant-live-evidence/coordinator-sql.json`.

Decision IDL at `3bb9c295`: standalone vendored protoc exit 0; source SHA256
`8f5df61f1cdce1ed7e386da17a7ed8e3ea914eb24a117a39a85ec17cae26d6b9`, descriptor
SHA256 `639b449a2f27eee0d221cef74435afd14cc1389f6b77a1e91a445130cf7ae2e0`.
This is syntax qualification, not server execution. The root-owned response
delivery extension is agreed but not imported into this checkpoint.

Canonical avatar hook source `2561baa7` was consumed locally as `7a8fab63`.
Its owner still owes native qualification. Participant MutationPermit and
CheckedAuthority now forward its effect fence; default None is refused by product
writes. The actual BoundParticipantAuthority adapter is staged in `live/guarded.rs`
but deliberately unexported pending the compiled SDK. It binds the exact sealed
parent, request, release and original deadline to the real OwnerGuardScope and
registers the repository's actual transaction. It owns no completion/release API.
This source is not a qualified real authority adapter yet.

New journal/resolver Rust compile and native application test: NOT_RUN. Actual live
owner mutation, root/Gateway composition, browser and full signed-in multiuser
acceptance: NOT_RUN. Formatting, `git diff --check` and the plan checker PASS.
Phase A validation in `participant-producer.md` does not qualify these new changes.

Required completion remains implementation, meaningful native and public-route
validation, committed non-force delivery, documentation, and concrete review fixes.
