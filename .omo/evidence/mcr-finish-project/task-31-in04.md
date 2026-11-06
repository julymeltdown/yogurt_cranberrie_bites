# Task 31 — IN04 multiuser mural and moderation gate

- Plan row: 31 (`Pass IN04 multiuser mural and moderation gate`)
- Worktree: `/Users/lhs/Desktop/mcr-worktrees/exec-w5-in04`
- Branch: `codex/mcr-exec-w5-in04`
- Base: `13d2ee25f` (`master`)
- Gate driver: `backend/scripts/mcr/in04_wall_b_gate.py`
- Machine report: `task-31-in04-report.json` (this directory)
- Before-fix report: `task-31-in04-advertised-before.json` (this directory)
- Result: **PASS** on the authoritative post-fix run; cleanup PASS; **39/39** checks true.

Banner: **development stack, NOT_IN03_QUALIFICATION, NOT_RELEASE_QUALIFICATION,
QA08 NOT_RUN, no device or visual claim.** This run proves the functional
multiuser Wall B and moderation behavior on this checkout's native services. It
does not claim production admission, 200-user capacity, release qualification or
browser/device acceptance.

## Commits

| Commit | Purpose |
| --- | --- |
| `fc0a150d4` | `fix(mural): compose the operator admin producer` |
| `61e577d22` | `fix(mural): make the advertised snapshot restorable` |
| evidence refresh | the commit that adds this document, the driver step and the reports |

All commits are on `codex/mcr-exec-w5-in04`; nothing was pushed or merged.

## Why the product fixes were required

The Gateway mounted the four frozen C04 admin routes
(`getConcertAdminMural`, `controlConcertMural`, `lockConcertMuralRegion`,
`restoreConcertMural`) and the generated client consumed them, but the Concert
product dispatcher had no admin-mural producer. Every admin call reached
`product_rpc::operation` as `Unsupported product operation` (503), so pause,
resume, lock, restore and freeze were not reachable at all.

`fc0a150d4` adds the missing Concert producer on the incumbent aggregate:
closed admin bodies, the real PostgreSQL `OPERATOR` grant, receipt/outbox
execution, `PAUSE`/`RESUME`/`FREEZE`, region lock/unlock, named-checkpoint
restore and original-key status recovery. Every admin mutation is a new
non-pixel revision: it advances `reset_revision`, allocates a fresh
`snapshot_id`, writes an immutable checkpoint and emits a `RESOURCE_INVALIDATED`
outbox row through the same BE15 receipt transaction as member tools.

`61e577d22` fixes the restore identity defect: `MuralAdmin::state_in` returned
the current `state.snapshot_id` without materializing the corresponding
`concert_mural_snapshots` row, while `restore` selects that table. Only the
legacy member snapshot read inserted the row. An admin GET at a member-produced
revision therefore advertised an id that restore refused with 409
`INVALID_TRANSITION`; the admin UI defaults its restore target to exactly that
field. The fix materializes and validates the current checkpoint when the admin
state is projected, using the same insert-and-compare as the legacy reader.

The generated contract's `x-mcr-runtime-status: NOT_IMPLEMENTED` metadata was
not edited because `backend/docs/api/` is integrator-owned. **The admin routes
are now implemented; the frozen metadata is not updated.** This is a
contract-metadata follow-up, not a gate blocker.

## Stack

- Native binaries built from this checkout:
  `CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/exec-w5-in04 cargo build
  -p concert-service -p auth-service -p friend-service -p profile-service
  -p post-service -p notification-service -p api-gateway -j 3` (exit 0).
- Owned disposable PG16 cluster with `max_prepared_transactions=256`, owned
  Redis, owned local mail sink, real mTLS/PKI and a private temp root
  (`/private/tmp/mcr-pg-apr2m4k6`, owner marker). No foreign database or shared
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
  --report /tmp/in04-advertised-after2.json
# exit 0; result PASS; cleanup PASS; 39/39 checks true
```

Supporting checks on the final source:

| Command | Exit | Result |
| --- | --- | --- |
| `cargo test -p concert-service --lib mural_tools_b::admin` | 0 | 2 passed; 0 failed |
| `cargo build -p concert-service -j 3` (target dir above) | 0 | service binary rebuilt from the fixed formatted source |
| `rustfmt --edition 2021 --check` on the changed Rust files | 0 | clean |
| `cargo check -p concert-service` | 0 | no errors |

## The advertised-snapshot defect: before and after

The new step deliberately leaves the current revision member-produced and does
not call a member snapshot read. The operator reads `/admin/mural`, pauses, then
restores the **advertised** `snapshot_id`:

```
pencil                      200, revision 14
GET /admin/mural            200, revision 14, snapshot_id <advertised>
POST control PAUSE          200, PAUSED, revision 15
POST restore <advertised>   ...
```

Observed on the pre-fix producer (`task-31-in04-advertised-before.json`):

```
admin_read.http = 200, revision = 14, snapshot_id = 6a4482d2-7609-4ba1-a70a-9a965c9486ad
pause.http = 200, state = PAUSED, revision = 15
restore.http = 409
restore.reason = INVALID_TRANSITION
restore.target_snapshot_id = 6a4482d2-7609-4ba1-a70a-9a965c9486ad
expected_pixels_sha256 = c24c5eeed04d290afcfc407cf326f64539a7ec059987d4009b04726ceef7dfac
restored_pixels_sha256 = null
restored_exact_grid = false
gate failure = GateFailure: restore-advertised-409
```

Observed after the fix (`task-31-in04-report.json`):

```
admin_read.http = 200, revision = 14, snapshot_id = a45b1333-baeb-4e33-9322-01be3d7d221b
pause.http = 200, state = PAUSED, revision = 15
restore.http = 200
restore.state = PAUSED
restore.revision = 16
restore.target_snapshot_id = a45b1333-baeb-4e33-9322-01be3d7d221b
expected_pixels_sha256 = c24c5eeed04d290afcfc407cf326f64539a7ec059987d4009b04726ceef7dfac
restored_pixels_sha256 = c24c5eeed04d290afcfc407cf326f64539a7ec059987d4009b04726ceef7dfac
restored_exact_grid = true
```

The new check is `restore_advertised_snapshot`; it is one of the 39 passing
checks.

## Steps

All statuses are from the authoritative run. `sync frame` is the real product
sync payload for that step: the HTTP snapshot/delta response each member client
consumed. The product has no mural WebSocket frame type (see limits).

| Step | Request / effect | Status and exact evidence |
| --- | --- | --- |
| 1. Initial convergence | alice + bob GET `/mural/snapshot` | 200 / 200; revision 1, `EDITABLE`, same snapshot id, pixels SHA-256 `f29eaf4e…` |
| 2. Pencil across clients | alice POST `/mural/pixels` rev 1 | 200, revision 2; bob frame after 1: delta 1→2, `reset_required=false`, 1 operation |
| 3. Opaque erase | bob POST `/mural/erase` rev 2 | 200, revision 3, `kind=ERASE`, 1 changed cell; alice frame after 2: reset required, then snapshot revision 3 with the cell at background 0 |
| 4. Four-connected bounded fill | bob POST `/mural/fill` rev 3, seed 2361, color 3 | 200, revision 4, `kind=FILL`, 244 changed cells; independent oracle proves exactly the 244-cell component changed; alice converged |
| 5. Cooldown | alice pencil, then second pencil inside 1200 ms | first 200 revision 5; second **429 `MURAL_COOLDOWN`**; revision unchanged; no new charge |
| 6. Fill beyond 256 | bob fill of the 11616-cell background component | **409 `MURAL_FILL_TOO_LARGE`**; revision and pixel hash unchanged; no charge |
| 7. Undo revision guard | bob fill rev 6; alice foreign write rev 7; bob undo targeting the rev-6 action | 200 / 200 / **409 `MURAL_UNDO_NOT_LATEST`**; revision stayed 7 |
| 8. Eligible latest-own undo | bob erase rev 8; bob undo rev 8 | 200 `UNDO` revision 9; exact pre-erase grid restored |
| 9. Shared 512-cell budget | alice spends 1+1+244+244+1 = 491 cells; next 244-cell fill | **429 `MURAL_BUDGET_EXCEEDED`**; `remaining_cells=20`; revision and pixel hash unchanged |
| 10. Uncertain outcome | bob POST `/mural/pixels` with key; response discarded after send | command status 200 `SUCCEEDED` resource revision 13; own result 200 revision 13; authoritative snapshot revision 13; same-key replay 200 revision 13; charges unchanged; no new key, no duplicate |
| 11. Advertised admin restore | alice pencil rev 14; admin GET advertises the revision-14 snapshot; operator pause; restore that id | **PASS**: restore 200 `PAUSED` revision 16; expected and restored pixel hashes equal; the pre-fix output is 409 `INVALID_TRANSITION` |
| 12. Moderation | operator GET/POST admin routes; member attempts | see below |
| 13. World socket frames | alice + bob bootstrap → rooms → world ticket → `/world-ws` authenticate | PASS; each client received one real `snapshot` frame (928 bytes in, 326 out), distinct stream ids, shared room epoch |
| 14. Privacy + final convergence | foreign action/result reads; snapshot/delta inspection; final snapshots | foreign reads 404 `RESOURCE_UNAVAILABLE`; no `actor_id`, `before`, `cells`, `command` or `idempotency_key` in public snapshot/delta; both clients revision 26 `FROZEN`, pixels SHA-256 `a37b94f0…` |

### Moderation sub-steps

| Action | Result |
| --- | --- |
| operator GET `/admin/mural` | 200 `EDITABLE` revision 17 |
| member GET `/admin/mural` | **403 `PERMISSION_DENIED`** |
| member POST control `PAUSE` | **403 `PERMISSION_DENIED`** |
| operator POST control `PAUSE` | 200 `PAUSED` revision 18 |
| member write while paused | **409 `MURAL_PAUSED`**; revision unchanged |
| operator POST control `RESUME` | 200 `EDITABLE` |
| member write after resume | 200 revision 20 |
| operator POST locks (1 cell `[66,18)–[67,19)`) | 200; one lock returned |
| member write in locked cell | **409 `MURAL_REGION_LOCKED`** |
| operator POST locks `locked=false` | 200; locks `[]` |
| operator pause, then restore named snapshot | 200 `PAUSED` revision 24; restored pixel hash exactly equals the named baseline snapshot |
| operator `RESUME` | 200 `EDITABLE` |
| operator `FREEZE` | 200 `FROZEN` revision 26 |
| member write while frozen | **409 `MURAL_FROZEN`** |
| operator POST control `RESUME` after freeze | **409 `INVALID_TRANSITION`** (frozen is terminal) |

## Durable evidence

Final `concert_mural_state`: revision 26, `FROZEN`, reset revision 26, pixel
SHA-256 `a37b94f03495a83b993271c416a2bd1017be8fb5a126dd93059216e1ea912129`.

| Query | Result in the authoritative run |
| --- | --- |
| `concert_mural_snapshots` | 23 immutable checkpoints; initial revision 1, final revision 26 |
| `concert_mural_tool_actions` | 14 actions (PENCIL/ERASE/FILL/UNDO), one per successful member mutation |
| `concert_mural_tool_charges` | 14 immutable charges: alice 7 / 493 cells, bob 7 / 493 cells (986 total) |
| `concert_mural_tool_correlations` | 21 exact-key correlations, including refused attempts |
| `concert_command_receipts` | 33 receipts: successful mutations plus explicit failed receipts for cooldown, fill-too-large, undo-not-latest and budget |
| `concert_outbox` | 25 `RESOURCE_INVALIDATED` rows, including admin revisions |
| `concert_mural_locks` | 0 after the explicit unlock |

### Charge-total correction

An earlier revision of this evidence said `492 + 492`. That was wrong. The run
it described actually stored **492 + 493** (alice 6 charges, bob 7 including
the undo). The authoritative post-fix run adds one Alice pencil for the
advertised-snapshot step, so the observed totals are **493 + 493** across 7 + 7
charge rows. The count explanation is: every successful tool action writes one
immutable charge (14 actions = 14 charges), while correlations (21) and receipts
(33) are higher because refused attempts are correlated and receipted but never
produce an action or charge.

No access token, cookie, password or mail payload is serialized in the report.

## Failures and retries during execution

These are retained rather than hidden:

1. First run exited 1 with `KeyError: 'changed_cells'` in the driver's own check
   aggregation. The driver path was corrected.
2. One run exited 1 with `http.client.IncompleteRead(194 bytes)` while reading a
   409 response. No state change occurred; the driver now performs one bounded
   same-key retry. The authoritative run had zero retries.
3. One run exited 1 because enrollment signup returned 504 while a concurrent
   unrelated Cargo test saturated the host. The driver now retries principal
   enrollment; the authoritative run created all three principals on attempt 1.
4. Two earlier socket witnesses closed 4400 because the driver sent snake_case
   fields in the WebSocket authenticate frame. The frozen camelCase shape was
   used; the authoritative run received snapshot frames on both clients.
5. The first post-fix gate run still returned 409 because its binary was built
   at 20:07, before the 20:10 source edit. The service was rebuilt at 20:14
   from the fixed source and the gate rerun; the authoritative run is that
   rebuilt binary. This is a build-order mistake in the test attempt, not a
   product result.
6. The advertised-snapshot step was first run against the pre-fix producer and
   failed exactly as the defect predicts (`restore-advertised-409`,
   `INVALID_TRANSITION`); that report is retained.

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
- **Admin contract metadata**: the routes are implemented; the frozen OpenAPI
  `x-mcr-runtime-status` label still says `NOT_IMPLEMENTED` and was not edited.
