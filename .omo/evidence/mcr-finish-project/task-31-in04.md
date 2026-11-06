# Task 31 — IN04 multiuser mural and moderation gate

- Plan row: 31 (`Pass IN04 multiuser mural and moderation gate`)
- Worktree: `/Users/lhs/Desktop/mcr-worktrees/exec-w5-in04`
- Branch: `codex/mcr-exec-w5-in04`
- Base: `13d2ee25f` (`master`)
- Gate driver: `backend/scripts/mcr/in04_wall_b_gate.py`
- Machine report: `task-31-in04-report.json` (same directory)
- Result: **PASS** on the authoritative run; cleanup PASS; 38/38 checks true.

Banner: **development stack, NOT_IN03_QUALIFICATION, NOT_RELEASE_QUALIFICATION,
QA08 NOT_RUN, no device or visual claim.** This run proves the functional
multiuser Wall B and moderation behavior on this checkout's native services. It
does not claim production admission, 200-user capacity, release qualification or
browser/device acceptance.

## Why a product fix was required

The Gateway mounted the four frozen C04 admin routes
(`getConcertAdminMural`, `controlConcertMural`, `lockConcertMuralRegion`,
`restoreConcertMural`) and the generated client consumed them, but the Concert
product dispatcher had no admin-mural producer. Every admin call reached
`product_rpc::operation` as `Unsupported product operation` (503), so pause,
resume, lock, restore and freeze were not reachable at all. That made the
moderation half of the gate impossible, not merely unqualified.

`fix(mural): compose the operator admin producer` adds the missing Concert
producer on the incumbent aggregate:

- `backend/services/concert-service/src/mural_tools_b/admin.rs` (new): closed
  request bodies, real PostgreSQL `OPERATOR` role authority, receipt/outbox
  execution, `PAUSE`/`RESUME`/`FREEZE`, region lock/unlock, named-checkpoint
  restore and original-key status recovery.
- `protocol.rs`, `runtime.rs`, `mod.rs`, `query.rs`, `config/mural_tools_b.rs`,
  `repository.rs`: route parsing/admission/dispatch for the admin family and
  admin command-status recovery, plus the `INVALID_TRANSITION`/`PERMISSION_DENIED`
  projections.
- `backend/crates/concert-domain/src/mural/tools_b.rs`: closed admin value types,
  bounds/reason validation and the terminal transition table.
- `backend/crates/concert-application/src/mural/tools_b.rs`: two tool errors.

Every admin mutation is a new non-pixel revision: it advances `reset_revision`,
allocates a fresh `snapshot_id`, writes an immutable checkpoint and emits a
`RESOURCE_INVALIDATED` outbox row through the same BE15 receipt transaction as
member tools. Reads and writes require the actual
`concert_event_operator_roles` `OPERATOR` grant; a browser role claim is never
authority. Frozen is terminal; restore is accepted only while paused and leaves
the wall paused for an explicit resume. No private before/after history,
reason text, actor or lock metadata enters a public projection.

The frozen generated contract's `x-mcr-runtime-status: NOT_IMPLEMENTED` metadata
was intentionally not edited because `backend/docs/api/` is integrator-owned; the
runtime behavior is now implemented and the existing generated request/response
types were consumed unchanged. This is a contract-metadata follow-up, not a gate
blocker.

## Stack

- Native binaries built from this checkout:
  `CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/exec-w5-in04 cargo build
  -p concert-service -p auth-service -p friend-service -p profile-service
  -p post-service -p notification-service -p api-gateway -j 3` (exit 0).
- Owned disposable PG16 cluster with `max_prepared_transactions=256`, owned
  Redis, owned local mail sink, real mTLS/PKI and a private temp root
  (`/private/tmp/mcr-pg-tpwh4iy7`, owner marker). No foreign database or shared
  writable checkout was used.
- Seven real services ready: api-gateway, auth-service, friend-service,
  profile-service, post-service, notification-service, concert-service
  (gRPC + metrics + world listeners). Gateway readiness includes database,
  gateway→auth/friend/notification/post/profile and signing-keys all `up`.
- Event: `f1fd7b85-d393-5410-8bc3-5ea6b71aeb77`; concert product release digest
  `adcec15987c40769d0b6082df676182b87c658c5037c5e758ed89868548ea245`.
- Two independent member principals (alice, bob) and one operator principal were
  created through the real canonical enrollment → mail verify → login flow;
  profiles were initialized through the public profile HTTP route. All three
  principals were created on attempt 1 in the authoritative run.
- Mural state was provisioned at revision 1 from the qualified six-file pack
  `frontend/public/wall/initial-motif-v1/` (manifest SHA-256
  `3f1aa318f0e4b7d8fa7f079b1101f7386619291a8dbb3bb04ef70820952f34ce`), with
  explicit cooldown 1200 ms and `EDITABLE`. The pack's own connected components
  were used for the fill cases: 244-cell component at seed 2361 and the
  11616-cell background component at seed 0.

## Authoritative command

```
python3 -B backend/scripts/mcr/in04_wall_b_gate.py \
  --bin-dir /Users/lhs/Desktop/mcr-build/exec-w5-in04-bins \
  --report /tmp/in04-gate-9.json
# exit 0; result PASS; cleanup PASS; 38/38 checks true
```

Supporting checks on the final source:

| Command | Exit | Result |
| --- | --- | --- |
| `cargo test -p concert-service --lib mural_tools_b::admin` | 0 | 2 passed; 0 failed |
| `cargo build -p concert-service -j 3` (target dir above) | 0 | service binary built from formatted source |
| `rustfmt --edition 2021 --check` on all changed Rust files | 0 | clean |
| `cargo check -p concert-service` | 0 | no errors |

## Steps

All statuses below are from the authoritative run. `sync frame` is the real
product sync payload for that step: the HTTP snapshot/delta response each member
client consumed. The product has no mural WebSocket frame type (see limits).

| Step | Request / effect | Status and exact evidence |
| --- | --- | --- |
| 1. Initial convergence | alice + bob GET `/mural/snapshot` | 200 / 200; both revision 1, `EDITABLE`, same snapshot id, pixels SHA-256 `f29eaf4e…` |
| 2. Pencil across clients | alice POST `/mural/pixels` rev 1 | 200, revision 2; bob frame after 1: delta 1→2, `reset_required=false`, 1 operation; converged |
| 3. Opaque erase | bob POST `/mural/erase` rev 2 | 200, revision 3, `kind=ERASE`, 1 changed cell; alice frame after 2: reset required, then snapshot revision 3 with the cell at background 0 |
| 4. Four-connected bounded fill | bob POST `/mural/fill` rev 3, seed 2361, color 3 | 200, revision 4, `kind=FILL`, 244 changed cells; independent oracle proves the exact 244-cell four-connected component changed and no other cell; alice converged on revision 4 |
| 5. Cooldown | alice pencil, then second pencil inside 1200 ms | first 200 revision 5; second **429 `MURAL_COOLDOWN`**; revision unchanged; no new charge |
| 6. Fill beyond 256 | bob POST `/mural/fill` background component 11616 cells | **409 `MURAL_FILL_TOO_LARGE`**; revision and pixel hash unchanged; no charge |
| 7. Undo revision guard | bob fill rev 6; alice foreign write rev 7; bob undo targeting rev-6 action | 200 / 200 / **409 `MURAL_UNDO_NOT_LATEST`**; revision stayed 7 |
| 8. Eligible latest-own undo | bob erase rev 8; bob undo rev 8 | 200 `UNDO` revision 9; exact pre-erase grid restored; revision advanced, not rewound |
| 9. Shared 512-cell budget | alice spends 1+1+244+244+1 = 491 cells in the rolling window; next 244-cell fill | **429 `MURAL_BUDGET_EXCEEDED`**; `remaining_cells=20`; revision and pixel hash unchanged |
| 10. Uncertain outcome | bob POST `/mural/pixels` with key, response discarded after send | command status 200 `SUCCEEDED` resource revision 13; own result 200 revision 13; authoritative snapshot revision 13; same-key replay 200 revision 13; charges before/after `6|492` unchanged; no new key, no duplicate |
| 11. Moderation | operator GET/POST admin routes; member attempts | see below |
| 12. World socket frames | alice + bob bootstrap → rooms → world ticket → `/world-ws` authenticate | PASS; each client received one real `snapshot` frame (928 bytes in, 326 out), distinct stream ids, shared room epoch `149af704-…`; see limits |
| 13. Privacy + final convergence | foreign action/result reads; snapshot/delta inspection; final snapshots | foreign reads 404 `RESOURCE_UNAVAILABLE`; no `actor_id`, `before`, `cells`, `command` or `idempotency_key` keys in public snapshot/delta; alice and bob both revision 22 `FROZEN`, pixels SHA-256 `f8afbf43…` |

### Moderation sub-steps

| Action | Result |
| --- | --- |
| operator GET `/admin/mural` | 200 `EDITABLE` revision 13 |
| member GET `/admin/mural` | **403 `PERMISSION_DENIED`** |
| member POST control `PAUSE` | **403 `PERMISSION_DENIED`** |
| operator POST control `PAUSE` | 200 `PAUSED` revision 14 |
| member write while paused | **409 `MURAL_PAUSED`**; revision unchanged |
| operator POST control `RESUME` | 200 `EDITABLE` |
| member write after resume | 200 revision 16 |
| operator POST locks (1 cell `[66,18)–[67,19)`) | 200; one lock returned |
| member write in locked cell | **409 `MURAL_REGION_LOCKED`** |
| operator POST locks `locked=false` | 200; locks `[]` |
| operator pause, then restore named snapshot | 200 `PAUSED` revision 20; restored pixel hash exactly equals the named baseline snapshot |
| operator `RESUME` | 200 `EDITABLE` |
| operator `FREEZE` | 200 `FROZEN` revision 22 |
| member write while frozen | **409 `MURAL_FROZEN`** |
| operator POST control `RESUME` after freeze | **409 `INVALID_TRANSITION`** (frozen is terminal) |

## Durable evidence

Final `concert_mural_state`: revision 22, `FROZEN`, reset revision 22, pixel
SHA-256 `f8afbf43ff8775315c5fbb8bf9834994ebe2b7a8f92d399361ba97335a903e2e`.

| Query | Result in the authoritative run |
| --- | --- |
| `concert_mural_snapshots` | 19 immutable checkpoints; final revision 22, initial revision 1 |
| `concert_mural_tool_actions` | 13 actions (PENCIL/ERASE/FILL/UNDO) with private before/after cells and 600 s undo expiry |
| `concert_mural_tool_charges` | 13 immutable charges; alice 492 cells, bob 492 cells; rolling window rows |
| `concert_mural_tool_correlations` | 20 exact-key correlations, including refused attempts |
| `concert_command_receipts` | 29 receipts: successful mutations plus explicit failed receipts for cooldown, fill-too-large, undo-not-latest and budget |
| `concert_outbox` | 21 `RESOURCE_INVALIDATED` rows, one per successful mutation |
| `concert_mural_locks` | 0 after the explicit unlock |

The report contains the exact per-step request log, all redacted bodies, the
revision history with per-revision pixel hashes, the quota rows and the outbox
ledger. No access token, cookie, password or mail payload is serialized.

## Failures and retries during execution

These are retained rather than hidden:

1. First run exited 1 with `KeyError: 'changed_cells'` in the driver's own check
   aggregation (wrong nested path). The product and DB rows had passed; the
   driver path was corrected and the run repeated.
2. One run exited 1 with `http.client.IncompleteRead(194 bytes)` while reading a
   409 response. No state change occurred; the driver now performs one bounded
   same-key retry for an incomplete/closed response. The authoritative run had
   zero retries.
3. One run exited 1 because enrollment signup returned 504 while a concurrent
   unrelated Cargo test saturated the host. The driver now retries principal
   enrollment; the authoritative run created all three principals on attempt 1.
   No tests or builds were run concurrently with the authoritative gate.
4. Two earlier passing runs recorded the world-socket witness as `NOT_RUN` with
   close code 4400 because the driver sent snake_case fields in the WebSocket
   authenticate frame. The frame was corrected to the frozen camelCase wire
   shape; the authoritative run received real snapshot frames on both clients.

## NOT_RUN and limits

- **Undo wall-clock expiry (600 s)**: NOT_RUN as a real wait. The positive
  in-window undo and the revision guard executed; expiry is covered by the
  existing native unit cases and the runtime `undo_expires_at` guard. Consumer:
  `backend/services/concert-service/src/mural_tools_b/repository.rs`.
- **Socket frames**: PASS for the real world connection/snapshot frame on each
  client. The product has no mural WebSocket frame type; mural synchronization
  is the authenticated HTTP snapshot/delta poll recorded per step. Consumer:
  `frontend/src/features/mural/sync/controller.ts` (HTTP) and
  `frontend/src/world/network/transport/engine.ts` (world socket).
- **Device/visual/browser acceptance**: NOT_RUN. This is a backend multiuser
  gate; the 360/390/430 visual and device obligations remain with the IN04
  frontend runner in `docs/mcr/qa/in04-wall-b.md`.
- **Production/release/IN03/QA08**: not claimed. This is one owned development
  stack, one event and three principals; it is not a 200-user load run or a
  release admission.
- **Seed path**: the owned disposable cluster was seeded by direct SQL using the
  qualified pack's exact bytes. `mural_seed::provision` has no CLI/HTTP entry in
  this checkout, so the producer path itself was not exercised; the pack bytes
  and component oracle were.
- **Name/contract metadata**: the generated OpenAPI runtime-status label still
  says `NOT_IMPLEMENTED`; the behavior is implemented, but the shared contract
  file was not edited here.
