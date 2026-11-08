# Release evidence — WP-10 integration gate

Status: **INCOMPLETE**. Recorded 2026-11-09 against master `d2f545a89`.

This document is the assembled evidence for the profile/social integration. It
records exactly what was run, what it proved, and what it did not. Every row
carries a command, an exit code and an artifact, or it is marked `NOT_RUN` /
`BLOCKED`. No row is `PASS` without an artifact.

Machine-readable companion: [wp10-gate-report.json](evidence/wp10-gate-report.json)
(the full gate report, including every probe's status and reason).

## 1. Candidate

| Field | Value |
| --- | --- |
| Worktree | `/Users/lhs/Desktop/yogurt-worktrees/wp07-room` |
| Branch | `codex/yc-wp07-room` |
| Base | master `d2f545a89` ("docs: describe profile product and integration gaps") |
| Merged packages | WP-01 contract, WP-02 schema, WP-03 discovery, WP-04b identity, WP-06 profile posts, WP-07 room, WP-08 avatar |
| In flight (not on master) | WP-04 claims/visibility, WP-05 intro posts, WP-09 navigation, art layer, producer composition |
| Gate script | `backend/scripts/mcr/wp10_profile_social.py` (sha256 recorded in the report) |

## 2. Verification rows

| ID | Claim | Status | Command | Exit | Artifact |
| --- | --- | --- | --- | --- | --- |
| GUARDS | Repository guards (exact bytes, static imports) | **PASS** | `bash scripts/mcr/check.sh guards` | 0 | `violations: []`, 5990 content files, 4 reviewed extracts |
| BUILD-BE | Backend workspace compiles | **PASS** | `cd backend && cargo +1.88.0 check --locked --workspace` | 0 | — |
| BUILD-FE | Frontend typecheck | **PASS** | `cd frontend && corepack pnpm@11.7.0 run typecheck` | 0 | — |
| T-951 | Three-identity fixture | **PARTIAL** | `python3 backend/scripts/mcr/wp10_profile_social.py --bin-dir … --report …` | 1 | `wp10-gate-report.json` `identities` |
| T-952 | End-to-end scenario (six steps) | **BLOCKED** | same | 1 | `wp10-gate-report.json` `steps` |
| T-953 | Cross-surface consistency sweep | **BLOCKED** | same | 1 | `wp10-gate-report.json` `sweeps.cross_surface` |
| T-954 | Lifecycle independence sweep | **PARTIAL** | same | 1 | `wp10-gate-report.json` `sweeps.lifecycle` |
| V-11 | End-to-end scenario with three identities | **BLOCKED** | same | 1 | `wp10-gate-report.json` |
| NAVER-CONSOLE | Real NAVER console application | **BLOCKED** | — | — | see §4 |

The gate command exits **1** because its result is `INCOMPLETE`: the run completed
and proved what the merged surface allows, but the scenario it exists to prove is
not reachable yet. A `0` here would mean the release gate passed, which it has
not.

### 2.1 T-951 — three-identity fixture (PARTIAL)

The three identities are real accounts created through the real enrollment flow
(`/api/auth/enrollment/attempts` → signup → mail verification → login →
admission), on the real services, against an owned private PostgreSQL 16 cluster.

| Identity | Role | Verified |
| --- | --- | --- |
| author | A — publishes the profile posts, the intro post and the poster | not required (an author needs no claim) |
| viewer | B — must satisfy "Women · Ages 18–29" | **VERIFIED** (provider profile `gender=F`, `birthyear=2001`, `birthday=03-14`) |
| outsider | C — must fail the audience or be unverifiable | **VERIFIED as male** (`gender=M`, `birthyear=1990`) so it fails the gender condition; the unverified variant is not exercised |

The verification runs the **real** flow: `POST /api/me/identity/naver/verifications`
returns a provider authorization URL, the test provider issues a code, the browser
callback `GET /api/identity/naver/callback?code=…&state=…` answers `303`, and
`GET /api/me/identity/naver` reports `VERIFIED` with `claims` present.

The provider is a **local test application** (`NaverProvider` in the gate script)
configured through `NAVER_OAUTH_TOKEN_URL` / `NAVER_OAUTH_PROFILE_URL`. It is not
the NAVER console application; see §4.

`PARTIAL` rather than `PASS` because:

- the deterministic MCR/NANA catalog items are declared in the fixture
  (`CATALOG_ITEMS`) but **not seeded**: the favorites surface they feed is not
  reachable over HTTP on this master, so seeding them would prove nothing;
- the "unverified C" variant is not exercised; only the "fails the audience"
  variant is.

### 2.2 T-952 / V-11 — end-to-end scenario (BLOCKED)

The six steps of [product-plan.md](../product-plan.md) §9 were driven over real
HTTP. Every step is recorded with its request and its response in the report.

| Step | Status | What was observed | Blocked on |
| --- | --- | --- | --- |
| 1 — A publishes a selfie and an artwork | **BLOCKED** | `POST /api/me/media/uploads` → `403 PERMISSION_DENIED` | the profile product producer's closed operation set does not include the profile post and media operations |
| 2 — A publishes a restricted intro post; B satisfies it | **BLOCKED** | `POST /api/me/intro-posts` → `404`; `GET /api/discover/intro-posts` → `404` | WP-05 intro post routes are not registered by the gateway |
| 3 — A reaches B's profile from the like notification | **BLOCKED** | `GET /api/notifications`, likers list, `GET /api/profiles/{B}` recorded | WP-05 hearts, `INTRO_LIKED` notification, author-only liker list |
| 4 — A places a poster; the intro post and likes are unchanged | **PARTIAL** | `POST /api/me/room/posters` → `403`; intro delete → `404` | room producer not reachable over HTTP; intro delete not composed |
| 5 — C cannot view the restricted post, its attachments or its bytes | **BLOCKED** | three C requests recorded, none reachable | WP-04 audience decision and WP-05 surfaces |

The root cause of steps 1 and 4 is one composition gap, not four separate
defects: the gateway registers the profile product routes
(`/api/me/posts`, `/api/me/media/*`, `/api/me/room*`, `/api/me/avatar`,
`/api/profiles/{id}/posts`, `/api/profiles/{id}/room`), but the **profile producer
that answers them is the profile-service**, whose closed operation set is
`getOwnProfile`, `getProfile`, and the three identity operations. Every other
profile route therefore answers `403 PERMISSION_DENIED` from the producer. The
producers themselves exist and are tested (WP-06, WP-07, WP-08), but they are
`concert-service` modules and no transport dispatches to them yet.

### 2.3 T-953 — cross-surface consistency sweep (BLOCKED)

Eight surfaces were requested as C. None is reachable, so the sweep cannot assert
eight identical denials:

| Surface | Request | Status |
| --- | --- | --- |
| list | `GET /api/profiles/{A}/posts` | 403 PERMISSION_DENIED (producer gap) |
| detail | `GET /api/intro-posts/{id}` | 404 (route not registered) |
| attachment | `GET /api/intro-posts/{id}/attachments` | 404 (route not registered) |
| thumbnail | `GET /api/media/{id}` | 403 PERMISSION_DENIED (producer gap) |
| bytes | `GET /api/media/{id}/content` | 403 PERMISSION_DENIED (producer gap) |
| shared link | `GET /api/intro-posts/resolve` | 404 (route not registered) |
| like | `PUT /api/intro-posts/{id}/like` | 400 (route not registered) |
| comment | `POST /api/intro-posts/{id}/comments` | 400 (route not registered) |

The 403s and 404s are **not** the audience decision. They are a missing producer
and missing routes, and the sweep records them as such rather than counting them
as consistent denials. V-03 remains `NOT_RUN`.

### 2.4 T-954 — lifecycle independence sweep (PARTIAL)

Two layers were run.

**Layer 1 — durable (executed, PASS).** The same operations run against the gate's
own database, with full row captures before and after:

| Pairing | Result |
| --- | --- |
| room visibility PRIVATE | room changed; avatar, profile posts and media rows byte-identical |
| room visibility restored PUBLIC | room shape restored, revision advanced |
| avatar save | avatar changed; room, posts and media byte-identical |
| profile post delete | **NOT_RUN** — no profile post row exists (the producer is not reachable over HTTP) |
| intro audience ↔ room visibility | **BLOCKED** — no intro post surface on this master |

**Layer 2 — over HTTP (recorded, all refused).** `GET /api/me/room`,
`GET /api/me/avatar`, `GET /api/profiles/{A}/posts` and
`PATCH /api/me/room/visibility` each answer `403 PERMISSION_DENIED` for the same
producer gap as §2.2.

`PARTIAL`, not `PASS`: the intro half of V-08 is the half the task names first,
and it cannot be exercised.

## 3. What is NOT proven

Stated plainly, because the release decision depends on it:

1. **No profile-owned operation works over HTTP.** Profiles, profile posts,
   profile media, the room and the avatar are all refused at the gateway by the
   producer's closed operation set. Their packages' own tests pass; the composed
   product does not serve them.
2. **No intro post exists in the running product.** The intro post, its audience,
   its attachments, its likes, its comments and its shared link have no route.
   Every V-03, V-04, V-05, V-06 and V-13 case is `NOT_RUN`.
3. **The audience decision is never exercised end to end.** V-01, V-02 and V-03
   are `NOT_RUN`. The gate proves that B is verified as a woman aged 18–29 and C
   as a man; it does not prove that the product acts on it.
4. **The six-step scenario has not run once.** Not partially, not in a mock.
5. **No browser ran.** The gate is HTTP-only; the 360/390/430 px visual
   acceptance for every screen remains open.
6. **No real NAVER application exists.** See §4.
7. **QA08 performance is `NOT_RUN`** and is not claimed by this gate.

## 4. BLOCKED items and their exact conditions

| Item | Exact condition |
| --- | --- |
| NAVER-CONSOLE | There is no test NAVER application in this environment: no `NAVER_OAUTH_CLIENT_ID` / `NAVER_OAUTH_CLIENT_SECRET` issued by the NAVER developer console, and no console-registered callback `https://<host>/api/identity/naver/callback`. The gate therefore configures the product's provider client against a **local test application** with three fixed provider profiles. Everything downstream of the token exchange is real; the token exchange and the profile fetch are not NAVER's. A production deployment must refuse the placeholder client id (`local-` prefix) — that refusal is already implemented and tested (`naver_config.rs`), but it is not a substitute for a console approval. |
| V-11 / T-952 | The profile product producer does not serve the profile post, media, room, avatar or intro operations, and the intro routes are not registered by the gateway. |
| V-03 / T-953 | Same condition as V-11, plus: there is no intro post to restrict, so there is nothing to deny consistently. |
| V-01, V-02, V-04, V-05, V-06, V-07, V-13 | The intro post and audience surfaces do not exist in the running product. |
| V-08 (intro half) | No intro post surface; the room/avatar/post half is executed (§2.4). |
| V-09 (room/avatar HTTP half) | The room and avatar producers are not reachable over HTTP; their own package tests cover ownership and conflicts. |
| V-12 (live provider) | The flow is exercised against the local test application only; the live NAVER cases (`V-12-a` … `V-12-p`) are `NOT_RUN` against the real provider. |
| T-951 catalog seeding | The favorites surface is not reachable over HTTP, so seeding catalog items would not be observable. |
| Browser captures | No browser session was started against a live stack. |

## 5. Reproducing this document

```sh
# 1. Build the seven services (the gate exercises real binaries, not mocks).
cd backend && cargo +1.88.0 build --locked -p api-gateway -p auth-service \
  -p profile-service -p concert-service -p friend-service \
  -p notification-service -p post-service

# 2. Run the gate. It owns a private PG16 cluster under /tmp and removes it.
cd .. && python3 backend/scripts/mcr/wp10_profile_social.py \
  --bin-dir "$PWD/backend/target/debug" \
  --report /tmp/wp10/report.json
# exit 1 (INCOMPLETE) on this master; see §2.

# 3. The build checks.
cd backend && cargo +1.88.0 check --locked --workspace
cd ../frontend && corepack pnpm@11.7.0 run typecheck
cd .. && bash scripts/mcr/check.sh guards
```

The gate is reproducible from a clean database: it creates its own cluster, applies
every migration through the service binaries, and seeds the three identities
through the enrollment flow. Nothing is carried over between runs.
