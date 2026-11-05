# W04 virtual party-room invitation producer

Status: **IN_PROGRESS** for assigned producer ownership and documentation;
**implementation NOT_STARTED; application/native/vertical validation NOT_RUN**.
Original W04 invitation remains **OPEN** and whole BE11 remains **IN_PROGRESS**.
The existing single human policy question is **PENDING**. No policy, API, schema,
permission grant or product completion is established by this document.

## Durable objective and original completion criteria

Task `W04-PRODUCER-01` is the remaining virtual-party-invitation obligation within
original BE11 and IA W04; it adds no original task ID or scope cut. Implementation
owner is chat `01a1a1fd-6b5d-75b0-8d42-01ade79b9393`. Reuse its isolated worktree
`/Users/lhs/Desktop/mcr-worktrees/be-world-admission-leases-6b5d` and branch
`codex/mcr-be-BE10-BE11-admission-leases`. Full unbudgeted objective: implement the
actual human-decided virtual invitation producer through explicit disjoint grants,
canonical identity and current authorization, persistence and failure recovery;
perform relevant developer-owned domain/application/query/native/vertical
validation, fix findings, document provenance/results/limits, and commit a qualified
handoff directly to standing publisher412e for non-force publication and remote
verification. No King/SL/Root technical review or acknowledgment is a stage.

Fresh goal-tool inspection still returns historical incompatible
`BE-PROFILE-SELF`, **BLOCKED**. It is not reset, completed or relabeled. This durable
objective records the separately authorized assignment without claiming an aligned
API goal exists. Required unfinished implementation and delivery remain unfinished.

The specification `MCR_IMPLEMENTATION_PLAN_V2_EN.md:195` requires W04
`/world/rooms`: capacity, room switch, **party invitation**, full and denied states;
it maps to FE18/BE11. Original BE11 at line466 requires one owner process per room,
bounded capacity, session membership, join/switch/leave and disconnect grace;
concurrent joins must not exceed capacity, switching must not duplicate membership,
and ghost leases must expire. FE18 at line524 requires the room chooser, switch
feedback/cap states and accessible authorized-nearby list, preserving membership on
full-room failure and removing blocked users from both representations. Invitation
is a remaining IA obligation; FE18's previously qualified chooser criteria are not
silently reopened or used to erase it.

Completion therefore requires:

1. Resolve the existing policy question through its current Main/Root custodian;
   pin the actual answer and explicit actor/audience/lifecycle semantics.
2. Freeze the corresponding public/server contract with Root's shared custody;
   obtain exact new-file and integration-hunk grants before writing them.
3. Deliver the actual create/read/accept/revoke or alternative decided lifecycle,
   canonical authorization, bounded persistence and idempotent unknown-outcome
   recovery. Recheck expiry/revocation, identity, party audience and room state at
   their actual use boundaries. Exact lifecycle operations depend on the answer.
4. Preserve ordinary World admission, server capacity, owner process/epoch,
   user/session/event/room ticket binding, one-use consumption, atomic switch and
   disconnect-grace invariants. An invitation alone must not manufacture a World
   membership, physical arrival or a successful full-room transition.
5. Qualify decided lifecycle boundaries and adversarial/race/recovery cases with
   the actual implementation and an owned isolated database; integrate the actual
   authorized consumer through the responsible FE owner. Mock/component proof
   cannot certify a real two-person invitation flow.
6. Deliver committed scope, explicit source/contract pins, commands/exits, evidence,
   cleanup and limitations directly to412e, then consume its canonical publication
   receipt. Do not close W04/whole BE11 while a decided required flow remains open.

QA08 200-user performance qualification remains `DEFERRED_BY_USER`, not passed and
not an invented prerequisite. Functional concurrency, privacy and recovery remain.
No physical meeting/check-in/GPS or venue invitation substitutes for this W04 flow.

## Preserved membership delivery

The prior exact six-path membership source `51be127eb288c3d37d6854910ca8edf5616a1905`
remains frozen. It was published independently as master
`51af7f7a8fdcfe80e0b292b87f4b156864ff776e`, sole parent
`fe3a9712b11b0cbaa7ac1844f13dfcc5af80c7fe`. Responsible-owner qualification is
8 application tests and 2 actual PostgreSQL16.14 cases PASS, with migrate/verify,
cloned ledgers and cleanup PASS. Failures retain their original status.

Publisher's normal push exit0, fresh fetch and independent ls-remote agreed.
Canonical receipt:
`/private/tmp/mcr-publisher-412e-be10-be11-delivery-receipt.json`, SHA-256
`8b7490f1c0a2f4f39ccbfb858a80905563790a2d84cc28637c4489f535e50000`.
No proof-only successor or repeat test is needed for those unchanged bytes.
See `world-admission-leases-completion.md` for the scoped outcomes and limits.
This publication does not close invitations or certify current combined-master
runtime, public Auth/World, device, restart or load qualification.

## Current policy custody

Root1fc's explicit offer and acceptance identify this chat as the virtual-party
producer. Root1fc retains shared Concert RPC/proto/Gateway/public contract and
migration-number custody; World20f retains current World runtime/moderation.
World20f has not received invitation producer ownership through naming alone.
The initial sole new repository write grant is **this file only**.

Main confirmed that no human answer has arrived. Its one existing question remains
pending. The proposal is host to accepted-party members, the host's current virtual
room, five-minute expiry/revoke, ordinary capacity-checked admission and no slot
reservation. Every item in that proposal is **NOT_FROZEN**. Neither this document
nor a fixture chooses it on the user's behalf. Main will forward the actual answer;
no duplicate human question is introduced. Additional semantics such as redirecting
after host movement, reuse, per-recipient acceptance, list/discovery and replay
outcome must be pinned with that answer or the resulting contract; they are not
invented defaults in this preparation.

## Actual source interfaces and gaps

Read revision: published `51af7f7a8fdcfe80e0b292b87f4b156864ff776e` through local
Git objects, without a new checkout, fetch or materializing shared source files.
The retained sparse checkout remains at frozen `51be127`; missing sparse files are
not evidence of missing product implementation. Assessment and execution-plan
history were read alongside the specification. The functional amendment's current
W04 paragraph explicitly records OPEN/NOT_IMPLEMENTED and the unfrozen question.

| Existing interface | Observed behavior | Required invitation seam, conditional on actual policy |
| --- | --- | --- |
| `concert-domain/src/lib.rs`: `AuthUserId`, `EventId`, `ParticipantKey` | Canonical Auth UUID and event-scoped participant key; no second identity store. | Actor/session/event originate in verified owner context, never a browser issuer/role field. |
| `participant/live/guarded.rs`: `BoundParticipantAuthority::new`, `check_bound`, `check_live` | Actual sealed `OwnerGuardScope` binds actor/event/session/request, parent/body digest, operation/target and deadline; live policy checks fail closed. | Reuse the canonical owner mechanism with an invitation-specific bound adapter; do not copy participant constructors as generic invitation authority or treat a boolean as readiness. |
| `product_rpc/scoped.rs`: `ScopedProductProducer::dispatch` / `ScopedProductOwner` | Root acquires one canonical scope and publishes delivery before producer work; representation contains no release authority; scope lasts through actual outer HTTP EOF. | Proposed invitation producer accepts that held scope; registers its actual native transaction and recovery work, returning only a closed representation. Root owns acquisition/delivery/settlement and shared operation routing. |
| `community/party.rs`: `Party::member_access` | Requires active party (`Open` or `Closed`) and an Active member; pending application is distinct from approved membership. `audience_revision` fences private readers. | If accepted-party audience is selected, read current owner/member/state and audience revision under the Party lock. Membership removal, cancellation, block or suspension must deny later private use. Domain snapshot alone is not a current permission grant. |
| `party_live/mod.rs`: `PartyLiveOwnerGuard`, `dispatch_for_scope`, `PartyMutationProducer::authorize` | Guarded real authorization/native tracking; Party commands use event then Party root then pair locking, with current Auth/Friend exclusion retained. | Coordinate a narrow locked membership projection/read port with Party's actual owner; no direct transfer of this existing module. Resolve combined lock order before mixing Party, invitation and World rows. |
| `bootstrap/mod.rs`: `world_admission`; `world_rpc.rs`: `WorldCanonicalAuthority` | Current canonical participant/restriction and release checks complement sealed Gateway/Auth context; identifiers alone do not establish admission. | Fresh admission/eligibility must be obtained at the actual ticket/use boundary, not inferred from invitation creation or cached host state. |
| `world/mod.rs`: `PgWorldRepository::current_owned_membership` | Live connected membership requires matching stored/current owner process and epoch. It queries via its own pool connection. | For a transaction-consistent host-room capture, a new connection-bound read/lock adapter is needed; existing standalone lookup is not a held invitation lease. Exact host-room recapture/invalidation rule awaits policy. |
| `world/mod.rs`: `WorldRepository::admit` / `switch` | Transactional account/event/member/room/owner checks, canonical row identity, current pins and room-space check precede commit; failed full-room switch preserves current membership. | Keep this actual admission path authoritative. Decide and bind the invitation validity/authorization recheck to its use so an expiry/revoke/membership-removal race cannot authorize from an obsolete preflight. Proposed wrapper must not reserve space or bypass tickets unless an explicit later human decision changes the proposal. |
| `world/rpc.rs`: `POLICY`, `VerifiedWorld` | Closed ListRooms/IssueTicket/CurrentMembership methods; verified actor/event/session/target/body/deadline. | Existing World RPC grants do not automatically cover private invitation operations; Root must explicitly extend the chosen closed transport contract. |
| `health.rs`: `/readyz` database probe | A successful database SELECT1 is operational health only. | It is not invitation-producer, canonical authority, accepted-party, room-owner or actual-flow readiness. Missing real providers remain unavailable. |

No invitation-specific producer/module, operation, public DTO or persistence table
was established by this readset. Existing party application/approval, physical
beacon/rally and venue meeting flows are separate behavior, not an alias.

## Proposed disjoint new files: not yet granted

The following paths are a concrete ownership proposal, not permission to write:

| Proposed new path family | Responsibility |
| --- | --- |
| `backend/crates/concert-domain/src/world_invitation/mod.rs` | Pure decided invitation identity, revision/state and lifecycle invariants; no adapters or fabricated authority. |
| `backend/crates/concert-application/src/world_invitation/mod.rs` | Typed commands/queries, current authority and repository ports, clock/error handling; no browser or service implementation dependency. |
| `backend/crates/concert-domain/tests/world_invitation.rs` | Actual decided domain boundary/transition tests. |
| `backend/crates/concert-application/tests/world_invitation.rs` | Authority unavailable/denied, exact bindings, decided expiry/revoke and idempotency behavior, using deterministic ports. |
| `backend/services/concert-service/src/world_invitation/mod.rs` | Producer orchestration under Root's actual held scope; closed input/output/error validation. |
| `backend/services/concert-service/src/world_invitation/authority.rs` | Bound canonical/Party/World projection adapter; no second owner acquisition or release-on-return. |
| `backend/services/concert-service/src/world_invitation/repository.rs` | Invitation storage, DB-time checks, revisions and transactional receipts/outbox/recovery integration. |
| `backend/services/concert-service/src/world_invitation/protocol.rs` | Decided wire-to-command parsing/closed response mapping; field/operation names remain proposals until contract freeze. |
| `backend/services/concert-service/src/world_invitation/product.rs` | Candidate `ScopedProductProducer` adapter; actual Product versus World transport decision belongs to Root's shared composition. |
| `backend/services/concert-service/tests/world_invitation.rs` and `tests/support/world_invitation/round.py` | Actual-PG authorization/lifecycle/concurrency/recovery cases and owned fixture/migration-ledger/cleanup runner, after finite execution allocation. |

No current domain/application/service module export, Party module, World module,
shared contract or migration path is acquired by proposing these names. No new
dependency is currently justified; manifests and retained lockfiles stay with Root.
New tests/helper are to be authored only after their exact grant and implementation
prerequisites, without creating a separate manager validation stage.

## Exact shared integration requests for Root

| Root-owned seam | Bounded required change / decision |
| --- | --- |
| `backend/crates/concert-domain/src/lib.rs`; `backend/crates/concert-application/src/lib.rs`; `backend/services/concert-service/src/lib.rs` | One explicit `world_invitation` module registration in each if the proposed new families are selected. Root may delegate only those exact additions. |
| `backend/services/concert-service/src/product_rpc/mod.rs` | If Product transport is selected: one closed invitation operation family, exact operation IDs, target/body parser, verb/path/idempotency binding and representation selection. Existing Party and World operations remain unchanged. |
| `backend/services/concert-service/src/product_rpc/scoped.rs` and actual Root producer composition | Retain existing acquisition/delivery wrapper; mount the invitation producer in the actual dispatcher. No blanket rewrite or scope-settlement transfer requested. |
| `backend/services/concert-service/src/main.rs` and Root's current Product mount module | Construct real invitation providers/repository and register the producer under actual runtime readiness; missing dependencies remain unavailable. Root identifies the current mount owner before any shared write. |
| `backend/services/api-gateway/src/adapters/in/http/concert_product/mod.rs`; `backend/services/api-gateway/src/adapters/out/concert_product/mod.rs` | Candidate closed public route-to-Product operation mapping and guarded relay, using existing actor/session/deadline/body/EOF semantics. No broad router ownership is requested. |
| `backend/proto/concert/v1/product.proto` or `world.proto`; `backend/services/concert-service/src/world/rpc.rs`; Gateway `concert_world.rs` | Root chooses one transport family. Generic Product envelope may need no new proto RPC. WorldService alternative requires explicit method/actor/workload/deadline policy and generated-type changes; no dual implicit API or new caller trust. |
| `backend/docs/api/mcr-domain.openapi.json`; `contracts/generated/mcr-domain.openapi.json`; actual source registry/generator and generated public types | Root pins the source registry/generator and operation/state/error shapes; generate through its actual owner, never edit generated files as an untracked contract shortcut. |
| Existing Party authority/read projection; existing World membership/admission owner | Explicit narrow connection-held projection/use adapter, exact combined lock order and revocation/admission boundary. Root resolves direct Party owner and World20f coordination; this document does not grant edits to either shared implementation. |
| `backend/services/concert-service/migrations/<ROOT_RESERVED_NUMBER>_world_invitation.sql` | Only if durable invitation state is selected: Root reserves exact filename/number and grants table/index/foreign-key/receipt or outbox integration hunks. No number, table schema, TTL literal or retention policy is assigned here. |
| Existing command-status/BE15 recovery and invalidation registries | Select existing receipt/unknown-outcome and current audience/event/account revocation integration; delegate exact operation-specific registrations rather than editing unrelated history or recovery. |

Suggested command vocabulary is create/list-or-get/accept/revoke with opaque
server IDs, revisions, typed refusals and exact-body idempotency. It is a proposal
for Root/counterpart contract work, not a public API freeze. HTTP paths, recipient
scope, visibility, expiry semantics and reuse/accept behavior remain undecided.

## Validation plan and truthful gates

No compiler, test, SQL, PostgreSQL, browser, service, target or application payload
ran under the documentation allowance. Source/document inspection and Git commands
are preparation, not producer acceptance. Existing membership proof stays at its
actual source and is not replayed for this document.

After actual policy/interfaces/grants, the implementation owner will choose and
run relevant checks under fresh finite allocator offers:

- Pure tests for exact decided expiry/revision/state boundaries and authority
  unavailable/denied, spoofed actor/session/event/party/room, malformed bodies,
  idempotency replay versus changed-body conflicts and information minimization.
- Actual-PG current party audience, host-room/owner generation and DB-time state;
  concurrent accept/revoke/expiry/removal/account or event closure; receipt/outbox
  atomicity, crash/unknown outcome and reload recovery as applicable to the contract.
- Two real authorized accounts through actual transport and consumer, including
  denied unrelated/removed/blocked viewers and full-room preservation. Capacity,
  single membership and ordinary ticket consumption must remain canonical.
- Migration ledger, isolated persistence/restart where relevant to new durable
  state, cleanup/resource release, exact source/executable/contract pins and
  committed publication receipt. Consumer/mobile/accessibility duties stay with
  the explicit FE owner; no new King/SL/QA approval stage is created.

Exact test cases will follow the actual policy instead of enshrining this pending
proposal in fixtures. Full producer, integration and W04 acceptance remain open.

## Source-only preparation receipt

Offer `W04-PRODUCER-01-DOC16-0428` accepted at epoch `1793852856.044497`.
It allows only this new document plus bounded private control/Git metadata in the
retained worktree: TOTAL16MiB, processcap4, RSS512MiB, no app execution slot,
WORK<=1800s/FULL<=1860s and hard FULL05:05UTC, earlier deadline wins. The earlier
ordinary booking is not reused. The entire16MiB is charged once inside the same
BE10/11 family: NEW>=669,372,416/outside>=502,435,840, plus every higher maximum;
WHOLE carries at least67,127,062,528 bytes from applicable prior global accounting,
not fresh measured occupancy or a capacity promise. No deletion credit/cap reset.
Fresh free space and current combined ordinary/all-applicable future growth floors
are checked before helpers. Source allowance does not authorize application writes.

The first documentation read encountered an absent sparse functional-plan file;
it was then read from the canonical published Git object. No source file was
materialized or treated as missing product behavior. A peer's preaccept sampler
also found an incomplete ordinary-observation identity mirror. The owner corrected
future metadata to actual kernel PID/birth/image/cwd/role/script and declared typed
sys.argv digest before helpers. The old actual process birth was probed absent;
no historical sampler failure was upgraded or fake process exit asserted. Private
facts remain in `/private/tmp/mcr-w04-producer-6b5d-20261105/`.

Read provenance (all SHA-256 at published revision above):

| Path | SHA-256 |
| --- | --- |
| `MCR_IMPLEMENTATION_PLAN_V2_EN.md` | `e19732ac9c5c5d5397652bf047645da0640ec72c97f59869974680c4e7f69a36` |
| `docs/mcr/project-assessment.md` | `e8203528b95d46faa2bfd92d2929e5723b99f52df61dc6febd613696d69ca807` |
| `docs/mcr/execution-plan.md` | `5a9baf9537e3848faf8ca74f28523ffeeeb4003ad2e3f16cc6ceb509032bf634` |
| `docs/mcr/functional-delivery-plan.md` | `72aa19c6aef348eb8cccad4b4b7bad0e9cac6ae222aee0135e28f1bdd12420f0` |
| `backend/crates/concert-domain/src/community/party.rs` | `85f4cfa70ea937baa840f52cf179fbc2ea6a75f36b864a3c6755528474c85675` |
| `backend/services/concert-service/src/party_live/mod.rs` | `89a63c4d460fc620a04efdf2408d290f5c7844e594b76a0d4b47e5014803a492` |
| `backend/services/concert-service/src/product_rpc/scoped.rs` | `f4b51f65f60aa4b26eaf646f3b0368d5c0e31863b5af489facf75993a843bf4b` |
| `backend/services/concert-service/src/participant/live/guarded.rs` | `e01e126edcd2347708a340bf0e714d6a2c3f943ec84b2e1cc427534d4f9ce928` |
| `backend/services/concert-service/src/bootstrap/mod.rs` | `6f1a39e1356284d2616c231b65bea49f80951d9ef2a53915bb8c6f43e52cd829` |
| `backend/services/concert-service/src/world/mod.rs` | `dd3962147909c65f41a675cb3063b30f8e1a9df12d1e99d63a0e59be11f12721` |
| `backend/services/concert-service/src/world_rpc.rs` | `2a31a36f6c77de3bb408d1cb21be01baca5b471c1acb614c06051d7dddf6ea98` |
| `backend/services/concert-service/src/world/rpc.rs` | `67e1419231ca138aad4b330c70a55f1efa1500087d1055ca623ff37a4026c8fd` |
| `backend/services/concert-service/src/health.rs` | `7e83457576f5a74fd58d636c422f0d85963755855454572ab066ef4514e66680` |
