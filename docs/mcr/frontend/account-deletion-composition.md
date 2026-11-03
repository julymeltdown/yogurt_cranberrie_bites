# FE36-AUTH-ROOT canonical account deletion composition

Status: MOCK_READY for synthetic production-route qualification; canonical transport is CONTRACT_READY. No live deletion, combined cleanup, release or IN acceptance is claimed.

## Provenance and ownership

Assigned clean base: `36b8bc9cc4729997eb2a0ac36f05f77a28b2a5e8`; branch `codex/mcr-fe-account-deletion-composition`; isolated checkout `/Users/lhs/Desktop/mcr-worktrees/fe-account-deletion-composition`. Auth checkpoint: `52c2b357254b2e97f33b08c44561ac58ae545f6a`. The following composition commit descends directly from that checkpoint; resolve its identity from this file's Git history and the committed handoff. Only the integrator publishes to master.

The existing account consumer/decoder is inherited through the assigned base, including FE36 eight-target delivery integrated as `5050bd28`. The contract revision is `d3b7c38268924d2db6bcdefce7bb84b86dafe329`:

| Input | SHA-256 |
| --- | --- |
| `contracts/generated/mcr-core.openapi.json` | `5af06192296d1765cde2bbceedf07a86156715f76ae1850158b42d2c815afcfd` |
| `contracts/generated/mcr-domain.openapi.json` | `a5fbe73124ed069e6357f5797a264e2d2a1bec93b0179bfc8550bf1122b69fde` |

New rendering is authored for this composition. No legacy rendering or character artwork is copied. `docs/source/`, backend, contracts, shared HTTP/privacy, existing `features/account/**`, manifests, lockfiles, WorldShell and navigation composition remain unchanged.

Exact implementation paths:

- `frontend/src/features/auth/model/session.ts`: construct, expose and dispose the narrow port from existing canonical internals only.
- `frontend/src/features/auth/model/account-deletion.ts`: protected deletion transport and bounded recovery capability.
- `frontend/src/features/auth/__tests__/account-deletion/{fixtures.ts,transport.test.ts}`: canonical synthetic login/HTTP tests.
- `frontend/src/app/account-deletion-entry/{owner.ts,context.tsx,panel.tsx,link.tsx,recovery.tsx}`: retained consumer, deferred scope-bound navigation, private controls, Settings opener and public recovery.
- `frontend/src/app/account-deletion-entry/__tests__/{routes.test.tsx,preview.html,preview.tsx,native.mjs}`: actual default root route tests and local synthetic HTTP browser harness.
- `frontend/src/app/router.tsx`: outer ownership and default `/me/account` registration only; `PublicBoundary.tsx`: `/account/deleted` hookup only; `frontend/src/features/settings/SettingsPage.tsx`: account-controls link only.
- This assigned documentation file.

Dependencies are retained React/React Router/TanStack Query/Zod/Vite/Vitest/Playwright packages. Node `24.22.2` and existing `/Users/lhs/Desktop/mcr/frontend/node_modules` were used through a task-owned temporary symlink, removed before final handoff. No installs, lockfile reconciliation, Rust build or database was performed.

## Behavior and boundaries

The canonical Auth owner constructs a typed AccountDeletionPort using its existing HttpClient, memory session, QueryClient and privacy coordinator. Generic `SessionController.request` still rejects `/api/auth/`. There is no second client, session, query cache, credential store or domain receipt. DELETE `/api/auth/account` is once per captured identity/generation, without a body or idempotency key. GET `/api/auth/account/deletion` is manual. Neither operation refreshes, polls, retries or replays. Same-origin credentials, protected bearer, no-store and browser mutation intent remain canonical.

Recovery grants only the exact next deliberately suspended privacy scope in the consumer's immediate post-settlement turn. The original credential deadline and token version are immutable for this capability. A later policy/identity/event change, expiry, cancellation, credential replacement or disposal rejects stale work. Current 401 clears the canonical identity; old results cannot clear a newer actor. Raw saga data passes transiently to the existing eight-target decoder and is not cached by this adapter. Only the consumer's safe target/status projection reaches recovery UI.

The outer root owner retains the existing AccountController across WorldShell disposal. It creates no private world resources. Settings opens default `/me/account`; Back uses a known current Settings opener and restores its focus through existing navigation memory, with `/me/settings` as a safe fallback. Unmount clears typed confirmation. Restriction removes the world/private caches and opens `/account/deleted`. Public direct entry creates no controller or automatic status read and confirms no deletion. Pending, partial failure and uncertain outcomes retain only explicit manual recovery. Completion requires all eight valid steps and canonical logout; expiry or session replacement removes retained private progress.

A real composition race was found during qualification: synchronous privacy restriction withdraws the social owner while its navigation blocker is still installed. Navigating in that same callback is refused by the withdrawn guard. The new outer ownership queues navigation until the teardown commit's passive effects, then rechecks the captured canonical identity, privacy generation and gate before navigating. Existing guards and shared privacy code are unchanged.

## Validation

All final commands below exited 0. Tool commands used the pinned Node directory on PATH. Vitest, TypeScript, oxlint and Vite entrypoints came from retained dependencies. Tests and build ran in `frontend/`, except TypeScript's project command and repository scripts ran at repository root.

| Command | Result |
| --- | --- |
| `vitest run src/features/auth` | 274 tests, 10 files, including 15 new transport cases |
| `vitest run src/app/account-deletion-entry` | 6 default-root route tests |
| `vitest run --maxWorkers=1 src/features/auth src/features/account src/shared/query src/app/account-deletion-entry src/app/__tests__/routes.test.ts src/app/__tests__/shell.test.tsx src/app/__tests__/recovery.test.tsx src/app/__tests__/enrollment.test.tsx src/app/__tests__/public-entry.test.tsx src/app/account-entry src/features/settings` | 538 tests, 22 files |
| `tsc --project frontend/tsconfig.json --incremental --tsBuildInfoFile /tmp/mcr-fe36-auth-root.tsbuildinfo --pretty false` | PASS |
| `oxlint src vite.config.ts` | PASS, no warnings |
| `vite build` | PASS, 418 modules; existing large-chunk advisory remains |
| `node src/app/account-deletion-entry/__tests__/native.mjs` | Chrome PASS at 360/390/430, 120 synthetic API requests, zero page errors |
| `python3 scripts/mcr/check-plan.py` | PASS, original checksums and planning baseline preserved |
| `node scripts/mcr/check-ui-origin.mjs` | PASS, zero violations |

The first broad parallel Vitest run had three one-second lazy-import waits time out while the event fallback was visible (535/538 passed). Repeating the same selected scope with one worker passed all 538. No existing test or timeout configuration was changed.

Native evidence: `/tmp/mcr-fe36-auth-deletion-root-evidence/result.json` records exact source hashes, request methods/paths and booleans for bearer presence without credential values. `public-partial-360.png`, `public-partial-390.png`, and `public-partial-430.png` capture the public recovery state. The 360px image was visually inspected: eight readable statuses, usable controls, no left accent decoration and no horizontal overflow. Artifacts are local ephemeral evidence, not committed source.

Native coverage uses the actual `createAppRoutes` defaults and canonical SessionController, with no panel overrides. Every `/api/` request is intercepted by a loopback synthetic Vite service. It verifies Settings keyboard entry/Back focus/unchanged host, one DELETE with empty body/no key/browser intent, no automatic GET/refresh/replay/poll, eight-step pending and partial recovery, private-cache scrub, world disposal with retained manual GET, completion/logout, uncertain/legacy/malformed responses, truthful anonymous direct entry, 401, late actor replacement and the canonical expiry timer. World lifetime is measured with the existing instrumented world port (peak one game/connection); this does not certify real Phaser/WebSocket or multiuser service behavior.

Final `check-boundaries.py --base 36b8bc9cc4729997eb2a0ac36f05f77a28b2a5e8 --lane frontend --allow-doc docs/mcr/frontend/account-deletion-composition.md --allow-composition frontend/src/app/router.tsx` and `git diff --check` exited 0. Manual review verified all 17 changed paths against the exact assignment, no backend imports and unchanged native-evidence source hashes. The task-owned dependency link was removed. Real account deletion, live Auth cookies/providers, BE cleanup services, PostgreSQL, device flows, load, real multiuser tests and IN05 acceptance remain NOT_RUN. This delivery does not change the 112 required task or 61 required IA acceptance scope.
