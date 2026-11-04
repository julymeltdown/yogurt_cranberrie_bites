# C03 Gateway implementation

Status: CONTRACT_READY / IN_PROGRESS. Fresh Gateway build and controlled transport
checks pass at8888ff7a. Actual native owner join passes at3f763d4a with unchanged
producer inputs. Production startup mounting and public C03 save/reload remain
NOT_RUN; these component checkpoints do not certify that flow.

Root transferred these exact paths to Participant: new outbound/inbound
`concert_participant/` modules; `tests/concert_participant.rs` and its support;
this document; one additive isolated Participant/Decision proto pass in Gateway
`build.rs`; and one module export in each parent module. Root retains AppState,
configuration, manifests and root/router/startup composition. Backend only.
Base is qualified Participant handoff88de486f.

Construction:

- `GrpcConcertParticipantClient::connect(endpoint, token, tls, signer, release_digest)`
  validates a trusted configured release digest. It never reads release or actor
  identity from browser JSON or headers.
- `concert_participant::router(Arc<AppState>, Arc<GrpcConcertParticipantClient>)`
  returns the three C03 routes using its own typed state. Sealed HttpConcertAccess
  and actual configured Auth/Profile identity clients remain mandatory.
- Root mounts `protect_response` outside transport rejection layers and retains
  the actual outer RetainedScopeLayer to finish only terminal HTTP framing.

GET/PATCH `/api/concert/events/:event_id/me` and PUT `.../me/avatar` retain original
request bytes and authenticated identity/session. Browser mutations require an
allowed single Origin, browser intent and JSON content type, with an 8KiB body
limit. No idempotency key is invented for these optimistic-CAS operations.
Canonical owner reads and signed RPC dispatch share the original wall/monotonic
stop. C03 success disclosure checks exact object keys, nested public fields,
actor/event identity, revisions, preferences and the frozen avatar variants.
Unknown private fields or inconsistent failure code/reason pairs fail closed.

Success and closed typed errors retain the same internal completion token. The
capability never enters browser JSON/headers. Dropping/erroring a body does not
finish a scope. The actual response-wide disclosure ceiling guards each DATA frame, including
a closed typed error. Public activation still requires Root's actual production
composition and public-route validation.

The focused test target uses actual sealed JWT extraction and actual generated
signed client/guard transport with explicitly controlled Auth/Profile/Participant
peers. It covers forwarding, closed conflicts, EOF versus drop, private-response
refusal and pre-dispatch browser/auth/body refusal. It cannot certify a real
canonical-owner/public-stack flow. The reusable controlled identity fixture is
copied from `tests/concert_identity_composition.rs` at88de486f, without its test
cases or synthetic context constructor; production never imports it.

Root granted the exact response-wide disclosure extension after the draft:
ParticipantResponse field4 carries the actual scope ceiling, captured after
success or typed failure with the original scope Arc retained. Gateway refuses
absent, expired or extended ceilings and wraps both forms of response with the
shared DisclosureBody. The original C04 implementation and its three tests move
unchanged except crate visibility to `http/response_disclosure.rs`; the former
private C04 path is a narrow re-export shim. Party-owned C04 route code is
unchanged. Source provenance: Root published07791018 (which contains4d5c044's
qualified body guard and existing http-body workspace dependency). No new
manifest or lockfile change is required.

The new tests additionally cover actual C03 data withheld after an earlier
response ceiling expires and fresh EOF settlement after already consumed valid
data. The same original wall/monotonic bounds are retained. Production fields
are never copied into browser headers or JSON. The shared body guard is qualified by three focused checks and two additional
C03 DATA/EOF boundary checks. Gateway and Participant compile/native evidence
cover this additive internal-wire revision. Phase A/test response initializers
explicitly use zero, which the real Gateway refuses for public disclosure.

First qualification at3f763d4a: both exact Gateway and Concert no-run builds pass;
shared disclosure tests pass3, Participant focused8/integration8 pass, and the
actual three-main native join passes with the new field4 ceiling assertions,
COMMIT/save/reload/ABORT/owner-delivery/cleanup. The Gateway transport target
aborts on its first default-stack test. A diagnostic-only16MiB-stack replay of
that one case passes, which is not acceptance. The follow-up boxes large
identity/RPC/dispatch futures (as existing Gateway clients do) and test setup
futures. Fresh default-stack Gateway validation at8888ff7a passes all seven tests in
2.87s and the shared three tests; no production stack-size workaround is
introduced. The original compiled3f producer inputs are unchanged by this
Gateway-only correction (all206 recorded source pins match).


Qualification evidence is committed under `participant-live-evidence/`:
`gateway-binaries-8888ff7a.json` contains sanitized original Cargo artifact
metadata, executable hashes and source pins; `gateway-disclosure-8888ff7a.txt`
and `gateway-participant-8888ff7a.txt` contain the default-stack outputs (trailing blank lines normalized).
`gateway-native-join-3f763d4a.json` records actual Auth/Friend/Profile MAIN
migrations, signed canonical join, native COMMIT/save/reload/expired-PREPARE ABORT,
six root/owner completions, lost-envelope quarantine and all clean process exits.
The producer focused/integration outputs record eight passes each and explicit
ignored native cases; the separate actual native join supplies the positive
multi-process evidence. Private original compiler streams and unmodified
receipts remain outside the repository. No live build-target lease is held.

Reproduction (Rust1.88.0, locked/offline, jobs2, incremental0, dev/testdebug0;
use only a coordinator-allocated existing target):

```sh
cargo test --manifest-path backend/Cargo.toml --locked --offline -j2 -p api-gateway --lib --test concert_participant --no-run --message-format=json
cargo test --manifest-path backend/Cargo.toml --locked --offline -j2 -p concert-service --lib --test participant_producer --no-run --message-format=json
```

Both compile commands exited0. Gateway8888ff7a completed in29.17s with zero
compiler warnings; producer3f763d4a completed in39.57s. Copy the exact Cargo-emitted
executables outside the target and release the target before execution. Run the
Gateway lib copy with `response_disclosure::tests --test-threads=1` and the
participant integration copy with `--test-threads=1`; both exited0, using the
default Rust thread stack. The producer lib copy uses `participant::live::` and
its integration copy uses `--test-threads=1`; both exited0. The native driver
`backend/scripts/mcr/participant_live_join_smoke.py` takes the producer lib copy,
its original receipt, owner/Profile artifact manifests, PostgreSQL16 binaries and
an output report path. Run it against the exact3f source checkout with an
explicit isolated-PG allocation; it exited0. No broader release or browser
acceptance follows from these commands.

Remaining owner: Root composes actual Participant client/router,
`protect_response` outside transport rejection layers, and retained EOF scope
handling. Actual canonical Auth/Profile/Decision clients, supervised recovery,
release configuration and mTLS must be supplied by production startup. Validate
public signed-in GET, avatar PUT, preference/full-profile PATCH, reload, stale
revision denial, session/identity refusal and actual EOF owner settlement before
marking the assigned persistent live goal complete. No AppState, root mount,
configuration, manifest, lockfile, frontend or Party C04 route modification is
included in this handoff.


The production factory is committed separately at
`e7c0cbb8bb3be40d49f64f737c8ffe19cc29924c`, based on Root's `f2683a7c`.
Its compile/configuration status is CONTRACT_READY. A targeted read-only
Profile/Decision/recovery review found no actionable issues; fresh Rust build,
eight configuration tests and one key-role test pass. A fresh actual native
producer join also passes. Production factory mounting and the public C03 flow
were NOT_RUN at that checkpoint. The actual production HTTP qualification below
supersedes that status; earlier Gateway/native evidence does not certify its EOF
boundary.

Root calls `ParticipantRuntimeSettings::load(grpc_enabled)` before opening the
database and `settings.compose(pool, active_release)` with the actual
`Arc<ValidatedProductRelease>`. The returned ports are the actual
`GuardedParticipantService`, dedicated `ParticipantDecisionService` and verifier,
`ProducerSettlementService` backed by `ParticipantNativeResolver`, one coherent
`DeliveryPublisher`, and `ParticipantRecovery`, plus explicit caller/Gateway trust.
Root mounts the exact policies, retains response EOF handling and supervises
`recovery.run(shutdown_receiver)`. It must reject simultaneous optional Product
and Participant runtimes before opening the database: both cannot mount a second
Delivery/Settlement server or registry.

Required configuration:

| Setting | Required value |
| --- | --- |
| `CONCERT_PARTICIPANT_MODE` | `live`; absent or `off` creates no Participant runtime |
| `CONCERT_PARTICIPANT_AUTH_GRPC_ENDPOINT` | Explicit Auth HTTPS host and port |
| `CONCERT_PARTICIPANT_FRIEND_GRPC_ENDPOINT` | Explicit Friend HTTPS host and port |
| `CONCERT_PARTICIPANT_PROFILE_GRPC_ENDPOINT` | Explicit Profile HTTPS host and port |
| `CONCERT_PRODUCER_INSTANCE_ID` | Stable, nonzero canonical UUID |
| `CONCERT_PARTICIPANT_RELEASE_DIGEST` | Exact active release SHA256, lowercase hex |
| `CONCERT_PARTICIPANT_RECOVERY_KEYS_JSON` | Closed array of `{id, secret_hex}` records; first active, at most three retained, distinct nonzero 32-byte secrets and unique IDs |
| `GATEWAY_SERVICE_CREDENTIAL`, `AUTH_SERVICE_CREDENTIAL`, `FRIEND_SERVICE_CREDENTIAL`, `PROFILE_SERVICE_CREDENTIAL`, `CONCERT_SERVICE_CREDENTIAL` | Explicit distinct workload credentials; inbound `_PREVIOUS` rotation values remain distinct |
| `ACTOR_DELEGATION_VERIFYING_KEYS` | Explicit Gateway keyring for Concert audience |
| `CONCERT_RELATIONSHIP_SIGNING_SEED` | Dedicated canonical owner-read signer |
| `CONCERT_PROFILE_SELF_SIGNING_SEED` | Distinct dedicated SELF Profile signer |
| `PROFILE_PARTICIPANT_DECISION_VERIFYING_KEYS` | Distinct dedicated Profile-to-Concert Decision keyring |
| `CONCERT_GRPC_TLS_CERT_PATH`, `CONCERT_GRPC_TLS_KEY_PATH`, `CONCERT_GRPC_TLS_CA_PATH` | Actual Concert mTLS material, required even in local mode |

The existing platform encoding applies to signer seeds/keyrings (standard Base64);
only the new at-rest recovery `secret_hex` field uses lowercase hex. The factory
rejects known parity identities and overlap among Gateway, relationship, SELF
and Decision roles. It creates no optional public/Gift/Social authority: C03's
child settler refuses unsupported intents and leaves them unresolved. Recovery
reuses the actual Profile client and persisted original binding, and never
reissues a mutation or renews admission. These configuration and native checks are qualified as described below; actual
production mounting and the public C03 flow remain required.


The callable public probe is
`backend/services/api-gateway/tests/concert_participant_support/public_probe.py`.
The isolated runtime launcher imports `run_pair(base_url, event_id, origin,
principals)` with exactly two actual `{user_id, access_token}` records obtained
from real login after Profile/bootstrap. Tokens stay in memory and are omitted
from the report; the client refuses redirects, retries no mutation, and reads
each response through EOF using a new HTTP client. It checks selected-pack avatar
CAS, preference CAS, canonical display-name/bio CAS, fresh reload, stale-write
conflict, unauthenticated refusal, refusal-state preservation and isolation after
both principals write. It requires private-response headers on success and error.
Syntax validation passes. Actual public execution passes all 18 HTTP checks and
the post-fix native settlement oracles recorded below. The earlier failing EOF
runs remain preserved. The launcher must
separately inspect native owner/root settlement and clean process/database
shutdown. The probe explicitly cannot certify those from HTTP status alone.


Fresh factory qualification at e7c0cbb8 used Rust1.88.0, locked/offline jobs2,
incremental0 and dev/testdebug0. The exact `concert-service --lib --test
participant_runtime_config --no-run --message-format=json` build exited0 in32.17s.
The first attempt exited101 because `contracts-build` retained another
worktree's proto path; cleaning that package alone removed20 files/816.4KiB.
The same unchanged source then built. There were27 retained unused/dead-code
warnings and zero new factory warnings. Original Cargo streams, all source pins
and three copied executables are retained privately. The target was released
and no open handles were confirmed before executing copied tests.

`runtime-binaries-e7c0cbb8.json` records sanitized Cargo receipts and hashes,
including all206 producer pins and9 runtime/test pins.
`runtime-config-e7c0cbb8.txt` records eight configuration passes;
`runtime-key-roles-e7c0cbb8.txt` records the role-separation pass. The copied
producer lib also passed the exact existing actual Auth/Friend/Profile MAIN join
through `participant_live_join_smoke.py`: canonical Profile COMMIT/save/reload,
expired-PREPARE ABORT, six root/owner completions and lost-envelope quarantine.
`runtime-native-join-e7c0cbb8.json` records all migration commands and clean peer
exits/owned-PG cleanup. This join uses its existing guarded native composition;
it does not execute Root's production factory mount or public HTTP journey.
No build target or PG resources remain held.

The pre-fix production C03 qualification below was IN_PROGRESS. Root integrated the
factory at `161a3bd6` and production startup at
`f2898a642848cbf015cb32664ce3a41b9d83314d`. Its copied production executable
built successfully, and both startup supervision tests passed. With the retained
actual Auth/Friend/Profile service binaries and Gateway `687df567`, three
isolated runs created two accounts through actual signup, email verification and
login, wrote their Profiles, and bootstrapped both participants. All 18 public
probe checks passed on each run, including selected-pack avatar CAS, preferences,
canonical Profile update, fresh reload, stale CAS refusal, anonymous refusal and
cross-principal isolation. These HTTP results do not certify native settlement.

The first run failed an incorrect SQL oracle. Canonical `profiles.bio` is
`text NOT NULL`; `BioPatch::Clear` stores the empty string, while the public
representation is `bio: null`. The correct storage assertion is the exact
expected nickname and `bio=''`, not `bio IS NULL`. This follows the retained
`profile-service/migrations/0001_init_profile.sql`,
`src/concert_self_profile/store.rs` and the clear assertion in
`src/concert_self_profile/native.rs`. No production behavior or public assertion
was weakened to correct the oracle.

The second run passed that canonical storage assertion but failed the immediate
root-delivery count. The third added a bounded three-second read-only settlement
observation and proved a transport defect: all 16 authenticated C03 scopes stayed
ACTIVE with serialization recorded, zero root/Auth/Friend delivery completions
were recorded, and 32 owner prepared transactions remained. Both actual Profile
COMMIT receipts and both Concert-native COMMIT proofs passed. All three runs
stopped Concert and peers without forcing termination and cleaned up owned
resources. No manual completion RPC, fabricated root finish or mutation retry
was used.

`public-c03-f2898a64-before-eof-fix.json` preserves sanitized original report
hashes, driver hashes, binary cohort hashes, HTTP steps, actual database counts,
and cleanup results. The shared retained body advertises an exact byte size;
Hyper HTTP/1 can exhaust that length and drop the body before polling the EOF
that starts completion. Root owns the shared framing fix, real-wire regressions,
fresh Gateway executable and exact public rerun. Full C03 acceptance requires
16 completed root scopes, 16 Auth and 16 Friend settlements, two canonical Profile
and Concert-native COMMIT proofs, no remaining prepared transactions and clean
shutdown in the corrected production run. World and browser acceptance remain
separate and NOT_RUN.

Post-fix qualification is INTEGRATED for the actual public C03 HTTP journey and
the recorded native settlement oracles. Root's shared framing fix is
`3710a029d083e4b5123df45b82867056f64b3c8b`. With Rust 1.88.0, locked/offline jobs 2,
incremental 0 and debug 0, the platform lib, real-HTTP test target and production
Gateway built successfully in 5.77 s, 21.44 s and 15.64 s. The named build target was
released with zero handles at 03:49:26 UTC on 2026-11-04 before copied tests or
PostgreSQL execution. All three copied platform response-scope tests and all
three actual HTTP/1 tests passed. The HTTP tests cover fixed-length and empty
scoped bodies, terminal completion, and unchanged unscoped framing.

The resulting Gateway SHA256 is
`857e0874dd6a62544e9104c79fc0701f0300b55772828cc978d4c5dcc6e6512b`.
The new retained cohort uses that Gateway, the same actual Auth/Friend/Profile
and Post/Notification MAIN binaries, and the same production Concert f289
executable. Both principals were again created through actual signup, email
verification and login, then Profile update and participant bootstrap. All 18
public C03 checks passed. Both canonical Profile rows contained the exact new
nickname and the cleared empty-string bio; the public representation retained
`bio: null`. Fresh clients reloaded the saved avatar, preferences and full
Profile; stale and anonymous refusals preserved state and principal isolation.

The independent database observations now record 16 ROOT_FINISHED scopes with
delivery completion, 16 persisted Auth scope-settlement proofs, 16 persisted Friend
scope-settlement proofs, two COMMITTED Profile terminal receipts, two committed
Concert-native Profile operations and zero prepared transactions. The Auth/Friend
oracle counts `scope_settlement IS NOT NULL`; this report does not separately
count each lease's `state='COMPLETED'` or pending participant rows. These are the
precise observed oracles, rather than an inferred observation of those columns.
The framing fix was exercised through actual HTTP terminal consumption, without
manual completion RPCs, fabricated root finishes or mutation retries.

Concert and every retained peer stopped unforced with exit 0. Owned database
cleanup passed; independent inspection confirmed the owned directory and all
recorded process IDs were absent. The original launcher result remains PARTIAL:
World was OFF, and World multi-client/browser IN01 checks were NOT_RUN. No overall
IN01, production deployment or release acceptance follows from this C03 result.

`public-c03-3710a029-after-eof-fix.json` preserves sanitized original report,
cohort, Cargo receipt and test-log hashes, all 1945 build source pins, exact
commands/results, the target release record, all 18 HTTP steps, database counts,
canonical-storage oracle and cleanup observations. It binds the tested driver
to its 3710 source revision; later World-driver edits do not amend the old run.
The pre-fix evidence remains unchanged. Root owns serial integration and remote
master publication; those delivery checks remain required before completing the
assigned persistent goal. This handoff changes only this document and its new
passing evidence JSON; no Cargo/PG or shared-file writes are performed here.

## C03-CANCEL01: interrupted producer work (2026-11-04)

This is a new regression assignment after the prior C03 delivery goal completed.
Its implementation baseline is Root `a0730a4e1cadd288a2137bf2e16bbb0bb7857aef`.
Status is IN_PROGRESS; the previously recorded normal-response qualification
does not certify cancellation. The two private Root incident reports show one
published `getConcertParticipant` scope still ACTIVE and unserialized, with no
tracked transaction or child intent, and one expired SEALED lease in each owner.
Two prepared owner transactions remained, and a subsequent Auth login timed out.
The old runtime's normal shutdown is cleanup, not cancellation recovery evidence.

The Participant handler installs Canonical CR08 `OwnerGuardScope::run_owned`
immediately after successful acquisition, before another suspension point. It
moves the verified context, current Auth observation, service clone and exact
scope into the owned future. All subsequent checks, authority construction,
durable delivery publication, local/Profile work, final checks and response
construction run there. Successful responses keep the existing retained body
lifetime. The Participant patch adds no global lifecycle, Drop release, TTL
release, synthetic EOF or DeliveryCompleted path.

CR08 owns the supervisor and migration 0048. Caller interruption only signals
that supervisor. Actual destruction of its owned work future, closed admission,
and drained callbacks precede durable `response_abandoned_at`; actual owner and
child settlement precedes `cancellation_settled_at`. Neither is HTTP delivery:
`serialized_at` and `delivery_completed_at` remain NULL. Unknown callbacks or
native outcomes retain exclusions and capacity rather than obtaining a receipt.

Dedicated tests are `tests/participant_cancellation.rs` and
`tests/participant_cancellation_support/{host.rs,native.py}` under Concert.
The host composes the actual canonical core, Participant handler, native resolver,
Bootstrap handler, settlement and delivery services. A deterministic publisher
fixture calls the real durable publisher, then pauses before handing its token
to serialization. The controller uses actual Auth signup, private email delivery,
verification, login, Profile update and public Bootstrap. It resets an actual
public Gateway C03 TCP connection after publication, checks root/owner state and
prepared transactions, then performs a new real Auth login. Separate admitted
callback and registered-native-COMMIT-without-witness fixtures must stay held.
The latter deliberately violates the producer witness contract to test refusal;
it is not a product write path or an invented positive native receipt.

The new controller reuses the audited local helpers from the baseline:
`backend/scripts/mcr/{dev_stack,migration_smoke,runtime_smoke,world_in01_prepare,
world_in01_live,participant_live_join_smoke}.py`. It preserves their owned
directory, marker/inode, PostgreSQL identity, mTLS, real principal, PID/port and
shutdown checks. Build receipts pin the controller dependencies and all Rust,
manifest, proto and migration inputs. The selected product release still uses
the source-bound constructor and supplied source pack. No source artwork,
`docs/source/`, shared contract, manifest, router or production composition is
changed by this patch.

Validation so far: Python driver AST parse and Rust 1.88.0 formatting pass;
`python3 scripts/mcr/check-plan.py` exits 0 for planning/provenance only. Cargo
compilation, native cancellation/login qualification and committed handoff remain
NOT_RUN pending the CR08 dependency and named resource allocation. Root owns
production composition, serial master publication and the browser IN01 rerun.

### C03-CANCEL01 native qualification

The qualification status is now INTEGRATED for the dedicated real native HTTP
fixture. The earlier NOT_RUN checkpoint above remains historical. Compiled
source is `f8aeca1f541fbb82777089e9281c333479c25f74`, containing Participant commits
`16f017f8`, `128ed564`, `f8aeca1f` and Canonical `2e721109` consumed as `c43b9bce`.
Root takes the Participant commits separately; it must not duplicate CR08.

Under named lease `C03-CANCEL01-BACKEND-202611040509Z`, the exact command was
`cargo +1.88.0 test -p concert-service --test participant_cancellation --no-run --locked --offline --message-format=json`
with jobs 2, incremental 0, and dev/test debug 0. Attempt one exited 101 in 4.37 s:
borrowed target artifacts retained another worktree's absolute proto paths and
old application metadata, despite the required exports existing in current
source. The failed Cargo JSON and stderr remain preserved. The authorized repair
cleaned only `contracts-build`, `concert-domain` and `concert-application` (66
files, 39 MiB), with no source changes. Attempt two exited 0 in 30.74 s. Exactly
one requested test executable was copied and verified outside the build target;
its SHA256 is
`9a0bb77d3bef921a5302c656e99804ef6d6ec4b372bec3d1a9d92612d20f0f73`.
The target was explicitly released with zero handles/processes before native
execution. The portable [qualification receipt](../../../backend/services/concert-service/tests/participant_cancellation_support/qualification.json)
contains all 1772 build/controller source hashes, exact contract hashes, peer
binary hashes, command results, incident/report hashes, and limitations.

The copied native controller exited 0 in 23.73 s against the frozen 3710 Gateway
and actual Auth, Friend, Profile, Post and Notification MAIN processes, using a
new owned PostgreSQL 16.14 cluster and fresh mTLS keys. Three independent users
completed actual signup, delivered email verification, login, Profile update and
public Bootstrap. The normal public C03 GET returned 200 and completed its
original HTTP EOF/owner path, with serialization and delivery recorded and no
abandonment. The first gated GET was interrupted by a real public TCP reset
after actual delivery publication. Its supervisor recorded abandonment and
settlement while both serialization and delivery remained NULL. Auth and Friend
each held two COMPLETED scopes (normal plus interrupted), with zero prepared
transactions. A subsequent real `/api/auth/login` returned 200 with the same
canonical principal and a fresh access token; no SQL identity or refresh-token
insertion substituted for login.

The second actual GET retained an admitted callback for 4.5 s, beyond both the
original request TTL and CR08's two-second cleanup budget. It remained SEALED
with neither cancellation proof and with both owner fences held. Releasing the
actual callback after that bounded cleanup did not invent a later receipt. The
third actual PATCH registered and committed a real Concert transaction without
a Participant effect witness, using the exact request settlement reference.
The native resolver returned UNKNOWN with empty receipt evidence. Its root
could truthfully finish abandoned work, but cancellation settlement remained
NULL and both owner fences stayed held. Four prepared fences remained across
the two negative cases before disposable shutdown. Owner `scope_settlement`
alone was not treated as completion: the unknown-native leases remained SEALED
even after their root-work proof existed.

The Rust host passed its exact ignored test. Every retained MAIN and Redis
stopped unforced with exit 0; owned cleanup passed, and independent checks found
all recorded process IDs and the owned root absent. No prepared rollback,
manual completion RPC, synthetic EOF, mutation retry, or TTL-based release was
used. Expected negative fences disappeared only with whole-cluster teardown;
that teardown is not recovery proof. Measured whole-workflow growth peaked at
220,311,552 bytes under the 1.4 GiB bound, with about 39.2 GB free afterward. The
private capacity observer raced with directory removal during successful final
cleanup; its last sampled peak and this limitation are preserved separately
from the passing product/host results and independently verified cleanup.

Planning/provenance, Python AST, Rust 1.88 formatting, changed-path checks and
`git diff --check` pass. This demonstrates cancellation handling at a controlled
post-publication interruption point through actual HTTP and native owners.
Spontaneous browser timing, production runtime activation, the same-candidate
IN01 browser rerun, release acceptance and final master publication remain
Root/FE obligations; none are inferred from this dedicated integration fixture.

### Consumer cache-provenance follow-up

The original C03 native behavior result above remains PASS for its exact frozen
peer cohort. Its 1772 hashes pin the input snapshot; they do not independently
establish that reused platform metadata came from those same source bytes.
The successful Cargo JSON records `platform` as `fresh=true`. Earlier in that
same lease, other borrowed local packages demonstrably retained foreign proto
paths or stale application exports. This does not establish that the original
C03 platform artifact itself was stale, and no original report is relabelled.

Root subsequently reported a copied owner-policy mismatch in combined consumer
qualification. Canonical's source readback found the current owner policies
aligned with the current platform policy, while Root's frozen compile log
recorded reused test-profile platform metadata. A clean, named-allocation
platform rebuild and exact copied consumer rerun remain required to resolve that
provenance boundary; they are Root/Canonical owned. No Participant handler fix
has been established by this finding. The [successor provenance note](../../../backend/services/concert-service/tests/participant_cancellation_support/platform_provenance_followup.json)
preserves the original artifact/report hashes, observed Fresh metadata and the
bounded follow-up. This goal remains active through the qualified successor,
concrete consumer fixes if any, and verified Root publication. No additional
Cargo, PostgreSQL or shared-target work is started by this documentation patch.

### Clean-platform consumer checkpoint

Root rebuilt the seven requested artifacts at unchanged source
`d0f063d18a0549935f80a536077c30962d89f9e9` under named allocation
`ROOT-SEVEN-CACHE-REPAIR-D0F-20261104`. The earlier copied core failure
(reserved owner-authority policy mismatch, exit 101) remains preserved. Only
the platform package was cleaned; no Participant, fixture, policy or other
source change was needed. The first compile observer raced temporary metadata
removal, stopped its exact owned process group and received no validation
credit. The repaired observer retained that interruption and completed the
requested builds within the original allocation. Platform metadata was rebuilt
in that lease before subsequent same-pin reuse. All seven frozen artifacts
were verified and the target released before copied tests or PostgreSQL work.

All nine actual CR08 cancellation cases passed with exact report hashes and
owned-cluster cleanup. This resolves the reproduced policy mismatch for the
clean core consumer. The fresh C03 HTTP attempt did not reach service startup:
current source expected 18 Auth migrations while the retained historical 3710
Auth executable predates that manifest. Its migrate and verify commands exited
0, but the independent exact ledger check failed; HTTP statuses were empty,
prepared transactions were zero and owned cleanup passed. This is preserved as
a failed pre-service cohort attempt, with no Participant behavior claim.

The portable [consumer platform qualification](../../../backend/services/concert-service/tests/participant_cancellation_support/consumer_platform_qualification.json)
records the unchanged input pin, build/observer failures, clean metadata, seven
artifact hashes, nine core results, and failed HTTP preflight separately. Root
owns rebuilding coherent current peer MAINs and rerunning actual C03 HTTP.
The original f8a behavior PASS and its compiled-cache provenance limit remain
unchanged. No additional resource lease or test run was taken by this worker.

### Coherent native consumer qualification

C03-CANCEL01 is INTEGRATED in the dedicated native fixture at Root source
`d0f063d18a0549935f80a536077c30962d89f9e9`. Root rebuilt five current peer
MAINs under `ROOT-D0F-FIVE-MAINS-20261104`, retaining the same-pin clean Gateway
and Participant host. Exact Auth, Friend and Notification packages were cleaned
for their embedded migration inputs. All five builds exited 0 with freshly
compiled executables. The 2151 tracked backend/contract input hashes match the
clean-platform cohort; all six peer hashes and the host hash were verified.
The target was released at 05:38:18 UTC, before any copied native PostgreSQL
work at 05:38:19 UTC. No Participant Rust or Python edit was required.

The actual public HTTP run passed all 17 recorded statuses: three real users
completed signup, delivered-email verification, login, Profile update and
Bootstrap; normal C03 GET returned 200 through its original EOF path. The real
post-publication TCP reset left serialization and delivery false, recorded true
abandonment and cancellation settlement, completed both Auth/Friend owners
(two scopes each including the normal control), and left zero prepared
transactions. Subsequent real Auth login returned 200 with the same canonical
identity and a fresh token. The retained callback remained SEALED without
abandonment after expiry and delayed callback release. The registered, actually
committed native transaction resolved UNKNOWN without witness; abandonment
was truthful but cancellation settlement stayed false. Four expected negative
fences remained held until whole disposable-cluster teardown.

All six source-exact migration ledgers survived restart and independent
verification: Gateway 17, Auth 18, Profile 9, Friend 18, Post 31 and
Notification 6. The Rust host passed; owned cleanup passed; every peer and
Redis stopped unforced with exit 0. Independent readback found the owned
directory and all recorded peer/Redis PIDs absent. The whole-workflow guard
recorded 625,954,816 additional sampled bytes under its 1.5 GiB bound, with no
abort. The native report SHA256 is
`03e2f66b29051104a959629a779454c4aa2794b86a66b81cfb02b7a146980dac`.

The portable [coherent native qualification](../../../backend/services/concert-service/tests/participant_cancellation_support/consumer_native_qualification.json)
records source/contract and artifact pins, exact build commands and exit codes,
positive and negative behavior, migration/restart checks, resource release,
cleanup and limitations. Earlier failed and passing attempts remain unchanged.
This resolves the concrete cache/cohort consumer follow-up without weakening
authority checks. Root's non-force master publication remains to be verified;
full production Concert main composition, browser IN01 and release acceptance
remain separate Root/FE obligations. No resource is held by this worker.
