# FE18 rooms and accessible nearby

Status: **MOCK_READY for scoped integration review**. W04/W05 have implemented frontend views and verified deterministic transport flows. Whole FE18/W04/W05 acceptance remains open: party invitation authority, real BE11/BE13/BE37 behavior, actual renderer removal, two-account backend flows and IN01 are **NOT_RUN**. This delivery does not amend the specification.

Assignment base: `366cf8bb808d8a0d55124e0a73429a2f742d0fe1`; branch `codex/mcr-fe-FE18`; isolated checkout `fe-world-access`. Writable paths are exactly `frontend/src/features/world-rooms/**`, `frontend/src/features/world-nearby/**` and this document. Root routing/composition, backend, contracts, generated artifacts, dependencies, lockfiles and `docs/source/` are unchanged. Only the integrator publishes. The committed handoff and delivered ancestry accompany this document in the parent review receipt.

## Behavior and authority

The room reader uses the canonical Auth controller's `request` and FE02 `privacy.run`, with captured actor/event/generation/policy and cancellation. It makes protected core `GET /api/concert/events/{eventId}/rooms` requests, `cache: no-store`, without Query persistence or a feature-owned token/refresh/fetch client. The existing qualified canonical refresh owner alone performs C05's one refresh/read retry; the feature supplies no refresh override. The application must qualify that owner through its normal cookie-policy configuration.

`validateWorldRooms` is a feature request-binding decoder around the existing C05 `validate('WorldRooms', value)` oracle and generated `WorldRooms` type. It does not introduce another schema authority. It checks exact actor/event/generation/path/HTTP 200 context, response event, unique room IDs and included default room. The frozen closed schema supplies room-count/label/capacity/UUID/time constraints. Responses are cloned and frozen before publication. Loading clears the old catalog; safe 403/404/429/503/invalid-response states expose no raw server details. Rate retry is bounded by the canonical error delay; the user can read again after it expires. C05 requires 1–16 rooms, so an empty array is invalid; the valid no-choice state is a catalog with no alternative available room.

Catalog rows expose capacity and availability only. They cannot dispatch a full, unavailable, current, stale, duplicate or disconnected switch. A true result from `switchRoom` means dispatch only. Old membership remains authoritative during dispatch; admission requires a live replacement baseline with ROOM_SWITCH, the intended room, a new stream, and the same map/avatar resource pins. **Exact pending request-ID matching remains FE16's authority**: FE17's public boolean API does not expose the generated request ID. This feature accepts only the existing validated shell projection, never raw server frames. A replacement arriving before the dispatch promise resolves wins even if FE17 then returns false after its generation changes. Rejection preserves the old room/roster. Resync, invalid authority, suspension and epoch interruption cancel pending presentation intent; nothing is replayed.

Nearby derives only the existing authorized replication entities. Its immutable rows contain user ID, display name, self identity and moving/standing state. They contain no coordinates, distance, GPS, avatar images, historical roster or physical-arrival inference. Self is separate; others link through ShellLink to existing `/people/:userId` destinations. Atomic replacement/delta removal updates this list synchronously; resync, hidden world, null release, access withdrawal and FE02 identity/event/policy cleanup clear it. Text-only UI uses no legacy, synthetic or unqualified character artwork.

UI uses compact typography, horizontal separators, symmetric controls, 44px targets, ordinary keyboard focus, safe error/retry copy, current/last-confirmed labels, switch status and empty/unavailable states. There are no decorative left rails/borders/pseudo-bars. ShellLink IDs preserve FE07 keyboard navigation and Back opener focus. The existing shell pauses the world when its panel fully covers it; these views show the paused state rather than keeping a stale nearby list.

## Integrator composition

Public entrypoints are `features/world-rooms/index.ts` and `features/world-nearby/index.ts`. Create one `createWorldAccessController({ auth, world })` per authorized shell feature lifetime, inject it into `WorldRooms` and `WorldNearby`, compose the two panel routes centrally, and dispose the feature controller on shell teardown. Neither view creates, starts, reconnects or disposes a transport, socket, replication controller, Phaser instance or renderer.

The structural `WorldAccessPort` reads a nullable `WorldAccessRelease` and subscribes to release changes. Each release contains:

- `scope`: the **captured** FE02 PrivacyScope that authorized this bootstrap and replication owner.
- `capabilities`: the complete validated ShellCapabilities from that same authorized bootstrap.
- `replication`: `read(): ReplicationView | null`, `subscribe(listener): () => void`, `switchRoom(roomId): Promise<boolean>`.

This is structurally compatible with the composition chat's WorldReplicationRelease, whose additional `reconnect` method stays shell-owned. No dependency on another worktree or uncommitted composition implementation is introduced. A null replication read withdraws private catalog, pending switch, current membership and nearby synchronously while preserving its subscription for a fresh authorized baseline. Requests cannot read/commit rooms during null withdrawal. A changed outer release replaces subscriptions. FE02 cleanup also clears the feature and unsubscribes the old replication release; reauthorization requires a fresh outer release notification. Never restamp an old roster with a new policy generation. The real shell owner must withdraw/dispose its private renderer and replication on the same cleanup, then authorize any replacement through canonical bootstrap. The feature cannot grant that authority itself.

The feature starts lazily; only the rooms view requests room metadata. It uses no extra timers, global listeners, RAF, cache, or network owner. The parent must keep the existing FE07/FE14/FE15/FE16/FE17 single-owner composition and supply only its validated projection.

## Missing invitation contract

W04 explicitly includes party invitation. Frozen C05 WorldRooms exposes neither occupancy count nor an invitation command/grant. Existing `/meet` navigation is available only when the authorized bootstrap's meet capability permits it. The rooms view binds that existing navigation and clearly separates party plans from virtual membership; it does not invent an invite link, token, grant, destination or mutation. Invitation creation/acceptance/expiry/revocation/privacy/idempotency and its room/party binding need reviewed authority and frontend/backend assignments before W04/whole FE18 can be accepted. Capacity is not presented as measured occupancy. No scope cut has been recorded or inferred.

## Provenance

Product code and rendering are newly authored. Read-only existing imports at the assignment base are canonical Auth/FE02, FE07 capabilities/ShellLink, FE16 C05 validator/types, FE17 immutable/projection types and the frozen C05 operations. No Bigyeting rendering, application source or visual libraries were copied. No dependency was added.

The test-only native WebSocket framing is adapted from `frontend/src/world/network/__tests__/replication-browser.mjs` at the assignment base. It is a small local loopback harness using retained Node/Vite/Playwright dependencies, not a production server. The native preview owns actual WebSocketWorldTransport and createWorldReplication inside an actual FE07 WorldLifecyclePort fixture. The feature still receives only their read/subscribe/switch interface. Its text sink and synthetic metadata are test evidence, not production character art or a physical-location claim. Unit fixtures use the actual MockWorldTransport/TransportEngine and FE17 controller, rather than replacing switch semantics with an always-success mock.

C05 source pins (SHA-256; unchanged):

| Source | SHA-256 |
|---|---|
| `contracts/realtime/v1/operations.json` | `400d28e861ca5a87c54fde447f77b1680f7ea58e3047ee43b46db25816af6da0` |
| `contracts/realtime/v1/world.schema.json` | `1732e627037fe91533ac6f7fe74e36683ce9b846bfbfc020b3cce298ae84df36` |
| `contracts/realtime/v1/policy.json` | `3e77e21b036386803c7d5bff0145c434f111ba51b861219a211e853a43d0628b` |
| `contracts/fixtures/realtime/rooms.json` | `d04837604d57a40106e823596bc7f7c9d6eb07d8d8829ef0675849348d332529` |
| `contracts/fixtures/realtime/movement-heartbeat.json` | `fba34f3b7cba90286ae3b1a1ecd6578a79da496e79d88b920b89bd7d783499dd` |
| `contracts/fixtures/realtime/room-switch.json` | `5cf116eb8c37d11ed83c8b43d00b2236a46245511c74e30af2a0707d6f72b3fd` |

## Validation and limits

Toolchain: Node 24.22.2, retained pnpm 11.7.0, installed Chrome 156.0.8078.80. Commands run from assigned `frontend/` unless prefixed with repository-root. All final rows below exited 0.

| Command | Result |
|---|---|
| `pnpm typecheck` | Strict TypeScript passes, including owned fixture TSX. |
| `pnpm exec vitest run src/features/world-rooms/__tests__ src/world/network/__tests__/replication.test.ts --reporter=dot` | **48 tests pass**: 25 owned controller/DOM cases plus 23 existing FE17 regressions. |
| `pnpm build` | Strict TS + production build passes. Existing large-chunk warning remains; central route composition is outside this patch, so production bundling alone is not proof these views are mounted. |
| `pnpm lint` | No diagnostics. |
| `MCR_FE18_EVIDENCE=/tmp/mcr-fe18-browser-20261103-06 node src/features/world-rooms/__tests__/browser-smoke.mjs` | Native Chrome PASS at 390×844 and 1280×900, no page errors, cleanup PASS. |
| Repository-root `python3 scripts/mcr/check-plan.py` | Planning/source provenance PASS; 13 supplied archive checksums preserved. |
| Repository-root `node scripts/mcr/check-ui-origin.mjs` | C08 static origin/import PASS; no reviewed extraction added. |
| Repository-root `python3 scripts/mcr/check-boundaries.py --base 366cf8bb808d8a0d55124e0a73429a2f742d0fe1 --lane frontend --allow-doc docs/mcr/frontend/world-access.md` | Lane/path/import guard PASS; exact assignment and staged paths reviewed separately. |

Native final evidence is `/tmp/mcr-fe18-browser-20261103-06/report.json`; it pins frontend source bytes and contract hashes before launch. Screenshots include each width's rooms, scrolled room controls, safe 503, nearby, scrolled people, self-only, rejected switch and admitted switch. Phone controls use the normal independently scrolling shell panel. Inspected phone room controls/people and wide nearby/rejection are legible, without overflow or character art. Assertions cover canonical no-store room request, full/unavailable, 503 recovery, keyboard heading/Back focus, initial authorized self/others, delta and epoch removal in both DOM and sink, duplicate suppression, sent-versus-admitted, rejected membership retention, matching admission, gap/resync eviction/replacement, one owner through navigation, fresh foreground connection with peak one socket, 44px controls, synchronous policy eviction, zero tracked timers and complete socket/owner disposal.

Earlier native attempts 01–03 exposed harness assumptions: expanded panels legitimately suspend world visibility; replacement snapshots require zero input acknowledgment; and a block test must prevent its deterministic bootstrap from immediately reauthorizing the unchanged fixture roster. Those test-only assumptions were corrected. Runs 04/05 passed; 06 adds safe-error recovery and scrolled inspection artifacts. No product authority was weakened to make a fixture pass. Initial boundary command omitted required arguments (exit 2), then passed with the explicit base/lane/doc arguments above.

Remaining limitations: local deterministic HTTP/ticket fixtures and local WebSocket server; no BE11 capacity race, real block authorization/filter stream, second browser account, actual Phaser/approved-art sink, invitation transaction, screen-reader device, load, deployment or release acceptance. Sink+DOM removal proves the actual headless projection bridge, not rendered-avatar removal. Parent owns actual composition and integration checks. The persistent assigned goal remains ACTIVE through committed handoff and parent review fixes.
