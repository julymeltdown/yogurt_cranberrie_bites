# FE33-ENTRY — shell-owned Wall adapter

Status: **MOCK_READY**, bounded application adapter delivered for integration. Default-root registration, live backend/two-user/IN04 qualification and formal FE33 acceptance remain separate.

## Ownership, base and source pins

Exact base: `01a8e6bceb583a5aba080e401478c7fae4ec0d58`. Isolated worktree `/Users/lhs/Desktop/mcr-worktrees/fe-mural-entry`, branch `codex/mcr-fe-FE33-entry`. This document and NEW `frontend/src/app/mural-entry/**` are the complete writable scope. The delivered commit introducing this document has that exact parent. Only the integrator publishes master and records its resulting ancestry.

New application files are `ports.ts`, `owner.ts`, `provider.tsx`, and `index.ts`. New tests/native fixtures/build probe are under `__tests__/`; captures and validation manifests are under `__tests__/evidence/`. The exact complete changed-path list, source/contract hashes and artifact hashes are in `frontend/src/app/mural-entry/__tests__/evidence/index.json`.

Existing Auth/session/privacy/HTTP, mural editor/sync/HTTP/projection, world authority/sink/Phaser adapter, root/shell/router/ports, shared styles/primitives, contracts, manifests, lockfiles, backend and `docs/source/` are read-only and unchanged. No copied legacy source, new dependency, product artwork, backend endpoint, public WS frame, game/socket/scene constructor or token store is introduced in application code. The native development fixture separately creates one labeled synthetic incumbent Phaser world for qualification.

Inputs read: applicable working agreements, implementation specification (especially R01, §§5.1, 7.2–7.5, 8.4, FE32/33), assessment/execution plan, `mural-sync.md`, latest UI reference audit, existing account/meeting entry and actual route seam. Existing visual/editor behavior is reused without a repeat visual review gate. The frozen C04 mural schema currently retains a v0.1.0 title despite the delivered v0.1.2 own-result additions; exact bytes/operations are pinned instead of relying on its title. Last schema source revision: `bff7e53c0638d5462af819bf560468a1f857c88e`; C05 world schema revision: `69a5624cd153a216f2398cebabf062400dce23eb`. All actual input hashes/blobs are in the evidence index.

## Application lifetime and release boundary

`createMuralEntry` creates one existing `createMuralSync` and editor for one immutable full `PrivacyScope`. It uses the actual canonical Auth request path through `auth.privacy.run`, adds release/abort/current-scope fencing, and composes existing `createMuralHttp` plus `createMuralTextureSink`. Auth owns refresh and credentials. The mural entry never creates another session or disposes an externally owned world, connection or scene.

`MuralEntryRelease` is an explicit in-process capability: exact `PrivacyScope`, lifetime `AbortSignal`, qualified room/epoch/stream binding, authoritative read/subscribe port and an incumbent scene port with close notification. Root must qualify these inputs; bootstrap data and connection presence are not write authority. The owner freezes scope/binding and captures original source objects/methods. Missing, aborted, foreign, mismatched or withdrawn inputs expose the closed panel and dispatch no new request/write. A room/epoch/stream change that no longer matches the admitted scene binding requires a freshly qualified replacement release. Read-only authority still permits the confirmed raster, but blocks placement.

The shell provider survives ordinary panel navigation. Replacing or reconnecting a release calls `bind` on the existing owner; it never recreates the editor or silently drops an unresolved command. Release withdrawal aborts pending reads/commands, clears the confirmed raster, texture/object/frame resources and captured scene/source listeners, while the existing feature retains unknown command metadata for explicit status/result recovery. Temporary authority loss keeps only its observation/scene-close subscriptions for recovery; polling and texture resources are removed. Authority is read again at dispatch, so delayed change notification cannot authorize a new POST. No POST replay is added. A pending late response is fenced even when the external fetch ignores abort. Original-key receipt/result recovery remains the existing C04 read flow; snapshot pixel equality never proves a command result.

Privacy/event/identity/policy change permanently disposes this owner and erases its command and raster synchronously. Scene/signal withdrawal erases mounted DOM canvas dimensions before store/controller notifications, hides the protected view and clears owned scene resources. Recovered view generations remount the canvas presentation even if React batches withdrawal and rebinding, while retaining the same editor/command. Partial setup failure and throwing independent cleanup callbacks cannot prevent remaining resources from being released. Panel unmount removes only its presentation/withdrawal subscription; it does not dispose the shell owner.

Sync deadlines/cooldown elapsed time use the existing monotonic `Clock` (default `performance.now`). Authenticated receipt expiry uses a separate UTC epoch clock (default `Date.now`). An expired original receipt remains unresolved even when the monotonic clock is zero. These clock domains are not interchangeable.

## Exact missing default-root dependency

At this base, the default `/wall` feature registry has no mural consumer, and root does not expose a qualified incumbent scene/anchor/write capability. These are implementation dependencies, not approval waits. This patch deliberately does not register the default route or edit its owner.

The root owner must:

1. Observe its existing `WorldReplicationRelease` with `createWorldMuralAuthority`, supplying an explicit current authoritative write-policy callback. Do not derive permission from socket presence or bootstrap.
2. Supply `createPhaserMuralScene(incumbentScene, reviewedVirtualWallAnchor, exclusiveTextureKey)` from that same current world. The tested fixture's geometric anchor is synthetic and supplies no approved product anchor or venue guidance.
3. Bind the scene and authority to an exact current full privacy scope/binding and signal; withdraw before replacing/disposal of external scene. Supply a fresh release after a scene/room/epoch/stream change. The consumer takes ownership only of its own texture/object/listener leases.
4. Mount `MuralEntryProvider` inside the authorized shell above panel navigation, passing canonical Auth, current privacy scope and the qualified release. Change `release` without changing owner scope or remounting the provider on navigation/reconnect. Privacy changes require a fresh scope/owner.
5. Register `<MuralWallPanel />` at `/wall` via the actual route composition seam or reserved default registry. NEW tests/native fixtures use `createAppRoutes(..., { panels: { '/wall': <MuralWallPanel /> } })`; this is actual route composition evidence, not evidence of default registration.

No reserved feature API change was needed: the incumbent sync/editor already retain unknown commands through authority loss. Full page reload/browser shutdown durability is not added; the existing command lifetime is in-memory. FE39 moderation/closure and FE40 archive ownership remain separate.

## Verification and native evidence

Pinned Node 24.22.2 and retained dependencies were used through a temporary untracked `frontend/node_modules` symlink, removed before commit; no install or lock change. TypeScript incremental writes are disabled, Vitest uses `--configLoader runner --no-cache`, and Vite runner caches/build outputs go to private `/tmp` paths. The broad suite was not run.

| Check | Result |
| --- | --- |
| New owner/actual-route plus existing mural editor/sync/world suites | PASS, 11 files / 148 tests, exit 0 |
| Whole frontend strict TypeScript | PASS, `--noEmit --incremental false --pretty false`, exit 0 |
| Whole frontend Oxlint | PASS, exit 0 |
| Default production app build | PASS, 319 modules, exit 0; existing CommunityFeatures/Phaser size warning; default registry remains unchanged |
| Production build of new adapter entry | PASS, 103 modules, exit 0; explicit new entry probe avoids mistaking the unchanged root build for adapter bundling |
| Native actual-route/Phaser fixture | PASS, 360/390/430, 21 PNGs, no page errors |
| Planning provenance and scoped whitespace/path checks | PASS, exit 0; planning is not application acceptance |

The 15 new owner/route cases exercise missing/foreign/mismatched authority, read-only placement denial, canonical URL/method/bearer/intent/idempotency/body/no-store behavior, idempotent binding, null-authority recovery, unknown placement across navigation/reconnect/direct release replacement, status plus authenticated original-result recovery without POST replay, epoch replacement, late snapshot/command abort fencing, privacy/event/identity/policy cleanup, retained DOM pixel erasure, throwing source/frame/close cleanup, partial setup failure, immutable method capture and epoch receipt expiry independently of monotonic time. Existing feature suites retain gap/freeze/restore and projection coverage.

Native screenshots use the actual application route seam, canonical Auth/HTTP/privacy, actual one incumbent Phaser game, `createWorldMuralAuthority`, `createPhaserMuralScene`, sink, sync and editor. HTTP server, admission/write authority, session setup, binding, geometric raster and anchor are explicitly synthetic. There is no live socket/server or product art. Every native state verifies one created/active/peak game; unavailable/read-only issue no POST. Unknown placement uses keyboard preview and explicit Confirm, survives Wall/world/feed navigation and signal withdrawal/reconnection with exactly one POST and retained command; status recovery remains explicit. Policy withdrawal checks retained canvas dimensions zero, no mounted mural surface and no remaining mural texture.

Captures: loaded/read-only/unknown/unavailable at all three native widths (12), plus loaded/read-only/unknown scrolled action views (9). Action views prove Confirm and Check original command are reachable; keyboard focus on Check is visible. Fixture labeling stays visible in scrolled captures. Representative inspected images: `evidence/loaded-390.png`, `read-only-360.png`, `unknown-430.png`, `unavailable-390.png`, `unknown-controls-390.png`, `loaded-controls-360.png`. Hashes, commands, exit codes and corrected probe/setup failures are recorded in `evidence/report.json`, `validation.json` and `index.json`.

Real backend producer-consumer integration, multiple real users, admitted product world/anchor/art, default-root activation, real-device/screen-reader/performance/load, FE39/FE40 flows, IN04 and release acceptance are **NOT_RUN**. This evidence establishes the bounded adapter and synthetic native lifecycle, without advancing whole-project acceptance counts.
