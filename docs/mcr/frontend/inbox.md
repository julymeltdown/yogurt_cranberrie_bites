# FE35 notification inbox — bounded frontend handoff

Date: 2026-11-04. Status: **MOCK_READY**, not full FE35 acceptance. U10 `/notifications`, BE33 runtime, IN03 social/privacy qualification and release acceptance remain **NOT_RUN** in this handoff. In particular, the specification's canonical unread count is unfinished; the current public contract supplies no total. No scope cut or schema change is implied.

## Ownership, ancestry and provenance

- Delivery repository/branch: `julymeltdown/mcr`, `master`; only the designated delivery integrator publishes it.
- Worker: `codex/mcr-fe-FE35`, isolated `/Users/lhs/Desktop/mcr-worktrees/fe-inbox`, base `e6fef53d9f0841b3e16ce3baa4db01a834043957`.
- Exact writable scope: `frontend/src/features/inbox/**` and this document. No shared contracts, manifests, lockfile, root routing, backend or source archives changed.
- Product authority: `MCR_IMPLEMENTATION_PLAN_V2_EN.md` FE35/BE33, U10 and IN03. Assessment/execution plans and `docs/mcr/parallel-delivery.md` informed allocation, not product amendments.
- All inbox rendering, controller, adapters and tests are newly authored. No Bigyeting rendering, notification implementation or art is copied. Existing MCR headless `event-feed/contract.ts` supplies the strict generated-schema validator and deep freeze by import, at source revision `aa9d01c9bb9518b5c79f180fa871ba6b1ba5549c`.
- Canonical Auth/session, HTTP and privacy code is reused by import, not duplicated. The base includes `7bfea962136df4c35be43571c657d140d2982079` for the latest Auth/privacy-path change. Runtime dependencies remain React 19, canonical MCR Auth/transport/privacy and the existing generated public schema/operation registry. No package added; retained resolutions unchanged.
- Pinned contract inputs: `contracts/domain/v1/feed.schema.json` last changed at `d0649ec2a1ea8dd520627fc214792c877d1d4c22`; generated `contracts/generated/mcr-domain.openapi.json` and `frontend/src/shared/api/domain-operations.generated.ts` at `ca9f955cd22f7e4c1da029d034c1923f1e98bb9d`. Target schemas and registered operations come from that same generated contract.
- Synthetic tests import existing MCR Auth, party, giveaway and feed fixtures read-only from this base. The browser fixture supplies its own synthetic in-memory identity, HTTP DTOs and availability port; none is reachable through the production module entry.
- Delivery commit: the first commit containing this document and the paths below is the immutable worker handoff; any follow-up identifies its parent in Git. Do not infer a master merge/push from a worker commit.

## Changed paths and behavior

| Path within `frontend/src/features/inbox/` | Purpose |
| --- | --- |
| `index.ts` | Public controller/UI/navigation exports |
| `model/contract.ts` | Strict closed list, target, read result and captured command-receipt decoding; bounded deduplication |
| `model/controller.ts` | Canonical private reads, explicit commands/recovery, lifecycle clearing and stale response fences |
| `model/targets.ts` | Fresh authorized resource reads and reviewed shell destinations |
| `ui/Inbox.tsx`, `ui/inbox.css` | Newly authored inbox list, read controls, honest loaded count and neutral unavailable feedback |
| `__tests__/controller.test.ts`, `__tests__/targets.test.ts`, `__tests__/fixtures/data.ts` | Deterministic private/read/command/navigation tests |
| `__tests__/preview.html`, `__tests__/preview.tsx`, `__tests__/browser-smoke.mjs` | Isolated production-built canonical-session browser fixture |
| `__tests__/evidence/*` | Browser report with source hashes and screenshots at four widths |

Reads use only `listConcertNotifications` for the current canonical user/event. Unknown or private DTO fields, foreign event/target IDs, malformed timestamps, invalid unread-filter results and cursor loops fail closed. The controller keeps at most 200 rows and ten continuation requests, deduplicates overlapping IDs by revision, and rejects conflicting same-revision projections. It stores no raw errors, handles, location payloads, target projections or tokens. There is no persistent inbox cache or fallback account.

`unreadShown` counts only loaded unread rows. The UI says “unread shown,” including when a next cursor exists. This is not an authoritative unread total, badge or certification of all unread notifications. Read-all remains server-scoped, and its server cutoff is authoritative; the client never blanket-marks cached or future notifications read.

Individual mark-read sends `expected_revision`; mark-read and all-read send one canonical UUID idempotency key per explicit intent. They are SAFETY writes and have no invented `mutations` capability gate. Pending, network loss, malformed success, expired/retired history and failed/unknown receipts never authorize automatic mutation replay or a new key for the identical intent. Original operation/key recovery is an explicit user action. Receipt success is bound to actor/event/operation/key, expected resource/revision and result status 200, then followed by a fresh authorized list read. There is no notification-by-ID read contract; a missing paged row is not fabricated or treated as proof of absence. An older manual/filter read is canceled and fenced before a command's authoritative reread.

Identity/event changes, logout, block/consent policy generation changes, suspension, producer unavailability and disposal synchronously clear rows, targets and command bindings and cancel pending work. Delayed non-cooperative responses cannot repopulate old state. Block changes may reauthorize a new scoped list; the prior list is never retained as placeholder. Scoped invalidation drops existing rows/targets and requires an explicit refresh; it does not ingest a pushed notification or replay a write.

Opening a notification does not mark it read. The controller first fetches and validates the target through canonical Auth, then emits only a reviewed path plus the captured privacy scope. Party, application-to-party, published giveaway, current user's pickup reservation, participant social request, feed post and published notice destinations are supported. REPORT has no participant detail route and stays unavailable, with no moderator-evidence fetch. Deleted, denied, foreign, malformed or unavailable targets never navigate or render cached details. The destination page must authorize its own current read again; a successful preflight is not a durable permission grant.

## Root composition and remaining required work

The feature exports `createInboxController`, `Inbox`, `InboxController`, `InboxOptions`, `InboxState` and the scoped navigation types. The integration owner must mount it at U10 using the **existing shell Auth controller**, call `start()` once for that instance and `dispose()` when removing it. Do not create a second Auth/session/world instance. The root remains the owner of `/notifications`, badges, navigation and capabilities.

Composition must supply `access: { read, subscribe }` from actual producer readiness and current shell policy. No notification capability exists in the pinned seven-capability projection. Keep the feature unavailable until the canonical producer is available; do not infer readiness from another capability or substitute local production fixtures. `navigate({ path, scope })` must recheck `auth.privacy.isCurrent(scope)` before using the existing router. Authorized invalidation may call `invalidate(capturedScope)`; backend payloads never become UI notification authority. Shared root/contracts remain integrator-owned.

Required gaps, without reducing the specification:

1. **Canonical unread total:** delivery/contract owner must define the count response/update rules and affected consumers; BE33 must produce it, and the inbox/badge owner must implement and validate it. The current `NotificationList` has only `items` and `next_cursor`. Loaded counts cannot close this requirement. No unilateral schema extension was made.
2. **BE33 producer and read semantics:** real durable deduplication, server actor/event authorization, safe payload production, concurrent read-all cutoff, receipt retention and status behavior require executed backend evidence. Frontend fixtures do not prove them.
3. **Root and vertical flow:** wire U10, actual availability and safe navigation, current invalidation delivery and any count badge. Verify real duplicate deliveries, stale targets, cross-user/event changes and block/revoke propagation through HTTP, WS and cache under IN03.
4. **Device/release:** actual mobile devices, assistive technology and full release qualification remain NOT_RUN. Chrome viewport checks are not device acceptance.

## Validation evidence

Node `24.22.2`, pnpm `11.7.0`, TypeScript `6.0.2`, Vitest `4.1.9`, Vite `8.1.3`, native headless Chrome `156.0.8078.80`. A private offline frozen install populated this worktree's own ignored `node_modules`; no dependency or lockfile changes. Commands below run from the worker root unless a directory is stated. Put the pinned Node `bin` on `PATH`.

| Command | Result / scope |
| --- | --- |
| `python3 scripts/mcr/check-plan.py` | Exit 0; all 13 archive checksums, pinned narrative, exporters and routing pass. Planning only. |
| `node frontend/node_modules/vitest/vitest.mjs run frontend/src/features/inbox/__tests__ frontend/src/features/auth/__tests__/domain-request.test.ts frontend/src/features/auth/__tests__/session.test.ts frontend/src/shared/query/__tests__/privacy.test.ts frontend/src/shared/api/__tests__/http.test.ts frontend/src/features/event-feed/__tests__/controller.test.ts` | Exit 0; 278/278 tests in seven files, including 58 inbox tests. Canonical transport/privacy and strict decoder dependency regression. |
| `node frontend/node_modules/typescript/bin/tsc -p frontend --noEmit --incremental false` | Exit 0. |
| `frontend/node_modules/.bin/oxlint frontend/src/features/inbox` | Exit 0. |
| `node /Users/lhs/Desktop/mcr-tools/pnpm-11.7.0/package/bin/pnpm.cjs build` in `frontend/` | Exit 0; application production build. Existing chunk-size warnings remain. Root build does not imply inbox route integration; the separate fixture compiles the actual inbox UI. |
| `node frontend/src/features/inbox/__tests__/browser-smoke.mjs` | Exit 0; production fixture build/preview, four viewports 360/390/430/1280. Source hashes and check list in `frontend/src/features/inbox/__tests__/evidence/browser-report.json`. |
| `git diff --check`; assigned-path and cross-directory-import inspection | Exit 0; no backend imports, source edits or changes outside assignment. |

Focused coverage includes overlapping/duplicate pages and bounded duplicate-only chains; closed/private DTO rejection; availability and privacy clearing; delayed filter/target/command responses; no optimistic state or replay; explicit status binding/expiry/retirement; read-all new arrival; and participant target authorization. Browser coverage exercises empty/offline states, pending receipt recovery without a second PATCH, fresh denied target handling, read-all cutoff, invalidation, availability/logout clearing, visible keyboard focus, buttons at least 44 px high, no horizontal overflow, no decorative left borders/pseudo-bars and no browser errors. Screenshots were visually inspected at 390 px. These are fixture-level evidence only.
