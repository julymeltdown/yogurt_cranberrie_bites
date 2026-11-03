# FE19 independent integration review

Status: **IN_PROGRESS — hold for review fixes**. This is a bounded implementation review, not FE19/BE08/QA07 acceptance.

## Candidate and ownership

Review checkout `/Users/lhs/Desktop/mcr-worktrees/fe-places-review`, branch `codex/mcr-fe-FE19-review`. Integration base `6716ed7d9796586d7f2b52980ed2a2fcab1ab2f8`; reviewed candidate `1ba3f7f6b0bb568524fd88ede22558ad6fd163fc`, the cherry-pick of worker `072c817fbf9087982a7ca93da3b8a0ad83b8870f` from worker base `bed5ffa8cb52f5dc337bcde7d50a428f9bce74f9`. All 14 changed paths match the worker assignment. This report is the reviewer's sole tracked write; implementation fixes belong to the feature owner. No push.

Read the FE19/W02/W03/W08 specification, assessment/execution agreements, all 14 delivered paths enumerated in the receipt, generated C03 place operations/shapes, C06 provenance/map definitions, canonical SessionPort/SessionController and HTTP cancellation, FE14 map validation/runtime, FE15 camera/input, and shell route composition. No cross-directory backend implementation import or prohibited decorative left accent was found in the delivered UI. World/camera reuse and scope behavior were inspected at source level; this is not fresh production composition evidence.

## R1 [P2] — detail failures allow stale map targets after returning to overview

**Open on reviewed candidate.** `frontend/src/features/places/model/controller.ts:125` publishes only a detail error after 404/410 or `409 MAP_VERSION_MISMATCH`. It retains the ready catalog and `mapMismatch: false`. `PlaceDetail` cleanup at `frontend/src/features/places/ui/Places.tsx:125` and overview mount at line 65 clear selection. The selected-detail guard in `controller.ts:160–166` then no longer applies, so the retained OPEN catalog entry becomes targetable again. `showOnMap` at line 185 calls `camera.lookAt` for that stale entry. For map mismatch even camera controls remain available despite the detail's recovery message.

Reachable sequence: load the overview, open a previously listed landmark after the server has removed it or invalidated its map, receive the detail error, then use All places or browser Back and Show on map. This does not create physical arrival, but contradicts removed/mismatch recovery and reuses information the later authoritative response invalidated. Latch target invalidation across panel navigation until a successful catalog/release reconciliation; preserve readable metadata only with truthful stale/recovery presentation. Add a bounded regression for detail error → back → map action and successful recovery.

Independent failure reproduction used the actual `createMemorySession`, `createHttpClient`, controller, decoder and synthetic C06 release via Vite SSR. All three responses (404, 410, 409 map mismatch) produced `canShow: true` and `showOnMap: true` after `select(null)`, with a recorded `lookAt({x:80,y:48})`. This is a controller failure reproduction, not a native browser/camera execution. The probe exits 0 because it records the reproduced behavior; that is not a product pass.

## R2 — map reload recovery (feature-owner finding)

Parent relayed a separate worker finding: Reload places only refetches the catalog and cannot recover a stale live event map. Source corroborates that `Places.tsx` recovery invokes only `controller.refresh()`, and `PlaceMapPort` has no reload command. The feature owner is preparing an explicit event/browser reload action and native proof. This review has not independently executed that recovery. Hold final disposition until the follow-up arrives; no root route or composition edits are authorized here.

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

No additional actionable source finding at this checkpoint. Root map-parent/header-Back behavior is documented and integrator-owned. Production owner composition, real BE08 producer, authoritative multiplayer, approved venue provenance/photos, real devices and complete FE19/QA07 acceptance remain NOT_RUN. Broad 56-test/build repetition, Cargo and PostgreSQL were not run. Goal remains ACTIVE through parent review, implementation fixes and focused verification.
