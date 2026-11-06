# FE33 mural synchronization and world-wall publication

Date: 2026-11-04, Asia/Seoul. The assigned FE33 implementation and qualified
contract-fixture criteria are delivered for integration. The controller,
HTTP adapters, C04 v0.1.2 original-result recovery and scene sink have
**MOCK_READY** evidence. These fixtures qualify the bounded frontend task;
real service/multiuser/moderation/restore, device and IN04 checks remain **NOT_RUN**.

## Ownership and provenance

- Chat: `01a1a213-61d6-7a20-a8e7-476649312f8c`, `mcr(fe) FE33 mural synchronization`.
- Worktree: `/Users/lhs/Desktop/mcr-worktrees/fe-mural-sync`.
- Branch: `codex/mcr-fe-FE33`.
- Delivery-selected base: `98c2c793ba5c380b0cca334f4f8647123c9d30a9`, containing
  the integrated FE32 editor. The initial empty branch at worker FE32 commit
  `64b44cc` was safely rebased before any FE33 source edits; its stopped checkout
  is untouched. The initial goal's base pointer is superseded by this base.
- Exact reserved paths: `frontend/src/features/mural/sync/**`,
  `frontend/src/world/mural/**`, and this document. These explicitly adapt
  FE33's model/interaction proposal to disjoint delivery-owned paths.
- FE33 acceptance source: `MCR_IMPLEMENTATION_PLAN_V2_EN.md` §8.4 and FE33;
  frozen C05 v1.0 and the pinned C04 review candidate. No source archive,
  generated planning, editor, root composition or shared transport is authored
  by this patch.
- Delivery explicitly authorized shared dependency imports: original `a25b855`
  becomes `4c59edd2d92428a5b46cb7ccc6138feefc4332b4`, and correction `bff7e53`
  becomes `a4f1b86592666ad3be53e4958f784236b19c3730`. The latter is the FE33
  implementation patch's parent. These are dependency commits, not FE33-owned
  shared edits; delivery should integrate only the FE33 implementation commit
  when the originals are already present.
- C04 v0.1.2 mural schema at that parent has blob
  `1134e67f1dcb5786ed83001f34a976a92fa3f4f0`. C04 operations/policy provide
  authenticated snapshot, contiguous delta, placement, command status and
  the closed `mural.OwnPixelResult` read.
- C05 schema last changed at `69a5624cd153a216f2398cebabf062400dce23eb`, blob
  `8d92e39cee9576ca91696a068f236273561a09ce`. Frozen C05 has no mural frame;
  this implementation introduces no public WS message or second connection.
- Existing world transport interface blob:
  `2c898c20ad658f1b4efe6a4ad6eaaeedaaf0e00d`. The world release is observed,
  never connected, disconnected, disposed or otherwise taken over by FE33.
- FE32 is imported through its narrow controller/schema exports. All new logic,
  rendering adapter, panel and tests are newly authored. Test data imports FE32's
  deterministic geometric fixture by module reference; no old UI/headless code,
  character image or proprietary replacement art was copied. Dependency versions
  and retained lockfiles are unchanged.

## Implemented behavior

`createMuralProjection` validates the closed C04 public delta DTO, event binding,
strictly ordered contiguous revisions within a batch and unique operation IDs.
Private author fields and `next_allowed_at` on public operations are rejected.
It holds at most 512 pending operations and 512 recent deduplication records.
Reordered batches can fill a short gap; matching duplicates/overlap are ignored.
Conflicting revision/operation identities invalidate the baseline. Oversize
batches are rejected before per-item decoding. Pixels are updated only from
qualified server snapshots, contiguous public operations or a matching own ACK.

A reset discards pending operations and establishes a nondecreasing revision
floor. Restore/control revisions never rewind. Retention loss, future cursor,
noncontiguous response, reset barrier or queue overflow requests a new snapshot.
A snapshot older than an already advanced baseline cannot erase concurrent
updates. An equal revision cannot silently change pixels, palette or state.
FROZEN never returns to editable within the same controller lifetime.

`createMuralSync` binds one canonical actor/event/privacy generation to a current
world room/epoch/stream. Each asynchronous read/delivery carries a captured
generation and binding. Room/stream/epoch changes, resets and superseding reads
invalidate older results; privacy changes permanently dispose the consumer.
Pending/unknown placement metadata is retained across ordinary resync/room
changes, while the confirmed raster, preview, queued deltas and texture resources
are withdrawn. Logout/policy/identity/event changes also erase that command.

The controller uses existing C04 HTTP polling (default 2 seconds), one in-flight
read, a 200 ms bounded reorder window and an 8-second read deadline. It has no
retry loop after a failed/hanging read; explicit refresh or fresh world authority
is required. A frozen wall stops polling. Queue overflow coalesces into full
snapshot recovery. The world sink coalesces any number of pending publications
to one latest frame, with one owned texture and one owned object per consumer.

FE32 remains responsible for explicit selection/confirmation and its original
command lifetime. The bridge validates matching own ACK coordinates/color/event
and `expected_revision + 1` before publishing to the raster/world sink. Own
cooldown fields never enter the public texture frame. Server-clock anchors and
elapsed monotonic time prevent each public delta from restarting the cooldown
display. Unknown results are never reconstructed from snapshot color equality.
Rejected previews roll back through FE32. Pause/freeze/reset clears preview;
external read-only authority remains latched through refresh. A newer confirmed
snapshot is not overwritten by an old acknowledged command.

`createMuralHttp` uses the canonical shared HttpClient, exact C04 operation IDs,
actor/event/generation and route/status decoders, original key/body, and explicit
no replay. Successful command status metadata must match the actor, event,
operation, key, expected next revision and terminal result shape. It does not
constitute a pixel result. Only a retained, own SUCCEEDED `placeConcertPixel`
receipt with `resource_id` equal to the event/mural aggregate and the original
next revision permits `getConcertMuralOwnResult`:
`GET /api/concert/events/{eventId}/mural/commands/{key}/result`. It sends no body
or query and uses the canonical authenticated no-store transport. The closed
envelope must bind receipt ID and original key; its nested result must bind
event, original coordinates/color, next revision and original `next_allowed_at`.
The pixel operation UUID is separate from the aggregate UUID. Strict decoders
reject added author fields, forged context and malformed receipt/result data.
Pending, expired, retired, unavailable or unverified original results remain
blocked; this flow never issues another POST. There is no provisional callback
or invented C05 extension.

`createWorldMuralAuthority` observes the incumbent
`WorldReplicationRelease.read/subscribe` surface. It requires live/visible,
current canonical identity and a matching C05 baseline. Writes default to false;
delivery supplies an explicit authoritative event/capability policy callback.
No source hook is implicitly installed in the existing world runtime.

`createPhaserMuralScene` accepts an existing Phaser.Scene, a reviewed virtual wall
anchor and an exclusive texture key. `createMuralTextureSink` uses the existing
scene frame loop, a bounded 128×128 RGBA upload and nearest-neighbor texture. It
creates no game, RAF loop, socket or physical location authority. Null
publication/epoch change/disposal releases the object, texture and listeners.
Scene shutdown/destroy and partial allocation failure release resources and
notify the synchronization owner. Delivery must dispose FE33 before replacing
the incumbent scene and construct a new scene port for its successor.

`MuralSyncPanel` composes the existing editor with labeled synchronization and
unresolved-command status. No product route, navigation, shared registration,
moderation UI or new production debug hook is installed. No decorative left
accents or legacy art are used.

## Required delivery/composition work

1. The amended authenticated own-result operation is adopted and fixture-tested
   in `sync/http.ts`. Backend route deployment and real producer-consumer
   qualification remain external integration obligations, **NOT_RUN** here.
   Missing/foreign/expired/unavailable results cannot be replaced with status
   metadata or snapshot equality.
2. Wire the existing canonical privacy/HttpClient, incumbent world release,
   admission/event write capability, existing scene and reviewed virtual anchor
   into the owned exports. Only delivery writes root/shared runtime composition.
   Missing authority or an unavailable original result must fail closed.
3. Use `start`, `read/subscribe`, `lease` and `dispose` with the shell lifetime.
   Keep the consumer/editor through panel navigation to preserve unknown command
   recovery; create a new consumer after a canonical privacy change. The
   in-process `receiveDelta(capturedLease, dto)` method is for qualified delivery
   and deterministic tests, not an invented public WS extension.
4. IN04 QA may observe the existing model/editor/sink ports through a separately
   assigned read-only test adapter. No raw command bodies, keys or tokens should
   become production globals. Existing visible labels include `Wall
   synchronization status`, `Original placement status`, `Wall command status`,
   `Selection preview`, X/Y coordinates, Preview/Confirm pixel, and Check
   original command. Real `/wall` root wiring is not yet qualified here.

## Verification evidence

Executed in the assigned absolute worktree with Node 24.22.2 at
`/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node` (substitute
this path for `NODE`). Delivery authorized the temporary nontracked
`frontend/node_modules` symlink to its qualified installation; no reinstall or
shared dependency mutation occurred.

| Command | Exit | Evidence |
| --- | --- | --- |
| `python3 scripts/mcr/check-plan.py` | 0 | Planning provenance, archived hashes and required scope PASS; not application acceptance. |
| `python3 scripts/mcr/validate-domain.py` | 0 | 164 schemas, 108 operations, 41 fixtures; REVIEW_CANDIDATE, runtime NOT_IMPLEMENTED and freeze NOT_ACCEPTED remain the checker result. |
| `NODE frontend/node_modules/vitest/vitest.mjs run frontend/src/features/mural/sync/__tests__ frontend/src/world/mural/__tests__ frontend/src/features/mural/editor/__tests__/controller.test.ts frontend/src/features/mural/editor/__tests__/input.test.ts` | 0 | 131 tests / 7 files, including the unchanged 55 FE32 tests. Two-consumer convergence, ordering/dedup/gap, barriers, stale snapshots, queue bounds, canonical HTTP receipt/result binding, forged envelopes, unavailable own-result statuses, pause/freeze/read-only, unknown/rejected/ACK, privacy/late-result eviction, timeout, sink coalescing and scene shutdown. |
| `NODE frontend/node_modules/typescript/bin/tsc --project frontend/tsconfig.json --pretty false --incremental false` | 0 | Whole frontend strict types. |
| `NODE frontend/node_modules/oxlint/bin/oxlint frontend/src/features/mural/sync frontend/src/world/mural` | 0 | Scoped source lint, no warnings/errors. |
| `NODE frontend/src/features/mural/sync/__tests__/browser-smoke.mjs` | 0 | Actual Chrome 156.0.8078.80 with touch/mobile/reduced-motion emulation at 360/390/430×844, actual Phaser 3.90.0 canvas scene. |
| `git diff --check` | 0 | Whitespace; staged ownership/import inspection is part of the committed handoff. |

Browser output lives under
`frontend/src/features/mural/sync/__tests__/evidence/browser/`. `report.json`
binds source hashes, base, browser version, widths and assertions; each width has
converged/unknown/frozen PNGs. The fixture has two independent canonical actors,
one actual Phaser game, two owned mural textures, zero world sockets and an
in-process deterministic C04 HTTP server through the real shared HttpClient.
Checks cover convergence, preview-versus-confirm, keyboard submission, metadata
that remains unresolved without original-result evidence, explicit read-only
recovery without POST replay, higher pause/freeze revisions, no overflow,
44px controls and privacy resource eviction. These are fixture qualifications,
not real service or multiuser proof.

Production root composition, deployed own-result routing, real durable server
flows, moderation/restore across independent clients, native iOS/Android,
screen-reader hardware, load/FPS/memory qualification and IN04 are **NOT_RUN**.
The committed handoff completes this bounded frontend implementation assignment;
external live gates are tracked separately, without reducing their scope. Only
delivery integrates and pushes validated commits to `julymeltdown/mcr/master`.

## 2026-11-06 producer-backed closure (task 43)

**CLOSED for the current revised scope.** The real development stack served the
authoritative Wall at revision 1 (`GET /mural/snapshot` 200 `EDITABLE`, `GET
/mural/operations` 200 from 1 to 1). The actual `createMuralHttp`,
`createMuralSync` and `MuralSyncPanel` rendered `Wall synchronized at revision
1.` at 360/390/430; an invalid bearer produced a real 401 and the panel reported
`Wall synchronization unavailable. Reconnect the world and refresh before
editing.` without a fabricated revision. The shipped placement pins match the
authoritative server release map (`mcr_reference_scene_r3` r4, content
`sha256:736afd27…`, geometry `sha256:899aca9b…`), so no configuration edit was
needed. Evidence: `.omo/evidence/mcr-finish-project/task-43-fe-closure.md` and
`frontend/e2e/fe-closure/`. This is development-stack evidence, not release,
device, QA08 or a repeat of the IN04 multiuser gate.
