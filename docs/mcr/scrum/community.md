# Community delivery tracker

Updated 2026-11-04 KST. Progress coordinator: `01a1a1fe-c5f1-7e60-87c2-43785b07f941`.
Authority: [parallel delivery](../parallel-delivery.md) at source base
`f0e44bd1bba943b02319f9d9a42433e253e8de6f`. This chat adds no code-review or
approval gate. Implementation owners send validated commits directly to delivery
`01a1a1fc-7f3d-7ff1-9416-e3435c8a6f08`; main owns staffing.

## Live delivery

| Task | State and implemented deliverable | Assigned writer / scope | Validation and commit | Next action / blocking owner |
|---|---|---|---|---|
| BE16–18 | IN_PROGRESS: party aggregate rules authored; service transaction slice underway. Capacity includes owner; removal changes audience revision. | BE `01a1a1fd-6b5d-75b0-8d42-01ade79b9393`, isolated `be-community`, base `f0e44bd`; assigned community modules/tests, migration `0007_community.sql`, minimal named exports and backend evidence only. | New feature checks NOT_RUN at this snapshot; no new committed handoff or published ancestry yet. | Deliver party slice first. Delivery owns Cargo lease/composition; BE37 owns common relationship policy. |
| BE20–23 | NOT_STARTED: queued after party handoff; stock/reservation/release/expiry/pickup remains required. | Same BE writer; no ownership expansion. | NOT_RUN; no new commit. | Start domain/atomic commands with retained BE15 receipt/outbox; qualify five-way last-unit race, restart-safe expiry and one-time pickup. |
| FE22 | IN_PROGRESS: contract-validated authoring controller authored with scoped drafts, conflict preservation and unknown-outcome recovery; form/tests underway. | FE `01a1a1fd-6ec9-71e2-a2c6-b5dddca7542e`, isolated `fe-community-authoring`, base `f0e44bd`; `frontend/src/features/party-authoring/**` first. | Pinned offline frozen install exit 0. Feature tests/browser checks NOT_RUN; no new committed handoff or published ancestry. | Complete form and targeted checks; deliver directly. No backend-completion dependency. |
| FE23 | NOT_STARTED: owner management/rally follows FE22. | Same FE writer; assigned party-management/rally feature paths. | NOT_RUN; no new commit. | Use pinned contracts/fixtures. FE20 and BE09/19 are separate physical-flow dependencies, not an FE22 barrier. |
| FE24 | MOCK_READY: existing discovery/detail/own-activity delivery. | Existing scoped delivery; no new discovery writer assigned here. | Progress ledger records worker `595c07f`, integrated `3cb3f9c`/`8c33ba3`; browser fixture/affected checks available. No real reservation acceptance. | Retain consumer compatibility for FE25/26; delivery owns root composition. |
| FE25–26 | NOT_STARTED: giveaway owner tools and reservation/pickup follow party authoring/management. | Same FE authoring writer; assigned gift-authoring/gift-management/pickup paths only. | NOT_RUN; no new commit. | Deterministic fixtures permit independent implementation; later real stock/pickup qualification is mandatory. |
| BE09/19 | NOT_STARTED: no assigned writer confirmed by delivery. | Main owns allocation; proposed separate physical-meeting BE owner, no implicit transfer. | NOT_RUN. BE08 typed place repository and checked-current selection already available. | Main to allocate disjoint beacon/rally work; delivery to reserve migration/export ownership. Proposed `0008` is a request, not a reservation. |
| FE20 | NOT_STARTED: physical beacon UI absent from this source base. | Main owns allocation; separate meet-beacon feature writer proposed. | NOT_RUN. | Assign exact feature/test paths separately; fixtures can precede real place data. |
| IN02 | NOT_STARTED: two-client real party, physical rally, reservation and pickup journey not run. | Delivery owns combined qualification/composition. | Real public community routes, policy composition, physical provenance and multiuser gate NOT_RUN. | Keep IN01, BE09/19, BE23/24, FE20/22/23/25/26 evidence distinct. No unit/mock result certifies this gate. |

## Dependencies and ownership

- Direct BE↔FE and worker→delivery contacts established. Both workers were asked
  for actual `get_goal` status/objective at the next natural checkpoint. Worker
  active UI labels are not goal proof. This SL's own unbudgeted goal is ACTIVE,
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
  `ca9f955cd22f7e4c1da029d034c1923f1e98bb9d`; workers must name their exact pinned
  contract revision in handoffs. The full C04 freeze is not a blanket work barrier.
- Main was given a concrete disjoint staffing proposal for BE09/19 and FE20.
  No writer receives shared exports, migrations, root routers or manifests
  implicitly. Delivery alone integrates/publishes master serially.

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
