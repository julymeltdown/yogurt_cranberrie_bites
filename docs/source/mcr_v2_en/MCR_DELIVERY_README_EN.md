# MCR Concert World — English Development Package v2

## Canonical repository and scope

The delivery repository is **julymeltdown/bigyeting**, branch **master**, for both frontend and backend. The complete English specification is `MCR_IMPLEMENTATION_PLAN_V2_EN.md` at the repository root. That specification supersedes the earlier Korean plan wherever they conflict.

This delivery contains the specification and planning/export tools. It does **not** contain a completed application rewrite, and does not certify application builds, migrations, provider authentication, mobile performance, or deployment. All implementation tasks begin at `NOT_STARTED`.

The existing product UI and repository arrangement are not constraints. Reuse only reviewed **non-visual frontend logic** from Bigyeting; write all UI anew. Import the Port-rust backend foundations into this repository, port the required Bigyeting introduction/age/privacy logic, and deliver the resulting tested code to master. Preserve Git history, do not force-push, and do not destroy or migrate live data as part of this layout rewrite.

## Fixed frontend stack

| Concern | Decision |
|---|---|
| Application and language | React/React DOM `^19.2.7`; TypeScript `~6.0.2`, strict mode |
| Build and package manager | Vite `^8.1.1`; pnpm `11.7.0` |
| Routing | React Router `^7.18.1`; persistent authenticated world shell |
| Server/local state | TanStack Query `^5.101.2`; Zustand `^5.0.14` |
| HTTP and validation | ky `^2.0.2`; Zod `^4.4.3` |
| Forms | React Hook Form `^7.80.0`; resolver `^5.4.0` |
| Styling | Tailwind CSS `^4.3.2` and completely new CSS variables/components |
| 2D world | Add exactly Phaser `3.90.0`; Tiled JSON and versioned PNG atlases |
| Realtime | Native WebSocket; matching mock and real transport interfaces |
| Tests | MSW, Vitest, Testing Library, Playwright, Oxlint from the retained lockfile |
| Runtime qualification | Node 24 LTS selected; C02 pins a compatible patch and image digest |

The ranges above come from the inspected Bigyeting manifest at `5eecdcaffd1a9d505b9ab2a974f8d5623217e90a`. They are not independently tested resolved versions. Preserve retained lockfile resolutions and qualify them in C02. Phaser is the explicit new addition, not a claim about the latest release.

No old JSX controls, pages, widgets, CSS, theme objects, layouts, brand assets, animation treatment, or visual snapshots may be reused. A mixed UI/logic module must be split, with the headless logic reviewed and regression-tested. Do not import Mantine rendering components or retain Three.js, GSAP, FFmpeg, and old 3D/video behavior without an actual MCR requirement.

## Important integration corrections

Bigyeting's existing HTTP transport includes cookie credentials and CSRF handling, while the selected Port-rust authentication foundation uses an access-token/refresh arrangement. Its old `/v1/auth/email/...` requests are not drop-in replacements for Port-rust's auth routes. FE01/FE09 and BE03 explicitly implement and test that boundary. Keep access tokens in memory; do not store them in localStorage.

Existing query keys commonly use `activeXAccountId`. Replace this scope with canonical `user_id + event_id`; an X account is optional, not the identity required to enter the world. Port the full age-claim validity semantics, including expiry and the Seoul birthday boundary. Never label a self-declared age as verified.

## Plan inventory

| Work category | Count |
|---|---:|
| Foundation/contracts | 8 |
| Art and venue assets | 4 |
| Backend implementation | 44 |
| Frontend implementation | 41 |
| Verification | 10 |
| Integration/release gates | 5 |
| Optional extensions | 6 |
| **Total tasks** | **118** |
| **IA entries** | **64** |

There are 112 launch-required tasks and six optional extensions. The IA contains 61 launch-required entries and three optional entries. A screen entry can describe a page, panel, modal, or global state. Task priority P0/P1 and phase number 0–5 are separate concepts.

Each task has an ID, owner lane, dependencies, target implementation paths, concrete implementation behavior, required acceptance evidence, IA links, and repository/branch delivery requirements. The task graph has 297 dependency links and no cycles. No frontend task has a backend implementation task as a dependency; frontend feature work uses frozen contracts and deterministic mocks.

## Reproduce machine-readable exports

From the repository root, with Python 3.10 or newer:

```bash
python3 MCR_EXPORT_PLAN_EN.py \
  --source MCR_IMPLEMENTATION_PLAN_V2_EN.md \
  --output-dir mcr-plan-export
```

The downloadable task package also includes `PLAN_TABLES_EN.md`, a standalone transcription of the full specification's IA and task tables. To regenerate its supplied exports:

```bash
python3 MCR_EXPORT_PLAN_EN.py \
  --source PLAN_TABLES_EN.md \
  --output-dir generated
python3 TEST_PLAN_EXPORT_EN.py
```

The exporter creates `TASKS_EN.json`, `IA_EN.json`, `DEPENDENCIES_EN.json`, `VALIDATION_EN.json`, and `DEVELOPER_TASKS_EN.md`. The downloadable package additionally includes the exporter regression test and approved design reference. The canonical narrative specification remains in the GitHub repository; the package is the portable implementation ledger and execution guide, not a byte-for-byte repository archive.

## Release-policy clarification

A task's `depends_on` list describes scheduling prerequisites. It is **not** the entire release checklist. **IN05 must review every launch-required task and IA entry**, including public/authentication screens and operations screens that may not appear in its direct dependency list. The exporter lists the complete release-required task and IA sets. A scope cut must be explicitly approved and recorded as a limitation; deleting a dependency cannot silently waive acceptance.

Topological waves are diagnostic. Do not wait for every task in a wave to finish; start each task when its own prerequisites are satisfied. A single integration owner serializes changes to contracts, central routers, manifests, and lockfiles. Worker branches/worktrees may be used locally, but completed tested results must reach `origin/master` in this repository.

## Actual validation performed for this planning delivery

Structural validation checked expected counts, unique IDs, resolved dependencies, an acyclic graph, complete IA-to-task references, implementation ownership and acceptance text, and frontend independence from backend implementation.

Nine local exporter regression probes passed: duplicate task ID, missing dependency, cycle, unknown IA task, uncovered IA entry, accidental FE-to-BE coupling, empty-input false success, non-English-source detection, and deterministic repeated export. These are **planning-tool tests**, not application acceptance tests. The full application's future test obligations remain NOT_RUN until executed against the actual implementation candidate.

## Implementation completion record

Record task ID, source and contract revisions, changed paths, commands and exit codes, evidence paths, limitations, local commit, and verified remote master commit. Use `MOCK_READY`, `CONTRACT_READY`, `INTEGRATED`, and `ACCEPTED` accurately. Never report a mock-only flow or a test file that was merely generated as an executed end-to-end PASS.
