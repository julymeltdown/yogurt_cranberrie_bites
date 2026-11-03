# FE19 independent integration review

Status: **Scoped review PASS; R1/R2 closed on final candidate, awaiting parent disposition.** This is a bounded implementation review, not FE19/BE08/QA07 acceptance.

## Candidate and ownership

Review checkout `/Users/lhs/Desktop/mcr-worktrees/fe-places-review`, branch `codex/mcr-fe-FE19-review`. Integration base `6716ed7d9796586d7f2b52980ed2a2fcab1ab2f8`; reviewed candidate `1ba3f7f6b0bb568524fd88ede22558ad6fd163fc`, the cherry-pick of worker `072c817fbf9087982a7ca93da3b8a0ad83b8870f` from worker base `bed5ffa8cb52f5dc337bcde7d50a428f9bce74f9`. All 14 changed paths match the worker assignment. This report is the reviewer's sole tracked write; implementation fixes belong to the feature owner. No push.

Read the FE19/W02/W03/W08 specification, assessment/execution agreements, all 14 delivered paths enumerated in the receipt, generated C03 place operations/shapes, C06 provenance/map definitions, canonical SessionPort/SessionController and HTTP cancellation, FE14 map validation/runtime, FE15 camera/input, and shell route composition. No cross-directory backend implementation import or prohibited decorative left accent was found in the delivered UI. World/camera reuse and scope behavior were inspected at source level; this is not fresh production composition evidence.

## R1 [P2] — detail failures allow stale map targets after returning to overview

**Reproduced on initial candidate; closed by final verification below.** `frontend/src/features/places/model/controller.ts:125` publishes only a detail error after 404/410 or `409 MAP_VERSION_MISMATCH`. It retains the ready catalog and `mapMismatch: false`. `PlaceDetail` cleanup at `frontend/src/features/places/ui/Places.tsx:125` and overview mount at line 65 clear selection. The selected-detail guard in `controller.ts:160–166` then no longer applies, so the retained OPEN catalog entry becomes targetable again. `showOnMap` at line 185 calls `camera.lookAt` for that stale entry. For map mismatch even camera controls remain available despite the detail's recovery message.

Reachable sequence: load the overview, open a previously listed landmark after the server has removed it or invalidated its map, receive the detail error, then use All places or browser Back and Show on map. This does not create physical arrival, but contradicts removed/mismatch recovery and reuses information the later authoritative response invalidated. Latch target invalidation across panel navigation until a successful catalog/release reconciliation; preserve readable metadata only with truthful stale/recovery presentation. Add a bounded regression for detail error → back → map action and successful recovery.

Independent failure reproduction used the actual `createMemorySession`, `createHttpClient`, controller, decoder and synthetic C06 release via Vite SSR. All three responses (404, 410, 409 map mismatch) produced `canShow: true` and `showOnMap: true` after `select(null)`, with a recorded `lookAt({x:80,y:48})`. This is a controller failure reproduction, not a native browser/camera execution. The probe exits 0 because it records the reproduced behavior; that is not a product pass.

## R2 — map reload recovery (feature-owner finding)

Parent relayed a separate worker finding: Reload places only refetches the catalog and cannot recover a stale live event map. Source corroborates that `Places.tsx` recovery invokes only `controller.refresh()`, and `PlaceMapPort` has no reload command. The feature owner is preparing an explicit event/browser reload action and native proof. At the initial checkpoint this review had not executed that recovery. Final verification below independently executes the delivered action; no root route or composition edits were made here.

## Evidence checked

Receipt `/tmp/mcr-fe19-handoff/receipt.json` actual SHA-256 `ea175f2f5f1caf88a784db140cd909cfa50e54397c4ba5496e2a7e99a8ea1eba`. Byte counts and hashes independently match all 14 delivered files in this checkout, all nine command artifacts, five native artifacts, nine historical artifacts, and 15 of 16 inherited inputs. The one inherited difference is `frontend/src/world/runtime.ts`: review base includes the separately reviewed FE06 atomic skin API. C03/C06 contract bytes match. Worker browser results therefore remain worker evidence on their recorded source; they are not a fresh run on this integration candidate. Verification detail: `/tmp/mcr-fe19-review/receipt-verification.json` (SHA-256 `b761658927a812c9a0ebbc6457d24209c9f229dda07dd5591454edf0fdc3e802`).

Worker native report records Chrome 156.0.8078.80 at 360×800, 390×844, 430×932, persistent canvas, unchanged avatar foot, camera changes, keyboard disclosure/navigation, browser-Back focus, >=44px targets and cleanup. Its runner and UI tests were inspected. Overview/detail 390px screenshots were visually inspected independently; no left accents or obvious horizontal overflow appear in those views. Native runs' historical failures remain preserved in the receipt. No claim that static screenshots prove interactive accessibility.

Independent commands:

- Pinned Node 24.22.2 and pnpm 11.7.0 `install --offline --frozen-lockfile` in assigned frontend: exit 0, `/tmp/mcr-fe19-review-install.txt`; no manifest/lock change.
- `/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node /tmp/mcr-fe19-review/probe.mjs`: exit 0, reproduced R1; `/tmp/mcr-fe19-review/probe.log` and `probe.json`. No harness failure occurred in this probe.
- SHA-256/byte verification script: exit 0, differences described above. `git diff 6716ed7..HEAD --check`: exit 0. Import/accent source scan matched only test prose saying no backend.

Probe SHA-256: script `6ec7681bd5c1fca594cabc9d0ea90959afa542e66fbe8f5a94f9de35e8af5b7b`; log `a24189c97120960584ee21d95b21ee07d2ef15924aa291ab69d08514419e8a15`; JSON `1dfe120001156de0e9e4a0e958edde2c8329d67b31b4c719980fe0430884ac75`.

## Other reviewed gates and limits

Canonical session generations are published before old signals abort; the feature rebinds the new signal, clears old list/detail and fences ignored cancellations by generation, read ticket, release identity and selected ID. Map snapshots require the captured owner user/event/generation and separate expected map identity; catalog revision is not map revision. Catalog/detail full-item equality rejects divergent closure/provenance. Closed catalog or map places cannot target. The injected camera only changes view; feature code creates no world, socket, movement or arrival action. HTTPS source links reject credentials, unsafe schemes and raw whitespace/control/backslash inputs, use no-referrer/new-tab isolation, and never render unsafe source URIs. Concept-only and unverified/closed/source-unavailable states are explicit. No photo URL is invented from photo permission; contract lacks photo and route fields.

No additional actionable source finding at the initial checkpoint. Root map-parent/header-Back behavior is documented and integrator-owned. Production owner composition, real BE08 producer, authoritative multiplayer, approved venue provenance/photos, real devices and complete FE19/QA07 acceptance remain NOT_RUN. Broad 56-test/build repetition, Cargo and PostgreSQL were not run. Goal remains ACTIVE through parent review, implementation fixes and focused verification.


## Final fix review and disposition

Final application candidate **`5a56ee9354e483b43094cceed11b63abd1aa4639`** is the parent-applied cherry-pick of worker fix **`b52837429bf82d1524300bd5456b443f06465729`**, following this review's initial report commit `458ccf45729812e2b78e49ab9a4e9a33ec7b22e6`. All nine follow-up paths were inspected. Parent applied application changes; reviewer authored only this report. No application changes or push by reviewer.

**R1 closed.** The original independent failure logic was replayed on the final review checkout; only candidate metadata and output location were changed. All 404/410/409 after-Back target calls now return false. Missing-place invalidation is local to the target; map-version invalidation is release-wide. An additional bounded independent lifecycle probe asserts:

- 404/410 remains disabled after unselect/Back and same-catalog retry; unrelated target and camera remain available. Matching successful detail reconciles the missing target.
- 409 map-version rejection disables target, unrelated targets, camera availability, recenter and zoom. Same-pin retry cannot clear it. A different but mismatched map/catalog pair cannot clear it; a new matching pair can. Catalog revision 7 remains distinct from map revision 2.
- Canonical owner replacement clears former invalidations/data, rejects the old captured map scope, and permits a newly owner-bound release. Disposal clears metadata and invalidations, removes the map subscription, rejects subsequent actions and starts no further reads.

Both independent scripts exit 0. The original pre-fix script/log/JSON were preserved before replay under `/tmp/mcr-fe19-review-final/baseline/`; original files were not overwritten. The worker's `/tmp/mcr-fe19-r1-probe.*` is explicitly worker replay, not this independent closure.

**R2 closed within the synthetic fixture boundary.** Independently authored `reload.mjs` exercised only the actual Reload event action at 390×844 on the final integration review candidate, using installed Chrome 156.0.8078.80, the existing shell, actual FE14/FE15/Phaser runtime and deterministic responses. It proved a visible mismatch with disabled target, unchanged avatar before reload, changed document UUID after clicking Reload event, newly ready map owner, cleared mismatch, re-enabled target, one current canvas, no horizontal page overflow or browser page errors, and canvas/control cleanup. Exit 0; `reload.json`, before/after screenshots and logs are preserved. Those screenshots were visually inspected, including ordinary symmetric error border/focus outline; no decorative left accent was found. The test host serves the existing preview fixture at `/world/map` to permit a genuine document reload; it does not certify production route/bootstrap/Auth composition or a real changed server release. A reload can require sign-in because tokens remain in memory.

The first narrow browser attempt exited 1 before browser launch: dynamic import of the CommonJS-resolved Playwright entry did not expose `chromium`. It is preserved as `first-reload.{mjs,log,json}`. The corrected harness uses `require('@playwright/test')`; no application fix was needed for this harness failure. An earlier read-only diff command also hit zsh glob expansion on unquoted `HEAD^` (exit 1); the quoted command succeeded. Neither is an application failure.

Final worker receipt `/tmp/mcr-fe19-r1/receipt.json` independently matches SHA-256 **`4ed94eca4252a023fbffa92f68e0b215ef7da887e9304a1ada185ed9c9b11e59`**. Actual bytes/hashes match all 14 final delivery files, 10 check artifacts, five native artifacts, 19 historical artifacts and five review-evidence artifacts. Fifteen of 16 named read-only inputs match; `runtime.ts` retains the expected FE06 baseline difference. The native report's full 138-source hash table was also checked: 135 match; the exact three differences are `frontend/src/world/map/tiled.ts`, `frontend/src/world/runtime.ts`, and `frontend/src/world/scenes/MapScene.ts`. All three are inherited from the reviewed FE06 integration between worker base and review base, not FE19 changes. All three native-report contract hashes match. The new narrow native probe executes this review candidate with those inherited differences; broad worker evidence is not relabeled as a fresh integration run.

Final evidence directory `/tmp/mcr-fe19-review-final/`:

| Artifact | Actual SHA-256 |
|---|---|
| `review-receipt.json` (candidate, 154 current source hashes, artifact hashes, command exits) | `3b4f275d71cf661213f4f5ae73cc19fb1e106911f1c6dc1d670854463dc75329` |
| `receipt-verification.json` | `48ea3267dfbbb1e41077228559117bfb297898dea3afbf0955f3fba9aec406b5` |
| `original-replay.mjs` | `e8ebcb306176d4533c285e4cbc7b5971ac02d6b7d6687e005a48a9a988a7e71a` |
| `original-replay.json` | `e5865e65b3b2b374fe83fd1a037763387f9ced8cf33f2181fd767c553580ced8` |
| `lifecycle.mjs` | `fd2b930892c90d40b07600e6a36de85ff2e5dbf35734830e0b07521cf3649362` |
| `lifecycle.json` | `6c9fe4d10c425f9c527225c965993faf9f74b493c5d5a07d51b3853bec68008c` |
| `reload.mjs` | `51ce0eb6565f8869b4590ba9fbff1e44cbf21cee2588856bd301f4d0de867b0e` |
| `reload.json` | `2df978f99584c66fcd73de83bffa692f9cfa229c8ac354f16ada76dbe78ee428` |

Commands executed from the assigned review checkout: pinned Node 24.22.2 with each absolute script path above (`original-replay.mjs`, `lifecycle.mjs`, `reload.mjs`), each exit 0. Final `git diff --check` and report-only staged-path inspection passed before commit. No duplicate broad 61-test/build run, Cargo or PostgreSQL execution. Worker 61-test/build/lint/native results remain byte-verified worker evidence.

**Recommendation:** integrate the reviewed FE19 implementation/fix within its stated MOCK_READY boundary. No open actionable finding in this bounded review. Full FE19/QA07 acceptance, production shell/map/Auth composition, actual BE08/venue facts/photos, authoritative multiplayer and real devices remain NOT_RUN. Parent retains publishing and final disposition. Review goal stays ACTIVE until that disposition is confirmed.
