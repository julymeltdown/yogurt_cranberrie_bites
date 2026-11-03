# Project assessment — 2026-11-03

## Finding

MCR Concert World is a mobile-first social web application for My Chemical Romance concert fans. Its home is a real multiplayer 2D pixel world, with event community and coordination features presented in a persistent shell. It is substantial full-stack software: frontend, original art, authoritative realtime simulation, transactional event workflows, privacy controls, moderation, and operations all belong to the required scope.

The attachment is a planning delivery, not an application source archive. Its 118 tasks begin at NOT_STARTED. The new `julymeltdown/mcr` repository had no remote refs and the local workspace was empty at intake. This checkpoint imports the plan and establishes the working process; it does not complete C01, C02, or any application task.

## Product shape

| Surface | Required behavior |
|---|---|
| WORLD | Multiple real users move in the same room, see permitted presence and avatars, reconnect, switch rooms, inspect places, and customize appearance. |
| FEED | Chronological event posts, images, comments, reactions, notices, and safe links to related content. |
| MEET | Entry parties, applications and approvals, attendee introductions, explicit physical meeting beacons, and consent-based social sharing. |
| GIFTS | Giveaways, stock-aware reservations, expiry/cancellation, authorized pickup details, and one-time pickup confirmation. |
| WALL | Shared 128×128 pixel mural with 16 colors, server cooldown, moderation, revision recovery, and final freeze. |
| ME | Avatar and profile, preferences, social handles and requests, age/privacy state, notifications, blocks, and account deletion. |
| Operations | Event/place management, report handling, moderation, audit trail, feature switches, closure, cleanup, and a public read-only archive. |

REVENGE, PARADE, and DANGER change presentation over the same map, collisions, room membership, and place identifiers. The five reference compositions are visual inspiration for one product. Their chat boxes, levels, and decorative controls do not add requirements: free-text party chat is optional, while general DMs, voice/video, payments, resale, continuous GPS, and leveling are excluded.

The world survives panel navigation without creating another game instance or socket. Mobile panels, browser back, safe-area and keyboard handling, accessible DOM alternatives, and readable original avatars are required. Primary navigation and controls are English; user content can use other languages. See specification §§5–6 and the [reference image](../source/mcr_v2_en/reference/approved_mobile_world.png).

## Architecture and source reuse

| Layer | Selected direction | What still needs qualification |
|---|---|---|
| Browser | React 19, TypeScript, Vite, React Router, TanStack Query, Zustand, ky, Zod, React Hook Form, Tailwind; exact retained resolutions from the source lockfile | Clean lockfile install, exact Node 24 patch/image digest, engine compatibility, reviewed headless extracts, entirely new rendering |
| World | Phaser **3.90.0**, Tiled JSON, PNG atlases, native WebSocket | Original art and manifests, authoritative movement, filtered stream sequencing, recovery, actual mobile rendering |
| Backend | Rust workspace from port-rust; Axum/Tokio, Tonic/Prost, SQLx/PostgreSQL, Redis; one added Concert service | Reproducible import/build, path/tooling relocation, isolated databases, authenticated interservice calls |
| Integration | Generated HTTP contracts plus realtime/asset schemas and shared deterministic fixtures | Exact auth operation mapping, error/transition rules, mock/real parity, producer/consumer tests |

The pinned source trees and manifests were inspected during intake. Bigyeting's retained lockfile exists at `frontend/pnpm-lock.yaml`; the Rust workspace and lockfile live under `boilerplate-grpc-rust/`. The latter contains `proto/` and `docs/api/openapi.json`, so a proposed import maps that subtree to `backend/`. Actual module/dependency/license/tooling audits and source-to-test mappings remain C01/BE01 work. No application source has been imported yet.

Bigyeting's old cookie/CSRF client cannot be copied unchanged into the selected access-token/refresh backend. New access tokens stay in memory, refresh credentials use Secure HttpOnly cookies, and refresh/cookie mutations need appropriate origin/CSRF protections. One Auth UUID is canonical. Optional X/Instagram handles must not become the login prerequisite or cache identity. See specification §§2–4 and §7.

## Correctness requirements that affect scheduling

1. **Contracts before feature divergence.** C03–C06 freeze HTTP, state transitions, realtime, and assets. FE features can then progress with fixtures without waiting for matching backend features.
2. **Privacy early in the world path.** BE37 block/relationship policy feeds BE13 presence and BE09/BE16 event workflows. Its phase number is not a reason to defer it until the end.
3. **Real and virtual location are separate.** In-game movement never proves physical presence. Meeting guidance needs a reviewed place, explicit audience, expiry, and revision; the approved image cannot establish safe real-world routes.
4. **Database-backed transactions.** Party approvals cannot overfill capacity. Reservations, release, expiry, and pickup cannot oversell or double-consume stock. Idempotent command receipts recover uncertain outcomes.
5. **Consent is enforced on reads and streams.** Public profiles omit private social handles and claims. Reveal rechecks grants, expiry, blocks, and suspension; revocation and identity changes invalidate caches and subscriptions.
6. **Realtime is authoritative and recoverable.** One-use HTTP-issued tickets, server-owned movement, per-recipient stream sequence, bounded queues, room epochs, and fresh snapshots are mandatory.
7. **Acceptance requires actual execution.** A screenshot or mock flow does not satisfy the two-account real-server gates. The 200-connection/four-room scenario and 30-FPS mobile goal are qualification targets, not established capacity.

## Scope accounting

| Category | Tasks |
|---|---:|
| Foundations and contracts | 8 |
| Art and venue assets | 4 |
| Backend | 44 |
| Frontend | 41 |
| Verification | 10 |
| Integration/release | 5 |
| Optional extensions | 6 |
| Total | 118 |

There are 112 required tasks and 61 required IA entries; optional scope contains six tasks and three IA entries. All 297 dependency links resolve and the graph is acyclic. P0/P1 **priority** differs from phase 0–5. IN05 must inspect the complete release checklist even where its direct scheduling dependencies omit a required screen or task.

## Open decisions and evidence gaps

- C01 must finish a per-module import/reuse inventory, license/provenance checks, build-tool mapping, and an isolated environment plan. Verified source availability is not completion of that audit.
- C02 must qualify the specified versions instead of assuming the source manifest proves they install and build.
- C03/C04 must resolve exact inherited operations and command semantics, including compressed party transitions, the reservation READY transition, and inconsistent giveaway terminal labels between G03 and §8.2. Keep the supplied requirements visible until a documented contract decision resolves them.
- Service eligibility, real-provider availability, age-claim verification, and venue admission are separate concerns. Configuration and operational choices remain to be established before release.
- Venue/place verification, original production-ready assets, operator contacts, notice content, retention values, deployment environment, and actual device testing are outstanding.
- The source names a November 7 event; that statement is context from the source, not a fresh operational verification or a promise that this complete backlog fits before it. Recheck official operational information before publishing attendee guidance.

## Evidence for this checkpoint

The original archive's 13 listed hashes match. The upstream narrative was retrieved at a pinned commit and preserved separately. Narrative and table task content match. Both original and adapted exporters pass their nine supplied regression probes, their committed outputs reproduce, and all active task destinations resolve to `julymeltdown/mcr/master`. See [validation evidence](evidence/planning-validation.json) and [provenance](source-provenance.json).

The parallel [backend](../analysis/backend-assessment.md), [frontend](../analysis/frontend-assessment.md), and [acceptance](../analysis/acceptance-assessment.md) assessments support the [execution plan](execution-plan.md). They are analysis, not feature acceptance. Application builds/tests, database migrations, provider flows, real-device checks, load tests, and deployment are NOT_RUN. Accepted implementation tasks: **0/118**.
