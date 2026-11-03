# Parallel delivery authority — 2026-11-04

The user requested functional scrum leads and explicitly stated that the main
chat and scrum leads are progress coordinators, not mandatory code reviewers.
This direction supersedes older main-review and staffing restrictions in the
historical execution, capacity and coordination notes.

## Roles

- Main: cross-team progress, staffing, ownership conflicts, and user reporting.
- World SL: admission, realtime world, avatar rendering, assets, and IN01 progress.
- Community SL: parties, giveaways, real meeting/pickup flows, and IN02 progress.
- Social SL: shared relationship policy, feed, profiles, consent, and IN03 progress.
- Delivery integrator: sole master writer/publisher, shared-file ownership,
  integration checks and root composition. No main/SL approval is required.
- Implementation owners: deliver implementation, relevant validation, committed
  evidence, and fixes in isolated frontend/backend worktrees.
- QA: concrete risk/failure reviews and integration qualification, not blanket
  approval of every patch.

## Delivery rules

Send completed commits directly to the delivery integrator and notify the owning
SL. The integrator checks path ownership, compatibility and relevant validation,
integrates serially, and pushes verified checkpoints to julymeltdown/mcr master
without force-push. Do not repeat broad tests on unchanged code or wait for an
extra coordinator review. Preserve accepted requirements and honest evidence.

Each chat has a concrete unbudgeted goal. An active UI label is not work evidence.
SLs record the actual deliverable, next action, blocker owner, commit and remote
delivery state. Do not repeat unchanged polling/final messages. A completed
bounded handoff does not require indefinite waiting for main-chat acknowledgment;
required concrete fixes remain part of that handoff's goal.

Read frozen core/realtime contracts and pin the existing C04 candidate for
bounded feature work. Full C04 freeze is not an artificial barrier to independent
implementation. Changes crossing domains require an explicit owner and affected
consumer checks, not another management approval layer.

Backend and frontend never share a writable checkout. Shared library exports,
migrations, contract files, manifests and routers require named ownership. Reserve
migrations 0004 for bootstrap, 0005 for world admission, 0006 for relationship
policy, and 0007 onward by explicit coordination with delivery.

## Resource scheduling

Source work, lightweight frontend checks, SQL design and deterministic fixtures
run in parallel. The existing Rust target currently belongs to the bootstrap
worker; transfer that lease explicitly and use pinned Rust 1.88.0, two jobs and
debug=0. Do not duplicate full Rust targets: the disk has about 30 GiB free at
this checkpoint. Resource scheduling must serialize only expensive build phases,
not implementation. Use isolated owned PostgreSQL fixtures and preserve cleanup.

## First work wave

- Existing world workers continue bootstrap, admission/rooms, shared avatar
  rendering and the committed production-entry adapter. Delivery handles the
  pending handoffs and delegates exact root wiring as needed.
- Community backend starts party/giveaway aggregates and atomic commands using
  the existing receipt/outbox port and an explicit fail-closed policy port.
- Community frontend starts authoring/owner flows against pinned contracts and
  deterministic fixtures independently of backend availability.
- Social backend starts BE37 relationship policy and persistence, which unblocks
  community and realtime decisions; then continues its owned social work.
- Social frontend starts feed/thread screens against pinned contracts.
- Art starts actual selected-source derivatives and runtime-ready asset work,
  with no legacy or invented replacement characters and no synthetic product art.

English only, no model/reasoning overrides, no decorative left accents, no source
archive modification, no real-world arrival inferred from virtual coordinates.
