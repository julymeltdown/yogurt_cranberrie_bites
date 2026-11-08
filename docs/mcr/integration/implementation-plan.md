# Implementation plan — work packages and sequencing

Status: **PLANNING_COMPLETE / IMPLEMENTATION_NOT_STARTED**. No work package below
has started. The machine-readable register is
[work-packages.json](work-packages.json).

## 1. Ground rules

- One work package, one owner, one write scope. A package that needs a file
  outside its scope requests an explicit transfer and records it in the package.
- Backend packages write only `backend/` and `contracts/`; frontend packages write
  only `frontend/`. A package spanning both is split into a `-be` and a `-fe`
  package that share one contract.
- Every package ends with: implementation, destination tests, executed
  verification cases, a commit on `master`, and a short evidence record under
  `docs/mcr/integration/evidence/`.
- A package may start only when its dependencies are `INTEGRATED`. Contracts
  (`WP-01`, `WP-02`) are frozen before dependent code begins, so frontend and
  backend can proceed in parallel against the frozen shapes.
- No package may weaken an existing MCR guarantee (session, CSRF, origin,
  revision, no-store, fail-closed policy).

## 2. Work packages

### WP-01 — Freeze the contract

| Field | Value |
| --- | --- |
| Owner | Contract writer (single writer) |
| Write scope | `contracts/core/v1/`, `contracts/domain/v1/`, `contracts/generated/`, `frontend/src/generated/api-client/`, `backend/scripts/docs/` |
| Depends on | — |
| Deliverable | Every operation in [http-contract.md](http-contract.md) §2 declared with schemas; `Kind::INTRO_LIKED` and `TargetType::INTRO_POST` added to the proto and domain enum; generators run; `scripts/mcr/check.sh contracts` passes. |
| Definition of done | `python3 backend/scripts/docs/generate-mcr-domain.py --check` and `generate-mcr-core.py --check` exit 0; `python3 scripts/mcr/validate-domain.py` exits 0; generated client typechecks; a contract test asserts each new operation's rule, cache, and error set. |
| Blocks | Everything else. |

### WP-02 — Schema and migrations

| Field | Value |
| --- | --- |
| Owner | Backend |
| Write scope | `backend/services/concert-service/migrations/0080_*.sql`, `backend/services/profile-service/migrations/`, `backend/crates/concert-domain/src/` (new modules only) |
| Depends on | WP-01 |
| Deliverable | `profiles`, `profile_favorites`, `profile_rooms`, `profile_avatars`, `intro_posts`, `intro_attachments`, `intro_likes`, `intro_comments`, `profile_posts`, `profile_post_attachments`, `media_assets` (profile-scoped) with the invariants of [domain-contract.md](domain-contract.md) §4. |
| Definition of done | Migrations apply cleanly to an empty database and to a database at the current head; CHECK constraints reject the documented invalid rows; no migration edits an applied file; the migration ledger test passes. |
| Blocks | WP-04, WP-05, WP-06, WP-07, WP-08 |

### WP-03 — Favorites and discovery

| Field | Value |
| --- | --- |
| Owner | Backend + frontend (split) |
| Write scope | `backend/services/concert-service/src/discovery/`, `backend/services/profile-service/src/` (favorites), `frontend/src/features/favorites/` |
| Depends on | WP-02 |
| Deliverable | Catalog search restricted to three categories with K-pop excluded from music; up to five favorites per category; `discoverProfiles` returning genuine overlaps; `FavoritePicker` UI. |
| Definition of done | Discovery works for a profile with **no** intro post; overlap uses catalog IDs only; no score or ranking field exists in the response; a profile with no overlap is returned without a shared-interest claim. |
| Blocks | WP-09 |

### WP-04 — Identity claims and visibility policy

| Field | Value |
| --- | --- |
| Owner | Backend |
| Write scope | `backend/crates/concert-application/src/identity_claims.rs`, `backend/crates/concert-application/src/intro_visibility.rs`, `backend/services/concert-service/src/intro_posts/visibility_sql.rs`, `backend/services/concert-service/tests/intro_visibility_contract.rs` |
| Depends on | WP-02 |
| Deliverable | `IdentityClaimsPort` with the age adapter over the existing claim store and a gender adapter that returns `Unspecified` until D-01 is decided; `can_view_intro`; the SQL predicate; a shared case table executed against both. |
| Definition of done | V-01, V-02, V-03 pass. The pure function and the SQL predicate return identical decisions for the full case table. |
| Blocks | WP-05, WP-06, WP-07 |

### WP-05 — Intro posts, hearts, likers, notifications

| Field | Value |
| --- | --- |
| Owner | Backend (`-be`) and frontend (`-fe`) |
| Write scope | `backend/services/concert-service/src/intro_posts/`, `backend/services/concert-service/src/notification_live/`, `frontend/src/features/intros/` |
| Depends on | WP-04, WP-01 |
| Deliverable | Draft/save/publish/delete; audience fields; attachments; heart toggle; author-only liker list; `INTRO_LIKED` notification with dedupe; comments and replies; composer UI; likers panel; notification navigation. |
| Definition of done | V-03, V-04, V-05, V-10 pass. The heart never creates a follow, request, or disclosure. The liker list is author-only. |
| Blocks | WP-10 |

### WP-06 — Profile posts and profile media

| Field | Value |
| --- | --- |
| Owner | Backend (`-be`) and frontend (`-fe`) |
| Write scope | `backend/services/concert-service/src/profile_posts/`, `backend/services/concert-service/src/profile_media/`, `frontend/src/features/profile-media/` |
| Depends on | WP-04, WP-02 |
| Deliverable | Profile-scoped media upload, normalization, and owner-scoped read; ordinary photo/artwork posts with their own visibility; visitor-facing gallery; composer. |
| Definition of done | V-06, V-07 pass. No pixel conversion anywhere in this path. Attachments from restricted intro posts cannot be republished by media ID. |
| Blocks | WP-09, WP-10 |

### WP-07 — Personal room and posters

| Field | Value |
| --- | --- |
| Owner | Backend (`-be`) and frontend (`-fe`) |
| Write scope | `backend/services/concert-service/src/profile_room/`, `frontend/src/features/profile-room/` |
| Depends on | WP-04, WP-02 |
| Deliverable | Room layout persistence with revision; furniture grid placement; wall/floor styles; poster import → crop → pixel preview → place; up to four posters; room visibility; scene runtime with cleanup. |
| Definition of done | V-08, V-10 pass. Poster conversion is deterministic and browser-side. Ordinary photos are never converted. Room state is untouched by intro post changes. |
| Blocks | WP-09 |

### WP-08 — Avatar re-keying

| Field | Value |
| --- | --- |
| Owner | Backend (`-be`) and frontend (`-fe`) |
| Write scope | `backend/crates/concert-domain/src/profile_avatar.rs`, `backend/services/concert-service/src/profile_avatar/`, `frontend/src/features/profile-avatar/`, `frontend/src/entities/emo-avatar/` |
| Depends on | WP-02 |
| Deliverable | `GET/PUT /api/me/avatar` keyed by `owner_profile_id`; editor migrated from `MotionEditor`; persistence without an event ID; source pack preserved. |
| Definition of done | V-09 passes for the avatar cache; the avatar persists after the concert event closes; no test-only authority is reachable in production configuration. |
| Blocks | WP-09 |

### WP-09 — Navigation and profile screens

| Field | Value |
| --- | --- |
| Owner | Frontend |
| Write scope | `frontend/src/app/` (router, routes, shell), `frontend/src/features/profile/` |
| Depends on | WP-03, WP-06, WP-07, WP-08 |
| Deliverable | Three-section navigation (Discover, Activity, My Profile); profile page composing introduction, favorites, posts, room entry; visitor profile page; room as a subordinate screen; intro composer as an independent screen. |
| Definition of done | Every screen reachable; the audience control is labelled "Who can view my mutuals-intro post" and lives only in the composer/editor; no room preview inside an intro post; the concert screens remain reachable from a secondary menu. |
| Blocks | WP-10 |

### WP-10 — End-to-end integration and release evidence

| Field | Value |
| --- | --- |
| Owner | Integrator |
| Write scope | `frontend/e2e/`, `docs/mcr/integration/evidence/` |
| Depends on | WP-05, WP-06, WP-09 |
| Deliverable | The §9 scenario of [product-plan.md](product-plan.md) executed against real services with three identities; the ten verifications recorded with commands and exit codes. |
| Definition of done | Every verification in [verification-plan.md](verification-plan.md) is `PASS` with evidence, or explicitly `BLOCKED` with the blocking condition named. No claim of completion for a `NOT_RUN` item. |
| Blocks | — |

## 3. Waves

```text
Wave 0   WP-01  (contract freeze)
Wave 1   WP-02  (schema)
Wave 2   WP-03, WP-04, WP-08   (parallel; disjoint scopes)
Wave 3   WP-05, WP-06, WP-07   (parallel; disjoint scopes)
Wave 4   WP-09  (needs the screens' data)
Wave 5   WP-10  (needs everything)
```

Within a wave, backend and frontend packages of the same feature run in parallel
against the frozen contract with deterministic fixtures. A frontend package does
not wait for its backend package to finish; it waits for the contract only.

## 4. File ownership map

| Prefix | Owner | Notes |
| --- | --- | --- |
| `contracts/` | WP-01 | Shared; any later change needs a recorded decision. |
| `backend/crates/concert-domain/src/{intro_audience,profile_avatar,profile_room}.rs` | WP-02, WP-08, WP-07 | One new module per owner; no two packages edit the same module. |
| `backend/crates/concert-application/src/{identity_claims,intro_visibility,intro_likes}.rs` | WP-04, WP-05 | |
| `backend/services/concert-service/src/intro_posts/` | WP-05 | |
| `backend/services/concert-service/src/profile_posts/`, `profile_media/` | WP-06 | |
| `backend/services/concert-service/src/profile_room/` | WP-07 | |
| `backend/services/concert-service/src/profile_avatar/` | WP-08 | |
| `backend/services/concert-service/src/discovery/` | WP-03 | |
| `backend/services/api-gateway/src/adapters/in/http/` | Gatekeeper | New route modules only; the shared router registration is a single-writer transfer. |
| `frontend/src/features/intros/` | WP-05 | |
| `frontend/src/features/profile-media/`, `frontend/src/features/media/` | WP-06 | `features/media/` is shared with WP-05; assign it to WP-06 and let WP-05 consume it. |
| `frontend/src/features/profile-room/` | WP-07 | |
| `frontend/src/features/profile-avatar/`, `frontend/src/entities/emo-avatar/` | WP-08 | |
| `frontend/src/features/favorites/` | WP-03 | |
| `frontend/src/app/` | WP-09 | Router, routes, shell. Single writer. |
| `frontend/src/shared/ui/` | WP-09 | New primitives only; do not restyle existing ones. |

## 5. Definition of done for every package

1. Code compiles and lints: `scripts/mcr/check.sh backend` /
   `scripts/mcr/check.sh frontend`.
2. Contract checks pass: `scripts/mcr/check.sh contracts`.
3. Boundary and origin guards pass: `scripts/mcr/check.sh guards`.
4. The package's verification cases from
   [verification-plan.md](verification-plan.md) are executed and recorded with
   command, exit code, and output path.
5. The package's row in [work-packages.json](work-packages.json) is updated with
   status, commit, and evidence path.
6. The commit message names the source provenance for any adapted file.

## 6. Risks and mitigations

| Risk | Impact | Mitigation |
| --- | --- | --- |
| Gender has no source of truth | Audience gender control is unusable or misleading | Decide D-01 before WP-04 ships; ship `any`-only and hide the control otherwise. Never present a self-declared value as verified. |
| Age claim store is incomplete for many accounts | Restricted posts invisible to most users | This is correct fail-closed behavior. Surface an explicit "verify your age to see this post" state that does not reveal content. |
| Room layout conflicts under fast dragging | Lost user work | Revision conflict returns the current layout; the client keeps the unsaved layout and offers resolution. |
| Phaser instance duplication | Memory and lifecycle bugs | One instance per shell; the room scene acquires a lease and releases it on unmount. Verification V-10 covers cleanup. |
| Contract drift between pure policy and SQL | Access bypass | One case table executed against both (WP-04). |
| Media authorization broadened by accident | Restricted photo leak | Media authorization always joins the attached post; V-06 tests the leak explicitly. |
| Bigyeting baseline mismatch (`dd47ed5` vs `5ee…`) | Adapting a file that changed | Reconcile per [source-register.md](source-register.md) §3 before adapting. |
| Concert world left in the primary navigation | Product focus diluted | P-02 decision; default hides it from primary tabs. |
