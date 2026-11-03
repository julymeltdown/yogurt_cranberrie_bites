# Community delivery tracker

Updated 2026-11-04 KST. Progress coordinator: `01a1a1fe-c5f1-7e60-87c2-43785b07f941`.
Authority: [parallel delivery](../parallel-delivery.md) at source base
`f0e44bd1bba943b02319f9d9a42433e253e8de6f`. This chat adds no code-review or
approval gate. Implementation owners send validated commits directly to delivery
`01a1a1fc-7f3d-7ff1-9416-e3435c8a6f08`; main owns staffing.

## Live delivery

| Task | State and implemented deliverable | Assigned writer / scope | Validation and commit | Next action / blocking owner |
|---|---|---|---|---|
| BE16–18 | IN_PROGRESS: party aggregate rules authored; service transaction slice underway. Capacity includes owner; removal changes audience revision. | BE `01a1a1fd-6b5d-75b0-8d42-01ade79b9393`, isolated `be-community`, base `f0e44bd`; assigned community modules/tests, migration `0007_community.sql`, minimal named exports and backend evidence only. | Source checkpoint `4bf2bd4c4ec30bc2ce536a2120d86372173ef6e2` committed; planning/rustfmt/py_compile exit 0. Cargo/native PG NOT_RUN pending lease; not a validated handoff or published feature. | Deliver party slice first. Delivery owns Cargo lease/composition; BE37 owns common relationship policy. |
| BE20–23 | IN_PROGRESS: giveaway domain source proceeding alongside party validation queue; stock/reservation/release/expiry/pickup remains required. | Same BE writer; no ownership expansion. | NOT_RUN; no new commit. | Start domain/atomic commands with retained BE15 receipt/outbox; qualify five-way last-unit race, restart-safe expiry and one-time pickup. |
| FE22 | MOCK_READY: scoped RHF authoring form/controller committed with conflict preservation, explicit draft restore and receipt recovery. | FE `01a1a1fd-6ec9-71e2-a2c6-b5dddca7542e`, isolated `fe-community-authoring`, base `f0e44bd`; `frontend/src/features/party-authoring/**` first. | Worker `1c38ad459bf133411f9a11e9e836410642fca3fa`, parent `f0e44bd`; eight focused tests, typecheck/build/lint and Chromium 360/390/430/1280 checks exit 0. Delivery integrated as `e82a220`, parent eight focused tests PASS; combined build running at report time. Remote publication not yet verified. | Direct delivery handoff sent; delivery owns shell integration and affected checks. Owner continues FE23 without waiting. |
| FE23 | IN_PROGRESS: owner management/rally started after FE22 commit. | Same FE writer; assigned party-management/rally feature paths. | NOT_RUN; no new commit. | Use pinned contracts/fixtures. FE20 and BE09/19 are separate physical-flow dependencies, not an FE22 barrier. |
| FE24 | MOCK_READY: existing discovery/detail/own-activity delivery. | Existing scoped delivery; no new discovery writer assigned here. | Progress ledger records worker `595c07f`, integrated `3cb3f9c`/`8c33ba3`; browser fixture/affected checks available. No real reservation acceptance. | Retain consumer compatibility for FE25/26; delivery owns root composition. |
| FE25–26 | NOT_STARTED: giveaway owner tools and reservation/pickup follow party authoring/management. | Same FE authoring writer; assigned gift-authoring/gift-management/pickup paths only. | NOT_RUN; no new commit. | Deterministic fixtures permit independent implementation; later real stock/pickup qualification is mandatory. |
| BE09/19 | IN_PROGRESS: separate physical meeting owner is coding beacon TTL/audience/revision invariants before rally. | BE `01a1a202-3b96-7822-ab54-2f7d29aa42e5`, isolated `be-physical-meeting`, base `48458d5`; domain/application `beacon/**` + `rally/**`, service `meeting/**`, assigned tests/scripts/evidence. | NOT_RUN. BE08 typed place repository and checked-current selection already available. | Deliver beacon slice first using transaction-aware place/policy/member ports. Migration `0008_meeting_beacons.sql` reserved. Delivery owns public adapters. |
| FE20 | IN_PROGRESS: separate scoped beacon form/detail/explicit-arrival owner implementing against pinned fixtures. | FE `01a1a202-3ff2-7a93-95c3-374c6df97281`, isolated `fe-physical-meeting`, base `48458d5`; `frontend/src/features/meet-beacon/**`, `docs/mcr/frontend/meeting-beacon.md`. | NOT_RUN. | Deliver route-ready controller/form/detail and explicit-arrival controls; FE23 owns actual rally orchestration. Qualified photo attachment lacks a contract field. |
| IN02 | NOT_STARTED: two-client real party, physical rally, reservation and pickup journey not run. | Delivery owns combined qualification/composition. | Real public community routes, policy composition, physical provenance and multiuser gate NOT_RUN. | Keep IN01, BE09/19, BE23/24, FE20/22/23/25/26 evidence distinct. No unit/mock result certifies this gate. |

## Dependencies and ownership

- Direct BE↔FE and worker→delivery contacts established. FE reported actual `get_goal` ACTIVE for FE22/23/25/26 implementation,
  validation, documentation, committed handoffs and fixes; BE likewise confirmed
  actual ACTIVE unbudgeted BE16–18→BE20–23 goal. New BE09/19 and FE20 workers
  were asked for actual goal proof at a natural checkpoint. Worker active UI labels are not goal proof. This SL's own unbudgeted goal is ACTIVE,
  verified with `get_goal` before creation and the returned `create_goal` result.
- BE37 owner `01a1a1fe-16e1-7ee3-a9dc-8b5f037cc24c` sent the BE worker a planned
  typed event/actor/subject/field query, bounded decision and transaction-aware
  authorization API. Committed compatibility and actual authority composition
  remain to be verified. No duplicate permanent policy engine is authorized.
- Delivery confirms retained BE15 ports unchanged from `f0e44bd`. Cargo lease
  order: admission → delivery Gateway/startup combined → community party → BE37.
  Serialize expensive builds only; no duplicate Rust target. Worker coding and
  lightweight frontend checks continue independently.
- BE08 exposes `PlaceRepository::select_physical` and
  `PgPlaceRepository::select_physical_in_transaction`. A separate BE09/19 owner
  can implement audience/revision/DB-time TTL and explicit self-reported rally
  invariants against typed place/policy/membership ports. Community BE retains
  party membership writes. Real place provenance is still a publication/runtime
  requirement, never supplied by virtual coordinates or fixtures.
- C04 remains a review candidate. Source-base domain history ends at
  `ca9f955cd22f7e4c1da029d034c1923f1e98bb9d`; FE pins the C04 artifact at `f0e44bd`, operations SHA256
  `42ed48ee0cad846b94f9f7a5d39a38beeeb5f5b41e0129f157e51ff31c635223`; BE must name
  its exact pinned contract revision in the handoff. The full C04 freeze is not a blanket work barrier.
- Main staffed the proposed disjoint BE09/19 and FE20 owners. Delivery reserved
  `0008_meeting_beacons.sql`; the exact assignment permits domain/application
  `pub mod beacon` / `pub mod rally` and service `pub mod meeting` lines. This
  supersedes the earlier unified-module proposal; no competing writer exists.
  No writer receives root routers, RPC/proto or manifests implicitly. Delivery alone integrates/publishes master serially.

## Evidence and limitations

Source: required specification/task cards, project assessment, execution plan,
progress ledger and actual worker command snapshots. BE authored community
aggregate files; FE authored party-authoring controller files and completed the
pinned install. These observations are implementation progress, not test results.
Existing FE21 is MOCK_READY at integrated `78cfb7c`; its party-discovery controller
is a consumer prerequisite already available to FE23, with real-server acceptance
still open. BE24 image handling is a separate IN02 dependency, not notifications.

Delivery reported bootstrap integrated at `48458d5` and published checkpoint
`920631d64c85355df33163a51cd4d1643f4fa109`. At this snapshot `git ls-remote origin
refs/heads/master` independently returned that SHA (exit 0). This does not imply
any new community feature commit is published. Each feature handoff must record
its source base, changed paths, commit, validation commands/exit codes and actual
master ancestry independently.

Tracker validation: `python3 scripts/mcr/check-plan.py` exit 0, PASS (planning
provenance/structure/exporter/routing only). Application build/database/browser/
device/load/release checks were not run by this documentation-only coordinator.
No generated planning files, source archives, application code or other checkout
were modified. No acceptance counts are changed.

## FE22 first committed milestone

Read the committed `docs/mcr/frontend/community-authoring.md` at `1c38ad4` and
its changed-path list: only the assigned party-authoring feature and lane
evidence. Worker validation includes eight meaningful failure/privacy/receipt
tests and native-browser unknown-outcome recovery without a second POST. No
coordinator code review is required. `git merge-base --is-ancestor 1c38ad4
origin/master` returned 1 at this checkpoint, so this tracker does not claim
publication. FE22 root wiring, real services and devices remain open.

FE04 owner `01a1a202-375e-7401-bfa1-ff8721e90ed6` is a shared media dependency
for FE25/20. FE20 reports the pinned beacon description has no photo attachment
field: display is optional/qualified, upload attachment unavailable pending an
integrator-owned contract amendment. This is routed to delivery, not a reason
to stop the text-based physical beacon. Actual party membership writes stay
with community BE; meeting owner requested its fresh transactional authority
and audience-revision seam directly.

## Next ready work and concrete blockers

The human requires continuous team follow-through. Bounded handoffs go directly
to delivery; owners continue their next assigned slice and required fixes. A
scope transfer requires the former writer to stop first. Current queues:

| Writer | Current deliverable | Next ready scope |
|---|---|---|
| Community BE | Party source `4bf2bd4`, Cargo/native proof pending | Giveaway/reservation domain and transactions; then expiry/pickup qualification |
| Community FE | FE22 `1c38ad4` handed off/integrated `e82a220`; FE23 coding | FE25 owner tools then FE26 pickup; concrete FE22/23 fixes remain owned |
| Physical BE | BE09 beacon TTL/audience/receipt-backed adapter | BE19 explicit member-only rally and native authorization/restart proof |
| Physical FE | FE20 form/detail/arrival controls, validation | Required fixes; proposed exact production meeting-entry adapter/test, only after delivery grants shared-path ownership |

FE20's proposed follow-on adapter is a ready staffing request, not permission to
edit root/shared composition now. Existing root ownership stays with delivery.
No completed worker is kept active merely to satisfy a staffing count.

- **Receipt convention:** delivery qualified member references as `user_id` plus
  revision, bound to the exact canonical party/user HMAC target. Outbox
  `RESOURCE_INVALIDATED` invalidates event community queries; member revisions
  are never compared across parties. FE23 was sent this convention. Rally schema
  identity still needs the actual BE09/19 owner's proposal; uncertain outcomes
  remain locked until receipt and current resource validate.
- **Place revisions:** FE19 supplies catalog revision while the beacon command
  requires a distinct place revision. FE20 requires qualified per-place
  revisions and disables publishing without them. Delivery owns public BE08
  projection/root exposure; catalog revision cannot substitute.
- **Photo binding:** delivery explicitly keeps text-based C04 now. Qualified
  optional display is permitted; photo attachment remains unavailable pending
  an exact contract amendment and privacy/upload-binding consumer checks.
- **Maintenance outbox:** delivery reports an explicit 00:31 KST shared-scope
  grant to community BE: `0011_outbox_maintenance.sql`, application
  `commands/maintenance.rs`, repository `outbox_maintenance.rs`, exact
  `outbox.rs` delivery optional-receipt/typed-origin changes, and dedicated
  tests/script/doc. Use existing leases/dedup; stable transaction-local metadata
  UUIDs, no recipient impersonation or receipt reuse. Delivery-owned
  `command_receipts.rs` / `0009` / tests remain excluded during pending work.
  This is a recorded grant, not a new coordinator request.

Tracker commits `7a1ed34`, `f29a82a`, `181878b` were handed directly to delivery;
master now contains the corresponding documentation history (observed local
master `54900ec`). At the last remote check, `refs/heads/master` was still
`920631d`; FE22 integration and latest tracker publication were not yet proven.
