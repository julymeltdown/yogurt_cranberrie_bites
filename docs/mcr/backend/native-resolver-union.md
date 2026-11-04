# CU01 native resolver union

Status: IN_PROGRESS. Source implementation is present; compilation and native
PostgreSQL qualification are NOT_RUN pending a fresh named resource allocation.
This is a bounded BE15/BE44 contribution, not acceptance of either parent task.

## Scope and provenance

Base: `845557272fae890dcb1ba7b6924ec4a5a07ffcfb` in the isolated
`codex/mcr-be-CU01` branch. Production changes are limited to the new
`backend/services/concert-service/src/config/native_resolver.rs` and its public
module declaration in `config/mod.rs`. No handler, runtime, recovery loop,
activation flag, listener, manifest, lockfile or contract changes are included.
Root owns integration and activation, including subsequent Intro attachment.
`docs/source/` remains unchanged.

The actual existing adapters are composed without changing their behavior:

| Signed parent and operation family | Selected adapter | Commit evidence retained |
| --- | --- | --- |
| Participant SetAvatar/UpdatePreferences and exact matching HTTP operation | ParticipantNativeResolver | Existing immutable avatar-effect/profile witnesses and confirmed registration |
| Product Dispatch and 13 exact Social operation/routes | SocialNativeResolver | Existing immutable same-transaction Social witnesses |
| Product Dispatch and 14 exact Operations operation/routes | OperationsNativeResolver | Existing immutable Operations witnesses |
| Product Dispatch and four existing scoped Age operation/routes | AgeTransactionResolver | Existing scoped Age transaction witness |
| Other valid bindings | PgNativeAbortResolver | None; a committed XID remains UNKNOWN |

All five concrete fields receive clones of one configured pool and the same
non-nil instance. A closed pool or foreign configured database namespace is
refused. The pool must explicitly target `mcr_concert` or a lowercase
`mcr_concert_` test/isolated suffix. Constructor checks do not establish live
database identity: each actual resolver retains its native system/database,
instance, registration and producer-evidence checks. Root must use this same
pool/instance for its canonical ledger and registry.

Selection validates OwnerBinding and uses the immutable signed parent method,
operation, HTTP method and event target. Operation families are disjoint.
Participant and Age targets are exact, Social uses its retained route parser,
and Operations mirrors only its existing supported routes. Query bytes remain
signed; producer validation is not replaced. The Age erase operation is an
existing scoped-authority operation, not a claim that AgeRuntime publicly
dispatches a new route. Selection calls exactly one adapter and directly returns
its result, including UNKNOWN or error. There is no fallback chain or new
commit-status inference. Guard and effect digests remain distinct.

Fixture copies are recorded with source commit, original SHA256 and destination
in `tests/native_resolver_union_support/provenance.json`. Auth/Friend RPC guards,
PKI/socket ownership and migration helpers retain their pinned implementations.
No new external dependency was added. Supplied artwork and historical source
documents were neither copied nor changed; the Participant fixture reads only
the selected source-pack manifest metadata.

## Qualification design and limits

The dedicated `native_resolver_union` integration artifact contains three
non-native tests covering the complete closed route table, cross-family overlap,
foreign/malformed bindings and typed constructor refusals. The ignored native
case requires the owned PG16 controller, actual canonical Auth/Friend RPC guards,
and actual producer witness writers. It exercises one witnessed commit from each
of the four adapters, foreign instance/system/database/effect refusals, actual
in-progress and aborted XIDs, an unsupported committed transaction, and selected
adapter UNKNOWN/error propagation without fallback.

Participant uses the actual preferences repository transaction and SQL witness
trigger with a fixture bridge forwarding a real held canonical scope's checks
and registration. Its signed parent and release correlation are fixture data;
this is not full GuardedParticipantService or release/bootstrap qualification.
Age and Social exercise their actual scoped transaction hooks; Operations uses
its actual runtime. Seeded identities/admissions are explicit setup, not signup
or browser proof. No fake scope or positive child resolver inserts commit
evidence. Child effects are unsupported and refused. The two intentional UNKNOWN
cases retain four owner prepared fences; destroying the whole owned cluster is
cleanup, never settlement evidence.

The controller pins committed Rust/SQL/protobuf/manifests, retained Cargo inputs,
selected manifest metadata, helpers, Cargo artifact identity and copied binary
SHA256. It bounds runtime, sampled whole additional disk use and free space,
retains failed logs, and verifies teardown of its exact owned PG cluster. A
sampled bound does not prove an unsampled peak. Compilation and native runs need
separate recorded commands, exit codes and allocation receipts. No shared target
or native PG resource may be touched without a fresh named OFFER/ACCEPT.

## Commands and current evidence

- `python3 scripts/mcr/check-plan.py`: exit 0, planning consistency only.
- `git diff --check`: exit 0 before source handoff.
- Rust 1.88.0 rustfmt on only the new Rust module/test/support: exit 0.
- Python AST parse of the new controller: exit 0.
- `cargo +1.88.0 test -p concert-service --test native_resolver_union --no-run --locked --offline --message-format=json`: NOT_RUN.
- Frozen artifact route/constructor tests and owned native PG16 case: NOT_RUN.
- Combined Root activation, full parent acceptance, browser/multiuser flows and
  final remote publication: NOT_RUN by CU01.

Qualification receipts and committed handoff ancestry will be appended after
fresh execution. Root's final publication and owned postimages must be verified
before this chat's persistent goal completes.

## Preserved first compile failure

Source `3520652d752df4894207f22fa39463c470f7784d` under allocation
`CU01-MEDIA2-202611040636Z` reached Cargo exit 101 in 37.76 seconds:
`E0432` in the copied test owner helper imported its old `capture` module name.
The production union compiled; this is not a successful artifact qualification.
The follow-up aliases `product_capture` in that copied helper. The original
fixture remains unchanged. Target release proved no handles and no Cargo/rustc
processes before ending the allocation; PostgreSQL was NOT_RUN. Retained target
growth was 81,723,392 bytes. A preceding private monitor preflight falsely
matched `.cargo` in unrelated process environments and started no build; its
failure receipt is retained, and the corrected monitor checks executable names.
