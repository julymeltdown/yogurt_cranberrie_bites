# MCR Concert World

A mobile-first fan community centered on a persistent multiplayer pixel world. Fans enter with an avatar, meet other attendees, organize entry parties, arrange explicit physical meetups, reserve giveaways, share posts and social handles with consent, and contribute to a collaborative mural.

**Delivery:** [`julymeltdown/mcr`, `master`](https://github.com/julymeltdown/mcr/tree/master). The user's repository choice supersedes the old Bigyeting delivery destination in the original package. Bigyeting and port-rust are source foundations.

**Current checkpoint (2026-11-03): foundation implementation.** The original React foundation and imported six-service Rust workspace build. Accessible UI primitives pass component and Chrome checks. All 84 inherited migrations pass in six isolated databases and survive a database restart. The email/core v0.1 contract is frozen for implementation; service startup, authentication and the multiplayer product remain outstanding. Frontend/backend workers use separate worktrees and file ownership, with reviewed integration into `master`. See the [execution record](docs/mcr/progress.md).

## Start here

- [Project assessment](docs/mcr/project-assessment.md): product, architecture, hard requirements, and known gaps.
- [Execution plan](docs/mcr/execution-plan.md): parallel lanes, dependencies, review checkpoints, and integration rules.
- [Active implementation specification](MCR_IMPLEMENTATION_PLAN_V2_EN.md): complete requirements adapted to this delivery repository.
- [Developer task cards](generated/DEVELOPER_TASKS_EN.md): 118 tasks, including 112 required and six optional.
- [Information architecture](generated/IA_EN.json): 64 pages, panels, modals, and global states, of which 61 are required.
- [Source provenance](docs/mcr/source-provenance.json) and [original delivery](docs/source/mcr_v2_en/MCR_DELIVERY_README_EN.md).
- [Approved visual direction](docs/source/mcr_v2_en/reference/approved_mobile_world.png): five reference compositions for one application with three themes; the image is not a verified venue map or a production sprite pack.

## Technical direction

React/TypeScript/Vite and Phaser 3.90.0 provide the new UI and 2D world. The backend imports the selected port-rust Rust foundation, retaining Auth/Profile/Friend/Post/Notification/Gateway boundaries, and adds one Concert service with PostgreSQL persistence and native WebSocket realtime. Existing Bigyeting headless logic is selectively audited and adapted; all rendering is new.

All feature work uses frozen HTTP, realtime, and asset contracts. FE workers use the same deterministic fixtures as server contract tests. Real-service integration is separately demonstrated with multiple identities.

## Reproduce planning checks

Python 3.10 or newer is required; no third-party Python dependency is needed.

```sh
python3 scripts/mcr/check-plan.py
```

To intentionally regenerate the active baseline after a reviewed specification change:

```sh
python3 MCR_EXPORT_PLAN_EN.py --source MCR_IMPLEMENTATION_PLAN_V2_EN.md --output-dir generated
python3 TEST_PLAN_EXPORT_EN.py
python3 scripts/mcr/check-plan.py --report docs/mcr/evidence/planning-validation.json
```

The check verifies preserved source hashes, narrative/table agreement, both original and adapted regression probes, deterministic committed exports, delivery routing, and the full required release checklist. It is **not an application build, security, integration, device, capacity, or release certification**. Generated task statuses describe the original planning baseline; execution progress must be tracked separately.

## Run the foundation

Install Node **24.22.2**, pnpm **11.7.0** via Corepack, and Rust **1.88.0**.
The [toolchain record](infra/mcr/toolchain.json) pins archive/image provenance.

```sh
scripts/mcr/check.sh frontend
scripts/mcr/check.sh backend
scripts/mcr/check.sh guards
cd frontend
corepack pnpm@11.7.0 dev
```

The browser opens a minimal original foundation screen, not a working concert
world. The backend command builds only: do not launch imported services with their
source-default database addresses. Isolated MCR launch and migration commands are
being qualified separately. `infra/mcr/Dockerfile.frontend-check` is a build-only
image definition; Docker execution and production serving are not yet qualified.

<!-- check: integration baseline commit 2026-11-08 -->

