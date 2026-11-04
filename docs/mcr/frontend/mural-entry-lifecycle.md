# FE33 incumbent world mural entry lifecycle

Date: 2026-11-04, Asia/Seoul. Module status: **MOCK_READY**. Original **FE33 frontend task-local criteria: SATISFIED under the frozen contract/fixture scope**, as mapped below. This is an original-task disposition, not an additional completed parent or live IN04 acceptance.

## Scope and provenance

Initial module assignment base: `a3526254a716e14e3134de551c3b277379a1096f`; branch `codex/mcr-fe-FE33-mural-entry-lifecycle` in the retained isolated `fe-ref-gifts` worktree. Module qualification reconciled peer master `72685ad98f26437ce88ec7a51ffde0aa2bc29539`; module publication is `7c2f3af7b9fd7bab2ed2d863cc2262527cc8553e`. That commit contains only NEW `frontend/src/world/mural-entry-lifecycle/**` and this document. A subsequent explicitly allocated caller follow-up adds exactly two existing-file hunks, recorded below. Runtime/MapScene, production loader, resolver/overview, ports/router/main, existing tests, renderer, editor/sync, contracts/generated outputs, manifests, locks, backend and supplied originals remain outside both write grants.

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

The initial module handoff identified actual caller binding as a separate useful assembly gap: `WorldPanels` still passed `release={null}`. That gap is now closed by the explicitly allocated two-hunk follow-up below, through the existing production custom-load seam and actual composition/provider. Original FE33 task-local criteria were already satisfied before this follow-up and are not counted again. Production activation still requires a real qualified supplier; it is not implied by client assembly.

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

At initial module qualification the optional generator check found committed Feed schema hash `f9b4b8912b2bf4513ab474ad690efdeaa23d64f97b220f9626c5b37c5e2e99ed` (blob `0b5b1543bc564d7399e49817e3784bfab85da157`), while the generated header retained `590cce6a…`. An in-memory actual projection differed only in that comment. This pre-existing issue was sent to the shared owner and resolved independently by the allocated FE20 writer in `65f91166` (`chore(api): regenerate meeting-photo feed schema provenance`). The caller follow-up consumes that published output unchanged and its generation check now passes for 108 operations. No generated-output ownership was acquired here.

A full app build and production activation were not run for the initial new-only module. The native Vite fixture loads the module and exercises real Phaser interactions. Fixture Auth/admission, HTTP responses, anchor, eligibility and world observations remain synthetic; no actual producer HTTP/DB, admitted socket, physical device, product art or real-user IN04 result is claimed. Developers own these checks and fixes; kings/SLs receive status/criteria/staffing handoffs without a review or rerun stage.

## Completed caller assembly follow-up

The former root writer `01a1a153-1227-7e72-801f-a2a6b91981e1` explicitly stopped and released the exact hunks; FE king recorded this developer as their sole active writer. The final two-file allocation supersedes crossed three-file proposals: `production.ts` remains excluded. The module goal had actually completed and `get_goal` returned null before the crossed allocation arrived, so the follow-up used a new unbudgeted goal and branch `codex/mcr-fe-FE33-mural-caller-assembly` from `7c2f3af7`. No completed goal was reset. Final reconciliation preserves peer master `d4bf83e9228d5cdf9fc8642c1c197f1b75afe9ef`; all native source pins still match after that reconciliation.

Exactly these existing hunks changed:

- `frontend/src/world/composition/index.ts`: additive lifecycle import/types, optional `WorldCompositionOptions.mural` qualification factory, borrowed `attachControls` scene/context capture, early source/lifecycle withdrawal before engine cleanup, and optional `WorldPresentationRelease.mural` read/subscribe publication.
- `frontend/src/app/world-panels/index.tsx`: subscribe to that optional mural port and pass its release into the existing `MuralEntryProvider`. Places projection/provider, presentation context, era/theme and provider tree/lifetime remain intact.

All remaining follow-up source/tests/native fixtures/evidence stay under the already owned lifecycle subtree and this document. No runtime/MapScene/production loader/resolver/reference-overview/main/ports/router/existing-test hunk is edited. The root/v3 owner can reclaim the released hunks after committed handoff/STOP through the lane ownership record.

`world/mural-entry-lifecycle/source.ts` exports `MuralQualificationFactory`, `MuralQualificationSource`, `MuralQualificationContext` and `MuralEntryReleaseSource`. A factory receives an immutable scope/context with the existing scene callback, world observation, prepared release and engine lifetime signal; it returns an explicit qualification source with `read/subscribe`, or null. Its world capability contains only bound `read/subscribe` at runtime too: reconnect/switch/transport methods are not leaked through a type-only narrowing. The source must provide a fresh qualification after withdrawal. `attachMuralEntrySource` binds those publications to the existing lifecycle and unsubscribes on parent abort/privacy disposal; failed suppliers close only the mural publication.

The actual production adapter already forwards the entire presentation object, so no production-loader or public shell-port change is necessary. An explicitly configured custom load can construct `createWorldComposition({auth, resolveRelease, mural: qualifiedSourceFactory})`. The ordinary production loader supplies no factory and stays closed. The client does not create or infer a production source, anchor, permission, live flag or countdown. Real supplier activation/configuration remains with root owner `01a1a153-1227-7e72-801f-a2a6b91981e1` and Product/root `01a1a1fc-7f3d-7ff1-9416-e3435c8a6f08` once BE-WALL-LIVE and canonical admission/eligibility are qualified. There is no remaining caller-assembly code required within this allocation.

Follow-up validation uses the actual production custom-load adapter, world composition, stable `WorldPanels`, entry provider, sync/editor and HTTP adapter. Unit tests use a memory engine; the new native runner uses the actual default MapScene/Phaser engine, asset decoder and one MockSocket connection. Fixture identity translation adapts the existing C04 fixture server's actor/event IDs to the established world fixture; all actual consumer decoders remain in the path. No production authorization is claimed by that fixture translation.

| Final follow-up check | Result |
| --- | --- |
| From `frontend/`, `MCR_FE14_PYTHON=<retained contracts-validation Python> node node_modules/vitest/vitest.mjs run src/world/mural-entry-lifecycle/__tests__ src/app/mural-entry/__tests__/owner.test.ts src/features/mural/editor/__tests__/controller.test.ts src/features/mural/sync/__tests__ src/world/mural/__tests__ src/world/composition/__tests__ src/world/__tests__/runtime.test.ts src/world/theme/__tests__/replacement.test.ts` | Exit 0; **385 tests / 16 files**, including seven new assembly tests. |
| From `frontend/`, `node node_modules/typescript/bin/tsc -p tsconfig.json --pretty false --incremental false` | Exit 0. |
| From `frontend/`, `node node_modules/oxlint/bin/oxlint src/world/mural-entry-lifecycle src/world/composition/index.ts src/app/world-panels/index.tsx` | Exit 0. |
| From `frontend/`, `node src/world/mural-entry-lifecycle/__tests__/assembly-native.mjs` | Exit 0; actual MapScene/Phaser at 360/390/430, plus an omitted-source closed case; zero page errors. |
| From repository root, `python3 scripts/mcr/generate-domain-transport.py --check` | Exit 0; 108 operations, after the independent shared-owner correction. |
| Native source hash verification; staged exact-path/hunk check; `git diff --check` | Pass. |

The initial broad runtime attempt ran from repository root and failed 20 existing cwd-relative asset/Python probes. Rerunning the same matrix from its intended `frontend/` cwd passed all 385; no tests or production code were weakened. The new jsdom harness also received the existing ResizeObserver stub and current `REV 10 · EDITABLE`/conditional Confirm selectors; production UI was not edited to satisfy historical selectors.

Retained evidence is `frontend/src/world/mural-entry-lifecycle/__tests__/evidence/caller/native.json`, `checks.json` and `caller-{360,390,430}.png`. The native receipt pins 18 actual source files, including both caller hunks, unchanged production loader/runtime/MapScene, provider and lifecycle. Each viewport verifies one engine/game, one socket, one factory, one wall texture/object; a single lost POST retains the original command through source withdrawal and explicit fresh qualification; Check resolves it without another POST. Privacy invalidation removes the mural, source subscriptions, world socket and engine. The omitted supplier makes no mural request/POST while World stays live. The 390px capture was visually inspected as fixture behavior evidence, not product layout/FE32/art acceptance.

Production remains unavailable without the separately qualified supplier and ordinary root activation. No real backend HTTP/DB/child recovery, admitted server socket, device, art fidelity, moderation or IN04 gate is claimed. Original FE33 frontend criteria remain SATISFIED; this follow-up closes the identified assembly gap without another task count. Owned native caches and dependency symlink are removed at final STOP.
