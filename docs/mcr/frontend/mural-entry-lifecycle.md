# FE33 incumbent world mural entry lifecycle

Date: 2026-11-04, Asia/Seoul. Module status: **MOCK_READY**. Original **FE33 frontend task-local criteria: SATISFIED under the frozen contract/fixture scope**, as mapped below. This is an original-task disposition, not an additional completed parent or live IN04 acceptance.

## Scope and provenance

Assignment start base: `a3526254a716e14e3134de551c3b277379a1096f`; branch `codex/mcr-fe-FE33-mural-entry-lifecycle` in the retained isolated `fe-ref-gifts` worktree. Final qualification reconciles peer master `72685ad98f26437ce88ec7a51ffde0aa2bc29539`. The exact writable scope is NEW `frontend/src/world/mural-entry-lifecycle/**` and this NEW document. Existing runtime/composition/MapScene/root callers, ports, renderer, editor/sync, contracts/generated outputs, manifests, locks, backend and supplied originals remain untouched by this commit.

All new lifecycle code is newly authored. Existing entry, Auth/privacy, C04 HTTP adapter, sync/editor, projection and scene sink are composed by reference, not duplicated. Tests reuse existing deterministic entry/server/world fixtures. The native fixture creates one explicitly synthetic incumbent Phaser game solely to exercise the borrowed scene callback; application code creates no game, scene, socket, HTTP client, privacy owner, frame loop or retry scheduler.

Frozen consumer port source: `aedbb020e637993fe9ef1831e31bd6ae5316ae0c`, `frontend/src/app/mural-entry/ports.ts` (blob `ea4511a0c1cfc91b21afe07886b91b132634704f`). Existing C04 v0.1.2 own-result recovery source: `bff7e53c0638d5462af819bf560468a1f857c88e`; mural schema blob `1134e67f1dcb5786ed83001f34a976a92fa3f4f0`. Optional cooldown correction: source `dc4a3f93cdd4d7df658a2fbc2d1ac18f9e7141af`, coherent frontend publication `faeb6ed4939a623627d1e9ac2c5016b986a2efea`. Their unchanged behavior is consumed here; missing timing is never made into a countdown or writable eligibility.

The native receipt at `frontend/src/world/mural-entry-lifecycle/__tests__/evidence/native.json` pins the actual module, runner, consumer port/owner, scene, sink, HTTP/sync/editor and canonical schema inputs by SHA-256. At qualification the common schema hash is `65f2c2f19594942b89613e67b97bd3181e0889e7f47e4476cda752d9aab10584`; mural schema hash is `345871ce89fabff02f06087fcd3e8907651a5712b34ccbd53091158af123b6ca`. All native source pins were checked again against the delivery tree. The final delivery commit is obtained with `git log -1 --format=%H -- docs/mcr/frontend/mural-entry-lifecycle.md`; direct non-force publication is followed by remote ancestry verification and a commit/API handoff.

## Usable API and ownership

Import `createMuralEntryLifecycle` and `MuralEntryQualification` from `world/mural-entry-lifecycle`.

```ts
const lifecycle = createMuralEntryLifecycle({ privacy: auth.privacy, world })
// Only an independently qualified incumbent supplier can construct this input.
lifecycle.bind(qualification)
const release = lifecycle.read() // MuralEntryRelease | null
const stop = lifecycle.subscribe(() => consume(lifecycle.read()))
// On final caller disposal:
stop()
lifecycle.dispose()
```

`world` is only `Pick<WorldReplicationRelease, 'read' | 'subscribe'>`. The qualification explicitly supplies the full `PrivacyScope`, exact event/room/epoch/stream binding, supplier `AbortSignal`, existing `MuralAuthorityPort`, existing `IncumbentMuralScene` callback, reviewed `{x,y,width,height,depth}` anchor and exclusive `textureKey`. There is no default anchor or key. Anchor dimensions/finite coordinates/key and binding shape are checked before the scene callback is invoked. These structural checks do not establish product provenance, admission or server authorization.

The lifecycle captures scope/binding/anchor and borrowed methods, then verifies current privacy plus a live, visible incumbent world with the same session identity and exact baseline binding. Supplied authority must match that binding. Its `writable` boolean is preserved unchanged; world liveness, scene presence and anchor validity never promote it. The existing `createWorldMuralAuthority` remains default read-only unless a separate canonical supplier qualifies writing.

`bind(input|null): boolean` withdraws the previous qualification before replacing it. Rebinding the same live object is a no-op. Each input object is consumed once: an invalidated qualification cannot reactivate when the world reconnects, becomes visible or returns to the former room. The caller must provide a fresh explicit qualification. Invalid inputs fail closed without scene allocation. `read` and captured authority/scene methods also check current authority, fencing changes even before a notification arrives.

World loss, hidden state, room/epoch/stream replacement, missing/mismatched authority, supplier abort, scene close and scene allocation/upload failure withdraw the release synchronously. Full privacy/user/event/generation/policy/gate loss permanently disposes this lifecycle. Its output becomes null before the derived release signal aborts and observers run. All subordinate object/texture/frame/close/authority leases are independently released, even with throwing observers/cleanup or no mounted entry consumer. Cleanup is idempotent; reentrant bind during teardown is rejected. The borrowed engine, world connection and privacy owner are never disposed.

The intended downstream consumer remains the existing `MuralEntryProvider` or `createMuralEntry.bind`. Keep that entry owner stable for the same privacy scope; feed it release changes from `read/subscribe`. It already owns sync/editor/HTTP fencing and original command recovery. Ordinary world withdrawal removes raster, preview, polling and private cooldown but preserves an unresolved original command. Fresh qualification reconnects that same entry owner; explicit Check reads the original key/result and never sends a second POST. Privacy invalidation disposes the existing entry owner too and erases the command.

## Original FE33 criteria and remaining frontend behavior

The specified FE33 implementation is snapshot/delta reconciliation, commit receipts, pause/freeze/restore reset and world-wall publication. Existing sync/projection/HTTP/editor/entry modules implement those behaviors; this delivery supplies the remaining reusable incumbent-to-entry lifecycle bridge. The final combined suite exercises the existing implementations directly.

| Original required criterion | Current evidence and disposition |
| --- | --- |
| Two clients converge | SATISFIED in deterministic client scope. Two release-driven consumers receive reordered/duplicate contiguous operations, converge to identical pixels, then converge again after an authoritative restore snapshot. Existing sync/projection regression suites also pass. |
| Rejected local preview rolls back | SATISFIED. Actual entry/sync/editor with the C04 fixture HTTP adapter clears a rejected selection while retaining confirmed revision; unknown command outcomes remain distinct and retain the exact original command. |
| Restore discards obsolete deltas | SATISFIED. Restore changes snapshot/control revision, clears selection, leaves PAUSED read-only, and rejects the pre-restore read lease/delta; both scene rasters converge to the restored snapshot. |

The module, these criteria and their fixture/contract evidence are complete. No further code is required within this assigned new-only scope. Real backend/device/IN04 gates absent from these client criteria do not withhold this task-local disposition.

Concrete remaining frontend behavior is **actual root caller binding**: instantiate this lifecycle for the canonical world/privacy owner, supply independently qualified incumbent scene/anchor/key and authority, feed `read/subscribe` releases into the stable entry provider, and dispose it with that caller lifetime. Current root owner `01a1a153-1227-7e72-801f-a2a6b91981e1` retains that caller lease for an explicitly assigned follow-up; no existing caller file is transferred or activated here. This is an application integration dependency, not a missing lifecycle API, technical approval gate or reason to repeat already met client criteria.

There is currently no production anchor/release supplier or canonical server-backed writable eligibility established by this delivery. BE-WALL-LIVE owner `01a1a21e-ab80-7012-b9fa-cdb69eed097b` owns the real mural producer/fence. Product/root owner `01a1a1fc-7f3d-7ff1-9416-e3435c8a6f08` owns sealed parent/config/delivery and the four Product/Gateway mappings, with the canonical SDK owner and incumbent BE World owner `01a1a20f-8d37-71c3-a429-8a55bb6d6c49`. Their actual release/admission/authority qualification and real multiuser/moderation/restore/freeze IN04 remain separate. No Wall/live/readiness flag is enabled.

## Developer qualification

Retained Node 24.22.2/Vitest 4.1.9/TypeScript 6/Oxlint dependencies were borrowed through a temporary `frontend/node_modules` symlink, with no install or lock change. The symlink and owned native cache are removed at STOP. Disk was 71 GiB at intake and 69 GiB before evidence capture, above the 8 GiB floor.

| Command | Observed result |
| --- | --- |
| `node frontend/node_modules/vitest/vitest.mjs run --root frontend src/world/mural-entry-lifecycle/__tests__ src/app/mural-entry/__tests__/owner.test.ts src/features/mural/editor/__tests__/controller.test.ts src/features/mural/sync/__tests__ src/world/mural/__tests__` | Exit 0; 181 tests / 11 files, including 43 new tests / 2 files. |
| `node frontend/node_modules/typescript/bin/tsc -p frontend/tsconfig.json --pretty false --incremental false` | Exit 0; full frontend typecheck. A fixture-only Phaser lookup typo was corrected against the retained types before final qualification. |
| `node frontend/node_modules/oxlint/bin/oxlint frontend/src/world/mural-entry-lifecycle` | Exit 0. |
| From `frontend/`: `node src/world/mural-entry-lifecycle/__tests__/native.mjs` | Exit 0; Chrome 156.0.8078.80, 360/390/430px, zero page errors. |
| `python3 scripts/mcr/check-plan.py` | Exit 0; planning provenance only. |
| `git diff --check` and staged explicit-path check | Exit 0; only the new module subtree and this document. |
| `python3 scripts/mcr/generate-domain-transport.py --check` | Exit 1: pre-existing Feed input-hash comment drift, described below. No generated file is edited by this assignment. |

New tests cover exact scope/baseline/anchor/key admission, read-only propagation, terminal withdrawal, fresh qualification, late ignored-abort snapshots and committed commands, original-key recovery, privacy erasure, reentrant callbacks, partial setup failure, resource disposal and scene allocation/upload failure. Native interaction clicks Confirm/lost response → Withdraw world → Fresh qualification → Check original → Privacy invalidation through the actual incumbent Phaser scene, sink, entry, HTTP adapter and editor. All three widths retain one game and one mural object, one POST with the original key, no old texture after withdrawal, revision 11 after recovery, and no mural object/texture/world listener/timer after privacy invalidation. Captures `recovered-{360,390,430}.png` show the actual fixture raster/anchor; the 390px capture was visually inspected. No UI fidelity or FE32 requalification is implied.

The optional generator check found the existing committed Feed schema hash is `f9b4b8912b2bf4513ab474ad690efdeaa23d64f97b220f9626c5b37c5e2e99ed` (blob `0b5b1543bc564d7399e49817e3784bfab85da157`), whereas the existing generated header retains `590cce6a…`. Running the actual generator into an in-memory capture showed that **only that hash-comment line differs**; operation/query/error rules are unchanged, and contracts did not change between assignment base and qualification base. This pre-existing shared-output issue was sent directly to owner `01a1a1fc-7f3d-7ff1-9416-e3435c8a6f08` for its current single writer. It does not affect the lifecycle API or passing client journeys; it is not reported as a passing drift check.

A full app build and actual root activation were not run for this new-only module. The native Vite fixture loads the module and exercises real Phaser interactions. Fixture Auth/admission, HTTP responses, anchor, eligibility and world observations remain synthetic; no actual producer HTTP/DB, admitted socket, physical device, product art or real-user IN04 result is claimed. Developers own these checks and fixes; kings/SLs receive status/criteria/staffing handoffs without a review or rerun stage.
