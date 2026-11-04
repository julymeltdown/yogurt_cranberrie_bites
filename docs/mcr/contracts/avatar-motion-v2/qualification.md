# Reviewed motion v2 producer qualification

The tested source is `917b2d8042e622e4b09a8d66f9fad39a4606b4c8`, based on
`980fc4d85020f297b36fee8195e1ca92079ff33f`. All 19 normal Rust tests and six
explicit native tests passed. This qualifies the producer's typed catalog,
resource binding and native persistence. Public release activation is NOT_RUN.

The exact source, contract pins, changed paths, payload ordering, executable
hashes and remaining gates are in `reviewed-handoff.json`; portable execution
receipts are in `qualification/`. The original visual receipt and all candidate
bytes remain immutable. The separate runtime receipt records later execution.

## Build and artifact boundary

BackendSL offered `C06-MOTION-MEDIA-20261104T0447Z` for the clean source pin. The
fresh preflight found 14.662 GiB free and zero MEDIA handles. The hard total new
compile/frozen/owned-PG allowance was 1 GiB. The four executable targets were:

- `concert-domain`: `avatar`, `avatar_motion_v2`.
- `concert-service`: `avatar`, `avatar_motion_v2_runtime`.

Commands used Rust 1.88.0, `test --no-run --locked --offline -j 1
--message-format=json`, the two exact test names per package, debug level zero
for dev/test, and incremental compilation disabled. Although the command selected
only integration tests and did not request a main/bin target, Cargo also emitted
a non-test `concert-service` binary automatically. This extra build artifact was
not frozen or run. Its disk usage is included in the guarded total below. The
final metadata audit exposed this deviation from the intended four-output scope;
it was reported directly to the allocator and integrator.

The domain build exited zero. Two Concert attempts exited 101 because the cached
`contracts-build` build-dependency variant still held another checkout's proto
root. A direct library rebuild exited zero but did not refresh that variant.
The successful Concert command added only
`--config profile.dev.package.contracts-build.opt-level=1`, forcing that package's
current source to be rebuilt and linked into its callers. No source change,
cache deletion, new target or lockfile change was used to resolve it.

The successful four-artifact build finished 124.8 seconds after the first build
started. Target growth was approximately 338.2 MiB; copied artifacts, metadata,
logs and the 4,335,486-byte source archive used another 61.03 MiB. Cargo JSON,
source archive and four checksummed executables were frozen outside the target.
The exclusive target was unconditionally released with zero open handles before
any copied test or PostgreSQL cluster ran. There was no subsequent target access.

## Executed checks

| Check | Result |
| --- | --- |
| Legacy domain decoder/catalog | 3 passed, exit 0 |
| Candidate and reviewed typed motion/catalog/resources/parser | 11 passed, exit 0 |
| Existing service preference/projection/witness checks | 3 passed, exit 0 |
| Real source-bound product constructor and candidate/wrong-root refusal | 2 passed, exit 0 |
| New native reviewed-v2 transactions and real restart persistence | 2 passed, exit 0 |
| Existing native legacy/selected transactions and real restart persistence | 4 passed, exit 0 |
| Both owned PG16.14 clusters and cleanup | PASS |

The normal service runs reported their six native tests as ignored. Those exact
six tests were subsequently executed with `--ignored --exact`; they are counted
only once in the six native results above, never as normal-run passes.

The new native runner was invoked with the frozen Concert Cargo JSON, exact
four-artifact receipt and PG16 tool directory. The existing `avatar_smoke.py`
runner consumed the same receipt and its exact legacy `avatar` executable.
Both runners created fresh marker/inode-owned private PostgreSQL clusters,
listened on private Unix sockets, applied all 28 migrations present in the
qualified checkout (including 0044), and removed their owned roots afterward.
Each migration and native command returned zero. Each runner stopped and started
its actual postmaster and verified a changed PID before the persistence read.

The new native flow stores and reads all 15 shared positive configurations through
Bootstrap and production avatar persistence. Twelve concurrent commands against
revision 1 produce exactly one revision-2 commit and 11 conflicts. An unfenced
mutation is refused, and the post-effect rejection rolls back before commit.
Nine malformed direct SQL updates advance the revision correctly, then fail with
SQLSTATE 23514 and the exact `concert_avatar_representation` constraint. This
avoids a false pass from the independent revision trigger. Two successful avatar
writes produce exactly two committed effect witnesses. A preset survives both
repository reconstruction and PostgreSQL restart, including Bootstrap projection.

Before migration 0044 the runner inserts one real legacy 1.0 row and one 1.1 row.
Their complete stored JSON, revisions and timestamps are identical afterward.
The existing native suite additionally verifies old layered/selected writes,
rollback, current authority/fence failures and restart behavior.

The domain tests verify actual pinned bytes and resource tamper refusals,
all shared config fixtures, closed inventory combinations, required nullable
keys, raw duplicates and candidate refusal. The actual
`from_source_bound_files` constructor verifies the reviewed identity, visual
receipt, exact resources, existing source-bound map/themes and defaults. Its
negative tests reject the immutable candidate and an unbound manifest location.

## Limits and downstream work

Auth/Friend owner-scope registrations inside native tests are explicit component
fixtures. The real SQL effect-fence, transaction identity, CAS and durable witness
paths execute, but this is not real public Auth/Friend, HTTP/WS or operator
admission. Capabilities remain disabled in the fixture; no production registry,
main entrypoint or active event selection was changed. Full AR03/IN acceptance
is not established by these producer checks.

The gateway patch applies cleanly to committed baseline `3710a029`, preserves
all non-avatar response/status bytes and adds three dedicated regression groups.
Its compilation/execution is still NOT_RUN: Root owns applying it to the coherent
Gateway source and running that consumer's checks. It is delivered as an artifact
because this producer's old base lacks those Gateway module prerequisites.

Root must integrate the candidate Rust prerequisite `83b23afd` before backend
commits `b3060896`, `26e58013` and `917b2d80`. The complete historical source/docs
payload ordering is recorded in the handoff. Shared `0dd9f443` was already
published as `d839bc20` with the necessary FE registry consumers; its domain
mirror preserves concurrent Intro GET/policy-version entries. Shared source
publication alone was never runtime admission. Root retains coherent backend
integration, the Gateway consumer check and any later public activation.
