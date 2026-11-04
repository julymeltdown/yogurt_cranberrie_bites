# Participant native effect witness

Status: CONTRACT_READY for the frozen ABI; implementation/native qualification
of the new repository hook and settlement resolver is NOT_RUN.

Root explicitly assigned migration
`backend/services/concert-service/migrations/0035_participant_effect_witness.sql`
and participant-owned resolver/tests to the Participant owner. The Avatar owner
alone edits the application trait, avatar repository, existing avatar tests and
avatar documentation. Frozen migration SHA256:
`113900167fdc2806a9f5126c508b8ae737bcc9ca718ce7dbf408e913d1c3bebc`.

`AvatarEffectFence::witness_binding()` returns immutable correlation, never
authority or a commit assertion. Its default is Unavailable. The descriptor has
four UUIDs (`producer_instance_id`, `scope_id`, `operation_id`,
`session_family_id`), four exact nonzero `[u8;32]` digests (`parent_digest`,
`guard_binding_digest`, `body_digest`, `effect_binding_digest`), and a 64 lowercase
hex `release_digest` equal to the execute-time release. The held sealed scope
supplies these values. No boolean, new scope, SQLx type or private Profile content
crosses the application boundary.

The repository captures the actual writer tuple and registers/confirms it with
the existing held scope before effects. It validates the descriptor once before
effects. The actual successful UPDATE is followed by the witness INSERT on that
same SQLx transaction, with authority/fence checks before and after the await and
immediately before commit. Every successful avatar or preferences mutation
advances revision, including identical values, and requires a witness. Reads and
explicit synthetic `None` fences produce none. No ON CONFLICT success shortcut.

The INSERT has exactly these 20 bound parameters, in this order:

| Position | Column | Rust / SQL type |
| --- | --- | --- |
| 1 | operation_id | UUID / uuid |
| 2 | actor_id | actual repository user UUID / uuid |
| 3 | event_id | actual repository event UUID / uuid |
| 4 | session_family_id | UUID / uuid |
| 5 | producer_instance_id | UUID / uuid |
| 6 | scope_id | UUID / uuid |
| 7 | parent_digest | exact 32 bytes / bytea |
| 8 | guard_binding_digest | exact 32 bytes / bytea |
| 9 | body_digest | exact 32 bytes / bytea |
| 10 | release_digest | 64 lowercase hex String / text |
| 11 | effect_binding_digest | exact 32 bytes / bytea |
| 12 | identity_digest | exact 32 bytes / bytea |
| 13 | mutation_kind | `AVATAR` or `PREFERENCES` / text |
| 14 | expected_revision | actual validated expected i64 / bigint |
| 15 | committed_revision | actual resulting i64 = expected + 1 / bigint |
| 16 | concert_system_identifier | actual canonical decimal String / text |
| 17 | concert_database_oid | actual u32 bound as i64 / bigint |
| 18 | concert_full_xid | actual canonical decimal String / text |
| 19 | concert_backend_pid | actual i32 / integer |
| 20 | concert_backend_started | actual DateTime UTC / timestamptz |

`created_at` uses the database default. The table is
`public.concert_participant_effect_witnesses`. Identity hashing uses the existing
`platform::actor_delegation::owner_binding::identity_digest` with producer
`CONCERT`, the actual five-part writer tuple, the descriptor effect digest,
producer instance UUID, and `operation_id.as_bytes()` as settlement reference.
The scope UUID is joined separately and does not change that canonical recipe.

The participant avatar effect recipe remains SHA256 of the concatenation:
`mcr participant avatar effect v1\0`, guard digest (32 bytes), release digest
(64 ASCII bytes), raw body SHA256 (32 bytes). It is distinct from the guard
digest. Both avatar and preferences use this recipe. The Profile coordinator
keeps its existing separate expected Profile effect recipe and 0027 journal.

0035 joins the immutable exact root scope/guard and registered identity through
foreign keys. Its INSERT trigger requires the actual executing native tuple,
published ACTIVE scope, confirmed owner tracks, exact immutable root binding and
transaction identity, and target revision/xmin from an actual same-transaction
write. UPDATE and DELETE are refused. Root fixture rows in a component test do
not establish real owner authority. A settlement adapter must still check the
configured actual store, native commit status, full identity and all original
request bindings; raw native COMMITTED without this witness remains UNKNOWN.

This is a C03-specific contract, not a generic receipt or a Social/Post witness.
A pre-effect insertion in an authority check cannot prove a successful domain
effect and must not replace the repository hook.

## Actual joined native checkpoint, 2026-11-04

Source `a7ace884d216446872f1c93a23d0e62dcf0068ff` compiled with Rust 1.88,
locked/offline, two jobs, incremental disabled and dev/test debug disabled.
Successful Cargo JSON, build-input hashes, copied binary receipts and native
reports are retained privately under `/private/tmp/mcr-participant-join-a7ace884`.
The actual copied lib binary SHA256 is
`fbfcdc36521067cd451dbbb1f71592cf4bcbc1c9adf32a2668e76aae2c8b3e14`.
The named MEDIA target was released before native PostgreSQL execution.

`join-2-diagnostic.json` and `join-3-diagnostic.json` capture read-only state before
owned cleanup. Actual Auth/Friend/Profile main processes use their independently
qualified immutable binaries, dedicated signing keys, mTLS and private PG16.14.
GET, avatar PUT and preferences PATCH completed root delivery and both owner
leases. Avatar and preferences each committed a same-transaction 0035 witness;
Auth/Friend independently accepted those CONCERT transactions as committed.
This establishes an actual C03 witness/native-resolver prefix, not whole-flow
acceptance.

The fourth, full Profile PATCH remains **FAIL / IN_PROGRESS**: the actual Concert
journal decision is COMMIT, native committed, revision 2→3, while Profile remains
prepared and both owner leases retain its PENDING transaction. Root serialization
and ROOT_FINISHED are durable; delivery remains incomplete. Profile and Concert
operation IDs, guard/effect digests and prepared GID match. No private content or
credentials are needed for this diagnosis. All three services shut down normally
and the owned PG cluster cleanup passed on each run.

Static inspection identified a decision-handler deadline mismatch: Profile uses
a five-second wall ceiling while preserving its shorter incoming monotonic stop;
the Participant handler compared that query expiry with its already shortened
two-second wall ceiling and rejected it. The follow-up clips the query to the
verified original wall and monotonic bounds and the local two-second ceiling.
Its regression and the complete joined flow require a fresh qualified build and
native rerun; the causal fix is not yet runtime accepted. Public Gateway HTTP,
operator admission and full BE-PARTICIPANT-LIVE acceptance remain NOT_RUN/open.

## Qualified joined recovery follow-up

Source `6767ee8e805db17291d1d77a25c72d881e9f743a` corrects the deadline comparison
by using the minimum of the incoming query and verified original wall expiry,
while preserving the original monotonic stop and applying the two-second local
ceiling. The response echoes the unmodified query. The focused regression proves
a larger incoming ceiling succeeds without renewal, a shorter query still
shortens the call, and expired query/inherited stops fail.

Fresh named BACKEND compilation exited 0 in 31.16 seconds (zero errors, 28 retained
unused/dead-code warnings). The exact Cargo-emitted lib copy SHA256 is
`fc35b975663f4aa1b9061bbe2420987651df1b025c79c6a3255b74685ef5df18`.
The target was explicitly released with no handles before copied-binary tests
and private PG execution. Focused Participant checks: 8 passed, 4 native tests
ignored; integration checks: 8 passed, 2 native tests ignored; Python artifact
boundary checks: 9 passed. The exact joined native test then passed separately
in 5.28 seconds using actual Auth, Friend and Profile main processes.

The joined test proves actual canonical reads, avatar/preferences commits and
same-transaction witnesses, inline Profile PREPARE/COMMIT and save/reload within
the original request, exact native commit recovery and refusal of eight changed
native dimensions plus changed body, stale-CAS typed-failure settlement, and a
post-PREPARE timeout with actual Concert rollback and Profile ABORT reconciliation.
The aborted prepared content never becomes visible and participant revision
remains 3. Abort recovery is a real existing-operation recovery flow; this report
does not attribute it exclusively to Profile's five-second background worker or
the separately running Participant worker.

Six returned envelopes reached durable root delivery completion and completed
both actual owner leases. The seventh lost-envelope request remains ACTIVE and
unserialized at the root with SEALED owner leases and unresolved scope proof;
no timeout/drop fabricates serialization or owner release. All three actual main
processes shut down with exit 0 without force or unexpected exits, and owned
cluster cleanup passed.

Published evidence is under `docs/mcr/backend/participant-live-evidence/`:
`binary-6767ee8e.json`, `focused-6767ee8e.txt`, `integration-6767ee8e.txt` and
`joined-native-6767ee8e.json`. Original private receipts retain exact Cargo JSON,
local binary paths and source hashes. Auth/Friend qualified source is
`d4c604f03b4645b19cf6448526d4ff4577336f96`; Profile is
`a8776a61f853f4552825eb41e965f29f2274625e`. The report records exact peer binary
hashes and source-bound release SHA. Initial actor/session/admission rows remain
explicit test fixtures; the owner services, signatures, transactions, decision
query and terminal settlement are actual production implementations.

The minimal committed handoff is based on Avatar's production-only delivery
`2b67265370ac20155ce5eb82b923061e582c7c57`. All 201 other compiled source pins
match; the sole changed pin is Avatar's independently qualified controller
metadata-only artifact lookup, whose modified functions are not called by this
join. Preserve that fix. This does not claim a fresh build at the eventual root
integration SHA. Root owns final startup composition, combined validation and
public Gateway HTTP. Full BE-PARTICIPANT-LIVE remains IN_PROGRESS; public signed-in
Gateway HTTP and operator admission remain NOT_RUN.
