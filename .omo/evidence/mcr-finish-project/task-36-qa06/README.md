# QA06 mobile visual acceptance — task 36

Candidate `65b70a1e8cdd1d9a406b89efa9e10b77df5a2f93` on `codex/mcr-exec-w6-qa06`, base `7562f3b96`. Harness: `frontend/e2e/qa06-mobile/` drives the actual default router, production ports, canonical Auth, Phaser world and the selected whole-character renderer through `e2e/visual/qa06/preview.html`. HTTP/C05 are deterministic synthetic fixtures; map/theme admission is TEST ONLY. This is desktop Chrome touch-viewport and Android-emulator evidence, not physical-device acceptance.

## Counts

Desktop Chrome 156.0.8078.80, viewports 360/390/430 CSS px, height 844 (one 420 short-height case), DPR 1:

| Verdict | Cases |
| --- | --- |
| PASS | 95 |
| DIFFER | 0 |
| NOT_REACHABLE | 111 |
| FAIL | 0 |
| **Total** | **206** |

Case matrix: 61 retained IA entries (M10/R02/X02 are the optional P1 rows outside the required 61) at three widths = 183 captures, plus nine family-state cases totalling 23 captures = 206. `S02` (route error boundary) is one of the 61 IA entries, not an extra case.

Of those nine family-state cases only four add pixels. The other five are byte-identical to an IA capture at every width and are recorded as aliases with `additionalCoverage: false`, both per case and in `report.json` `familyCaptureCoverage`:

| Family state | Capture ids | Widths | Coverage |
| --- | --- | --- | --- |
| PARADE era world | W01-parade | 360/390/430 | DISTINCT — differs from `W01` at every width |
| REVENGE era HUD | PI08-revenge-era | 360/390/430 | ALIAS of `W01` — same hash at every width, no additional coverage |
| DANGER era HUD | PI08-danger-era | 360/390/430 | DISTINCT — differs from `W01` at every width |
| Feed text-only card | PI02-text-only | 360/390/430 | ALIAS of `F01` — same hash at every width, no additional coverage |
| Thread reply composer | PI03-inline-reply | 360/390/430 | ALIAS of `F03` — same hash at every width, no additional coverage |
| Shared unavailable/status chrome | PI01-status | 360/390/430 | ALIAS of `O01` — same hash at every width, no additional coverage |
| Wall grid and palette | PI09-grid | 360/390/430 | ALIAS of `R01` — same hash at every width, no additional coverage |
| Short height (420px) | PI01-short | 390 | DISTINCT — no IA capture uses 420px |
| Keyboard focus | PI01-focus | 390 | DISTINCT — no IA capture holds focus |

Only the IA captures and the four distinct family states (`W01-parade`, `PI08-danger-era`, `PI01-short`, `PI01-focus`) add pixels; `F02` and the other IA routes are already counted as IA captures. The 23 family captures therefore reduce to 8 distinct + 15 alias captures, and the five alias states (retained, not deleted) certify that the named state renders the same geometry as the IA capture it repeats. The per-family case-verdict counts below include those alias captures.

Android sdk_gphone64_arm64, Android 15 (SDK 35), Chrome 124.0.6367.219: 36 captures, 36 captured, 0 failed. Qualification: `ANDROID_15_EMULATOR_CHROME_NOT_PHYSICAL`. The same 36 entries are recorded in `report.json` under `androidEmulator.cases` with totals `36 captured / 0 failed`, so the report and this README agree. These emulator captures are from the earlier capture session (provenance block: earlier-session, candidate not claimed) and were not re-run here because no emulator was attached; the desktop matrix above was re-produced at the candidate shown. The Android captures are not a claim about candidate `65b70a1e8`.

## Checks

- Vitest, changed-file scope for the delivered source diff (`npx vitest run --changed 7562f3b96` from `frontend/`): 50 test files, 464 tests, all passed (exit 0). The earlier handoff note's 549-test figure was not reproduced; 464 is the measured count. **This count requires pinned Node v24.22.2** (`/tmp/mcr-node24/bin/node --version` → `v24.22.2`, the `node` field in `report.json`). Under system Node 26.7.0 every operations-composition run over that scope failed with 86 passed / 1 failed, always on an abort assertion: `aborts pending settings and ignores its late completion after canonical policy replacement` (`src/app/operations-composition/__tests__/composition.test.tsx`), and in one multi-file run `aborts a pending list read and rejects its late completion across policy change` (`src/app/operations-composition/__tests__/reports-production/composition.test.tsx`). One standalone v24.22.2 whole-directory run also failed `aborts pending settings` (86 passed / 1 failed) before two later v24.22.2 runs passed 87/87, so the assertion is timing-sensitive; the recorded 464 still reproduces under v24.22.2 (50 files, 464 tests passed on the changed-file scope).
- `tsc -b`: exit 0. `python3 scripts/mcr/check-plan.py`: PASS, exit 0.

## 12 design families

| Family | Scope | Family verdict | Case verdicts |
| --- | --- | --- | --- |
| PI01 | Shared tokens, frame, fields, tabs, buttons, dialogs, statuses | PARTIAL | NOT_REACHABLE 3 · PASS 2 |
| PI02 | Feed, authors, media/text-only cards, notices, reaction row, toolbar | PASS | PASS 9 |
| PI03 | Thread, inline replies, composers, edit/delete states | PASS | PASS 12 |
| PI04 | Giveaway discovery/detail/own activity, authoring, reservation, pickup | NOT_REACHABLE | NOT_REACHABLE 21 |
| PI05 | Meet discovery, applications, create/manage/rally, People, introductions | PARTIAL | NOT_REACHABLE 18 · PASS 6 |
| PI06 | Own/public Profile, profile edit, activity and preferences | PASS | PASS 12 |
| PI07 | Portrait framing, avatar popover, complete avatar-editor controls | PASS | PASS 3 |
| PI08 | World HUD, viewport/context strip, six-tab navigation, eras | PASS | PASS 15 |
| PI09 | Working Wall B editor/palette/tools/confirmation, sync, degraded states | NOT_REACHABLE | NOT_REACHABLE 6 |
| PI10 | Landing, sign-in/signup/verification/reset/callback/legal/deleted | PARTIAL | PASS 12 · NOT_REACHABLE 12 |
| PI11 | Map/place/rooms/nearby/check-in/help presentation | PARTIAL | NOT_REACHABLE 9 · PASS 9 |
| PI12 | Social/privacy/settings links, inbox, reports, recovery, operations, archive | PARTIAL | NOT_REACHABLE 42 · PASS 15 |

## 61 retained IA entries

| IA | Route | Verdicts 360/390/430 |
| --- | --- | --- |
| A01 | / | 360:PASS 390:PASS 430:PASS |
| A02 | /login | 360:PASS 390:PASS 430:PASS |
| A03 | /signup | 360:NOT_REACHABLE 390:NOT_REACHABLE 430:NOT_REACHABLE |
| A04 | /verify-email | 360:NOT_REACHABLE 390:NOT_REACHABLE 430:NOT_REACHABLE |
| A05 | /reset-password | 360:PASS 390:PASS 430:PASS |
| A06 | /auth/callback | 360:NOT_REACHABLE 390:NOT_REACHABLE 430:NOT_REACHABLE |
| A07 | /legal/privacy | 360:NOT_REACHABLE 390:NOT_REACHABLE 430:NOT_REACHABLE |
| A08 | /account/deleted | 360:PASS 390:PASS 430:PASS |
| F01 | /feed | 360:PASS 390:PASS 430:PASS |
| F02 | /feed/new | 360:PASS 390:PASS 430:PASS |
| F03 | /feed/00000000-0000-4000-8000-00000000003c | 360:PASS 390:PASS 430:PASS |
| F04 | /feed/00000000-0000-4000-8000-0000000000aa/edit | 360:PASS 390:PASS 430:PASS |
| F05 | /notices/00000000-0000-4000-8000-000000000060 | 360:PASS 390:PASS 430:PASS |
| G01 | /gifts | 360:NOT_REACHABLE 390:NOT_REACHABLE 430:NOT_REACHABLE |
| G02 | /gifts/new | 360:NOT_REACHABLE 390:NOT_REACHABLE 430:NOT_REACHABLE |
| G03 | /gifts/00000000-0000-4000-8000-000000000005 | 360:NOT_REACHABLE 390:NOT_REACHABLE 430:NOT_REACHABLE |
| G04 | /gifts/00000000-0000-4000-8000-000000000005/manage | 360:NOT_REACHABLE 390:NOT_REACHABLE 430:NOT_REACHABLE |
| G05 | /gifts/00000000-0000-4000-8000-000000000005/reservation | 360:NOT_REACHABLE 390:NOT_REACHABLE 430:NOT_REACHABLE |
| G06 | /gifts/pickups/00000000-0000-4000-8000-000000000041 | 360:NOT_REACHABLE 390:NOT_REACHABLE 430:NOT_REACHABLE |
| G07 | /gifts/mine | 360:NOT_REACHABLE 390:NOT_REACHABLE 430:NOT_REACHABLE |
| M01 | /meet | 360:NOT_REACHABLE 390:NOT_REACHABLE 430:NOT_REACHABLE |
| M02 | /meet/parties/new | 360:NOT_REACHABLE 390:NOT_REACHABLE 430:NOT_REACHABLE |
| M03 | /meet/parties/00000000-0000-4000-8000-000000000010 | 360:NOT_REACHABLE 390:NOT_REACHABLE 430:NOT_REACHABLE |
| M04 | /meet/parties/00000000-0000-4000-8000-000000000010/manage | 360:NOT_REACHABLE 390:NOT_REACHABLE 430:NOT_REACHABLE |
| M05 | /meet/parties/00000000-0000-4000-8000-000000000010/rally | 360:NOT_REACHABLE 390:NOT_REACHABLE 430:NOT_REACHABLE |
| M06 | /meet/beacons/00000000-0000-4000-8000-000000000070 | 360:NOT_REACHABLE 390:NOT_REACHABLE 430:NOT_REACHABLE |
| M07 | /meet/people | 360:PASS 390:PASS 430:PASS |
| M08 | /meet/intro/edit | 360:PASS 390:PASS 430:PASS |
| M09 | /people/00000000-0000-4000-8000-000000000002 | 360:PASS 390:PASS 430:PASS |
| O01 | /admin | 360:NOT_REACHABLE 390:NOT_REACHABLE 430:NOT_REACHABLE |
| O02 | /admin/event | 360:NOT_REACHABLE 390:NOT_REACHABLE 430:NOT_REACHABLE |
| O03 | /admin/places | 360:NOT_REACHABLE 390:NOT_REACHABLE 430:NOT_REACHABLE |
| O04 | /admin/reports | 360:NOT_REACHABLE 390:NOT_REACHABLE 430:NOT_REACHABLE |
| O05 | /admin/users/00000000-0000-4000-8000-000000000002 | 360:NOT_REACHABLE 390:NOT_REACHABLE 430:NOT_REACHABLE |
| O06 | /admin/content | 360:NOT_REACHABLE 390:NOT_REACHABLE 430:NOT_REACHABLE |
| O07 | /admin/mural | 360:NOT_REACHABLE 390:NOT_REACHABLE 430:NOT_REACHABLE |
| O08 | /admin/audit | 360:NOT_REACHABLE 390:NOT_REACHABLE 430:NOT_REACHABLE |
| O09 | /admin/archive | 360:NOT_REACHABLE 390:NOT_REACHABLE 430:NOT_REACHABLE |
| R01 | /wall | 360:NOT_REACHABLE 390:NOT_REACHABLE 430:NOT_REACHABLE |
| S01 | /report?targetType=POST&targetId=00000000-0000-4000-8000-00000000003c | 360:NOT_REACHABLE 390:NOT_REACHABLE 430:NOT_REACHABLE |
| S02 | /feed | 360:PASS 390:PASS 430:PASS |
| S03 | /world | 360:PASS 390:PASS 430:PASS |
| U01 | /me | 360:PASS 390:PASS 430:PASS |
| U02 | /me/avatar | 360:PASS 390:PASS 430:PASS |
| U03 | /me/profile | 360:PASS 390:PASS 430:PASS |
| U04 | /me/social | 360:NOT_REACHABLE 390:NOT_REACHABLE 430:NOT_REACHABLE |
| U05 | /me/requests | 360:NOT_REACHABLE 390:NOT_REACHABLE 430:NOT_REACHABLE |
| U06 | /me/age | 360:PASS 390:PASS 430:PASS |
| U07 | /me/settings | 360:PASS 390:PASS 430:PASS |
| U08 | /me/blocks | 360:NOT_REACHABLE 390:NOT_REACHABLE 430:NOT_REACHABLE |
| U09 | /me/account | 360:PASS 390:PASS 430:PASS |
| U10 | /notifications | 360:PASS 390:PASS 430:PASS |
| W01 | /world | 360:PASS 390:PASS 430:PASS |
| W02 | /world/map | 360:NOT_REACHABLE 390:NOT_REACHABLE 430:NOT_REACHABLE |
| W03 | /world/places/00000000-0000-4000-8000-000000000030 | 360:NOT_REACHABLE 390:NOT_REACHABLE 430:NOT_REACHABLE |
| W04 | /world/rooms | 360:PASS 390:PASS 430:PASS |
| W05 | /world/nearby | 360:PASS 390:PASS 430:PASS |
| W06 | /world/theme | 360:PASS 390:PASS 430:PASS |
| W07 | /world/check-in | 360:NOT_REACHABLE 390:NOT_REACHABLE 430:NOT_REACHABLE |
| W08 | /world/help | 360:PASS 390:PASS 430:PASS |
| X01 | /archive | 360:NOT_REACHABLE 390:NOT_REACHABLE 430:NOT_REACHABLE |

## Material differences fixed

1. **Profile edit footer link target** — `frontend/src/features/me/ui/MePages.tsx:39`: the "Back to world" link measured 21px high at all widths. It now uses the shared `shell-link` class (44px minimum, glyph size unchanged), verified at 360/390/430.
2. **Admin legacy access action target** — `frontend/src/app/shell.css:54`: the native "Check current access" button measured 25px high. `.shell-actions > button` now enforces a 44px minimum, verified at 360/390/430.

Both were the only automated target-size findings; after the fixes the audited set has zero `TARGET_UNDER_44` findings.

## Remaining material differences (recorded, not fixable in this scope)

- **Landing (PI10/FE08)**: the supplied ten-screen poster uses a distressed band wordmark, rose, moon/skyline and four-character skyline. The current app deliberately ships an independent fan-project title ("ONE WORLD. EVERY ERA."), the four selected source-pack characters, a crimson entry action and a legal/status block; brand-specific reference art is not reintroduced. The poster ends mid-viewport at 390/430 with plain paper below.
- **World (PI08)**: the target's festival scene, banners and crowd density are not the current studio scene supplied by the reviewed local input; HUD frame, era tabs, context strip and six-tab navigation match the Parade Ink anatomy. The reference level/points chip is omitted because no leveling system exists.
- **Feed (PI02/PI03)**: the target's illustrated media post cannot be rendered because no media/upload provider is composed; text-only cards, author row, reaction/share row and bottom composer match. Second post is test-fixture content.
- **Customize (PI07)**: the selected whole-character producer exposes a preset carousel (1–3/6, Random/Reset/Save), not the reference right-hand category rail and hair palette; layered per-part editing is not an available authority.
- **Profile / People (PI06/PI05)**: recent activity rows and other members' portraits are unavailable because activity/share/avatar providers are not composed; the current UI states this explicitly instead of fabricating rows.
- **Short height (PI01)**: at 420px the feed keeps its title, tabs and footer visible with the post list scrolling; no clipping or horizontal overflow.

## NOT_REACHABLE cases and their blocking producer

| Blocking producer | IA cases |
| --- | --- |
| email signup/verification provider not composed in this fixture | A03, A04 |
| OAuth provider sign-in not composed in this fixture | A06 |
| approved legal document copy not supplied | A07 |
| reviewed siteplan + landmark provider not composed in this fixture | W02 |
| reviewed landmark/place detail provider (place response rejected by the places entry contract) | W03 |
| reviewed landmark/place provider not composed | W07, M06 |
| community clock/entry authority not composed; gift and party data cannot be admitted | M01, M03, G01, G03, G07 |
| community entry release/clock authority not composed (no VITE_MCR_COMMUNITY_CLOCK_INPUT qualification) | M02, M04, M05, G02, G04, G05, G06 |
| qualified world mural-scene release/anchor not composed | R01, PI09-grid |
| requests/social provider not composed for this fixture | U04, U05 |
| safety service producer (blocks/report transport) not composed in this fixture | U08, S01 |
| published archive content producer not supplied | X01 |
| operator role/event settings server authority not available to a participant fixture | O01, O02, O03, O04, O07, O08, O09, PI01-status |
| operator/moderator review authority not available to a participant fixture | O05, O06 |

## NOT_RUN

- **Physical iOS Safari**: no iOS simulator or physical device is attached (`xcrun simctl list devices booted` returned none). Desktop viewport resizing is not substituted.
- **Physical Android Chrome**: no physical Android device is attached. The Android 15 emulator with Chrome 124.0.6367.219 is used and labelled as emulator evidence.
- **Software keyboard and hardware safe-area insets**: not exercised; the captures use a zero-inset desktop/emulator viewport. Keyboard-open and notch/home-indicator behaviour remain NOT_RUN.
- **Live services and real multiuser journeys**: synthetic HTTP/C05 only; no live server, database or second account.

## Reproduction

```sh
# from the worktree root with the retained dependency tree linked into frontend/node_modules.
# Pinned Node v24.22.2 only: `node --version` must print v24.22.2.
# System Node 26.7.0 changes the operations-composition abort results (see Checks).
PATH=/tmp/mcr-node24/bin:$PATH QA06_MOBILE_OUTPUT=$PWD/.omo/evidence/mcr-finish-project/task-36-qa06 \
  node frontend/e2e/qa06-mobile/native.mjs
PATH=/tmp/mcr-node24/bin:$PATH QA06_MOBILE_OUTPUT=$PWD/.omo/evidence/mcr-finish-project/task-36-qa06 \
  node frontend/e2e/qa06-mobile/compare.mjs
```

The Android pass additionally requires a booted emulator, `adb reverse tcp:5319 tcp:5319` and `adb forward tcp:9222 localabstract:chrome_devtools_remote`; run `android.mjs` after those forwards.

## Evidence files

- `report.json` — 206 desktop cases with case id, family/IA, width, capture path, terminal `status` and `verdict`, measurements, blocking producer and material difference where applicable; plus `familyCaptureCoverage` (distinct vs alias family captures) and `androidEmulator` with the 36 Android entries and their own earlier-session provenance block.
- `captures/` — 206 desktop PNGs + 36 Android PNGs.
- `sha256.txt` — capture hashes for all 242 PNGs.
- `comparison.html` — same-aspect target crops, Parade Ink component crops, family and IA tables.
- `reference-crops/` — developer-measured crops of the unchanged references.
- `audit-pass/` — the pre-fix audit run (U03/O05 target-size findings) retained for provenance.
- `android.json` — emulator device, Chrome version, per-case captures and an earlier-session provenance block (`candidateClaim: NONE`).
- `venue-state-map.json` — the 48 venue state-map cases with their QA06 NOT_REACHABLE disposition and blocking inputs.
- `harness/` and `harness-sha256.txt` — frozen copies and hashes of the capture harness used for this run.
