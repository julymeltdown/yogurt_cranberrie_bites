# Yogurt Cranberrie Bites

A profile-centered social application for discovering people through shared music,
anime, and game interests, then getting to know them through introductions,
photographs, original artwork, and independent mutuals-intro posts.

Music, especially emo, is the entry point. Users choose favorite bands and artists
(K-pop excluded), anime, and games. Discovery shows the actual items people share;
it does not calculate compatibility scores. A personal pixel room belongs to the
profile and is a separate form of self-expression.

**Implementation is in progress. The integrated product has not been accepted for
release.** Feature modules, migrations, and package-level test evidence exist, but
the main application still uses the inherited concert shell and several production
request paths are unfinished. The application snapshot assessed below is
[`02c0b321`](https://github.com/julymeltdown/yogurt_cranberrie_bites/commit/02c0b32163bad1226332b5174aa32c43abee033a),
recorded on **2026-11-09 (Asia/Seoul)**.

- **Current delivery:** [`julymeltdown/yogurt_cranberrie_bites`, `master`](https://github.com/julymeltdown/yogurt_cranberrie_bites/tree/master).
- **Product requirements:** [profile, mutuals-intro, photo, and room plan](docs/mcr/integration/product-plan.md).
- **Implementation findings:** [dated assessment, evidence, and remaining work](docs/mcr/integration/implementation-assessment-2026-11-09.md).
- **Engineering entry point:** [integration document index](docs/mcr/integration/README.md).

## Product model

| Surface | Intended behavior |
| --- | --- |
| Profile | Persistent nickname, introduction, profile image, emo avatar, and selected favorites. |
| Discover | People with shared catalog items; a separate list of mutuals-intro posts. Up to five favorites in each of music, anime, and games. |
| Mutuals-intro post | Optional introduction with its own audience, images, hearts, comments, and replies. No decorated room is required. |
| Heart | An explicit like on an intro post. It does not create a friend request, an approval, an automatic follow, or a contact disclosure. |
| Profile posts | Ordinary selfies, photographs, and original artwork, published independently of intro posts and rooms. |
| Personal room | One saved room per profile, with an emo character, furniture, and up to four image-derived pixel posters. |
| Activity | Likes received, comments, replies, and independent follow notifications. |

The intended primary navigation is **Discover / Activity / My Profile**. The
current router still defines **World / Feed / Meet / Gifts / Wall / Me**; replacing
that composition is remaining implementation work.

### Independence and visibility

- A room belongs to a profile, never to an intro post. Intro deletion and room
  visibility must not change one another.
- Ordinary photographs are not pixel-converted. Conversion belongs only to the
  explicitly initiated room-poster flow.
- Restricted intro attachments must not appear automatically in a profile gallery
  or room.
- An intro's age/gender audience decides **who may view that post**. It is not a
  viewer-side search preference or a global registration gate.
- NAVER supplies verified age/gender attributes. Missing viewer attributes fail
  closed for restricted intros; unrestricted use must not require verification.
- Likes and follows are independent. No application/acceptance or reward flow is
  part of the product.
- Rooms do not claim online presence. Direct messages, real-time room visiting,
  voice chat, video, and multiple rooms are outside the initial scope.

These are requirements, not a claim that every user flow is already connected.
See the [product plan](docs/mcr/integration/product-plan.md) and
[integration charter](docs/mcr/integration/integration-charter.md).

## Implementation snapshot

The purpose-change commit was
[`745ea437`](https://github.com/julymeltdown/yogurt_cranberrie_bites/commit/745ea437e28deef6d989d075e444e250c3c4ebe9).
Between that commit and the assessed application snapshot there are **19 commits:
12 non-merge commits and seven merges**, changing **278 files**. Generated
contracts and fixtures account for much of the line-count growth; these counts
are not a completion percentage.

| Area | Present in the assessed source | Remaining before an integrated user flow |
| --- | --- | --- |
| Contracts — WP-01 | 43 added operation declarations, schemas, generated clients, notification enum additions. | Reconcile runtime payloads with the frozen shapes and add producer/consumer conformance evidence. |
| Schema and transport — WP-02 | Profile-owned identity, favorites, claims, social/media tables, and event-free profile transport. | Provision new identities after registration, resolve account-to-profile IDs, complete mutations and cross-service dispatch. |
| Favorites and discovery — WP-03 | Seed catalog, search/query modules, picker and people-list components. | Persist favorites, populate discovery projections, mount APIs, compose the screen. The intro list is a slot. |
| NAVER verification — WP-04b | Start, callback, status, disconnect, provider client, gateway transport, and UI components. | Mount the landing/status UI, qualify a real provider round trip, and verify re-verification subject-key invariants. |
| Intro audience — WP-04 | Schema/contract groundwork and inherited event-scoped age logic. | Profile-scoped claims adapter and one consistent policy across every intro access path. |
| Intro posts and hearts — WP-05 | Tables, constraints, operation declarations, and notification kind. | New profile-scoped commands, reads, hearts, likers, comments, and notification delivery. |
| Profile posts/media — WP-06 | Upload lifecycle, post handlers, authorization helpers, composer/gallery. | Production dispatch, profile/media response reconciliation, and main-screen composition. |
| Room/posters — WP-07 | Persistence, editor, pixel conversion, scoped scene, and subordinate route. | Production dispatch, owner entry, poster-byte route, shared scene lifecycle, and browser acceptance. |
| Profile avatar — WP-08 | Profile ownership, persistence/handlers, editor adaptation, account-scoped client caches. | Profile producer dispatch and complete server/client account-context behavior. |
| Navigation — WP-09 | Existing shell plus partial room/avatar additions. | Event-independent profile shell and complete Discover / Activity / My Profile composition. |
| Integrated acceptance — WP-10 | Scenario and verification definitions. | Complete real-service, multi-identity execution with current evidence. |

Package documents that say `INTEGRATED` must be read with their explicit
limitations. A registered gateway route, a handler tested directly against PostgreSQL,
or a component tested with a substitute server is not proof that the full browser
request reaches that handler.

### Immediate integration priorities

1. Establish the canonical account/profile mapping and provision/synchronize
   profile identities for new and existing users.
2. Complete profile mutations, service-to-service authority/dispatch, and discovery
   projection updates; keep public/owner responses conformant with the contract.
3. Finish the intro audience and heart/comment/notification flow.
4. Compose the new shell, profile pages, NAVER return route, and owner room flow;
   reconcile media routes and Phaser ownership.
5. Qualify account deletion, privacy changes, provider behavior, and the full
   three-identity scenario; repair current CI failures and reconcile task evidence.

These priorities retain the approved scope. They do not remove discovery or rooms,
accept unfinished work, or alter the historical MCR task register. Detailed owners,
source locations, acceptance checks, and plan inconsistencies are in the
[assessment](docs/mcr/integration/implementation-assessment-2026-11-09.md).

## Verification status

The most recent application CI run for the assessed snapshot is
[`42525306588`](https://github.com/julymeltdown/yogurt_cranberrie_bites/actions/runs/42525306588),
which **failed**.

| Check | Observed result at `02c0b321` |
| --- | --- |
| Frontend typecheck, lint, build | Passed. |
| Frontend tests | 5,894 passed, 19 failed, 14 skipped; five failing files. |
| Backend `cargo check --workspace --all-targets --locked` | Failed: an existing integration-test fixture omits the new `naver_oauth` and `identity_subject_salt` fields. |
| Plan integrity, architecture guards, generated-client/transport checks | Passed within their defined scopes. |
| QA01 contract coverage | Failed: 13 pinned inputs differ from the recorded baseline. |
| QA10 release-evidence consistency | Passed; this does not establish the new product's release acceptance. |
| Real NAVER provider / complete new-product browser scenario | Not established by the checked evidence. |

The purpose-change baseline already had 16 failing frontend tests. The assessed
snapshot adds three contract-expectation failures and the backend/QA01 failures;
do not attribute every inherited failure to the new feature packages.
[Baseline CI](https://github.com/julymeltdown/yogurt_cranberrie_bites/actions/runs/42493910623)
and the [assessment](docs/mcr/integration/implementation-assessment-2026-11-09.md)
explain the distinction.

NAVER console review is recorded as **approved** in the task breakdown. The
verification package separately records that its test environment lacked the
configured provider application needed for a real round trip. Approval and
executed provider evidence are different facts.

## Architecture and repository layout

The application reuses the MCR foundation; selected Bigyeting headless logic is
adapted with provenance. The backend keeps Auth, Profile, Friend, Post,
Notification, Gateway, and Concert service boundaries. New profile/social/room
modules currently live mainly in Profile and Concert; their directory names do
not define the new product's event scope.

| Path | Contents |
| --- | --- |
| `frontend/` | React 19, TypeScript, Vite, React Router, and Phaser 3.90.0. |
| `backend/` | Rust services, PostgreSQL migrations, gRPC transport, HTTP gateway, and tests. |
| `contracts/` | Schemas, operation registries, fixtures, and generated contracts. |
| `docs/mcr/integration/` | Current product requirements, task definitions, assessment, and package evidence. |
| `docs/mcr/backend/` | Backend package implementation records and limitations. |
| `infra/mcr/`, `scripts/mcr/` | Pinned tooling, runtime support, checks, and evidence harnesses. |
| `generated/`, `docs/source/` | Preserved MCR planning/provenance baseline; not the new product's execution status. |

Source baselines and adaptation rules:
[integration charter](docs/mcr/integration/integration-charter.md),
[source register](docs/mcr/integration/source-register.md), and
[migration plan](docs/mcr/integration/migration-plan.md).

## Development checks

Pinned versions: **Node 24.22.2**, **pnpm 11.7.0**, and **Rust 1.88.0**.
The package manifests, lockfiles, and
[toolchain record](infra/mcr/toolchain.json) are the source of truth. Contract
checks additionally use [the Python requirements](scripts/mcr/requirements-contracts.txt).
Database-dependent package verification uses isolated PostgreSQL 16 stores.

From the repository root:

```sh
python3 scripts/mcr/check-plan.py
bash scripts/mcr/check.sh guards
bash scripts/mcr/check.sh contracts
bash scripts/mcr/check.sh frontend
bash scripts/mcr/check.sh backend
```

`check-plan.py` verifies the **preserved MCR planning baseline**. It does not
validate completion of the 110 integration tasks. The frontend script performs
install/build/lint checks, and the backend script builds the workspace; neither
is a full application acceptance test. The
[CI workflow](.github/workflows/mcr-ci.yml) declares the broader checks and their
current exclusions.

After installing the frontend dependencies with the pinned toolchain:

```sh
cd frontend
corepack pnpm@11.7.0 dev
```

This opens the current application, whose production composition still depends
on the inherited event shell. It does not launch the Rust services or provide a
working replacement profile product by itself. Use the owning package's runtime
instructions and isolated databases for backend verification.

## Documentation and status accounting

Read the [integration index](docs/mcr/integration/README.md) before starting work.
The 110-task register defines scope, dependencies, and acceptance obligations;
its current `NOT_STARTED` values are stale planning entries, not a live progress
counter. The dated assessment separates source implementation, request
reachability, package test evidence, and integrated acceptance.

The original [MCR specification](MCR_IMPLEMENTATION_PLAN_V2_EN.md) and its
112 required tasks remain historical requirements for the concert-world product.
Their statuses and performance deferrals must not be reused as acceptance of this
profile/social integration.
