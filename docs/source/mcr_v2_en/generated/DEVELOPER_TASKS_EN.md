# MCR Developer Task Cards — English

These are implementation specifications, not completed work. All task statuses are NOT_STARTED.
Each unit targets julymeltdown/bigyeting on master. All rendering must be newly authored.
Scheduling prerequisites do not replace the full launch acceptance checklist.

## C01 — Source and rewrite inventory

**Phase:** 0 | **Priority:** P0 | **Lane:** LEAD
**Status:** NOT_STARTED
**Dependencies:** None
**IA coverage:** Cross-cutting implementation

### Owned implementation paths

- `docs/mcr/source-map.md`

### Implementation

Record source commits, copied logic, old entrypoints to retire, actual proto paths, and data-environment separation.

### Required acceptance evidence

Every reuse entry names source/destination/test; no production-data deletion in cutover plan.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## C02 — Reproducible workspace and toolchain

**Phase:** 0 | **Priority:** P0 | **Lane:** LEAD
**Status:** NOT_STARTED
**Dependencies:** C01
**IA coverage:** Cross-cutting implementation

### Owned implementation paths

- `frontend/package.json`
- `frontend/pnpm-lock.yaml`
- `infra/mcr/`
- `scripts/mcr/check.sh`

### Implementation

Freeze retained package resolutions, add Phaser 3.90.0, pin Node patch/image, map backend lockfile, and create scoped checks.

### Required acceptance evidence

Clean frozen-lockfile install and minimal frontend/backend build; record versions and commands.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## C03 — Core HTTP and identity contract

**Phase:** 0 | **Priority:** P0 | **Lane:** LEAD
**Status:** NOT_STARTED
**Dependencies:** C01
**IA coverage:** Cross-cutting implementation

### Owned implementation paths

- `backend/docs/api/`
- `contracts/fixtures/core/`

### Implementation

Map actual auth operation IDs and tokens; define Bootstrap, public/private profile, avatar, places, errors, and capabilities.

### Required acceptance evidence

Schema-valid success/failure examples; exact frontend/backend auth mapping reviewed.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## C04 — Domain HTTP and transition contracts

**Phase:** 0 | **Priority:** P0 | **Lane:** LEAD
**Status:** NOT_STARTED
**Dependencies:** C03
**IA coverage:** Cross-cutting implementation

### Owned implementation paths

- `backend/docs/api/`
- `contracts/fixtures/domain/`

### Implementation

Define party, reservation, social, age, feed, mural, admin commands; actor permissions, idempotency, and versions.

### Required acceptance evidence

Every command has actor, precondition, resulting state, error reason, and retry rule.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## C05 — Realtime wire protocol

**Phase:** 0 | **Priority:** P0 | **Lane:** LEAD
**Status:** NOT_STARTED
**Dependencies:** C03
**IA coverage:** Cross-cutting implementation

### Owned implementation paths

- `contracts/realtime/`
- `contracts/fixtures/realtime/`

### Implementation

Define authenticate, snapshot, input, delta, resync, revoke, room switch, and stream sequence rules.

### Required acceptance evidence

Reject unknown/oversized messages; filtered streams have no false sequence gaps.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## C06 — Map and avatar interfaces

**Phase:** 0 | **Priority:** P0 | **Lane:** LEAD
**Status:** NOT_STARTED
**Dependencies:** C01
**IA coverage:** Cross-cutting implementation

### Owned implementation paths

- `contracts/assets/`
- `docs/mcr/assets.md`

### Implementation

Define atlas frames/anchor, layer IDs, map version, collisions, interactions, and place-source metadata.

### Required acceptance evidence

Small golden asset fixtures validate and align across all themes.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## C07 — Generated clients and fixture harness

**Phase:** 0 | **Priority:** P0 | **Lane:** LEAD
**Status:** NOT_STARTED
**Dependencies:** C03, C04, C05, C06
**IA coverage:** Cross-cutting implementation

### Owned implementation paths

- `scripts/mcr/contracts/`
- `contracts/generated/`
- `contracts/fixtures/`

### Implementation

Generate TS API types, validate Zod compatibility, deterministic clock/seed, mock scenarios, and drift checks.

### Required acceptance evidence

Generated output is reproducible; a deliberate schema mismatch fails CI.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## C08 — New-UI and source-boundary gate

**Phase:** 0 | **Priority:** P0 | **Lane:** LEAD
**Status:** NOT_STARTED
**Dependencies:** C01
**IA coverage:** Cross-cutting implementation

### Owned implementation paths

- `scripts/mcr/check-ui-origin.mjs`
- `docs/mcr/reuse-allowlist.json`

### Implementation

Inventory legacy rendering files and hashes; allow only reviewed headless extracts; prohibit legacy styles and visual imports.

### Required acceptance evidence

Gate detects an imported or copied legacy control; manual visual review remains required.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## AR01 — Venue source and landmark pack

**Phase:** 0 | **Priority:** P0 | **Lane:** ART
**Status:** NOT_STARTED
**Dependencies:** C06
**IA coverage:** O03

### Owned implementation paths

- `frontend/public/world/places/`
- `docs/mcr/venue-provenance.md`

### Implementation

Verify relative buildings/paths from authorized references; distinguish temporary unverified gates and public meeting places.

### Required acceptance evidence

Every physical place has source, review state, photo permission, and closure handling.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## AR02 — Walkable venue tilemap

**Phase:** 1 | **Priority:** P0 | **Lane:** ART
**Status:** NOT_STARTED
**Dependencies:** AR01, C06
**IA coverage:** W02

### Owned implementation paths

- `frontend/public/world/maps/`

### Implementation

Build terrain, collision, overdraw, interaction, spawn, and landmark layers from the reviewed source pack.

### Required acceptance evidence

All spawn points are walkable; interaction IDs resolve; blocked routes cannot be traversed.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## AR03 — Original avatar atlases

**Phase:** 1 | **Priority:** P0 | **Lane:** ART
**Status:** NOT_STARTED
**Dependencies:** C06
**IA coverage:** Cross-cutting implementation

### Owned implementation paths

- `frontend/public/world/avatars/`

### Implementation

Deliver six presets and agreed layered assets using the fixed anchor and direction/frame layout.

### Required acceptance evidence

No clipping, missing frames, or theme-dependent identity changes at actual mobile scale.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## AR04 — Three world skin packs

**Phase:** 1 | **Priority:** P0 | **Lane:** ART
**Status:** NOT_STARTED
**Dependencies:** C06
**IA coverage:** W06

### Owned implementation paths

- `frontend/public/world/themes/`

### Implementation

Produce Revenge/Parade/Danger decorations and palettes without changing geometry or collision IDs.

### Required acceptance evidence

Pixel readability and contrast pass for all skins; manifest hashes and licenses recorded.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## BE01 — Import port-rust foundations

**Phase:** 1 | **Priority:** P0 | **Lane:** BE-CORE
**Status:** NOT_STARTED
**Dependencies:** C01, C02
**IA coverage:** Cross-cutting implementation

### Owned implementation paths

- `backend/Cargo.toml`
- `backend/Cargo.lock`
- `backend/crates/`
- `backend/services/`

### Implementation

Import the selected workspace with provenance; retire old competing executable entrypoints; preserve required service boundaries.

### Required acceptance evidence

Workspace resolves and builds; one documented process/port map; no accidental old DB target.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## BE02 — Relocate build and migration tooling

**Phase:** 1 | **Priority:** P0 | **Lane:** BE-CORE
**Status:** NOT_STARTED
**Dependencies:** BE01
**IA coverage:** Cross-cutting implementation

### Owned implementation paths

- `backend/scripts/`
- `infra/mcr/backend/`

### Implementation

Repair relative paths, protobuf generation, SQLx migrations, health checks, image contexts, and isolated DB credentials.

### Required acceptance evidence

Fresh isolated database migrates; services restart and persist a smoke record.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## BE03 — Browser auth contract adapter

**Phase:** 1 | **Priority:** P0 | **Lane:** BE-CORE
**Status:** NOT_STARTED
**Dependencies:** BE02, C03
**IA coverage:** A02, A03, A04, A05, A06

### Owned implementation paths

- `backend/services/api-gateway/src/adapters/in/http/`
- `backend/services/auth-service/`

### Implementation

Wire signup/verify/login/reset/refresh/logout/provider discovery to the mapped contract; enforce safe redirects and cookie protections.

### Required acceptance evidence

Real isolated signup/login/refresh/logout; expired/reused credentials and invalid OAuth state rejected.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## BE04 — Canonical participant identity adapter

**Phase:** 1 | **Priority:** P0 | **Lane:** BE-CORE
**Status:** NOT_STARTED
**Dependencies:** BE03, C03
**IA coverage:** M09, U03

### Owned implementation paths

- `backend/crates/concert-application/src/identity/`
- `backend/services/concert-service/src/adapters/out/identity/`

### Implementation

Resolve Auth UUID, basic Profile projection, suspension state, and explicit provider-to-user binding; remove X-account prerequisite.

### Required acceptance evidence

Email-only user participates; foreign identity cannot attach claims; private profile fields never leak.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## BE05 — Concert service and core schema

**Phase:** 1 | **Priority:** P0 | **Lane:** BE-CONCERT
**Status:** NOT_STARTED
**Dependencies:** BE01, C03
**IA coverage:** Cross-cutting implementation

### Owned implementation paths

- `backend/crates/concert-domain/`
- `backend/crates/concert-application/`
- `backend/services/concert-service/`

### Implementation

Add hexagonal modules, typed errors, migrations for events/participants/avatar settings, clock and policy ports.

### Required acceptance evidence

Migration uniqueness checks and dependency-boundary tests; service health and isolated startup.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## BE06 — Bootstrap and participant defaults

**Phase:** 1 | **Priority:** P0 | **Lane:** BE-CONCERT
**Status:** NOT_STARTED
**Dependencies:** BE04, BE05
**IA coverage:** U01

### Owned implementation paths

- `backend/crates/concert-application/src/bootstrap/`
- `backend/services/concert-service/src/adapters/in/grpc/bootstrap.rs`
- `backend/services/concert-service/tests/bootstrap.rs`

### Implementation

Implement idempotent participant creation, preset avatar, event capabilities, manifests, and server clock aggregation.

### Required acceptance evidence

Repeated bootstrap creates one participant; degraded optional dependencies do not fabricate data.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## BE07 — Avatar and preference persistence

**Phase:** 1 | **Priority:** P0 | **Lane:** BE-CONCERT
**Status:** NOT_STARTED
**Dependencies:** BE05, C06
**IA coverage:** W06, U02

### Owned implementation paths

- `backend/crates/concert-domain/src/avatar/`
- `backend/crates/concert-application/src/avatar/`
- `backend/services/concert-service/src/adapters/out/repository/avatar.rs`

### Implementation

Validate asset IDs/layers/version; update avatar and theme with optimistic concurrency and bounded payloads.

### Required acceptance evidence

Invalid URL/item rejected; stale version conflicts; other users' outfits remain unchanged.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## BE08 — Places and map-release reads

**Phase:** 1 | **Priority:** P0 | **Lane:** BE-CONCERT
**Status:** NOT_STARTED
**Dependencies:** BE05, C06
**IA coverage:** W02, W03, O03

### Owned implementation paths

- `backend/crates/concert-domain/src/place/`
- `backend/crates/concert-application/src/place/`
- `backend/services/concert-service/src/adapters/out/repository/place.rs`

### Implementation

Store reviewed landmarks and immutable map manifests; expose closure and verification; separate concept-only objects.

### Required acceptance evidence

Unknown/closed place cannot be selected for a new physical beacon; manifest mismatch has a typed error.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## BE09 — Physical beacon lifecycle

**Phase:** 2 | **Priority:** P0 | **Lane:** BE-CONCERT
**Status:** NOT_STARTED
**Dependencies:** BE08, BE37, C04
**IA coverage:** W07, M06

### Owned implementation paths

- `backend/crates/concert-domain/src/beacon/`
- `backend/crates/concert-application/src/beacon/`
- `backend/services/concert-service/src/adapters/out/repository/beacon.rs`

### Implementation

Create/read/update/revoke scoped beacons; DB-time TTL; revision changes; audience authorization through policy ports.

### Required acceptance evidence

Expired, revoked, blocked, and unrelated viewers receive no private meeting details.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## BE10 — One-use world admission tickets

**Phase:** 1 | **Priority:** P0 | **Lane:** BE-CORE
**Status:** NOT_STARTED
**Dependencies:** BE04, BE05, C05
**IA coverage:** W01

### Owned implementation paths

- `backend/crates/concert-application/src/world_ticket/`
- `backend/services/api-gateway/src/adapters/in/http/concert_world.rs`

### Implementation

Issue user/session/event/room-bound 30-second tickets; atomic consumption; no credential in URLs/logs.

### Required acceptance evidence

Reuse, wrong room, expired ticket, wrong origin, and unauthenticated frames rejected.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## BE11 — Room registry and membership leases

**Phase:** 1 | **Priority:** P0 | **Lane:** BE-CONCERT
**Status:** NOT_STARTED
**Dependencies:** BE10
**IA coverage:** W01, W04

### Owned implementation paths

- `backend/services/concert-service/src/world/rooms.rs`
- `backend/services/concert-service/src/world/admission.rs`

### Implementation

One owner process per room; bounded capacity, session membership, join/switch/leave, and disconnect grace lease.

### Required acceptance evidence

Concurrent joins never exceed cap; switching cannot duplicate membership; ghost leases expire.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## BE12 — Authoritative movement loop

**Phase:** 1 | **Priority:** P0 | **Lane:** BE-CONCERT
**Status:** NOT_STARTED
**Dependencies:** BE11, C06
**IA coverage:** W01

### Owned implementation paths

- `backend/services/concert-service/src/world/simulation.rs`
- `backend/crates/concert-domain/src/world/`

### Implementation

Fixed-step movement from normalized input; speed limits, collision grid, valid spawn, and interaction range.

### Required acceptance evidence

Teleport, excessive speed, invalid axis, sequence replay, and wall penetration tests.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## BE13 — Recipient-specific snapshots and deltas

**Phase:** 1 | **Priority:** P0 | **Lane:** BE-CONCERT
**Status:** NOT_STARTED
**Dependencies:** BE12, BE37
**IA coverage:** W01, W05

### Owned implementation paths

- `backend/services/concert-service/src/world/replication.rs`
- `backend/services/concert-service/src/world/interest.rs`

### Implementation

Project authorized players, stable snapshot baseline, per-stream sequence, avatar versions, bounded/coalesced queues.

### Required acceptance evidence

Two viewers with different block filters resynchronize correctly; slow receiver cannot grow memory unboundedly.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## BE14 — Reconnect and authorization revocation

**Phase:** 1 | **Priority:** P0 | **Lane:** BE-CONCERT
**Status:** NOT_STARTED
**Dependencies:** BE13
**IA coverage:** W01

### Owned implementation paths

- `backend/services/concert-service/src/world/session.rs`
- `backend/services/concert-service/src/world/recovery.rs`

### Implementation

Handle epoch changes, expired session, suspension, close codes, resync, and app foregrounding without motion replay.

### Required acceptance evidence

New snapshot after server restart; removed/expired session cannot recover its old subscriptions.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## BE15 — Domain outbox and command receipts

**Phase:** 2 | **Priority:** P0 | **Lane:** BE-CONCERT
**Status:** NOT_STARTED
**Dependencies:** BE05
**IA coverage:** Cross-cutting implementation

### Owned implementation paths

- `backend/crates/concert-application/src/commands/`
- `backend/services/concert-service/src/adapters/out/repository/outbox.rs`
- `backend/services/concert-service/src/workers/outbox.rs`

### Implementation

Transactional event enqueue; idempotency-body hashes; bounded retries and delivery deduplication.

### Required acceptance evidence

Crash after commit does not lose event; duplicate event consumed once; changed payload/key conflicts.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## BE16 — Party aggregate and CRUD

**Phase:** 2 | **Priority:** P0 | **Lane:** BE-CONCERT
**Status:** NOT_STARTED
**Dependencies:** BE05, BE37, C04
**IA coverage:** M02

### Owned implementation paths

- `backend/crates/concert-domain/src/party/`
- `backend/crates/concert-application/src/party/create.rs`
- `backend/crates/concert-application/src/party/update.rs`

### Implementation

Event/ticket section, owner, capacity, rendezvous, lifecycle and version constraints; public projection excludes applicants.

### Required acceptance evidence

Non-owner edits fail; invalid capacity/window rejected; cancelled party is not joinable.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## BE17 — Applications and atomic approval

**Phase:** 2 | **Priority:** P0 | **Lane:** BE-CONCERT
**Status:** NOT_STARTED
**Dependencies:** BE16, BE15
**IA coverage:** M03

### Owned implementation paths

- `backend/crates/concert-application/src/party/apply.rs`
- `backend/crates/concert-application/src/party/decide.rs`
- `backend/services/concert-service/tests/party_capacity.rs`

### Implementation

Unique application, owner decision, transaction-guarded member count, idempotent response and outbox notification.

### Required acceptance evidence

Two approvals for last slot produce one membership; duplicate apply does not create extra rows.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## BE18 — Leave, remove, and cancel party

**Phase:** 2 | **Priority:** P0 | **Lane:** BE-CONCERT
**Status:** NOT_STARTED
**Dependencies:** BE17
**IA coverage:** M04

### Owned implementation paths

- `backend/crates/concert-application/src/party/membership.rs`
- `backend/services/concert-service/tests/party_removal.rs`

### Implementation

Enforce actor permissions, leader cancellation/transfer rule, capacity release and immediate audience revocation.

### Required acceptance evidence

Removed member loses rally/beacon access; owner cannot orphan an active party.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## BE19 — Rally and real-arrival commands

**Phase:** 2 | **Priority:** P0 | **Lane:** BE-CONCERT
**Status:** NOT_STARTED
**Dependencies:** BE18, BE09
**IA coverage:** M05

### Owned implementation paths

- `backend/crates/concert-domain/src/rally/`
- `backend/crates/concert-application/src/rally/`
- `backend/services/concert-service/tests/rally.rs`

### Implementation

Member-only state and meeting revision; explicit ON_MY_WAY/HERE/MET/NEED_HELP; no virtual-position inference.

### Required acceptance evidence

Nonmember rejected; venue change invalidates stale rally instructions; self-state actor checked.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## BE20 — Giveaway aggregate and catalog

**Phase:** 2 | **Priority:** P0 | **Lane:** BE-CONCERT
**Status:** NOT_STARTED
**Dependencies:** BE05, BE37, C04
**IA coverage:** G01, G02, G03, G04, G07

### Owned implementation paths

- `backend/crates/concert-domain/src/giveaway/`
- `backend/crates/concert-application/src/giveaway/`
- `backend/services/concert-service/tests/giveaway_catalog.rs`

### Implementation

Owner CRUD, public filters, quantity and pickup-window validation, pause/cancel and optimistic stock edits.

### Required acceptance evidence

Quantity cannot drop below committed units; recipients absent from public lists and details.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## BE21 — Atomic reservation command

**Phase:** 2 | **Priority:** P0 | **Lane:** BE-CONCERT
**Status:** NOT_STARTED
**Dependencies:** BE20, BE15
**IA coverage:** G05, G07

### Owned implementation paths

- `backend/crates/concert-application/src/reservation/reserve.rs`
- `backend/services/concert-service/src/adapters/out/repository/reservation.rs`

### Implementation

Guarded stock decrement/hold, unique active user reservation, request receipt, and pickup-window expiry.

### Required acceptance evidence

One unit with five simultaneous buyers yields exactly one hold; retry returns original receipt.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## BE22 — Cancellation and expiry worker

**Phase:** 2 | **Priority:** P0 | **Lane:** BE-CONCERT
**Status:** NOT_STARTED
**Dependencies:** BE21
**IA coverage:** G06

### Owned implementation paths

- `backend/crates/concert-application/src/reservation/release.rs`
- `backend/services/concert-service/src/workers/reservation_expiry.rs`

### Implementation

Conditional terminal transition and stock release in one transaction; DB clock, bounded batches, restart-safe scans.

### Required acceptance evidence

Cancellation/expiry/claim races do not double-release stock; restart resumes pending expiry.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## BE23 — Pickup confirmation and receipts

**Phase:** 2 | **Priority:** P0 | **Lane:** BE-CONCERT
**Status:** NOT_STARTED
**Dependencies:** BE22
**IA coverage:** G04, G06

### Owned implementation paths

- `backend/crates/concert-application/src/reservation/claim.rs`
- `backend/services/concert-service/tests/pickup.rs`

### Implementation

Bind confirmation to reservation/authorized actors; hashed expiring code, guess limits, terminal claim receipt.

### Required acceptance evidence

Duplicate claim consumes no extra stock; wrong owner/code/expired hold is rejected.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## BE24 — Authorized image upload pipeline

**Phase:** 2 | **Priority:** P0 | **Lane:** BE-CORE
**Status:** NOT_STARTED
**Dependencies:** BE05, BE37, C04
**IA coverage:** F02, G02

### Owned implementation paths

- `backend/crates/concert-application/src/media/`
- `backend/services/concert-service/src/adapters/out/media/`
- `backend/services/concert-service/tests/media.rs`

### Implementation

Initialize/finalize upload; byte/MIME/dimension limits, decode/reencode, metadata removal, ownership and private read control.

### Required acceptance evidence

Forged MIME, oversized image, foreign upload ID, and unauthorized private media read fail.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## BE25 — Extract introduction and age policy

**Phase:** 3 | **Priority:** P0 | **Lane:** BE-CORE
**Status:** NOT_STARTED
**Dependencies:** C01, C04
**IA coverage:** Cross-cutting implementation

### Owned implementation paths

- `backend/crates/concert-domain/src/intro_policy/`
- `backend/crates/concert-domain/src/age_policy/`

### Implementation

Port pure rule semantics and fixtures; replace source identity types and database-bound helpers with explicit ports.

### Required acceptance evidence

Half-open range, null bounds, unknown claims, supported limits, and no public policy leakage.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## BE26 — Verified-age claim persistence

**Phase:** 3 | **Priority:** P0 | **Lane:** BE-CORE
**Status:** NOT_STARTED
**Dependencies:** BE25, BE04, BE05
**IA coverage:** U06

### Owned implementation paths

- `backend/crates/concert-application/src/age_verification/`
- `backend/services/concert-service/src/adapters/out/age_provider/`
- `backend/services/concert-service/tests/age_claims.rs`

### Implementation

Port claim expiry, next-birthday and canonical replacement; verified callback-to-user binding; provider availability capability.

### Required acceptance evidence

Seoul birthday boundary, expired/replaced claims, callback replay, and missing provider fail closed.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## BE27 — Introduction authoring and discovery

**Phase:** 3 | **Priority:** P0 | **Lane:** BE-CONCERT
**Status:** NOT_STARTED
**Dependencies:** BE26, BE37
**IA coverage:** M07, M08

### Owned implementation paths

- `backend/crates/concert-application/src/intro/`
- `backend/services/concert-service/src/adapters/out/repository/intro.rs`

### Implementation

Owner draft/publish/close, eligibility-aware discovery/detail, approved public profile projection, pagination.

### Required acceptance evidence

Direct ID and list paths enforce the same eligibility; unknown verification cannot bypass a restriction.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## BE28 — Private social-handle storage

**Phase:** 3 | **Priority:** P0 | **Lane:** BE-CONCERT
**Status:** NOT_STARTED
**Dependencies:** BE05, BE37, C04
**IA coverage:** U04

### Owned implementation paths

- `backend/crates/concert-domain/src/social_handle/`
- `backend/crates/concert-application/src/social_handle/`
- `backend/services/concert-service/src/adapters/out/crypto/`

### Implementation

Normalize platform handle, encrypt at rest with managed secret, version changes, removal, and no plaintext logging.

### Required acceptance evidence

Public DTO serialization rejects handles; replacement invalidates old-version grant bindings.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## BE29 — Mutual social-sharing decisions

**Phase:** 3 | **Priority:** P0 | **Lane:** BE-CONCERT
**Status:** NOT_STARTED
**Dependencies:** BE28, BE15
**IA coverage:** U05

### Owned implementation paths

- `backend/crates/concert-application/src/social_request/`
- `backend/services/concert-service/tests/social_consent.rs`

### Implementation

Requester's explicit consent, recipient approval/rejection, per-platform reciprocal grants, expiry and revocation.

### Required acceptance evidence

No grant before both relevant consents; duplicate decision is idempotent; blocked pair rejected.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## BE30 — Authorized reveal endpoint

**Phase:** 3 | **Priority:** P0 | **Lane:** BE-CONCERT
**Status:** NOT_STARTED
**Dependencies:** BE29
**IA coverage:** M09, U05

### Owned implementation paths

- `backend/crates/concert-application/src/social_reveal/`
- `backend/services/api-gateway/src/adapters/in/http/social_reveal.rs`

### Implementation

Recheck grant versions, platform, expiry, block, suspension and actor; no-store response and privacy invalidation event.

### Required acceptance evidence

Approved, unrelated, expired, revoked, changed-handle and blocked scenarios; no leaked error metadata.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## BE31 — Event feed and post adapter

**Phase:** 3 | **Priority:** P0 | **Lane:** BE-CORE
**Status:** NOT_STARTED
**Dependencies:** BE04, BE15, BE37, C04
**IA coverage:** F01, F02, F04

### Owned implementation paths

- `backend/crates/concert-application/src/feed/`
- `backend/services/api-gateway/src/adapters/in/http/concert_feed.rs`
- `backend/services/post-service/`

### Implementation

Add event scoping, latest cursor feed, public party/giveaway references and ordinary post CRUD through the owner service.

### Required acceptance evidence

Cross-event/private references excluded; stable cursor order; linked entity is reauthorized.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## BE32 — Comments, reactions, and content controls

**Phase:** 3 | **Priority:** P0 | **Lane:** BE-CORE
**Status:** NOT_STARTED
**Dependencies:** BE31
**IA coverage:** F03

### Owned implementation paths

- `backend/services/api-gateway/src/adapters/in/http/concert_comments.rs`
- `backend/services/post-service/`

### Implementation

Map comments/reactions, owner edit/delete, visibility filtering and moderation receipts to inherited contracts.

### Required acceptance evidence

Blocked reply paths fail; deleted/hidden post cannot be recovered through a comment route.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## BE33 — Concert notification adapter

**Phase:** 3 | **Priority:** P0 | **Lane:** BE-CORE
**Status:** NOT_STARTED
**Dependencies:** BE15, BE04
**IA coverage:** U10

### Owned implementation paths

- `backend/services/concert-service/src/adapters/out/notification/`
- `backend/services/notification-service/`

### Implementation

Deduplicate domain event delivery, safe summary text, own inbox/read state and authorized target links.

### Required acceptance evidence

No handle/location secret in payload; repeated delivery creates one notification; stale target handled.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## BE34 — Mural writes and cooldown

**Phase:** 3 | **Priority:** P0 | **Lane:** BE-CONCERT
**Status:** NOT_STARTED
**Dependencies:** BE05, BE37, C04
**IA coverage:** R01

### Owned implementation paths

- `backend/crates/concert-domain/src/mural/`
- `backend/crates/concert-application/src/mural/place_pixel.rs`
- `backend/services/concert-service/tests/mural_concurrency.rs`

### Implementation

Grid/palette bounds, server-time per-user cooldown, atomic operation/revision/outbox update, write freeze check.

### Required acceptance evidence

Parallel requests cannot bypass cooldown; invalid coordinates/colors rejected; DB failure produces no false commit.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## BE35 — Mural snapshot and realtime changes

**Phase:** 3 | **Priority:** P0 | **Lane:** BE-CONCERT
**Status:** NOT_STARTED
**Dependencies:** BE34, BE15
**IA coverage:** R01

### Owned implementation paths

- `backend/crates/concert-application/src/mural/snapshot.rs`
- `backend/services/concert-service/src/world/mural_subscription.rs`

### Implementation

Produce consistent snapshot revision and ordered deltas; resume or full-resync on missed retention window.

### Required acceptance evidence

New subscriber and reconnecting subscriber converge to the same pixels and revision.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## BE36 — Reports and evidence access

**Phase:** 4 | **Priority:** P0 | **Lane:** BE-CORE
**Status:** NOT_STARTED
**Dependencies:** BE05, BE37, C04
**IA coverage:** S01, O04

### Owned implementation paths

- `backend/crates/concert-application/src/report/`
- `backend/services/concert-service/src/adapters/out/repository/report.rs`

### Implementation

Typed targets, reason limits, duplicate/spam handling, reporter privacy and role-scoped evidence retrieval.

### Required acceptance evidence

Target never sees reporter identity; ordinary member cannot inspect moderator evidence.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## BE37 — Shared relationship policy port

**Phase:** 1 | **Priority:** P0 | **Lane:** BE-CORE
**Status:** NOT_STARTED
**Dependencies:** BE04, BE05, C04
**IA coverage:** W05, U08

### Owned implementation paths

- `backend/crates/concert-application/src/ports/relationship_policy.rs`
- `backend/services/concert-service/src/adapters/out/friend_policy.rs`

### Implementation

Adapt inherited blocks/suspensions to canonical users; common authorization and invalidation hook for HTTP/WS/projections.

### Required acceptance evidence

Bidirectional block and unblocking semantics; unavailable policy cannot reveal private data.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## BE38 — Event settings, notices, and flags

**Phase:** 4 | **Priority:** P0 | **Lane:** BE-CORE
**Status:** NOT_STARTED
**Dependencies:** BE08, BE05, C04
**IA coverage:** A07, F05, O01, O02

### Owned implementation paths

- `backend/crates/concert-application/src/admin/event.rs`
- `backend/crates/concert-application/src/admin/places.rs`
- `backend/crates/concert-application/src/admin/notices.rs`

### Implementation

Operator roles, reviewed publication, source timestamps, place closure, feature availability and read-only gates.

### Required acceptance evidence

Client-hidden feature still rejected server-side; nonoperator writes forbidden.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## BE39 — Moderation commands and audit trail

**Phase:** 4 | **Priority:** P0 | **Lane:** BE-CORE
**Status:** NOT_STARTED
**Dependencies:** BE36, BE38
**IA coverage:** O04, O05, O06, O08

### Owned implementation paths

- `backend/crates/concert-application/src/admin/moderation.rs`
- `backend/services/concert-service/src/adapters/out/repository/audit.rs`

### Implementation

Suspend/hide/restore with reason and immutable receipt; invoke owner services and terminate affected sessions.

### Required acceptance evidence

Action/result tied to actor; failed downstream action is not recorded as successfully applied.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## BE40 — Mural pause, lock, and restore

**Phase:** 4 | **Priority:** P0 | **Lane:** BE-CONCERT
**Status:** NOT_STARTED
**Dependencies:** BE35, BE38
**IA coverage:** O07

### Owned implementation paths

- `backend/crates/concert-application/src/admin/mural.rs`
- `backend/services/concert-service/tests/mural_restore.rs`

### Implementation

Region locks, global pause/freeze, restore from authorized snapshot into a new revision; force client resync.

### Required acceptance evidence

Writes during freeze rejected; restore never reuses old sequence numbers; audit receipt exists.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## BE41 — Event closure and archive jobs

**Phase:** 4 | **Priority:** P0 | **Lane:** BE-CONCERT
**Status:** NOT_STARTED
**Dependencies:** BE15, BE30, BE35, BE38
**IA coverage:** X01, O09

### Owned implementation paths

- `backend/crates/concert-application/src/event_lifecycle/`
- `backend/services/concert-service/src/workers/cleanup.rs`

### Implementation

Idempotent read-only transition, permitted archive projection, grant/beacon expiry and resumable retention jobs.

### Required acceptance evidence

Partial failure resumes; public archive excludes private locations, recipients, and handles.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## BE42 — Account-deletion integration

**Phase:** 4 | **Priority:** P0 | **Lane:** BE-CORE
**Status:** NOT_STARTED
**Dependencies:** BE14, BE30, BE41
**IA coverage:** A08, U09

### Owned implementation paths

- `backend/crates/concert-application/src/account_deletion/`
- `backend/services/auth-service/`

### Implementation

Extend deletion orchestration to Concert; revoke sessions/tickets/grants, scrub retained personal fields, retry durable steps.

### Required acceptance evidence

Repeated deletion safe; removed user cannot reconnect or reveal; failed step remains visible for retry.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## BE43 — Boundary and abuse hardening

**Phase:** 4 | **Priority:** P0 | **Lane:** BE-CORE
**Status:** NOT_STARTED
**Dependencies:** BE03, BE10, BE24, BE30
**IA coverage:** Cross-cutting implementation

### Owned implementation paths

- `backend/services/api-gateway/src/`
- `backend/services/concert-service/src/adapters/in/`
- `infra/mcr/security/`

### Implementation

Scope-specific rate limits, content bounds, header/origin rules, redacted logs, no-store private responses, dependency audit.

### Required acceptance evidence

Negative tests for auth/CSRF/origin/replay/upload/oversized WS; no secrets in captured logs.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## BE44 — Reproducible real-stack startup

**Phase:** 4 | **Priority:** P0 | **Lane:** BE-CORE
**Status:** NOT_STARTED
**Dependencies:** BE02, BE38, BE41, BE43
**IA coverage:** Cross-cutting implementation

### Owned implementation paths

- `infra/mcr/compose.yml`
- `scripts/mcr/backend-smoke.sh`

### Implementation

Wire all inherited services and Concert, persistent volumes, real/mock provider separation, health and shutdown ordering.

### Required acceptance evidence

Fresh startup and restart smoke; disabled provider is honestly unavailable, not simulated success.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## FE01 — Extract and adapt HTTP transport

**Phase:** 1 | **Priority:** P0 | **Lane:** FE-APP
**Status:** NOT_STARTED
**Dependencies:** C02, C03
**IA coverage:** Cross-cutting implementation

### Owned implementation paths

- `frontend/src/shared/api/http.ts`
- `frontend/src/shared/api/errors.ts`

### Implementation

Port cancellation, error normalization and safe upload helpers; replace endpoint/auth assumptions with typed client ports.

### Required acceptance evidence

Mock 401/409/429/network abort; no old `/v1` calls or token persistence.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## FE02 — Identity-neutral queries and invalidation

**Phase:** 1 | **Priority:** P0 | **Lane:** FE-APP
**Status:** NOT_STARTED
**Dependencies:** FE01, C03
**IA coverage:** Cross-cutting implementation

### Owned implementation paths

- `frontend/src/shared/query/keys.ts`
- `frontend/src/shared/query/privacy.ts`

### Implementation

Replace activeXAccountId with user/event scope; cancellation and private-cache eviction on auth/policy changes.

### Required acceptance evidence

Account A's delayed response cannot populate account B; no retained reveal after revoke.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## FE03 — Extract pure form and policy helpers

**Phase:** 1 | **Priority:** P0 | **Lane:** FE-APP
**Status:** NOT_STARTED
**Dependencies:** C04, C08
**IA coverage:** Cross-cutting implementation

### Owned implementation paths

- `frontend/src/shared/validation/`
- `frontend/src/entities/intro/model/`

### Implementation

Port required Zod/pure logic only; separate mixed TSX logic from rendering; adapt canonical IDs and error enums.

### Required acceptance evidence

Existing semantic fixtures pass; no import from old pages/components/styles.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## FE04 — Headless image upload controller

**Phase:** 2 | **Priority:** P0 | **Lane:** FE-APP
**Status:** NOT_STARTED
**Dependencies:** FE01, C04
**IA coverage:** F02

### Owned implementation paths

- `frontend/src/shared/media/`
- `frontend/src/features/media/model/`

### Implementation

Extract image compression, preview cleanup, progress, abort, finalize and retry state; exclude video/FFmpeg.

### Required acceptance evidence

Abort frees resources; failed finalize is not shown as uploaded; no credential on presigned PUT.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## FE05 — New accessible UI primitives

**Phase:** 1 | **Priority:** P0 | **Lane:** FE-APP
**Status:** NOT_STARTED
**Dependencies:** C02, C08
**IA coverage:** Cross-cutting implementation

### Owned implementation paths

- `frontend/src/shared/ui/`
- `frontend/src/shared/styles/base.css`

### Implementation

Author buttons, fields, dialogs, sheet, tabs, status, toast, skeleton and error panels from new markup/styles.

### Required acceptance evidence

Keyboard/focus/dialog dismissal tests; 44px targets; no legacy visual imports.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## FE06 — Three-theme token system

**Phase:** 1 | **Priority:** P0 | **Lane:** FE-APP
**Status:** NOT_STARTED
**Dependencies:** FE05, C06
**IA coverage:** W06, U07

### Owned implementation paths

- `frontend/src/shared/styles/themes/`
- `frontend/src/features/theme/`

### Implementation

New Revenge/Parade/Danger tokens, selection persistence, asset-loading fallback, reduced-motion transitions.

### Required acceptance evidence

Theme switch preserves geometry, room and avatar identity; failed asset load restores previous skin.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## FE07 — Persistent world shell and routes

**Phase:** 1 | **Priority:** P0 | **Lane:** FE-APP
**Status:** NOT_STARTED
**Dependencies:** FE05, C03
**IA coverage:** W01, S02

### Owned implementation paths

- `frontend/src/app/router.tsx`
- `frontend/src/app/WorldShell.tsx`

### Implementation

Build header, six tabs, nested panels, history/back semantics, safe return paths, lazy public/admin route boundaries.

### Required acceptance evidence

Panel navigation creates no duplicate world/socket; direct gift link initializes shell correctly.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## FE08 — New public landing and legal views

**Phase:** 1 | **Priority:** P0 | **Lane:** FE-APP
**Status:** NOT_STARTED
**Dependencies:** FE05, FE07
**IA coverage:** A01, A07

### Owned implementation paths

- `frontend/src/features/public/`

### Implementation

New world-first landing, entry CTA, versioned legal/safety/contact pages; signed-in redirect.

### Required acceptance evidence

Anonymous pages load without Phaser; signed-in home resolves to world; no old homepage markup.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## FE09 — Session controller and sign-in

**Phase:** 1 | **Priority:** P0 | **Lane:** FE-APP
**Status:** NOT_STARTED
**Dependencies:** FE01, FE02, FE07, C03
**IA coverage:** A01, A02

### Owned implementation paths

- `frontend/src/features/auth/model/`
- `frontend/src/features/auth/ui/SignIn.tsx`

### Implementation

In-memory token, single-flight refresh, safe bootstrap, login/logout, provider availability, complete private-state cleanup.

### Required acceptance evidence

Concurrent 401s perform one refresh; failed login and unavailable provider have truthful states.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## FE10 — Signup and verification views

**Phase:** 1 | **Priority:** P0 | **Lane:** FE-APP
**Status:** NOT_STARTED
**Dependencies:** FE09
**IA coverage:** A03, A04

### Owned implementation paths

- `frontend/src/features/auth/ui/SignUp.tsx`
- `frontend/src/features/auth/ui/VerifyEmail.tsx`

### Implementation

New forms using mapped server flow, validation, consent, challenge expiry and server resend countdown.

### Required acceptance evidence

Expired verification cannot complete; double submit does not create extra requests.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## FE11 — Reset-password and callback views

**Phase:** 1 | **Priority:** P0 | **Lane:** FE-APP
**Status:** NOT_STARTED
**Dependencies:** FE09
**IA coverage:** A05, A06

### Owned implementation paths

- `frontend/src/features/auth/ui/ResetPassword.tsx`
- `frontend/src/features/auth/model/callback.ts`

### Implementation

New request/confirm reset views and OAuth callback error/return-target handling; sanitize local redirects.

### Required acceptance evidence

Unknown email uses neutral response; hostile return URL rejected; denial returns safely.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## FE12 — Shared avatar compositor

**Phase:** 1 | **Priority:** P0 | **Lane:** FE-WORLD
**Status:** NOT_STARTED
**Dependencies:** C06, C02
**IA coverage:** W01, U02

### Owned implementation paths

- `frontend/src/world/avatar/`
- `frontend/src/entities/avatar/`

### Implementation

Deterministic layer composition, frame/anchor mapping, texture cache, and shared profile-thumbnail export.

### Required acceptance evidence

Golden-config render aligns across world/profile; missing asset uses documented fallback.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## FE13 — New avatar editor

**Phase:** 1 | **Priority:** P0 | **Lane:** FE-WORLD
**Status:** NOT_STARTED
**Dependencies:** FE12, FE05, C03
**IA coverage:** U02

### Owned implementation paths

- `frontend/src/features/avatar/`

### Implementation

New large preview, presets and item selectors; draft/save/cancel; optimistic-version conflict without losing draft.

### Required acceptance evidence

Save changes world and profile consistently; cancelling retains persisted outfit.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## FE14 — Phaser runtime and map loader

**Phase:** 1 | **Priority:** P0 | **Lane:** FE-WORLD
**Status:** NOT_STARTED
**Dependencies:** C06, C02
**IA coverage:** W01

### Owned implementation paths

- `frontend/src/world/runtime.ts`
- `frontend/src/world/scenes/`
- `frontend/src/world/map/`

### Implementation

Single lifecycle-managed game, manifest version/hash, layers, nearest-neighbor textures and resize/cleanup.

### Required acceptance evidence

Repeated React mount cleanup leaks no canvas/listeners; invalid map fails visibly.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## FE15 — Mobile movement, camera, and interactions

**Phase:** 1 | **Priority:** P0 | **Lane:** FE-WORLD
**Status:** NOT_STARTED
**Dependencies:** FE14
**IA coverage:** W01, W08

### Owned implementation paths

- `frontend/src/world/input/`
- `frontend/src/world/camera/`
- `frontend/src/world/interaction/`

### Implementation

Pointer-safe joystick/tap movement, keyboard control, collision prediction, recenter, selection and blocking-panel pause.

### Required acceptance evidence

Pointercancel/blur stops movement; keyboard opening and panel input never move the avatar accidentally.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## FE16 — Realtime transport implementations

**Phase:** 1 | **Priority:** P0 | **Lane:** FE-WORLD
**Status:** NOT_STARTED
**Dependencies:** C05, FE01
**IA coverage:** W01

### Owned implementation paths

- `frontend/src/world/network/WorldTransport.ts`
- `frontend/src/world/network/mock.ts`
- `frontend/src/world/network/websocket.ts`

### Implementation

Same interface for mock and real transport; ticket/authenticate, typed messages, sequence and close handling.

### Required acceptance evidence

Identical scenario suite runs with both transports; mock code excluded from production mode.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## FE17 — Replication and reconnect controller

**Phase:** 1 | **Priority:** P0 | **Lane:** FE-WORLD
**Status:** NOT_STARTED
**Dependencies:** FE16, FE14, FE12
**IA coverage:** W01, S03

### Owned implementation paths

- `frontend/src/world/network/replication.ts`
- `frontend/src/world/entities/`

### Implementation

Snapshot baseline, interpolation, bounded prediction/reconciliation, epoch reset and visibility-driven reconnect.

### Required acceptance evidence

Missing delta requests resync; restart removes ghosts; no replay of old pickup/reservation commands.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## FE18 — Rooms and accessible nearby list

**Phase:** 1 | **Priority:** P0 | **Lane:** FE-WORLD
**Status:** NOT_STARTED
**Dependencies:** FE17, FE05
**IA coverage:** W04, W05

### Owned implementation paths

- `frontend/src/features/world-rooms/`
- `frontend/src/features/world-nearby/`

### Implementation

New channel chooser, switch feedback, room cap states and DOM list of authorized nearby avatars.

### Required acceptance evidence

Full room handled without losing current membership; blocked character disappears from both representations.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## FE19 — Map, place details, and help

**Phase:** 2 | **Priority:** P0 | **Lane:** FE-APP
**Status:** NOT_STARTED
**Dependencies:** FE07, FE14, C03
**IA coverage:** W02, W03, W08

### Owned implementation paths

- `frontend/src/features/places/`
- `frontend/src/features/world-help/`

### Implementation

New overview and place sheet, source/closed states, physical/virtual legend, accessible actions and controls help.

### Required acceptance evidence

Concept-only place never offered as verified physical destination; map mismatch produces recovery UI.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## FE20 — Physical check-in and meeting beacon

**Phase:** 2 | **Priority:** P0 | **Lane:** FE-APP
**Status:** NOT_STARTED
**Dependencies:** FE19, FE02, C04
**IA coverage:** W07, M06

### Owned implementation paths

- `frontend/src/features/beacon/`

### Implementation

New audience/TTL form, last-confirmed time, photo/identification, moved/stale/revoked views and explicit arrival actions.

### Required acceptance evidence

Virtual avatar motion does not change physical check-in; expired beacon cannot masquerade as current.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## FE21 — Party discovery and application

**Phase:** 2 | **Priority:** P0 | **Lane:** FE-APP
**Status:** NOT_STARTED
**Dependencies:** FE07, FE02, C04
**IA coverage:** M01, M03

### Owned implementation paths

- `frontend/src/features/party-discovery/`

### Implementation

New filters/list/detail, apply/cancel mutation state and capability-driven actions, MY PLAN link.

### Required acceptance evidence

Empty/full/private/cancelled states; uncertain request reconciles by receipt instead of duplicate application.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## FE22 — Party authoring form

**Phase:** 2 | **Priority:** P0 | **Lane:** FE-APP
**Status:** NOT_STARTED
**Dependencies:** FE05, FE03, C04
**IA coverage:** M02

### Owned implementation paths

- `frontend/src/features/party-authoring/`

### Implementation

New create/edit form for capacity, ticket section, time, place, and intro; RHF validation and safe draft restore.

### Required acceptance evidence

Invalid time/capacity rejected locally and server errors mapped; version conflict keeps draft.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## FE23 — Party management and rally

**Phase:** 2 | **Priority:** P0 | **Lane:** FE-APP
**Status:** NOT_STARTED
**Dependencies:** FE21, FE20
**IA coverage:** M04, M05

### Owned implementation paths

- `frontend/src/features/party-management/`
- `frontend/src/features/rally/`

### Implementation

New application decisions/member list and explicit ON_MY_WAY/HERE/MET/NEED_HELP controls; remove/cancel confirmations.

### Required acceptance evidence

Last-slot conflict updates UI; removed member loses panel access; moved meeting visibly supersedes old card.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## FE24 — Giveaway discovery and own activity

**Phase:** 2 | **Priority:** P0 | **Lane:** FE-APP
**Status:** NOT_STARTED
**Dependencies:** FE07, FE02, C04
**IA coverage:** G01, G03, G07

### Owned implementation paths

- `frontend/src/features/gift-discovery/`
- `frontend/src/features/my-gifts/`

### Implementation

New list/detail and GIVING/RECEIVING views, remaining-stock/time filters and current action eligibility.

### Required acceptance evidence

No recipient identities in public view; paused/sold-out/ended states prevent invalid actions.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## FE25 — Giveaway authoring and owner tools

**Phase:** 2 | **Priority:** P0 | **Lane:** FE-APP
**Status:** NOT_STARTED
**Dependencies:** FE24, FE04, FE03
**IA coverage:** G02, G04

### Owned implementation paths

- `frontend/src/features/gift-authoring/`
- `frontend/src/features/gift-management/`

### Implementation

New item/photo/quantity/window/place form and owner reservation inventory table; pause/cancel/edit flows.

### Required acceptance evidence

Stock-edit conflict preserves form; unauthorized owner-tools route is unavailable.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## FE26 — Reservation and pickup journey

**Phase:** 2 | **Priority:** P0 | **Lane:** FE-APP
**Status:** NOT_STARTED
**Dependencies:** FE24, FE20
**IA coverage:** G05, G06

### Owned implementation paths

- `frontend/src/features/pickup/`

### Implementation

Idempotency-key lifecycle, hold receipt, expiry, meeting detail, confirmation code, owner claim and cancellation.

### Required acceptance evidence

Timeout/reload reconciles with server; expired/sold-out/repeated pickup never shows fake success.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## FE27 — Event timeline and notices

**Phase:** 3 | **Priority:** P0 | **Lane:** FE-APP
**Status:** NOT_STARTED
**Dependencies:** FE07, FE02, C04
**IA coverage:** F01, F05

### Owned implementation paths

- `frontend/src/features/feed/`

### Implementation

New compact chronological feed, filters, cursor loading, linked party/gift references and source-stamped notices.

### Required acceptance evidence

Stable ordering; revoked linked target safely disappears; no private activity projection.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## FE28 — Thread, composer, and comments

**Phase:** 3 | **Priority:** P0 | **Lane:** FE-APP
**Status:** NOT_STARTED
**Dependencies:** FE27, FE04
**IA coverage:** F02, F03, F04

### Owned implementation paths

- `frontend/src/features/thread/`
- `frontend/src/features/post-authoring/`

### Implementation

New post/comment/reaction views, edit/delete, image upload, draft handling and report entry.

### Required acceptance evidence

Deleted/hidden/conflicting content handled; double submission and failed upload recover cleanly.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## FE29 — People and introduction discovery

**Phase:** 3 | **Priority:** P0 | **Lane:** FE-APP
**Status:** NOT_STARTED
**Dependencies:** FE07, FE03, C04
**IA coverage:** M01, M07

### Owned implementation paths

- `frontend/src/features/people/`

### Implementation

New people list and filters, avatar-focused items, opaque eligibility/unavailable states and profile entry.

### Required acceptance evidence

User without X account can browse allowed content; exact age and secret handles absent.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## FE30 — Introduction editor, profile, age status

**Phase:** 3 | **Priority:** P0 | **Lane:** FE-APP
**Status:** NOT_STARTED
**Dependencies:** FE29, FE12
**IA coverage:** M08, M09, U06

### Owned implementation paths

- `frontend/src/features/intro/`
- `frontend/src/features/profile/`
- `frontend/src/features/age-status/`

### Implementation

New intro/profile rendering, private condition form and verified/unknown/expired/provider-unavailable flow.

### Required acceptance evidence

Half-open boundary fixtures; policy detail visible only to owner; no self-declared verified badge.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## FE31 — Private social-sharing UI

**Phase:** 3 | **Priority:** P0 | **Lane:** FE-APP
**Status:** NOT_STARTED
**Dependencies:** FE02, FE05, C04
**IA coverage:** U04, U05

### Owned implementation paths

- `frontend/src/features/social-sharing/`

### Implementation

New handle settings, explicit request consent, decisions, reveal, revoke and expiry views; no-store sensitive fetch.

### Required acceptance evidence

Network payload before approval contains no handle; revocation clears UI/cache; no auto-follow claim.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## FE32 — Mural editing surface

**Phase:** 3 | **Priority:** P0 | **Lane:** FE-WORLD
**Status:** NOT_STARTED
**Dependencies:** FE05, C04
**IA coverage:** R01

### Owned implementation paths

- `frontend/src/features/mural/editor/`

### Implementation

New grid/palette, pan/zoom and pixel preview; confirm action; touch disambiguation and DOM controls.

### Required acceptance evidence

Dragging does not place pixels; bounds/color/cooldown states correct on 360px screen.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## FE33 — Mural synchronization and world object

**Phase:** 3 | **Priority:** P0 | **Lane:** FE-WORLD
**Status:** NOT_STARTED
**Dependencies:** FE32, FE16
**IA coverage:** R01

### Owned implementation paths

- `frontend/src/features/mural/model/`
- `frontend/src/world/interaction/mural.ts`

### Implementation

Snapshot/delta reconciliation, commit receipts, pause/freeze/restore reset and world-wall texture update.

### Required acceptance evidence

Two clients converge; rejected local preview rolls back; restore discards obsolete deltas.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## FE34 — Personal profile and preferences

**Phase:** 3 | **Priority:** P0 | **Lane:** FE-APP
**Status:** NOT_STARTED
**Dependencies:** FE07, FE12, FE02
**IA coverage:** U01, U03, U07

### Owned implementation paths

- `frontend/src/features/me/`
- `frontend/src/features/settings/`

### Implementation

New ME hub, profile form and low-power/motion/sound/theme preferences; links to party and gift activity.

### Required acceptance evidence

Optional profile fields do not block world entry; private settings remain identity-scoped.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## FE35 — Notification inbox

**Phase:** 3 | **Priority:** P0 | **Lane:** FE-APP
**Status:** NOT_STARTED
**Dependencies:** FE02, FE07, C04
**IA coverage:** U10

### Owned implementation paths

- `frontend/src/features/inbox/`

### Implementation

New unread count/list, mark-read/all-read and safe deep-link routing with stale-target handling.

### Required acceptance evidence

Duplicate notification event does not duplicate UI; inaccessible target never exposes cached details.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## FE36 — Report, block, and deletion controls

**Phase:** 4 | **Priority:** P0 | **Lane:** FE-APP
**Status:** NOT_STARTED
**Dependencies:** FE02, FE05, C04
**IA coverage:** A08, U08, U09, S01

### Owned implementation paths

- `frontend/src/features/safety/`
- `frontend/src/features/account/`

### Implementation

New report dialog, block list/unblock, logout/delete confirmation and progress views; clear private state.

### Required acceptance evidence

Blocking closes affected panels; unblock does not restore old consent; failed deletion is not displayed as complete.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## FE37 — Moderator review screens

**Phase:** 4 | **Priority:** P0 | **Lane:** FE-APP
**Status:** NOT_STARTED
**Dependencies:** FE07, FE05, C04
**IA coverage:** O04, O05, O06

### Owned implementation paths

- `frontend/src/features/admin/reports/`
- `frontend/src/features/admin/members/`
- `frontend/src/features/admin/content/`

### Implementation

New plain tables/forms for report evidence, suspension, hide/restore and action reasons; server-role-aware entry.

### Required acceptance evidence

Nonmoderator direct route blocked; failed action remains pending/failed rather than successful.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## FE38 — Event operations screens

**Phase:** 4 | **Priority:** P0 | **Lane:** FE-APP
**Status:** NOT_STARTED
**Dependencies:** FE07, FE05, C04
**IA coverage:** O01, O02, O03, O08

### Owned implementation paths

- `frontend/src/features/admin/event/`
- `frontend/src/features/admin/places/`
- `frontend/src/features/admin/audit/`

### Implementation

New settings, place review/closure, notice publication, capability flags and audit views.

### Required acceptance evidence

Confirm risky changes; stale versions conflict; source and review state remain visible.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## FE39 — Wall control and event closure UI

**Phase:** 4 | **Priority:** P0 | **Lane:** FE-APP
**Status:** NOT_STARTED
**Dependencies:** FE38, FE33
**IA coverage:** O07, O09

### Owned implementation paths

- `frontend/src/features/admin/mural/`
- `frontend/src/features/admin/closure/`

### Implementation

New pause/region lock/restore controls and closure step-progress/retry presentation.

### Required acceptance evidence

Restore confirmation names snapshot; partial closure failure is visible and recoverable.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## FE40 — Read-only archive views

**Phase:** 4 | **Priority:** P0 | **Lane:** FE-APP
**Status:** NOT_STARTED
**Dependencies:** FE27, FE33, C04
**IA coverage:** X01

### Owned implementation paths

- `frontend/src/features/archive/`

### Implementation

New end-of-event page and permitted final mural/feed views; remove all write controls and private activity.

### Required acceptance evidence

Archive does not contain handles or physical beacons; deleted content stays inaccessible.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## FE41 — Global degraded and lifecycle states

**Phase:** 4 | **Priority:** P0 | **Lane:** FE-APP
**Status:** NOT_STARTED
**Dependencies:** FE07, FE17
**IA coverage:** S02, S03

### Owned implementation paths

- `frontend/src/app/errors/`
- `frontend/src/features/connection-status/`

### Implementation

New offline/reconnect/maintenance/version/404 states; stale-data labels, foreground resync and render throttling.

### Required acceptance evidence

No endless spinner; offline pickup is not queued silently; keyboard and reconnect preserve usable navigation.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## QA01 — Contract and fixture coverage

**Phase:** 4 | **Priority:** P0 | **Lane:** QA
**Status:** NOT_STARTED
**Dependencies:** C07
**IA coverage:** Cross-cutting implementation

### Owned implementation paths

- `scripts/mcr/contracts/`
- `docs/mcr/evidence/contracts/`

### Implementation

Validate every fixture and generated response shape; run drift and missing-operation probes.

### Required acceptance evidence

Broken schema and unknown enum cause failure; no empty-test success.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## QA02 — Zero legacy-UI audit

**Phase:** 4 | **Priority:** P0 | **Lane:** QA
**Status:** NOT_STARTED
**Dependencies:** C08, FE05, FE07
**IA coverage:** Cross-cutting implementation

### Owned implementation paths

- `docs/mcr/evidence/ui-origin/`

### Implementation

Static import/hash check plus manual comparison of all routes against old rendering and approved MCR direction.

### Required acceptance evidence

No reused old control/layout/theme; new visual snapshots are reviewed, not blindly accepted.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## QA03 — Database race suite

**Phase:** 4 | **Priority:** P0 | **Lane:** QA
**Status:** NOT_STARTED
**Dependencies:** BE17, BE22, BE23, BE34
**IA coverage:** Cross-cutting implementation

### Owned implementation paths

- `backend/services/concert-service/tests/concurrency/`

### Implementation

Run parallel capacity, reservation, cancellation, expiry, claim, idempotency and mural-cooldown scenarios against PostgreSQL.

### Required acceptance evidence

Invariants hold with forced transaction interleavings and worker restarts.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## QA04 — Privacy and authorization matrix

**Phase:** 4 | **Priority:** P0 | **Lane:** QA
**Status:** NOT_STARTED
**Dependencies:** BE30, BE37, BE39, FE31, FE36
**IA coverage:** Cross-cutting implementation

### Owned implementation paths

- `backend/services/concert-service/tests/authorization/`
- `frontend/e2e/privacy/`

### Implementation

Test owner/member/stranger/blocked/suspended/expired/admin across list/detail/mutation/WS/cache.

### Required acceptance evidence

No unauthorized handle, claim, evidence, beacon or recipient data in responses or cached UI.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## QA05 — Realtime failure suite

**Phase:** 4 | **Priority:** P0 | **Lane:** QA
**Status:** NOT_STARTED
**Dependencies:** BE14, BE35, FE17, FE33
**IA coverage:** Cross-cutting implementation

### Owned implementation paths

- `frontend/e2e/realtime/`
- `backend/services/concert-service/tests/realtime/`

### Implementation

Packet delay/gap, slow client, background/foreground, ticket replay, server restart and room switch.

### Required acceptance evidence

Bounded queues, no ghost player or duplicated room membership, eventual state convergence.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## QA06 — Mobile visual acceptance

**Phase:** 4 | **Priority:** P0 | **Lane:** QA
**Status:** NOT_STARTED
**Dependencies:** AR02, AR03, AR04, FE13, FE23, FE26, FE30, FE33
**IA coverage:** Cross-cutting implementation

### Owned implementation paths

- `frontend/e2e/visual-mcr/`
- `docs/mcr/evidence/mobile/`

### Implementation

Test widths 360/390/430, actual iOS Safari and Android Chrome, keyboard, safe area and all three skins.

### Required acceptance evidence

Avatars remain legible, controls reachable, no panel clipping or old UI; record device/browser versions.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## QA07 — Accessibility and input suite

**Phase:** 4 | **Priority:** P0 | **Lane:** QA
**Status:** NOT_STARTED
**Dependencies:** FE18, FE19, FE32, FE36
**IA coverage:** Cross-cutting implementation

### Owned implementation paths

- `frontend/e2e/accessibility/`

### Implementation

Keyboard/focus/screen-reader labels, DOM alternatives, reduced motion and pointer cancellation.

### Required acceptance evidence

Essential social/pickup tasks work without precise canvas interaction; focus returns on sheet close.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## QA08 — Capacity and performance qualification

**Phase:** 4 | **Priority:** P0 | **Lane:** QA
**Status:** NOT_STARTED
**Dependencies:** BE44, FE41
**IA coverage:** Cross-cutting implementation

### Owned implementation paths

- `scripts/mcr/load/`
- `docs/mcr/evidence/performance/`

### Implementation

Initial 200 sockets/four rooms scenario, mobile rendering/bundle/memory measurements, slow-network and reconnect storms.

### Required acceptance evidence

Publish measured ceilings and chosen caps; no claim of 200-user support without results.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## QA09 — Recovery and lifecycle rehearsal

**Phase:** 4 | **Priority:** P0 | **Lane:** QA
**Status:** NOT_STARTED
**Dependencies:** BE44, BE42, FE39
**IA coverage:** Cross-cutting implementation

### Owned implementation paths

- `scripts/mcr/recovery/`
- `docs/mcr/evidence/recovery/`

### Implementation

Database restart/backup restoration, outbox replay, pending pickup, deletion and partial event closure.

### Required acceptance evidence

Restored state is consistent; no lost committed pickup; shutdown and cleanup are resumable.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## QA10 — Candidate provenance and release checks

**Phase:** 4 | **Priority:** P0 | **Lane:** QA
**Status:** NOT_STARTED
**Dependencies:** BE43, QA01, QA02
**IA coverage:** Cross-cutting implementation

### Owned implementation paths

- `scripts/mcr/release-check.sh`
- `docs/mcr/evidence/release/`

### Implementation

Verify same-candidate versions, genuine tests, no production mocks/secrets and task-to-commit traceability.

### Required acceptance evidence

Missing evidence is NOT_RUN/BLOCKED, not PASS; no stale revision substituted for current candidate.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## IN01 — Two-user world gate

**Phase:** 1 | **Priority:** P0 | **Lane:** INTEGRATOR
**Status:** NOT_STARTED
**Dependencies:** BE06, BE07, BE14, FE09, FE13, FE17, FE18, AR02, AR03, AR04
**IA coverage:** Cross-cutting implementation

### Owned implementation paths

- `frontend/e2e/real/world.spec.ts`

### Implementation

Two independent accounts enter the same live backend world, move, customize, change skin, navigate panels and reconnect.

### Required acceptance evidence

Both observe authorized movement; no duplicate game/socket; actual art passes visual check.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## IN02 — Real meetup and pickup gate

**Phase:** 2 | **Priority:** P0 | **Lane:** INTEGRATOR
**Status:** NOT_STARTED
**Dependencies:** IN01, BE19, BE23, BE24, FE23, FE25, FE26
**IA coverage:** Cross-cutting implementation

### Owned implementation paths

- `frontend/e2e/real/meetup-pickup.spec.ts`

### Implementation

Party request/approval, physical beacon/rally, giveaway reservation and confirmed pickup on two clients.

### Required acceptance evidence

Full journey against real services; virtual walking never changes physical arrival.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## IN03 — Social and privacy gate

**Phase:** 3 | **Priority:** P0 | **Lane:** INTEGRATOR
**Status:** NOT_STARTED
**Dependencies:** BE27, BE30, BE32, BE33, BE37, FE28, FE30, FE31, FE35, FE36
**IA coverage:** Cross-cutting implementation

### Owned implementation paths

- `frontend/e2e/real/social.spec.ts`

### Implementation

Intro discovery, feed/comment, consent/reveal/revoke, notifications and block propagation.

### Required acceptance evidence

Restricted age and revoked/blocked cases fail closed on HTTP, WS and client cache.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## IN04 — Shared mural and moderation gate

**Phase:** 3 | **Priority:** P0 | **Lane:** INTEGRATOR
**Status:** NOT_STARTED
**Dependencies:** BE40, FE33, FE39
**IA coverage:** Cross-cutting implementation

### Owned implementation paths

- `frontend/e2e/real/mural.spec.ts`

### Implementation

Multiple clients paint, hit cooldown, experience pause, restore and final freeze.

### Required acceptance evidence

All clients converge to authoritative revision; unauthorized moderation fails.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## IN05 — Release decision

**Phase:** 4 | **Priority:** P0 | **Lane:** INTEGRATOR
**Status:** NOT_STARTED
**Dependencies:** IN02, IN03, IN04, FE40, QA03, QA04, QA05, QA06, QA07, QA08, QA09, QA10
**IA coverage:** Cross-cutting implementation

### Owned implementation paths

- `docs/mcr/release-decision.md`

### Implementation

Review same-candidate evidence, scope cuts, operator settings and known limitations; publish pass/block decision.

### Required acceptance evidence

All required gates executed; master contains tested code and evidence; no false implementation claims.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## EX01 — Party-only free-text chat

**Phase:** 5 | **Priority:** P1 | **Lane:** FULLSTACK
**Status:** NOT_STARTED
**Dependencies:** IN05
**IA coverage:** M10

### Owned implementation paths

- `backend/crates/concert-application/src/party_chat/`
- `frontend/src/features/party-chat/`

### Implementation

Add authorized history/send/reconnect, spam limits, report support, and member-removal revocation.

### Required acceptance evidence

Removed or blocked user cannot read/send; persisted ordering survives reconnect.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## EX02 — Mural history and souvenir export

**Phase:** 5 | **Priority:** P1 | **Lane:** FULLSTACK
**Status:** NOT_STARTED
**Dependencies:** IN05
**IA coverage:** R02, X02

### Owned implementation paths

- `backend/crates/concert-application/src/archive_export/`
- `frontend/src/features/souvenir/`

### Implementation

Snapshot history/time-lapse and opt-in avatar keepsake; rights and consent-aware export.

### Required acceptance evidence

Private handles/locations excluded; opted-out/deleted avatars handled correctly.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## EX03 — PWA and offline read mode

**Phase:** 5 | **Priority:** P1 | **Lane:** FE-APP
**Status:** NOT_STARTED
**Dependencies:** IN05
**IA coverage:** Cross-cutting implementation

### Owned implementation paths

- `frontend/service-worker/`
- `frontend/src/features/offline/`

### Implementation

Cache public static assets and permitted read-only material; never cache private reveal responses.

### Required acceptance evidence

Offline mode labels staleness and cannot silently queue stock or consent mutations.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## EX04 — Additional identity providers

**Phase:** 5 | **Priority:** P1 | **Lane:** BE-CORE
**Status:** NOT_STARTED
**Dependencies:** IN05
**IA coverage:** Cross-cutting implementation

### Owned implementation paths

- `backend/services/auth-service/`
- `frontend/src/features/auth/`

### Implementation

Add only configured/verified providers and explicit account-linking conflict handling.

### Required acceptance evidence

Real provider acceptance and takeover-prevention tests; unavailable provider not advertised as working.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## EX05 — Additional cosmetics and scenery

**Phase:** 5 | **Priority:** P1 | **Lane:** ART
**Status:** NOT_STARTED
**Dependencies:** IN05
**IA coverage:** Cross-cutting implementation

### Owned implementation paths

- `frontend/public/world/`

### Implementation

More outfits, decorations and optional day/night variants on unchanged map geometry.

### Required acceptance evidence

New assets obey atlas contract and performance budget; identity remains readable.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.

## EX06 — Advanced discovery and saved items

**Phase:** 5 | **Priority:** P1 | **Lane:** FULLSTACK
**Status:** NOT_STARTED
**Dependencies:** IN05
**IA coverage:** Cross-cutting implementation

### Owned implementation paths

- `backend/crates/concert-application/src/discovery/`
- `frontend/src/features/discovery/`

### Implementation

Add bookmarks and deeper filtering after event-specific demand is demonstrated.

### Required acceptance evidence

Visibility changes remove cached/private saved-item previews; no unintended recommendation exposure.

### Delivery record

Record contract/source revisions, commands and exit codes, evidence paths, known limitations, and the verified remote commit on master. A mock-only journey is not real-server acceptance.
