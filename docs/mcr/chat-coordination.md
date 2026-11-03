# Active chat ownership

Updated 2026-11-03. Current allocation prioritizes completing real service flows.
Model and reasoning choices belong to the user; all five chats received the
instruction to preserve them without orchestration overrides.

| Task | Visible chat | Chat ID | Worktree / branch |
|---|---|---|---|
| BE04 HTTP fixture delivered by integrator; writer stopped, broad goal unfinished | `mcr(be) BE04 real HTTP owner chain` | `01a19f83-12cc-7f21-8ad3-55749f012d9b` | `/Users/lhs/Desktop/mcr-worktrees/be-http-real-owners` / `codex/mcr-be-http-real-owners` |
| BE08 places and map releases | `mcr(be) BE08 places and map releases` | `01a1a00c-2690-7bc0-94fc-52609655ea83` | `/Users/lhs/Desktop/mcr-worktrees/be-places` / `codex/mcr-be-BE08` |
| FE21 party discovery/applications | `mcr(fe) FE21 party discovery and applications` | `01a19f83-2c2a-7fa0-a98d-1c4f5f65cb3c` | `/Users/lhs/Desktop/mcr-worktrees/fe-party-discovery` / `codex/mcr-fe-FE21` |
| FE24 giveaway discovery/own activity | `mcr(fe) FE24 giveaway discovery and own activity` | `01a1a153-1227-7e72-801f-a2a6b91981e1` | `/Users/lhs/Desktop/mcr-worktrees/fe-gift-discovery` / `codex/mcr-fe-FE24` |
| FE24 independent review | `mcr(qa) FE24 giveaway review` | `01a1a00c-c94e-74a2-a306-1ff686372881` | `/Users/lhs/Desktop/mcr-worktrees/qa-gift-discovery` / `codex/mcr-qa-FE24` |

The entries below retain earlier assignment history; this table and the latest
allocation section at the end supersede their historical active/idle wording.

The original four increments started at `376cf21`; FE11 starts at `b39d5e5`
after the same chat completed FE08. They initially used `gpt-6.1-sol` with `high`
reasoning and no token budget. The user has now withdrawn that fixed-model
requirement: preserve each chat's selected model and reasoning setting and omit
overrides from coordination and continuation calls. Actual goal tools confirmed FE08 **COMPLETE**, the new FE11
goal **COMPLETE**, BE15 **COMPLETE**, FE16 **COMPLETE**, and the original BE04 goal
**BLOCKED** on central prerequisites. The completed BE15 chat was then reused at
`5863f91` for a new Auth-owner read goal, now tool-confirmed **COMPLETE** after parent review. The old
BE15/FE16/FE08 checkouts have no writer. Dispatch alone is not proof. Each bounded goal covers implementation, relevant tests,
documentation, committed handoff and concrete integration-review fixes. An
increment goal can complete after delivery while unqualified task gates remain
open; do not silently turn that into whole-task or release acceptance.

The standing language instruction is **English only in every chat**: replies,
progress, questions, reviews, handoffs, final answers, new titles and engineering
prose, even when the user writes in Korean. Preserve exact historical source
text and identifiers. Every continuation carries this instruction and retains
an aligned unfinished goal rather than creating incompatible parallel goals.

## Exact implementation ownership

Paths below are repository-relative. Read access does not grant write access.
All central changes outside the listed exceptions remain with the integrator.

| Task | Writable paths and bounded composition |
|---|---|
| BE04 | `backend/crates/concert-application/src/identity/**`, application `tests/identity.rs`; `backend/services/concert-service/src/adapters/out/identity/**`, service `tests/identity.rs`; `docs/mcr/backend/identity.md`. Exact composition exceptions: application `src/lib.rs` identity export and service `src/adapters/out/mod.rs` identity export only. |
| BE15 | Application `src/commands/**` and `tests/commands.rs`; service `src/adapters/out/repository/{receipt,outbox}.rs`, `src/workers/outbox.rs`, `tests/command_receipts.rs`, exclusively reserved `migrations/0002_command_receipts_outbox.sql`; `backend/scripts/mcr/command_receipts_smoke.py`, `test_command_receipts_smoke.py`; `docs/mcr/backend/command-receipts.md`. No implicit ownership of surrounding repository/composition files. |
| BE04 Auth owner | Application `auth-application/src/concert_identity.rs`, `tests/concert_identity.rs`; Auth service persistence `concert_identity.rs`, `tests/concert_identity_owner.rs`; backend smoke scripts `auth_identity_smoke.py`, `test_auth_identity_smoke.py`; `docs/mcr/backend/auth-identity-owner.md`. Exact application/persistence `mod` exports only. Parent owns migration `0013`, all manifests/locks/proto, method policies, RPC/state/config/Gateway/platform composition. No writes to the stopped BE15 checkout. |
| FE11 | Auth UI `ResetPassword.tsx`, `Callback.tsx`, `recovery.css`; auth model `recovery.ts`, `recovery-contract.ts`, `callback.ts`; new auth tests `recovery.test.ts`, `callback.test.ts`, `recovery-browser-smoke.mjs`, fixtures `recovery-*`; new app `__tests__/recovery.test.tsx`; `docs/mcr/frontend/password-recovery.md`. Exact composition exceptions: `PublicBoundary.tsx` dispatch for existing two routes only; `session.ts` recovery methods/cleanup through its single canonical session/client; `cookie-policy.ts` minimal typed recovery-confirm coordination. No other existing test, root routes, shared utility or dependency ownership. |
| FE16 | `frontend/src/world/network/WorldTransport.ts`, `mock.ts`, `websocket.ts`, `__tests__/**`, optional feature-local `transport/**` helpers; `docs/mcr/frontend/world-transport.md`. Excludes FE17 `replication.ts`, world runtime/scenes/entities and application composition. |

BE04 prepares pure identity/authority ports and deterministic adapter tests.
Canonical Auth session checks, explicit provider binding and suspension authority
need shared owning-service contracts. The original writer is stopped with a
blocked goal; the Auth-owner increment is separately assigned below. Profile existence does not prove session,
suspension or binding. The production Concert policy stays unavailable until
real authenticated authority is qualified. Auth/Profile/Gateway, platform,
proto, manifest and credential changes remain integrator-owned.

BE15 pins C04 candidate hashes as design input; it does not claim C04 frozen or
expose a new public command wire. Aggregate mutation, safe receipt and outbox
must share one database transaction; replay reauthorizes, retired keys never
rerun and lease completion is fenced. It must qualify crash/retry/deduplication
with a real owned PostgreSQL cluster. Production producers and worker startup
remain disabled. Integrator prerequisites `f74fb66` and `852d414` supplied retained
dependency/module exports, integrated with the implementation as `d76215c` and
`5874c81`. They were not published alone with missing modules. Combined compilation,
layer checks and real PostgreSQL verification now pass; earlier metadata checks
alone were never a compilation claim.

FE08's reviewed public entry is integrated and its goal complete. Its former
`fe-public` checkout is stopped. FE11 now implements anonymous reset request/confirm
and honest callback error/return presentation on its separate recovery checkout.
Source reset mail emits `BASE#token=...`; deployment base/mail delivery remain
unqualified. Secrets stay transient, fragment material is removed before rendered
actions, reset confirmation clears/fences local access and uses existing cohort
coordination. OAuth success/session import remains unavailable pending a reviewed
handoff; no provider is enabled. Production legal publication remains unavailable.

FE16 delivered frozen C05 consumers, with shared mock/native transport semantics,
strict frame validation, injected ticket acquisition and owned Chrome WebSocket
fixtures. Tickets stay out of URLs, storage and logs. No production lifecycle
wiring, independent auth store, replication controller or new wire contract is
delegated. Its reviewed goal is complete and writes stopped. Mock/native parity
does not prove a real BE10/IN01 world. BE15 likewise completed its bounded internal
foundation after parent PostgreSQL/restart/cleanup review; production producers,
Auth policy and runtime startup remain unqualified. See the integration evidence
for [BE15](evidence/command-receipts-integration.md) and
[FE16](evidence/world-transport-integration.md).

Build/evidence roots are separate: `/Users/lhs/Desktop/mcr-build/be04`, `be15`
and `fe16`; FE08 records its own evidence root. Rust uses retained 1.88.0 and two
build jobs per lane. No worker starts source-default databases or changes global
services. Stagger expensive checks and avoid unnecessary duplicate build trees.

## Integration, review and queued capacity

The main chat owns shared contracts, manifests/locks, root routers, service
registration, infrastructure, review and serial integration. Only it pushes
`master`. Workers commit exact assigned paths and report base, hashes, paths,
commands/exit codes, limitations and proposed status. The integrator checks every
changed path and cross-directory import, returns concrete findings to the same
owner, then runs the affected combined checks before publishing.

The previous BE05 and FE09 increments are integrated and reviewed:
[Concert foundation](evidence/concert-core-integration.md) and
[session/sign-in](evidence/session-sign-in-integration.md). Their old worktrees
`be-concert-core` and `fe-session` are preserved with no writer. Earlier stopped
worktrees are also preserved; no subagent writes current chat worktrees. FE09
remains MOCK_READY despite its delivered increment goal completing.

FE11 and the Auth-owner read are integrated with parent candidate-bound evidence,
and both actual bounded goals are COMPLETE. Their clean checkouts are stopped.
The original BE04 goal remains BLOCKED; the parent owns authenticated composition
and the selected [Gateway fresh reads and signed admission](identity-admission-decision.md)
direction. Binding timestamps are not generations. New assignments must transfer
ownership to separate worktrees and establish actual new goals.
The [capacity assessment](parallel-capacity.md) records why another writer is not
forced. Scale from ready disjoint work and review capacity, not a chat quota.

No decorative left accent lines, colored left borders or equivalent inset/pseudo
bars are permitted. Only the user-selected revision-3 character source pack and
reviewed derivatives are authorized; registered source sheets are not a qualified
production avatar atlas. These constraints apply to every new chat.

## Current continuation ownership

FE10's bounded goal is tool-confirmed **COMPLETE**, with all three commits
integrated and parent evidence retained. Its checkout is stopped. The Auth RPC
goal remains **ACTIVE** through parent integration review of clean `9bf3233`;
the worker has stopped source/build writes and transferred the retained target
to the parent. FE10 starts
at published `e725ae4`; the Auth RPC starts at integrator prerequisite
`a44f8fa711b1ac73ce97d9e18d13dfc2eeee2382`, on top of `e725ae4`. The prerequisite
is on its isolated branch and is not published as a functioning RPC: the method
still returns fixed unavailable until the real handler is reviewed. Parent ran
103 platform authority, two live guard, two Auth policy and thirteen retained
Gateway verification-transport tests. Parent retains shared proto/platform and
will integrate the prerequisite with the worker's completed real handler.

- FE10 owns new auth UI `SignUp.tsx`, `VerifyEmail.tsx`, `enrollment.css`; model
  `enrollment.ts`, `enrollment-contract.ts`; auth tests `enrollment.test.ts`,
  `enrollment-browser-smoke.mjs`, fixtures `enrollment-preview.{html,tsx}`; new
  app `__tests__/enrollment.test.tsx`; `docs/mcr/frontend/signup-verification.md`.
  Exact existing exceptions are `PublicBoundary.tsx` dispatch/imports for the
  existing two routes and minimal canonical enrollment methods/coordination in
  `session.ts`. No cookie-policy/shared utility/manifest/contract ownership.
- Auth RPC owns new service gRPC `concert_admission.rs`, service test
  `concert_admission_rpc.rs`, backend smoke/helper scripts
  `auth_admission_rpc_smoke.py`, `test_auth_admission_rpc_smoke.py`, and
  `docs/mcr/backend/auth-admission-rpc.md`. Exact existing exceptions are one
  AppState owner-port field and real adapter injection in config, one gRPC module
  export, replacement of only the new server stub, and new-method-only policy
  tests. No proto/platform/Gateway/Concert/frontend ownership.

Production signup remains unavailable without approved eligibility, consent and
publication authority; synthetic policy fixtures may qualify the forms. Existing
account verification/resend are separate source-backed operations. No invented
challenge expiry, age policy, recorded-checkbox consent or delivery proof.
The RPC increment must qualify the actual generated Auth server behind production
guard/deadline layers and real owned PostgreSQL. Neither increment enables
production Concert authority or accepts an integration/release gate.


The completed FE16 chat was reused for an independent review on a fresh QA
checkout, base `5c02a31`, with only
`docs/mcr/qa/admission-contract-review.md` writable. Review commit `e0b37cc`
was integrated as `e103da2`; no actionable defect was found against the pinned
event correction and Auth prerequisites. Sixteen focused QA regressions and
source-pin checks passed. Its actual bounded goal is COMPLETE and checkout
stopped. The real RPC handler was outside that review; parent review remains
required. No new chats were created to satisfy a numeric quota.


## Runtime and final RPC review continuation

FE14 now has a new tool-confirmed ACTIVE unbudgeted goal on fresh `fe-runtime`,
base `244d35a`. The completed FE10 checkout remains stopped. Exact ownership:
`frontend/src/world/runtime.ts`, new `world/map/**` and `world/scenes/**`, new
`world/__tests__/runtime.test.ts`, `runtime-browser-smoke.mjs` and feature-only
`world/__tests__/fixtures/runtime-preview.{html,tsx}`/data/helpers, plus
`docs/mcr/frontend/phaser-runtime.md`. It consumes the existing pinned C06
map/theme subset with real Phaser and explicitly synthetic resource fixtures.
No avatar artwork, contract mutation, production map, app/session/transport
wiring, manifests or backend edits are delegated. Production art, venue, FE12,
whole C06 and world acceptance remain unqualified.

The same QA chat has a new tool-confirmed ACTIVE goal on fresh `qa-auth-rpc`,
base `9dd8bc6`, only `docs/mcr/qa/auth-admission-rpc-review.md` writable. It reviews
the actual handler, owning adapter and generated-server/native evidence; it runs
no Rust build or database. Its P2 timing finding R1 was returned to the backend
worker, fixed as `5c59f1f` and integrated for final parent qualification. The
handler's runtime behavior is unchanged by that test-only correction. Parent
owns the retained Rust target after the worker's clean fix handoff. The original
QA prerequisite checkout is stopped; no redundant chat was created.


Final RPC disposition: worker `5c59f1f` and QA `9a61556` bounded goals are
both tool-confirmed COMPLETE. Parent reviewed and integrated the entire fix and
three QA document commits; final eleven controlled/five RPC/22 owner cases,
source pins, migration/restart and owned cleanup pass. No source/build writer
remains on either completed checkout. FE14 alone remains ACTIVE until a new
ready backend assignment is explicitly recorded. Whole BE04 and production
Concert admission remain open.


## Current Gateway access ownership

After tool-confirmed RPC completion, that backend chat received a new unbudgeted
goal and confirmed it ACTIVE on `be-gateway-concert-access`, branch
`codex/mcr-be-gateway-concert-access`, published base
`b16a679c42ee54c4a3e73575263b390f9334e064`. Its exact writable paths are new
Gateway `src/adapters/out/jwt/concert_access.rs`, new `tests/concert_access.rs`,
`docs/mcr/backend/gateway-concert-access.md`, plus the existing `jwt.rs` module
export and minimal shared verified-decoder extraction. Existing public legacy
identity shape, routes, validation/leeway/key rotation and all other behavior must
be preserved; broader changes require a concrete parent prerequisite.

The strict context retains authenticated canonical user/family and signed expiry
with the original monotonic request stop. Future expiry is normal; malformed,
already expired or overflowed expiry fails, and future not-before is denied when
present. It provides no owner facts, event roles or signed Concert admission.
No Auth client, HTTP/config, proto/platform/domain, dependency, frontend or art
writes are delegated. The backend owns the idle retained Rust target with two
jobs. FE14 proceeds independently on its frontend checkout; QA stays stopped
until another concrete review is ready. No extra chats were created.


## Strict Gateway handoff and asset-format QA

Gateway context worker `ed2f166` is integrated unchanged as `e7d208e`. Parent
review and 235 tests/three compile-fail checks/library check/binary build pass.
The worker's bounded persistent goal is COMPLETE and its exact four-path checkout
is stopped; parent retains the Rust target. FE14 remains independently ACTIVE.

The QA chat `01a1a00c-c94e-74a2-a306-1ff686372881` now has a new actual ACTIVE
unbudgeted audit goal on `/Users/lhs/Desktop/mcr-worktrees/qa-asset-formats`,
branch `codex/mcr-qa-asset-formats`, base published `f80b6d1`. Its sole writable
path is `docs/mcr/qa/asset-format-review.md`; no application/contracts/dependency
writes or Rust/DB/service work is delegated. Parent owns the separate central
C06 checker/plugin correction on `contract-format-prereq`, including concrete
QA review fixes. This reuse serves an independently reproduced tooling defect,
not a chat-count requirement. Only the parent integrates and pushes.


## Auth owner client and final format disposition

C06 correction `27c6bbc`/`0cfffca` and sole-path QA report `edd0226` are
integrated as `6a1e394`/`04a8268`/`bab2f1f`; both reproduced findings are closed
and parent combined 144 tests pass. QA's actual goal is COMPLETE and its checkout
stopped. Its immutable report remains the review evidence.

Backend chat `01a1a00c-2690-7bc0-94fc-52609655ea83` now has a new actual ACTIVE
unbudgeted goal on `/Users/lhs/Desktop/mcr-worktrees/be-gateway-auth-owner`,
branch `codex/mcr-be-gateway-auth-owner`, published base `e6efa4c`. Exact writable
paths: new Gateway `src/adapters/out/auth_client/concert_admission.rs`, new
`tests/concert_admission_transport.rs`, `docs/mcr/backend/gateway-auth-owner-client.md`,
and only one module export in existing `auth_client.rs`. It uses the reviewed
strict context and existing authenticated signer/channel; no HTTP/config/ports/
proto/platform/domain/Auth/Concert/frontend/manifest ownership is transferred.
The worker owns the retained Rust target, two jobs; prior JWT checkout stopped.
FE14's clean `a7dcb79`/`a1243ff` fifteen-path handoff retains its ACTIVE goal
through parent review. Parent serially integrates and alone publishes.

## Historical allocation after FE14 review

FE14 worker `9b9307c` and QA report `5bf0708` completed the exact review scope.
Both goal update and subsequent goal reads confirm COMPLETE. Their checkouts
are stopped. Parent final `77ae6bd` passes 871 tests, build/lint, native browser,
contract/origin/boundary checks; [integration evidence](evidence/phaser-runtime-integration.md)
records the three corrected P2 findings and limits.

The previously idle BE04 chat now actively implements the Profile-owner read at
published base `3fdb977d6a64798d76ea137099026879a68a67f9`. Exact seven paths: new
`profile-application/src/concert_identity.rs` and `tests/concert_identity.rs`;
new Profile service persistence `concert_identity.rs` and
`tests/concert_identity_owner.rs`; `docs/mcr/backend/profile-identity-owner.md`;
only module exports in the existing application `src/lib.rs` and persistence
`mod.rs`. It reads current self nickname/bio with deleted-row and owning deletion
fence semantics, through one bounded uncached query. No Auth authority, portable
proof, public RPC, migration or production activation is delegated. Real isolated
PostgreSQL qualification is required. Its compatible original goal still returns
BLOCKED and the available goal API has no resume transition; the explicit user
continuation is being executed without falsely completing that larger objective.

Auth client `868d02c` is clean and retains an ACTIVE review goal. Its worker
explicitly released the shared Rust target after all owned processes ended.
The Profile-owner worker now exclusively owns `/Users/lhs/Desktop/mcr-build/backend`,
Rust 1.88, locked/offline, two jobs, debug information disabled. Parent and QA
must not concurrently compile there. QA's fresh candidate is exact `868d02c`;
its sole writable path is `docs/mcr/qa/gateway-auth-owner-client-review.md`.
Source/evidence review can proceed without owning the target or a database.

FE15 starts at `b1a19c425a30641410d71986714d72b7101f43e8` and has a new tool-confirmed
ACTIVE unbudgeted goal. Writable paths are new world `input/**`, `camera/**`,
`interaction/**`, `__tests__/movement.test.ts`, `movement-browser-smoke.mjs`,
`__tests__/fixtures/movement-*`, and `docs/mcr/frontend/world-movement.md`.
Bounded existing-file exceptions are runtime typed control attachment/lifecycle
and MapScene camera/world-coordinate integration only. No Phaser input plugin,
network/replication/avatar/auth/app/contract/dependency/production-art ownership
is transferred. It implements real browser controls and local prediction against
validated geometry, with injected simulation parameters and actual Phaser camera
checks. The parent owns future shell and authenticated transport composition.

Recheck chat status during work and at every handoff. Assign the next ready task
when its predecessor's reviewed scope is complete; a message alone does not prove
execution or an ACTIVE goal. Preserve lane/file/resource boundaries and publish
reviewed units serially rather than accumulating them behind unrelated work.


## Latest allocation after owner/client review

Profile owner worker `81665a04` is integrated byte-identically as `92fb4ef`.
The parent reviewed all seven files and exact source/evidence pins; the 2 MiB
transfer-bound correction is covered by real PG tests. Auth worker `77890b9`
is integrated as `9dceb49`; parent 252 tests, six doctests and binary build pass.
QA reports `ad4e963` / `f0ed0a2` are integrated as `7594ec7` / `6d91e61`, preserving
the missed initial limiter finding and the corrected cancellation proof.
The independent Auth follow-up goal update and subsequent read confirm COMPLETE;
its new FE15 review goal is confirmed ACTIVE. The Auth implementation goal was
accepted for completion and a new Profile-client goal assigned; verify its actual
transition through the worker acknowledgement. No whole BE04 acceptance follows.

The original BE04 chat explicitly stopped the clean owner checkout and is now
implementing guarded self RPC at base `d63fb2b95d3d8e61406ef71a12ed86a3b0bf15fd`.
Its historical compatible goal had remained BLOCKED through the API. After the
user resumed it, the latest worker goal read now confirms ACTIVE with the same
aligned, unfinished BE04 objective.
Exact five writable paths: Profile service grpc `concert_identity.rs`, new
`tests/concert_identity_rpc.rs`, `tests/support/concert_identity_fixture.rs`,
`tests/support/concert_identity_native.py`, and `docs/mcr/backend/profile-self-rpc.md`.
Parent owns the unpublished prerequisite proto/policy/state/server/module/fixture
composition. Its unavailable stub must not be published alone. Required validation
covers actual signed generated transport, private read authority, original/proof
budgets, replay/refusal, minimal real owner projection, fresh content/deletion,
real isolated PG timeout/recovery and exact cleanup.

The other backend chat now implements the minimal Gateway Profile client at
`d1282c6c5c27099ce7cd2adeed30ee99e5c1896b`, including that frozen wire and the reviewed
Auth client. Exact four writable paths: Gateway `profile_client/concert_identity.rs`,
new `tests/concert_profile_transport.rs`, `docs/mcr/backend/gateway-profile-owner-client.md`,
and only the module export in existing `profile_client.rs`. Borrowed strict
context, shared real edge capacity/metrics, exact deadline, 1024-byte response
bound and minimal validated self content are required. No route, broad Profile DTO,
Auth grant or production artwork is delegated. Controlled fixtures permit parallel
implementation before the real producer finishes; they do not certify that producer.

FE15 retains its exact prior scope and ACTIVE goal. QA may read its evolving draft
and run independent temporary probes on its own browser/port, but must pin the final
committed candidate before disposition. Its sole tracked writable path is
`docs/mcr/qa/world-movement-review.md`; no worker-browser control or application writes.

Parent's combined Rust process has completed. The Profile RPC worker subsequently
finished its bounded checks and explicitly released the target after a fresh
process/owned-cluster audit; its final documentation and handoff remain in progress.
The Gateway Profile client now exclusively owns `/Users/lhs/Desktop/mcr-build/backend`
for compiled validation. Rust 1.88.0, locked/offline, two jobs and BOTH dev/test
debug information zero apply. Only the fixture owner may start/stop its private
native PG store. Parent, Profile RPC and QA run no concurrent Rust target work.
Continue compact chat checks, transfer resources promptly, and publish reviewed
checkpoints serially.


## Delivery cadence correction — 2026-11-03

The user challenged the slow accepted-task delivery. Prefer complete feature
flows over additional tiny prerequisite handoffs. Run focused checks during
changes and one relevant integration gate; reuse exact-source prior evidence
where inputs are unchanged. Repeat a broad suite only for a changed boundary,
failure or unresolved concern. Keep handoff prose concise. Do not advance task
acceptance from fixtures or remove required product scope.

Current exact ownership supersedes earlier allocations:

- Profile integration: base `c6404c3`; Gateway tests
  `concert_profile_owner_integration.rs`, support
  `concert_profile_owner_fixture.rs` and `concert_profile_owner_native.py`, plus
  `docs/mcr/backend/gateway-profile-owner-integration.md`. Actual lock-held
  PostgreSQL cancellation exposes pool starvation; two failed runs are retained.
- Production fix: base `c6404c3`; Profile service persistence and gRPC
  `concert_identity.rs` only, plus `docs/mcr/backend/profile-query-deadline.md`.
  Transaction-local statement timeout must use the captured original deadline.
  This writer exclusively holds `/Users/lhs/Desktop/mcr-build/backend` until
  explicit release; integration worker holds no build or PostgreSQL process.
- Real email frontend: base `b381d03`; `frontend/e2e/real/auth/**`, demonstrated
  real-journey fixes in `frontend/src/features/auth/**`, and
  `docs/mcr/frontend/real-email-flow.md`. No backend/root/shared/contract writes.
  Existing isolated backend launcher may be invoked unchanged. Current-source
  binary qualification precedes final service claims. Existing test-only
  enrollment-policy opt-in is confined to e2e; production consent remains gated.
- FE15 implementation and independent review goals are tool-confirmed COMPLETE.
  The original broad BE04 goal and new query-fix/email-flow goals remain active;
  scope completion never silently completes a broader product task.


## Additional frontend chat — FE06

The user explicitly requested a second frontend development chat. It starts at
`600582d6b5deda6037c190c303f80752ba4a6bb3`; creation omitted model and reasoning
overrides, and its unbudgeted implementation goal is confirmed ACTIVE. It owns
`frontend/src/shared/styles/themes/**`, `frontend/src/features/theme/**`,
`frontend/src/world/theme/**`, and `docs/mcr/frontend/theme-system.md`. Limited
`world/runtime.ts` and `world/scenes/MapScene.ts` seams are assigned exclusively
for atomic skin replacement on the current Phaser owner. Auth/e2e ownership is
unchanged. Parent owns the single CSS import, scoped shell/controller composition
and selector mount; existing world, room, camera and avatar identity must survive
switches. Production texture absence remains explicit; synthetic tests do not
approve replacement artwork. Focused tests and one native gate precede review.


## Current handoffs and next feature — 2026-11-03

The real-email increment is reviewed and integrated as `2f398a3` and `bed5ffa`.
Its actual goal is COMPLETE; the former checkout is stopped. FE19 starts at
`bed5ffa8cb52f5dc337bcde7d50a428f9bce74f9` with an actual ACTIVE unbudgeted goal.
It owns only `frontend/src/features/places/**`, `frontend/src/features/world-help/**`
and `docs/mcr/frontend/places-help.md`. C03 place catalog revisions remain distinct
from C06 map revisions. The integrator owns route/port composition. This feature
uses frozen contracts and fixtures without waiting for matching backend work.

Both new backend checkouts start at `60ca208`. HTTP owns `http/concert_access.rs`,
its explicitly delegated module/test registration and focused tests, and
`docs/mcr/backend/http-concert-context.md`. Owner composition owns
`adapters/out/concert_identity.rs`, its module export, focused composition test/support
files and `docs/mcr/backend/identity-owner-composition.md`. No production route or
AppState changes are delegated. The retained Rust target has one explicit owner;
HTTP released it to composition before further review fixes. Former Profile
checkouts are stopped. Actual HTTP timeout enforcement remains a review gate.

The second FE chat was explicitly requested by the user and handles FE06 in its
separate worktree. R1 reentrant texture commit ownership was fixed in `4cb28b5`;
the parent integrated it as `5710363` after initial `4f4df6d`. Main owns production
shell wiring and release catalogs. QA owns only its review document. Model and
reasoning overrides are omitted on every assignment and continuation.


## Current ownership after HTTP and identity integration

All five current goals are reported ACTIVE. The original broad BE04 goal remains
unfinished. The prior owner-composition and FE06 implementation/review goals were
tool-confirmed COMPLETE before their chats received new assignments. Former
checkouts are stopped. Every worker write must use an absolute assigned-worktree
path; a chat's inherited process directory is not write authorization.

- BE04 starts at `1f6dceb10f8147b5cfbbdc6bf486f994edf24c1e`. It owns only new Auth
  test `concert_http_owner_chain.rs`, optional same-name test support, new backend
  `scripts/mcr/concert_http_owner_chain_smoke.py` and focused helper tests, and
  `docs/mcr/backend/http-owner-chain.md`. Actual guarded Auth and Profile owners,
  real owned PostgreSQL and production HTTP timeout must be joined without fake
  successful owner replies. It exclusively owns the retained Rust target until
  explicit release; no other worker or parent compiles there concurrently.
- BE08 starts at `f440df302c32e5490c1f95a9b1a7a1f61c72c2d2`. It owns domain and
  application `src/place/**`, their exact `lib.rs` exports and `tests/place.rs`;
  Concert repository `place.rs` and its exact module export; exclusively reserved
  `migrations/0003_places_map_releases.sql`; service `tests/places.rs`; new backend
  `scripts/mcr/places_smoke.py` and focused helper test; and
  `docs/mcr/backend/places.md`. It is source-only until target transfer. Parent
  owns central manifests, migration-count checks, HTTP/RPC and authority wiring.
- FE19 retains its exact feature ownership. Ordered handoff `072c817`, `b528374`
  awaits independent review of stale-target invalidation and actual event reload.
  The reviewer has those exact changes atop integrated FE06, plus its own report,
  and may write only `docs/mcr/qa/places-help-review.md`. No application ownership
  transfers to the reviewer.
- C04 review starts at `6716ed7d9796586d7f2b52980ed2a2fcab1ab2f8` and may write
  only `docs/mcr/qa/domain-readiness-review.md`. Its first report is returned for
  concrete factual corrections; it neither freezes C04 nor changes task scope.

The main chat reviewed and integrated the HTTP and composition worker chains,
then wired the configured reader. It retains shared contracts and publishing.
Five purposeful chats currently suffice; no duplicate frontend writer is added.


## Current frontend allocation after FE19 delivery

FE19 implementation and independent review both report actual COMPLETE goals,
followed by null goal reads, and clean stopped checkouts. Parent integrated both
worker commits and both sole-report commits. Combined61 focused tests/build/lint
pass; final source and independent evidence hashes match. The new assignments
both start at `4dc3bb3dd8015abcf26839c34954f7ca91870ceb`, with tool-confirmed
ACTIVE unbudgeted goals and unchanged model settings.

- FE17 owns new `frontend/src/world/network/replication.ts`, optional new
  `world/network/replication/**`, headless `world/entities/**`, new
  `world/network/__tests__/replication.test.ts` and replication-only fixtures/
  browser runner, and `docs/mcr/frontend/world-replication.md`. It consumes the
  existing transport; no second connection, invented physics, command replay,
  legacy artwork or production placeholder is authorized. Existing runtime,
  transport engine, app, contracts and manifests remain outside its ownership.
  FE12 rendering and real-server qualification are explicit later gates.
- The original frontend chat independently reviews only FE21/FE24 candidate
  schemas/operations and actual FE01/FE02/FE09 consumers. Sole tracked write:
  `docs/mcr/qa/domain-frontend-consumer-review.md`. It identifies exact shared
  transport/idempotency/error and contract prerequisites before feature writes;
  it cannot freeze C04. Temporary probes stay outside the repository.

BE04 retains exclusive Rust target ownership. BE08 continues source work and has
parent dependency prerequisite `ec4728d` cherry-picked as `097651c`; no package
version changed. Its source is not yet compile-ready. QA C04 report corrections
are still under parent review. Only the integrator publishes reviewed work.


QA readiness audit disposition: the corrected report chain is integrated through
`518f7ea`, with parent count/header corrections in `2cb59b3`. Actual goal update
returned COMPLETE and a subsequent read returned null; its clean checkout is
stopped. Four development/review chats remain active. No duplicate audit is
assigned while the independent frontend consumer review is already running.


## FE21 and FE24 implementation after consumer qualification

The parent integrated FE17 as `79e4556`, independent review as `1fa598c`, and
passed 181 combined transport/replication tests plus build/lint. Both bounded
worker and QA goals are tool-confirmed COMPLETE and their former checkouts stopped.
The independent consumer/context goals are also COMPLETE. The shared request and
capability fixes passed 287 focused tests and 26 independent consumer cases;
context qualification passed 30 new independent methods and 93 combined contract
tests. [The bounded consumer contract](party-gift-consumer-contract.md) is frozen
at `5f884ed`; whole C04 and full task acceptance remain open.

Two frontend implementation chats now use distinct worktrees from full base
`5f884edfe636a3a9a8f8b08fb33044c5d57cd0aa`:

- FE21 owns only `frontend/src/features/party-discovery/**` and
  `docs/mcr/frontend/party-discovery.md`. Its new unbudgeted persistent goal is
  tool-confirmed ACTIVE through full implementation, focused/native validation,
  documentation, committed handoff and review fixes.
- FE24 owns only `frontend/src/features/gift-discovery/**`,
  `frontend/src/features/my-gifts/**` and `docs/mcr/frontend/gift-discovery.md`.
  Its new concrete unbudgeted goal is tool-confirmed ACTIVE. It uses the existing
  scoped privacy.run/cleanup seam and remains active through delivery/review fixes.

Neither worker may modify shared clients, contracts, root routers, manifests,
locks, the other feature lane or backend. Parent owns actual panel composition.
Both must use absolute worktree paths, preserve the selected model, use English,
avoid decorative left accents and use only approved character derivatives.
There is no automatic mutation replay or invented production feature readiness.
BE04 retains exclusive shared Rust target ownership for the real owner-chain
gate; BE08 continues its isolated source work until that lease is released.
Only the integrator publishes reviewed changes. Accepted counts remain 11/112
required and 11/118 total; the full project goal stays ACTIVE.

## Owner-chain delivery and current validation transfer

The HTTP worker explicitly stopped editing/builds and released its checkout and
target. The integrator took over the preserved fixture, repaired incorrect
case assertions and session reuse after the owning account-status trigger,
and delivered `ca26683` as `e6c629d`. The fresh native run and exact cluster cleanup
pass; see [bounded owner-chain evidence](backend/http-owner-chain.md). The broad
BE04 goal remains ACTIVE; no signed Concert or event-policy completion is claimed.

BE08 exclusively owns `/Users/lhs/Desktop/mcr-build/backend` from that completed
run onward. Its candidate `c135bff` received independent source review at
`64d1b02385c3f058b25c44ff28b3e1703c9a9696` and factual correction
`c0989d4977bf645187ed9cecb50ba7c0032acbd7`. It now fixes the SQL timestamp conflict
and adds a caller-held selection transaction with a real concurrent-closure
test before handing off. Its goal remains ACTIVE.

The boundary checker now supports an exact explicitly assigned
`--allow-harness backend/scripts/mcr/<name>.py` exception. It is limited to
that backend path, rejects other central/opposite-lane paths and still rejects
symlinks.46 checker tests pass (one platform-dependent skip). This accommodates
the already authorized HTTP and places harnesses; it is not broad script ownership.

FE24 handed off implementation `5f1005d` and receipt `595c07f`; its clean checkout
is available for review fixes, with its goal ACTIVE. QA has a fresh isolated
checkout at the latter commit, with sole report ownership
`docs/mcr/qa/gift-discovery-review.md`. The completed BE08 source-only review goal
must close before creating the new unbudgeted FE24 review goal. Main owns root
panel/controller composition and publication; FE21 continues its disjoint feature.
