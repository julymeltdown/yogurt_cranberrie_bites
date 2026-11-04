# CU01 native resolver union

Status: CONTRACT_READY. The closed resolver component compiled and passed owned
PostgreSQL qualification at the exact source revision recorded below.
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
- `cargo +1.88.0 test -p concert-service --test native_resolver_union --no-run --locked --offline --message-format=json`: exit 0 at the qualified source below.
- Frozen artifact route/constructor tests: 3 passed, exit 0; owned native PG16
  case: 1 passed, exit 0 at the qualified source below.
- Combined Root activation, full parent acceptance, browser/multiuser flows and
  final remote publication: NOT_RUN by CU01.

The qualification receipt is committed alongside the support fixture. Root's
final publication and owned postimages must be verified before this chat's
persistent goal completes.

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

Source `45a2c8ca61fd428004098c640158dcfda15e7568` compiled at exit 0 in
2.013 seconds under `CU01-MEDIA3-202611040640Z`. The frozen executable SHA256
was `7518e88861c0a251c34d334cc20c358ffc8e62f17c9ab0c773c9840de56eb52c`.
Target release proved zero handles/processes before PostgreSQL. The controller
then stopped before any test: its database `mcr_concert_cu01` violated the retained
cluster helper's letters/underscore-only namespace policy. The earlier diagnosis
that `client_env(postgres)` caused this refusal was incorrect; that helper call
is valid. The follow-up uses `mcr_concert_cu`, without broadening the retained
cluster policy. Whole-owned cluster cleanup passed and its root is absent.
Neither route tests nor native qualification receive credit from this run.

## Exact successful qualification and handoff

Source `d20a1a2d239f14f9d2c3915a91db39a439c7aeca` passed under the fresh named
`CU01-MEDIA4-202611040645Z` allocation. Cargo exit 0 in 33.414 seconds produced
one requested executable (SHA256
`bca4ee184711660d43de22b4fdf33e723598fe709cd83d187c27030dd08a4b44`).
Rust 1.88.0, locked/offline, jobs 2, debug 0, incremental 0 and explicit
`--config profile.dev.package.contracts-build.opt-level=2` were retained.
The exact Cargo argument list, environment, artifact metadata and receipt hashes
are in `tests/native_resolver_union_support/qualification.json`.

The copied binary's three owner manifest paths were inspected and their live
SQL migrations matched the qualified source. All 2,249 complete compile/runtime
input hashes and 1,791 controller input hashes were independently reread, along
with the executable hash. Target release proved zero handles and no Cargo/rustc
processes before any copied tests/PostgreSQL; the target was never read afterward.
The successful controller command used the new support `native.py`, the exact
compiled checkpoint, PG16 tools, a new report, `--hard-whole-bytes 1288490188`
and `--fixed-target-growth 177496064` (including prior retained growth/logs).

The frozen artifact's default invocation (`--test-threads=1 --nocapture`) passed
all three route/constructor tests with the native case explicitly ignored. The
separate `--ignored --exact
native::actual_producer_witnesses_dispatch_without_unknown_or_error_fallback
--test-threads=1 --nocapture` invocation passed against owned PostgreSQL 16.14
and actual canonical owner mTLS services in 1.334 controller-measured seconds.
The migration ledgers matched all 18 Auth, 18 Friend and 37 Concert migrations.
Four producer-specific witnessed commits resolved COMMITTED; foreign instance,
system, database and effect bindings were refused. Actual open/aborted XIDs
resolved IN_PROGRESS/ABORTED. Unsupported unwitnessed COMMIT stayed UNKNOWN.
A selected Participant UNKNOWN did not fall through to the abort resolver; a
selected Social storage error remained UNAVAILABLE. Four expected unresolved
owner fences remained until whole-owned teardown; cleanup passed and the root
is absent. No settlement claim is inferred from that cleanup.

Maximum sampled whole additional allocation was 312,614,912 bytes, including
prior retained attempts, new target growth, frozen copies, receipts, the owned
three-database cluster, mTLS and temporary files. The hard limit was
1,288,490,188 bytes, with a 16 GiB start requirement and continuous 8 GiB stop
floor. Setup and tests were observed; no unsampled peak is inferred. All
resources are released. Original failed receipts/logs are retained with hashes.

Delivery ancestry is the exact base -> `4a7812f0` implementation -> `3520652d`
parsed request fixture -> `45a2c8ca` capture import fix -> `d20a1a2d` namespace
and setup observer fix -> this evidence/documentation successor. Production union
code is unchanged after `4a7812f0`. Root's separately granted build-script
prerequisite `591d908a85a01fe0a494e04c0109bb0bec64a3e1` arrived during the frozen
workflow and was not silently included or qualified by these results. Root owns
its existing integration and subsequent combined qualification.
