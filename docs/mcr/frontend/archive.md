# FE40 / X01 public end-of-event archive

Status: **MOCK_READY** (2026-11-04, Asia/Seoul). The complete assigned read-only feature and its task-local fixture criteria are implemented. Real BE41 publication/deletion/closure, qualified public media, production routing and physical iOS/Android acceptance are **NOT_RUN**. This record does not accept IN05 or substitute client validation for server privacy enforcement.

## Assignment and provenance

- Writer: `mcr(fe) FE40 public event archive`, chat `01a19f83-2c2a-7fa0-a98d-1c4f5f65cb3c`.
- Isolated worktree `mcr-worktrees/fe-archive`, branch `codex/mcr-fe-FE40`, exact assignment base `cd7192febf9c2764169b55d63a70d7e00d13fd08`. The commit containing this record supplies the delivered SHA and exact changed paths; Delivery alone integrates/pushes master.
- Exclusive changes: `frontend/src/features/archive/**` and this document. No root router, shared client/cache, contract/generated artifact, manifest/lockfile, backend, feed/mural source, infrastructure or source archive changed. The prior moderation checkout remains stopped and preserved.
- Requirements read: `MCR_IMPLEMENTATION_PLAN_V2_EN.md`, especially X01, FE40, public projection/privacy, final wall and task-local evidence; `docs/mcr/project-assessment.md`; `docs/mcr/execution-plan.md`; root/frontend working agreements.
- Newly authored: archive controller/context/HTTP reader, minimized projection, React markup, canvas renderer, CSS and test fixtures. No legacy rendering, character artwork, live-feed state, live mural subscription/write controller, Phaser instance or WebSocket is reused. The abstract colored mural in fixtures is deterministic test content, not production art or an avatar replacement.

| Source | Pinned provenance and use |
| --- | --- |
| `contracts/domain/v1/admin.schema.json` | Last changed `5c02a31f26d2822f274bb09743178295fdbd110f`; SHA-256 `b8a4f8b91440d561bbcf23dc68bc196919a8893cf744ca09ac05ecaabff72cb8`; exact `Archive`, `ArchiveFeed`, `ArchivePost` allowlists |
| `contracts/domain/v1/mural.schema.json` | SHA-256 `345871ce89fabff02f06087fcd3e8907651a5712b34ccbd53091158af123b6ca`; exact `ArchiveMural` frozen snapshot |
| `contracts/domain/v1/operations.json` | SHA-256 `411fcee8222db2f5a7938866ac37791033e85682f11d024ceb895c245c708500`; `getConcertArchive` PUBLIC GET, PageQuery, no-store, no command/idempotency |
| `contracts/generated/mcr-domain.openapi.json` | Last changed `bff7e53c0638d5462af819bf560468a1f857c88e`; SHA-256 `edd8f89cda100a3f031d6e9b47ff39b615136938faab59259c7351851e139f1b`; resolved schemas consumed read-only |
| `frontend/src/features/event-feed/contract.ts` (FE27) | Last changed `aa9d01c9bb9518b5c79f180fa871ba6b1ba5549c`; import only existing headless `parse`, `freeze`, `ordered`, `uuid`. Closed schema validation and fractional-second latest-first ordering are regression-tested. No feed UI/runtime/cache/media reader imported. |
| FE33 source `1ae55459` | Read `mural/sync/{projection,contract}.ts` for immutable revision/reset/freeze boundaries. No code or live synchronization capability copied/imported; final wall is rendered directly from C04 public frozen pixels. This keeps the feature buildable at its assigned base without waiting for FE33 integration. |
| `frontend/src/shared/api/{domain,http}.ts` | Existing operation/query qualification and canonical session interfaces at assignment base. Only `domainOperation`, `matchesDomainRequest` and the `SessionPort` type are imported in production. No shared files edited. |

Existing retained React/React DOM, TypeScript, Vite/plugin-react, Vitest and Playwright dependencies are used. No dependency or lockfile resolution changed. A temporary user-authorized dependency symlink supplied tooling, then was removed before the final boundary/commit. TypeScript uses `--incremental false`; Vitest uses `--no-cache`; fixture builds go to `/tmp` without a shared dev-server optimizer cache.

## Public projection and lifetime

`projectArchive` first validates the entire closed `Domain_admin_Archive` schema. A post contains only `post_id`, permitted `text`, `media_ids` and `created_at`. Unknown author/profile/handle/location/beacon/recipient/private-activity/state/deletion fields reject the entire response before feature state is populated. The event must match both the outer archive and final mural. Mural state is `FROZEN`, exactly 128×128 pixels / 16 palette colors, with schema bounds and valid timestamps. The retained mural drops cooldown, next-allowed and server-clock metadata. No author or moderator metadata is constructed.

The canvas displays the complete frozen wall, supplies a DOM caption/image description and has no drawing, commit, restoration or subscription interaction. Posts are plain escaped text and optional explicit image reads. There are no compose/comment/reaction/edit/delete/consent/rally/pickup controls, live-thread links or protected-profile/handle requests. The screen is newly authored semantic HTML/CSS, supports safe-area spacing and keyboard focus, and uses no decorative left accents.

`createArchiveContext(eventId, optionalCanonicalSession)` exposes only `{eventId, generation}`. The route event is explicit and does not require authentication. Optional canonical-session observation listens to identity/event-generation aborts without retaining user ID or token; login/logout/account/event changes advance the archive lifetime and synchronously scrub loaded data/images. `setEventId` does the same for route event changes. The context is independent of private eligibility, live feed, QueryClient keys, session credentials and participant caches. There is no persistent/offline cache or fallback data. An anonymous public archive works without a session.

Each load cancels its predecessor and clears old visible posts/wall/images. Late responses after event/account change, disposal or cancellation cannot repopulate the view. Only one current public page, one final snapshot and at most one 10 MiB Blob URL are kept. Next-page navigation replaces the prior page rather than accumulating posts. Cursor loops, duplicate posts, incorrect order and changes to archive revision/frozen snapshot are rejected. At most 50 pages are traversed before requiring a fresh first-page read. The controller retains only public ordering coordinates/IDs for pagination, not prior-page post text. Missing/deleted/unavailable resources clear visible data rather than exposing a stale successful view.

`invalidatePost(eventId, postId)` is a narrow explicit public removal notification from composition. It clears the current view and fences pending reads and remembers that removed ID for the context lifetime, so a stale public response cannot bring it back. More than 1,000 such invalidations fails closed until a new context lifetime. It does not subscribe to authenticated feed state or infer public permission from it. The producer must independently exclude currently deleted/hidden content on every read.

The BE41 owner (`01a1a267-37a0-7723-a4e9-a3a6f5b27f52`) confirmed these intended semantics: allowlisted public `admin.Archive`, `(created_at, post_id)` descending, signed event/revision/upper-tuple cursors, current removal checks, revision-invalidated pagination, owner-applied frozen snapshots, owner-reviewed public text/media, and unavailable defaults without live/private fallback. This coordination is not real BE41 runtime evidence. C04 is unchanged. The browser cannot determine whether an arbitrary permitted text string semantically contains a private handle; producer redaction/review remains authoritative and unverified here.

## Narrow route composition

Import `ArchivePage`, `createArchiveController`, `createArchiveContext`, `createPublicArchiveReader` and, if necessary, `unavailableArchiveReader` from `features/archive/index.ts`.

```tsx
// Integrator-owned /archive route; event selection is explicit.
const context = createArchiveContext(routeEventId, canonicalSession)
const controller = createArchiveController({ context, reader: createPublicArchiveReader() })
// Render <ArchivePage controller={controller} />.
// Update context.setEventId(newRouteEventId) when the selected event changes.
// Dispose controller, then context when the route owner releases them.
```

Create these objects once per owning route lifetime; do not recreate them on every React render. `ArchivePage` owns its subscriptions/initial read, while the route owns controller/context disposal. It is usable without the authenticated shell and adds no game or connection. Route registration is an exact remaining Delivery composition change; no root file is patched by this worker.

The current shared HTTP client rejects public event/domain requests in `prepare()`, although C04 registers this operation as PUBLIC. That mismatch was reported to Delivery. The feature-local `createPublicArchiveReader` is deliberately limited to the registered `getConcertArchive` GET/path/PageQuery via existing operation qualification. It uses native same-origin fetch with `credentials: omit`, `cache: no-store`, `redirect: error`, no referrer, only an Accept header, a 15-second deadline and a 2 MiB response limit. It has no Auth parameter, mutation, automatic retry, refresh or alternate endpoint capability. Errors retain only safe failure categories, not response diagnostics. This is a public reader, not a weakening of protected shared transport.

Optional `ArchiveMediaPort.read({eventId, archiveRevision, postId, mediaId, signal})` must independently qualify the image against that current public archive projection and return matching context plus PNG/JPEG/WebP Blob bytes. The feature verifies association, type/size and lifetime before allocating an ephemeral URL, revoking it on replacement/load/context withdrawal/disposal. Missing or unavailable composition displays a safe unavailable state. It never calls protected `readConcertMedia`, publicizes a private URL, injects credentials or guesses a public-media endpoint. Qualified production media delivery remains NOT_RUN; the deterministic producer seam validates the complete feature behavior.

## Validation and durable evidence

Commands ran from the assigned worktree using Node `24.22.2` and retained dependencies:

| Command | Exit / result |
| --- | --- |
| `python3 scripts/mcr/check-plan.py` | 0 / PASS: source archive hashes/export regressions and required scope remain intact |
| `frontend/node_modules/.bin/vitest run --root frontend src/features/archive/__tests__/archive.test.ts --maxWorkers=1 --no-cache` | 0 / 49 tests pass |
| `frontend/node_modules/.bin/tsc --noEmit --incremental false --project frontend/tsconfig.json` | 0 / strict frontend typecheck passes |
| `frontend/node_modules/.bin/oxlint frontend/src/features/archive` | 0 / no warnings |
| `node frontend/src/features/archive/__tests__/browser.mjs` | 0 / native desktop Chrome `156.0.8078.80`, mobile-touch 360/390/430×844, reduced motion; all pass |
| `node scripts/mcr/check-ui-origin.mjs` | 0 / PASS, followed by manual source/visual inspection |
| `python3 scripts/mcr/check-boundaries.py --base cd7192febf9c2764169b55d63a70d7e00d13fd08 --lane frontend --allow-doc docs/mcr/frontend/archive.md` | 0 / PASS after temporary dependency symlink removal |
| `git diff --check`; explicit staged path/import review; source-archive diff | 0 / no whitespace, scope or source-preservation violations |

Tests cover strict privacy/deleted/live payload rejection, foreign/mutable/malformed final walls, minimal immutable projection, pagination/order/fractional seconds/revision/cursor fencing, empty/loading/unavailable/offline states, synchronous account/event/deletion withdrawal, late/disposed work, event-bound deletion during pending reads and stale deleted-post reintroduction, public-only HTTP methods/query/no-store/credential omission, safe HTTP errors, invalid/malformed/oversized responses, and on-demand public image association/type/size/revocation/late results. No mock certifies real authorization.

The standalone browser runner builds only its test HTML entry to `/tmp/mcr-fe40-browser/site` and serves preview on loopback. It drives the actual component/controller/public-reader implementation with deterministic fetches. Touch, keyboard pagination/focus, decoded Blob imagery and two exact final-canvas pixel samples pass at all three widths. All observed requests are the archive GET, omit credentials, have no authorization and use no-store. It asserts zero write forms/inputs/live links, 44×44 enabled controls, no overflow/exceptions, deleted/private projection rejection, account/event/media withdrawal and unavailable/empty/offline rendering. This is native desktop Chrome with mobile/touch contexts, not actual iOS Safari or Android hardware.

Durable owner-run evidence is committed under `frontend/src/features/archive/__tests__/evidence/browser/`: exact `report.json` and all 15 screenshots (ready, next page, unavailable, empty and offline at each width). The report pins assignment/tested base, engine/context, checks, feature/test/contract source hashes and each screenshot hash. Source hashing excludes evidence artifacts to prevent self-hash cycles. Source and image bytes are checked against the report before commit. Supplemental local logs are `/tmp/mcr-fe40-{plan,unit,types,lint,browser,origin,boundaries}.log`; rerunning the committed runner reproduces fresh local evidence. The fixture bundles resolved contract schemas and emits a >500 kB Vite chunk warning; this is not a measurement of production route bundle size or a full application build.

No migrations or server state changed. Production router/public-media composition, BE41 actual publication and deletion authority, closure cleanup/privacy review, real HTTP/CDN behavior, multiuser operation, actual iOS/Android devices and release IN05 are NOT_RUN. Delivery receives the exact committed handoff and may publish this bounded unit independently of whole-project completion. The task-local fixture implementation is complete; later integration findings return as concrete scoped fixes.
