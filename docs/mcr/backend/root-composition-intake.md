# Root composition intake

Status: IN_PROGRESS. Combined component qualification passed at `34fa7a6d`.
Full production composition, complete IN01 and project acceptance remain open.
The actual sustained two-player/reconnect slice passed; customization remains open.
Existing immutable qualifications retain their exact source pins.

## Current-published World replay after CR10

The [fresh World receipt](root-world-eb02-attempt10.json) records published backend
`eb02a2e1`, qualified binaries `34fa7a6d` and actual browser App `e2375727`.
All 3,524 recorded inputs remained unchanged, including actual baked migration
paths. Genuine signup, verification, login, C03 and the two-principal native World
probe passed before the browser received fresh private inputs.

Browser attempt ten passed 91.4 seconds with two live players, real transport-cut
recovery, peer withdrawal, a new ticket/stream, the same canvas, and one active
Phaser instance and World socket per context. Fresh login, room entry, movement,
six panels and theme changes passed. The committed consumer packet is
`0793aaa20cfe2c4741fa5a9367fde6c43fbb758c`; its report hash is in the receipt.

IN01 remains incomplete. Actor A received two actual GET `/me` 504 responses at
07:28:57.125Z and 07:28:57.171Z; the character editor offered no choices and sent
no avatar PUT. Actor B's later `/me` returned 200. Response bodies were not
retained, so there is no evidence of a malformed successful DTO. Existing logs
do not identify the timed-out stage. A read-only owned-database snapshot at
07:31:41Z found no prepared transactions or active waiters, all 19 Auth/Friend
scopes completed and settled, 17 serialized/delivered roots finished and two
unserialized failed roots finished. This differs from the earlier incident.

Expiry-triggered refresh was NOT_RUN: a fresh login's actual 900-second access
lifetime exceeded the remaining window, and neither a one-shot 401 control nor
an App reload-resume path existed. No clocks, TTLs or tokens were changed. The
NOT_RUN expiry observation is informational and does not block original IN01.
There is no required 900-second duration, automatic renewal, proactive refresh
or cookie/reload restoration. The current original blocker is `/me` availability
and the resulting failed customization.

The browser fully released its resources at 07:31:23Z. The backend ended normally
at its original expiry, 07:36:38.785173Z, with exit zero, graceful Concert stop,
cleanup PASS, its exact owned PostgreSQL root absent and owner process absent.
Maximum sampled whole additional allocation was 143,589,376 bytes below the
1.5 GiB bound. No target was accessed, prepared transaction manually settled,
lease extended or retry silently launched. Cleanup does not accept the failed
customization flow or prove recovery.

## Participant read-lock diagnosis

Three [bounded native diagnostic runs](root-c03-me-diagnostics.json) reused the
qualified source/runtime inputs after the failed browser attempt. All 44 actual
HTTP reads returned 200: 14 without World connections, 15 with two real World
connections and normal movement, and 15 while both connections sent repeated
zero-axis inputs. The last run targeted 15 Hz but achieved 11.268 Hz per actor;
it does not qualify exact 15 Hz behavior. All three workflows released normally
with zero prepared transactions, delivered roots and owned cleanup PASS.

Without World connections, header latency was 82–303 ms. With World connections,
it increased to 675–924 ms. Closed query-class and blocker sampling identified
161 lock-wait observations of read-only Participant queries requesting
`FOR UPDATE` on `concert_events`, blocked by actual World transactions holding
share locks across event, participant, admission, avatar and membership rows.
This is observed unnecessary contention, not reproduction or resolution of the
original browser 504. The [RED/GREEN qualification](root-participant-read-share.json) now records an
actual failing regression at `c39a74ab` and passing repair at `7cd1e900`. Read-only
Participant checks use share locks across four checked rows; mutations and World
revocation fences retain their existing locking requirements. All five native
cases passed, including observed blocking of writers on each retained fence and
actual PostgreSQL restart persistence. Three pure tests passed separately.

The fresh production Concert main then passed genuine Auth/C03 and two-principal
World checks with 15 concurrent public `/me` reads, all 200, header latency
150.560–328.729 ms, no sampled lock waits, 31 delivered/finished roots and zero
prepared transactions. Each actor sent 264 zero-axis inputs at an achieved
14.9852 Hz after actual movement. Both named resource workflows fully released.
This qualifies the native repair; the current App customization replay is still
NOT_RUN and IN01 remains incomplete. No deadline widening or frontend decoder
change was made.

## Current qualified checkpoint

The [combined receipt](root-native-34fa7a6d.json) records 18 frozen Rust artifacts,
eight successful locked/offline Rust 1.88 build phases and 34 successful copied
controller checks. Normal execution passed 78 Concert library tests, 138 Gateway
library tests, three CR10 regressions and eight Party wire tests. Three private
Notification library tests were deliberately filtered there and then passed with
the seven binding tests in the actual PostgreSQL fixture. The 46 ignored Concert
tests and two Docker-dependent filtered Gateway tests receive no normal-run credit.

Actual native checks passed for CR10 delivery retention, C03 cancellation, NB02,
the CU01 resolver union, Party, Operations including restart persistence, all nine
CR08 cases, all ten CR09 cases and its real Gateway HTTP check, and all seven
physical CR01–CR06 restart cases. Each component retains its documented limits;
this does not certify the complete application or all parent tasks.

CR10's genuine Participant reply became a safe 503 after its shorter disclosure
ceiling expired. Actual HTTP EOF completed the root and both Auth/Friend owners;
a genuine Auth refresh returned 200. Unopened and partially consumed responses
remained serialized, undelivered and fenced beyond expiry. This qualifies the
bounded regression, not exact original JWT timing or a sustained browser replay.

The initial shared-target attempt at `c15cb5cf` failed because cached build scripts
mixed protocol paths from two checkouts. Root corrected four build scripts in
`591d908a` and controller lease/teardown boundaries in `a6892aac`. Auth, Friend,
Notification, Gateway and Concert production mains were rebuilt; retained Post
and Profile mains passed independent compile-input and live baked migration-path
comparisons before and after runtime. The receipt records the two deliberate
controller postimage changes after the earlier 80-path intake.

One external allocation observer interrupted fixture deletion, and a later
external launcher omitted the script import directory. Both failed attempts are
preserved. Exact successful retries used the same frozen source/artifacts and
original resource window. The partial owned directory was removed only after
identity, process and zero-handle checks; no manual prepared rollback occurred.

The shared target was released with zero handles at
`2026-11-04T06:56:59.607374Z`, before all copied tests and PG fixtures. Full workflow
release was `2026-11-04T07:06:29.607557Z`; every recorded owned root and runtime
process is absent. Maximum sampled whole additional allocation was 1,534,631,936
bytes below the 2 GiB bound. No released target was reread. Cleanup is distinct
from native settlement, especially for the expected negative quarantines.

CR10's eight and CU01's thirteen assigned postimages match their committed
handoffs exactly. CU01 composes actual Participant, Social, Operations and Age
native resolvers with an abort-only fallback for unsupported bindings. A selected
adapter's UNKNOWN/error never tries another adapter. Production main mounting,
the complete typed child-owner factory and supervised child-recovery activation
are still separate unfinished work. The Notification target-policy provider and
its successful public inbox flows remain NOT_RUN.

The canonical World input remains map four `mcr_reference_scene_r3` revision four
and the selected static avatar representation. V10/map six stays NOT_ADMITTED.
The failed earlier current-master IN01 is not accepted by this backend result.

## Earlier source checkpoint after the serialized-response incident

The following records the earlier intake and incident state before the combined
qualification above. Its NOT_RUN statements are historical, not new results.

The [source intake receipt](root-source-intake-20261104.json) records six later
packets over 80 distinct paths: CR09 Product cancellation, CR01–CR06 physical
recovery, Party runtime, NB01/NB02 Notification binding, Operations, and V10
spatial qualification. It lists each original handoff, delivered local ancestry,
source/destination blobs and SHA256, and the two deliberate Operations composition
changes. The original Operations document hash is retained; its machine-local
evidence prefixes are now portable. Source originals are unchanged. Eighteen
Python files parse, and eleven imported JSON files parse. These checks do not
compile or qualify the combined runtime.

CR09's ten actual PostgreSQL/mTLS cases and its public HTTP check passed on its
original compiled source `c1fd3ec8`. NB02's seven integration and three private
library tests passed with actual PostgreSQL on `d7ca2faf`. The other component
results retain their original source pins and limitations in their own documents.
None is relabeled as a passing build or public journey on this combined source.
V10 remains `CONTRACT_READY` / `NOT_ADMITTED`; the canonical World input remains
map four with the selected static avatar representation.

The [participant incident receipt](participant-delivery-incident-20261104.json)
records a distinct failure during the fresh current-master frontend replay.
Initial real Auth/Profile/C03 and two-client World probes passed. Later, a C03
GET returned 401 and a genuine Auth refresh returned 504. Read-only inspection
found two `ACTIVE` roots already serialized but lacking delivery completion, with
two Auth and two Friend prepared fences. Auth refresh rotation timed out waiting
for the canonical relationship revision row. No live blocking backend PID was
present in the later snapshot; that does not release a prepared fence.

The owned runtime expired normally at `2026-11-04T06:09:03.072023Z`, with exit zero,
no guard abort, exact private PostgreSQL root absent and owner process absent.
Its sampled additional allocation was 146,251,776 bytes below a 1.5 GiB ceiling.
There was no manual prepared rollback or runtime extension. This proves cleanup,
not recovery. The initial backend checks and earlier frontend-cohort pass cannot
accept the failed current-master replay; IN01 remains unfinished.

CR10 owns the isolated Gateway participant delivery-retention correction. Source
inspection identified post-gRPC identity/context checks and a route access check
that can reject a serialized reply before retaining its delivery capability.
This is a concrete hypothesis requiring a deterministic regression. The fix must
retain only authenticated valid capabilities, hide expired content, and complete
only after a genuine terminal HTTP response EOF. Dropped, partial, unknown, or
unwitnessed work must remain fenced. A real subsequent Auth refresh and negative
body-loss cases are required before another allocated browser replay. The
production typed resolver union, complete factory composition and relevant fresh
combined validation also remain open.

## Reviewed motion avatar v2

Producer base `980fc4d8`, required candidate Rust prerequisite `83b23afd`, and
ordered payload through `5f426f8e` were integrated serially. The implementation
and native-test paths match the qualified producer source `917b2d80` exactly;
central domain exports preserve the Root union. All eight handoff evidence,
schema, manifest and Gateway patch hashes were independently checked. The
producer receipt records 19 normal and six actual native PostgreSQL tests,
two physical restarts and clean owned teardown, with public activation NOT_RUN.
Its implicit extra Cargo binary is disclosed, not counted as a frozen/run output.

The exact bounded `gateway-response-v2.patch` SHA256
`d231d50a208adb660af78cb250f743536f82920d1953ca38648bb43acd00ea8f`
applied cleanly to the retained `3710a029` response blob `c1d01237`. It adds only
the closed reviewed-v2 branch to `avatar()` and three dedicated regression
groups; legacy response branches remain unchanged. Gateway compilation and
execution of those tests are NOT_RUN at this checkpoint. No active release,
operator registry, frontend implementation or admission input was switched.

Root source checks: direct Rust 1.88 rustfmt check passes; independent candidate
validation passes all 15 positive/16 negative fixtures, 38 resource occurrences,
source/raster checks and nine tamper refusals. The candidate remains denied for
production. The planning checker and whitespace check pass; neither is a
substitute for coherent Rust or actual public runtime qualification.

## Notification producer and remote child

The exact BE33 scoped patch from common base `819d30c3` to committed handoff
`b2554fd4` was integrated as one coherent packet, including the initial inbox,
actual producer SDK, child transport, native witness, migrations and both returned
Root match arms. `root-notification-intake.json` lists every path and exact worker
postimage versus deliberate Root composition. Four conflicts were adjacent module
exports/dependencies; both Root and Notification entries were retained. Existing
Party/Social/Age/Participant/C06 and CR07 source was preserved. Only the explicit
Notification producer mapping was added in the lifecycle adapter; CR08's separate
cancellation methods remain assigned to Canonical.

The original worker build qualifies five artifacts and 215 tests at `9bb4a98b`,
plus production Notification migration CLI and actual SQL/WAL probes. Root has
not rerun or relabeled those checks. Current Root source passes the three native
harness refusal tests, planning and whitespace validation. Coherent Rust compile,
actual canonical owner RPC, real target fencing, and the four public inbox flows
remain NOT_RUN. Missing providers stay unavailable; no product route was enabled.

## BE41 cleanup engine

The qualified engine packet from base `3471190d` through `eef18d5b` (tested
implementation `1f71e915`) was integrated over exactly 15 assigned paths. All
11 feature/test/script/migration postimages are exact worker bytes. Three shared
files retain all Root exports and add only their event-cleanup module; the worker
document substitutes portable artifact prefixes with original blob/hash recorded.
The copied native and freeze receipts independently match the reported hashes.

The worker's 14 passing tests and PostgreSQL restart qualify the durable batch
engine with explicit synthetic authorities/owners. They do not qualify actual
operator/system authority, reviewed retention, Mural freeze, consent expiry,
Post/media/archive privacy or World/community convergence. Full BE41 remains
BLOCKED. Root Python syntax and whitespace checks pass; coherent compilation and
actual production composition remain open. No main, public route, manifests or
existing owner lifecycle files were changed by this intake.

## CR08 and C03 cancellation

CR08 source `2e721109` is locally integrated as `e589f10e`; eight postimages are
exact and two preserve only the separate Notification subject/producer additions.
The implementation owner independently verified that composition. Documentation
receipt `8dd9c136` and the byte-exact portable `canonical-cancellation-core.json`
record nine actual isolated PostgreSQL/mTLS regressions at the original CR08 pin.
Four positive cases released actual fences; five negative cases retained actual
Auth/Friend prepared exclusions before owned cleanup. No expired, panicked,
unresolved, or lost-body case is relabeled as successful delivery.

C03 producer/test checkpoints `16f017f8`, `128ed564` and `f8aeca1f` were integrated
as `799ba5c6`, `30672f24` and `3654a72d`; all five assigned postimages match the
last owner checkpoint. The duplicate CR08 cherry-pick was omitted. Actual public
HTTP disconnect/fresh-login and unknown-work qualification is still in progress.
Root's combined compiler/native cohort and any fresh serving/browser evidence
remain NOT_RUN at this intake.

## Combined checkpoint at d0f063d1

The later [portable qualification receipt](root-native-d0f063d1.json) supersedes
the compilation and cancellation `NOT_RUN` statements above for the exact source
`d0f063d18a0549935f80a536077c30962d89f9e9`. It does not upgrade the separate
producer/public acceptance limits. All seven requested Concert/Gateway artifacts
compiled with Rust 1.88, locked/offline dependencies, jobs 2, debug 0 and
incremental 0. Five current peer MAIN executables were subsequently compiled at
the same source. Every executable was frozen and hashed before unconditional
zero-handle target release; PostgreSQL runs used only those copies.

The first actual CR08 case failed before producer work because a reused platform
test artifact rejected the current matching Auth/Friend owner policy. Source
inspection confirmed that the fixture uses production guards and that all three
current policies already match. An exact `platform` package clean and rebuild,
without any source or policy edit, made all nine actual PostgreSQL/mTLS cases
pass. Four positive cases had no remaining prepared fences; five negative cases
retained one Auth and one Friend fence before owned cluster shutdown. Their
shutdown is cleanup evidence, not settlement. The first failure remains recorded.

During that repair an external allocation observer raced deletion of a compiler
temporary file. Its exact owned compile group was stopped, the failed attempt
was retained, and the observer gained bounded retries and unconditional process
cleanup. The original allocation clock and baseline were retained. The receipt
does not claim an uninterrupted sampled peak over that interruption.

The first C03 attempt then stopped before service startup: the retained Auth
MAIN had 17 embedded migrations while this source requires 18. The old peer
cohort remains immutable. Exact Auth/Friend/Notification package repairs and five
current peer MAIN builds produced a coherent cohort; no Participant source fix
was needed. All six peers passed actual migration verification and restart, with
ledger counts Gateway 17, Auth 18, Profile 9, Friend 18, Post 31, Notification 6.

The coherent public HTTP run passed three real email-auth accounts, Profile and
bootstrap, normal Participant HTTP 200 with EOF settlement, a real TCP reset
after durable publication, and fresh Auth login HTTP 200 after safe cancellation.
The safe read was abandoned and settled with serialization/delivery still null.
A held callback stayed SEALED, and a registered actual COMMIT without its witness
remained UNKNOWN and unsettled. Their four negative prepared fences remained
until exact owned shutdown. All peer/mail/Redis stops were non-forced exit 0;
PostgreSQL cleanup, independent owned-directory absence and process absence pass.

The repaired copied tests pass 3 canonical constructors, 3 reviewed-motion
regressions, 78 normal Concert tests, and 138 normal Gateway tests. Two existing
Docker-backed Gateway checks failed on the unavailable Docker socket in the
first complete run; the repaired run explicitly filters those exact checks.
Those two checks are not credited as passing. The candidate motion remains
denied and the map4/static-avatar1 World baseline is unchanged.

The general typed child-settler/native resolver union, production main worker
supervision, unavailable product providers, spontaneous browser cancellation and
renewed IN01 remain open. No full MCR acceptance is claimed by this checkpoint.
