# QA02 — zero legacy UI origin audit

Original QA02 criteria: **SATISFIED at `f0d0663a4d95361eea74040a8dee28598e9a7ff9`**. No reused legacy control, layout or theme was found in the scoped source and native presentation review. This closes the audit at this candidate; it does not certify product fidelity, feature readiness, QA01, QA06, live services, devices or release acceptance.

The user-authorized checkout is the preserved `fe-ref-public` worktree on `codex/mcr-qa-QA02-ui-origin`. Base: `0a8c422645458c9397af9289e15089ee3f9cb0ad`. The active unbudgeted QA02 goal covers evidence, validation, required fixes and committed publication. Exclusive writes are `docs/mcr/evidence/ui-origin/**`, plus the later exact support-file grant `scripts/mcr/tests/ui-origin.test.mjs`. No checker, policy, allowlist, application, manifest, other tests, generated files or QA01 coverage was edited. Kings and SLs are allocation/bookkeeping contacts, not technical review gates.

## Source and comparison

The pinned legacy source is `julymeltdown/bigyeting@5eecdcaffd1a9d505b9ab2a974f8d5623217e90a` (710 frontend blobs). The complete candidate scan reads **3498 tracked frontend blobs from Git**, including sparse-omitted PNGs: zero matches with all 422 excluded legacy hashes. The existing static import/hash checker separately passes on materialized code; its omissions and lexer limitations remain explicit in the C08 documentation. Four already-reviewed FE03 headless extracts are retained, with source/destination hashes, symbols and regression references unchanged.

The supplemental comparison tokenizes 155 non-test JSX/style files (173,288 tokens), removing whitespace/comments and looking for shared windows of 32 consecutive tokens against legacy JSX/styles. It found zero candidates. This is supporting evidence, not a proof of originality. Manual source inspection covers both routers, old/new buttons, legacy global tokens versus MCR base/era tokens, the new ConcertWindow/icon/shell composition, account/feature presentation and the Phaser MapScene boundary. The new button adds its own busy/disabled/ARIA handling and mcr classes; it does not import or reproduce the old buttonStyles/size system. Symmetric perimeter scuffs and focus outlines were checked separately from prohibited left accents; no decorative left rail was found.

Historical legacy screenshots were opened from the exact pinned source, not rerun or relabeled as fresh legacy execution. Compared views: Tchinso dark home feed, light login and light account/posts. The inspected working files agree byte-for-byte with their Git blobs; [input pins](inputs.json) record source paths and SHA-256 values. The approved MCR reference is `docs/mcr/design/reference-target-2026-11-04.png`; the selected character source pack remains revision 3. No source archive, supplied original, legacy fallback or product art was modified.

## Manual visual decisions

| Surface family | Comparison and decision |
|---|---|
| Public / sign in | The fresh poster and cemetery/print sign-in visibly follow the supplied MCR direction. They do not reuse the old Tchinso brand header, purple next-step form, provider stack or spacing. Reviewed at 360, 390 and 430px. |
| World / map / customization | Current native Phaser world uses selected characters, concert navigation and MCR scene art. The shell has six labeled tabs and three eras. It does not reproduce the old five-tab social shell or theme. Current reference-scene and actual default-loader views were recaptured after peer reconciliation. Source-resolved scene fixtures do not certify backend admission. |
| Feed / meet / gifts / profile / wall | Reviewed framed lists, new authoring forms, selected-portrait profile, giveaway rows, mural palette and DOM controls. Black/cream/red concert framing differs from old Tchinso feed/account layouts. Avatar-unavailable labels and question-mark placeholders were reviewed as unavailable states, not approved replacement art. Some secondary forms remain plain or dark: new-origin review is not a fidelity pass. |
| Safety / account / operations / archive | Reviewed new field, table, confirmation/progress and read-only archive presentations. All nine current admin routes correctly show the current authorization boundary in this fixture; isolated operations/moderation previews cover the implemented privileged presentation without pretending those routes are integrated. |
| Errors / maintenance | Actual route error boundary and seven actual ConnectionStatusNotice view states were rendered and reviewed. Plain-language labels, symmetric borders and new actions are used. These props are deterministic and do not test network recovery. |

All six original review sheets and the fresh current route sheets were opened and inspected; [supplemental sheet](state-review.jpg) covers the additional states. Critical poster, native world, primitives and maintenance images were also inspected at full size. No screenshots were blindly accepted or promoted to golden tests.

## Validation and limitations

| Command / run | Exit and evidence |
|---|---|
| `node scripts/mcr/check-ui-origin.mjs` | 0; [static.json](static.json), zero violations. Materialized code + assets only; full Git-byte scan supplements sparse coverage. |
| `python3 docs/mcr/evidence/ui-origin/source-audit.py` | 0; [source-audit.json](source-audit.json), complete tracked byte scan and normalized source comparison. |
| Original `node --test scripts/mcr/tests/ui-origin.test.mjs` | 1; 13/14, [initial failure](static-tests.log). Stale shipped-policy assertion expected an empty allowlist. |
| Same suite after exact support-file fix | 0; 14/14, [fixed tests](static-tests-fixed.log). The approved four destination set, inventory/source/destination hashes and regression paths are checked. Existing rejection tests for copied assets, renamed controls, prohibited imports, visual allowlist sources/destinations and missing provenance remain intact. No policy/checker relaxation. |
| `node --check` for support test and capture scripts; `git diff --check` | 0. |
| `python3 scripts/mcr/check-plan.py` | 0; [plan.log](plan.log). Planning/provenance only. |
| Initial `capture.mjs` | 0 as an evidence driver, not an all-preview test pass: 59 routes plus 81 supplementary cases attempted; 8 missing captures and 1 captured preview runtime error are NOT_RUN. [Raw report](browser.json). The generic driver does not supply every feature HTTP harness. |
| `QA02_ROUTES_ONLY=1 node …/capture.mjs` after peer merge | 0; [current report](browser-current.json), 59/59 routes + 4 mobile public/auth captures, no page errors or horizontal route overflow. |
| `node …/state-capture.mjs`; `node …/review-index.mjs` | 0; [states.json](states.json), 24 supplemental captures with no page errors; actual primitives/control layout, four legal documents, privileged operation variants, error/query routes, seven notice states, current source-resolved scene and three default-loader era views. |

The initial component corpus is pinned to the base candidate. Between base and the refreshed candidate, the sole production frontend change is `app/world-entry/reference-scene-overview.ts`: it now recognizes spatial reference claims and cannot silently fall back. Its diff/source was inspected; current routes, scene and era views were recaptured. Remaining changed frontend files were fixtures/evidence. Older fixture screenshots with unavailable assets or a missing HTTP producer do not become functional failures or passes. All 61 required IA entries are accounted for below; root placeholders remain placeholders and no scope is removed. No build, live/backend, device, load or fidelity acceptance is inferred from this documentation/test-only change.

The QA02 support fix alone is not a new FE03 or C08 completion claim. No concrete application-origin defect or unresolved required review fix remains. The eight generic capture failures and the additional captured runtime error are enumerated in [review.json](review.json); equivalent current gates and the full visual-source inventory were reviewed rather than presenting them as successful feature flows.

## Required IA index

| IA | Route / state | Reviewed native evidence |
|---|---|---|
| A01 | `/` | [NO_LEGACY_UI_FOUND](snapshots/route-00.jpg) |
| A02 | `/login` | [NO_LEGACY_UI_FOUND](snapshots/route-01.jpg) |
| A03 | `/signup` | [NO_LEGACY_UI_FOUND](snapshots/route-02.jpg) |
| A04 | `/verify-email` | [NO_LEGACY_UI_FOUND](snapshots/route-03.jpg) |
| A05 | `/reset-password` | [NO_LEGACY_UI_FOUND](snapshots/route-04.jpg) |
| A06 | `/auth/callback` | [NO_LEGACY_UI_FOUND](snapshots/route-05.jpg) |
| A07 | `/legal/:document` | [NO_LEGACY_UI_FOUND](snapshots/route-06.jpg) |
| A08 | `/account/deleted` | [NO_LEGACY_UI_FOUND](snapshots/route-07.jpg) |
| W01 | `/world` | [NO_LEGACY_UI_FOUND](snapshots/route-09.jpg) |
| W02 | `/world/map` | [NO_LEGACY_UI_FOUND](snapshots/route-10.jpg) |
| W03 | `/world/places/:placeId` | [NO_LEGACY_UI_FOUND](snapshots/route-11.jpg) |
| W04 | `/world/rooms` | [NO_LEGACY_UI_FOUND](snapshots/route-12.jpg) |
| W05 | `/world/nearby` | [NO_LEGACY_UI_FOUND](snapshots/route-13.jpg) |
| W06 | `/world/theme` | [NO_LEGACY_UI_FOUND](snapshots/route-14.jpg) |
| W07 | `/world/check-in` | [NO_LEGACY_UI_FOUND](snapshots/route-15.jpg) |
| W08 | `/world/help` | [NO_LEGACY_UI_FOUND](snapshots/route-16.jpg) |
| F01 | `/feed` | [NO_LEGACY_UI_FOUND](snapshots/route-17.jpg) |
| F02 | `/feed/new` | [NO_LEGACY_UI_FOUND](snapshots/route-18.jpg) |
| F03 | `/feed/:postId` | [NO_LEGACY_UI_FOUND](snapshots/route-19.jpg) |
| F04 | `/feed/:postId/edit` | [NO_LEGACY_UI_FOUND](snapshots/route-20.jpg) |
| F05 | `/notices/:noticeId` | [NO_LEGACY_UI_FOUND](snapshots/route-21.jpg) |
| M01 | `/meet` | [NO_LEGACY_UI_FOUND](snapshots/route-22.jpg) |
| M02 | `/meet/parties/new` | [NO_LEGACY_UI_FOUND](snapshots/route-23.jpg) |
| M03 | `/meet/parties/:partyId` | [NO_LEGACY_UI_FOUND](snapshots/route-24.jpg) |
| M04 | `/meet/parties/:partyId/manage` | [NO_LEGACY_UI_FOUND](snapshots/route-25.jpg) |
| M05 | `/meet/parties/:partyId/rally` | [NO_LEGACY_UI_FOUND](snapshots/route-26.jpg) |
| M06 | `/meet/beacons/:beaconId` | [NO_LEGACY_UI_FOUND](snapshots/route-27.jpg) |
| M07 | `/meet/people` | [NO_LEGACY_UI_FOUND](snapshots/route-28.jpg) |
| M08 | `/meet/intro/edit` | [NO_LEGACY_UI_FOUND](snapshots/route-29.jpg) |
| M09 | `/people/:userId` | [NO_LEGACY_UI_FOUND](snapshots/route-30.jpg) |
| G01 | `/gifts` | [NO_LEGACY_UI_FOUND](snapshots/route-31.jpg) |
| G02 | `/gifts/new` | [NO_LEGACY_UI_FOUND](snapshots/route-32.jpg) |
| G03 | `/gifts/:giveawayId` | [NO_LEGACY_UI_FOUND](snapshots/route-34.jpg) |
| G04 | `/gifts/:giveawayId/manage` | [NO_LEGACY_UI_FOUND](snapshots/route-35.jpg) |
| G05 | `/gifts/:giveawayId/reservation` | [NO_LEGACY_UI_FOUND](snapshots/route-36.jpg) |
| G06 | `/gifts/pickups/:reservationId` | [NO_LEGACY_UI_FOUND](snapshots/route-37.jpg) |
| G07 | `/gifts/mine` | [NO_LEGACY_UI_FOUND](snapshots/route-33.jpg) |
| R01 | `/wall` | [NO_LEGACY_UI_FOUND](snapshots/route-38.jpg) |
| U01 | `/me` | [NO_LEGACY_UI_FOUND](snapshots/route-39.jpg) |
| U02 | `/me/avatar` | [NO_LEGACY_UI_FOUND](snapshots/route-40.jpg) |
| U03 | `/me/profile` | [NO_LEGACY_UI_FOUND](snapshots/route-41.jpg) |
| U04 | `/me/social` | [NO_LEGACY_UI_FOUND](snapshots/route-42.jpg) |
| U05 | `/me/requests` | [NO_LEGACY_UI_FOUND](snapshots/route-43.jpg) |
| U06 | `/me/age` | [NO_LEGACY_UI_FOUND](snapshots/route-44.jpg) |
| U07 | `/me/settings` | [NO_LEGACY_UI_FOUND](snapshots/route-45.jpg) |
| U08 | `/me/blocks` | [NO_LEGACY_UI_FOUND](snapshots/route-46.jpg) |
| U09 | `/me/account` | [NO_LEGACY_UI_FOUND](snapshots/route-47.jpg) |
| U10 | `/notifications` | [NO_LEGACY_UI_FOUND](snapshots/route-48.jpg) |
| X01 | `/archive` | [NO_LEGACY_UI_FOUND](snapshots/route-08.jpg) |
| S01 | `/report?targetType=:type&targetId=:id` | [NO_LEGACY_UI_FOUND](snapshots/route-49.jpg) |
| S02 | `route-error-boundary` | [NO_LEGACY_UI_FOUND](snapshots/route-error.jpg) |
| S03 | `connection-and-maintenance-overlay` | [NO_LEGACY_UI_FOUND](snapshots/degraded-maintenance.jpg) |
| O01 | `/admin` | [NO_LEGACY_UI_FOUND](snapshots/route-50.jpg) |
| O02 | `/admin/event` | [NO_LEGACY_UI_FOUND](snapshots/route-51.jpg) |
| O03 | `/admin/places` | [NO_LEGACY_UI_FOUND](snapshots/route-52.jpg) |
| O04 | `/admin/reports` | [NO_LEGACY_UI_FOUND](snapshots/route-53.jpg) |
| O05 | `/admin/users/:userId` | [NO_LEGACY_UI_FOUND](snapshots/route-54.jpg) |
| O06 | `/admin/content` | [NO_LEGACY_UI_FOUND](snapshots/route-55.jpg) |
| O07 | `/admin/mural` | [NO_LEGACY_UI_FOUND](snapshots/route-56.jpg) |
| O08 | `/admin/audit` | [NO_LEGACY_UI_FOUND](snapshots/route-57.jpg) |
| O09 | `/admin/archive` | [NO_LEGACY_UI_FOUND](snapshots/route-58.jpg) |

## Reproduction

Use the retained Node 24.22.2 executable and existing frontend dependencies; do not install packages. From repository root, set `QA02_NODE_MODULES` to the retained frontend `node_modules` directory. The evidence drivers use installed Chrome in headless mode and loopback Vite, with temporary cache directories. `capture.mjs` renders the full registry and existing component previews; `QA02_ROUTES_ONLY=1` restricts it to current routes and mobile public/auth. `state-capture.mjs` captures supplemental actual components; `review-index.mjs` creates the review sheet; `build-report.py` checks coverage and emits this index. `source-audit.py` reads the pinned legacy source checkout and Git objects without modifying them. Drivers are evidence tools: unqualified fixture attempts remain explicitly NOT_RUN, and manual comparison must be repeated after rendering changes.

Publication ancestry and any reconciliation after the audited candidate are recorded in [delivery.json](delivery.json).
