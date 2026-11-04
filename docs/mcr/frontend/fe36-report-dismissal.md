# FE36 report dismissal repair

This increment repairs the retained safety controller after an ordinary report dismissal. Escape and Close report now scrub the draft and return the current authorized owner to `idle`, so the same Report and Block controls remain usable. Actual privacy cleanup still closes and scrubs the old owner. Unresolved commands retain their original binding and idempotency key.

## Scope and provenance

- Task: FE36 report-dismissal repair, raised by QA07. Initial QA07 reproduction: `cf706882ed91a2708f3ef1e514b1fc81886cffc5`; controller last changed at `a0e8332acc85a6901dd0cf77d4a577a2ca3cfc09`.
- Independent repair base: `ca0d2d7b3d3a73abb7a338dada30d5cdd5987d94`. Isolated branch: `codex/mcr-fe-FE36-report-dismissal`.
- Delivered source/regression commit: `705da35a5f77e2207dc805d38efd518968085e26`, rebased onto peer delivery `1383ae452fc81dfe5fd1e4ca4d37af8950700dda`. Its original local commit was `baa561d6`; corrected native receipts were refreshed against the delivered source commit after reconciliation. Publication ancestry is the source commit followed by this evidence/documentation commit, with any subsequent peer reconciliation preserved.
- Exact production ownership: `frontend/src/features/safety/controller.ts`, limited to the close/reset distinction. Added regression ownership: `frontend/src/features/safety/__tests__/qa07-report-dismissal/**`. Documentation ownership: this file.
- `panels.tsx`, the published preview, shared Dialog, Auth/privacy implementation, contracts, manifests, lockfile and root composition are inherited unchanged. No backend, old UI, source archive or character artwork was copied or modified. Tests reuse the existing synthetic safety/auth fixtures read-only.
- Contract revision is the unchanged safety contract and generated operation registry inherited from the repair base. Existing `safety-account.md` requires draft scrubbing without replacing an unresolved command, and actual owner disposal at the privacy lifetime boundary.

## Failure and classification

Use the actual published fixture route `/src/features/safety/__tests__/preview.html` at 390px. Tab/Enter Open report, Tab to What happened, type `QA07 keyboard report draft`, then Escape. Focus returns to Open report, with no API request or policy change. Before repair, Block user remains disabled and displays `Private views were cleared. Reopen safety controls with your current session.` Close report reproduces the same failure.

This is a feature-controller lifetime defect. The no-intent branch of `close()` called the privacy cleanup that publishes `closed`. `BlockUserButton` correctly locks a closed controller; changing that guard would weaken genuine revocation. The published fixture retains one controller, as the unresolved-command contract requires.

The repair extracts the existing abort/sequence/intent reset into a small local helper. Privacy cleanup still publishes `closed`. Benign close instead publishes the initial `idle` state. A current-scope guard prevents a later close from reviving a controller revoked by policy, account identity, event change or disposal. The unresolved-command branch remains separate: it scrubs drafts, fences late work and retains the original command for explicit status checks, without replay or a replacement key.

## Evidence and validation

Evidence root: `frontend/src/features/safety/__tests__/qa07-report-dismissal/evidence/`.

| Check | Command, run from frontend unless noted | Exit and result |
| --- | --- | --- |
| Independent failing regression | `node node_modules/vitest/vitest.mjs run src/features/safety/__tests__/qa07-report-dismissal/dismissal.test.ts` | 1: 3 failed / 10 passed; `initial/unit.txt` |
| Native initial reproduction | `DISMISSAL_RUN=initial DISMISSAL_WIDTH=390 node src/features/safety/__tests__/qa07-report-dismissal/native.mjs` | 1: Escape and Close report fail at disabled Block; policy and identity cleanup pass |
| Corrected regression and related owners | `node node_modules/vitest/vitest.mjs run src/features/safety/__tests__ src/features/account/__tests__ src/features/auth/__tests__ src/shared/query/__tests__` | 0: 447 tests / 17 files; `corrected/unit.txt` |
| Native corrected, after reconciliation | `node src/features/safety/__tests__/qa07-report-dismissal/native.mjs` | 0: 12/12 across 360/390/430px; `corrected/native.json` |
| TypeScript | `node node_modules/typescript/bin/tsc --project tsconfig.json --pretty false` | 0 |
| Scoped lint | `node node_modules/oxlint/bin/oxlint src/features/safety/controller.ts src/features/safety/__tests__/qa07-report-dismissal` | 0 |
| Production bundle | Vite `build({configLoader:'runner',publicDir:false,build:{outDir:'/tmp/mcr-fe36-dismissal-build',emptyOutDir:true}})` | 0; existing large-chunk warning |
| Planning provenance, repository root | `python3 scripts/mcr/check-plan.py` | 0; 13 archive checksums match; this does not certify application acceptance |
| Scope/whitespace | `git diff --check` and changed-path/import inspection | 0; only assigned paths |

Runs used pinned Node 24.22.2 and retained frontend dependencies, without an install or lockfile change. The temporary dependency symlink is excluded from delivery. Browser: installed macOS Chrome 156.0.8078.80 in native headless mode. The runner uses an isolated loopback port/cache and keyboard Tab/Shift+Tab/Enter/Escape rather than scripted focus, click or fill. Disk remained above the 8GiB floor. The first runner setup attempts had a path-resolution error and occupied-default-port error; both were corrected before the recorded reproduction. An unsupported `tsc -b --tsBuildInfoFile` invocation was replaced with the successful no-emit project check above.

Native JSON records source, preview, panel, runner and retained-lockfile SHA-256 pins. The initial candidate is the clean repair base. Corrected receipts identify source commit `705da35a5f77e2207dc805d38efd518968085e26`; earlier precommit runs used the same controller content hash. Unit logs normalize only the checkout prefix to `<frontend>` and trailing blank lines.

The 14 isolated regressions cover unsent and completed report dismissal, aborted authorization with a genuinely started ignored-abort proof, six real cleanup boundaries, pending/unknown/FAILED/RETIRED command retention, and dismissal during an in-flight write. They verify no replacement mutation/key, suppressed late results/callbacks, and recovery only through the original status key and current authorized report reread. The initial baseline had 13 cases; the in-flight status-recovery case was added afterward.

Native receipts include screenshots and accessibility snapshots. They verify both dismissal paths, empty draft on reopening, Block confirmation initial focus/reverse-Tab containment/Escape return, Cancel block return, and unchanged disabled controls after actual policy or identity invalidation. Every case records zero API requests. The before/after 390px screenshots were visually inspected. This proves controller/UI behavior with synthetic authority; it is not server mutation or real screen-reader evidence.

## Original FE36 disposition and remaining boundaries

The repair behavior is `ACCEPTED` within its source/fixture/native scope. The original FE36 implementation remains documented as `MOCK_READY` in `safety-account.md`; this increment does not accept the whole parent task or QA07. Original specification criteria still require blocking to close affected panels, unblock never restoring prior consent, and failed deletion never appearing complete. Existing related unit checks pass, including block/unblock canonical privacy invalidation and account deletion safeguards.

Real server authorization/inventory of private state, multiuser block/unblock behavior, real account-deletion execution and full-shell integration are `NOT_RUN` by this repair. QA07 owns its independent post-publication native recheck. Physical iOS/Android, real assistive-technology announcements and release/IN05 decisions are outside these receipts. No original task or IA scope is cut, and no unrelated landing/world issue is introduced as a dependency for accepting this bounded repair.
