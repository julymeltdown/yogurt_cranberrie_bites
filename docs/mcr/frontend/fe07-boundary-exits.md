# FE07 public/admin boundary exits

This bounded repair allows a current, clean social owner to leave the world shell for a known public or admin route. The actual router still owns boundary unmounting and destination authorization. Participant sign-in return targets remain unchanged. Unresolved commands and social drafts remain guarded when an exit would destroy their owner.

## Scope and provenance

- Assignment: original FE07 boundary-exit defect only. Whole FE07 qualification remains with the shell-journey owner `01a1a1fe-1ab7-79b0-adbc-d4c025701acb`.
- Fresh clean base: `a978d4ed57d7f7eac28b494e3e8550812791f1da`; branch `codex/mcr-fe-FE07-boundary-exits`, reusing the isolated checkout while preserving the completed FE36 branch.
- Original handoff source/regression commit: `daa1c757aba7a48f6841abfc6dbc815089e55c9d`, reconciled onto `e227d12ef4bb3ac92b9fdbbfaf954d4a56050081`. Original local source commit: `ba4fb095`. The designated delivery integrator published it as `a572b101b67c07c75fda1fa9be359c264ff7d3e8`; this worker did not push master.
- Exact production write: `frontend/src/app/social-composition/owner.ts`, navigation policy and necessary same-file helper/imports only. New tests/evidence: `frontend/src/app/social-composition/__tests__/fe07-boundary-exits/**`. New documentation: this file.
- No edits to the shared route policy, root router, SocialNavigationBoundary, WorldShell, Auth, other owners, old tests, manifests, lockfile, backend, source archives or artwork.

The native fixture adapts the shell-journey owner's working fixture read-only. `fixture-provenance.json` records its checkout revision, source/destination paths, exact content hash and transformations. At copy time the source checkout was `009327ca52d127db39b615e6049e4c66f37decef`; the fixture itself was working-tree content, not claimed as committed there. Its SHA-256 is `3df8c98e6d81d34b183e9466c335664c89c8cb311270cdae9a30d1c91541594a`. Adaptation adds a provenance comment, adjusts relative imports by one directory, and uses unique synthetic bridge/revision identifiers. It preserves canonical Auth login, `createAppRouter`, `createProductionPorts`, StrictMode and default real Phaser composition without loader, renderer, panel or engine overrides. Existing HTTP/C05/selected-character fixtures are dependencies, not live service authority. No dependency install or legacy UI copy occurred.

## Reproduction and repair

The independent native baseline starts fresh at World, waits for one live game/socket, and requests `/legal/privacy`. The actual pathname remains `/world`; the social blocker displays a disabled Continue navigation button. A second case edits the real Me profile, verifies the unsaved value survives a World/profile round trip, restores its exact initial value, then reproduces the same exit failure. Neither case sends a profile mutation. Both fail at 390px in `evidence/initial/native.json`, with screenshots and accessibility snapshots. The reporter's earlier candidates were `8de6c411` and `a84885f2`; the independent baseline uses the fresh base above.

`safeReturnTarget` intentionally accepts known participant destinations and excludes public/admin redirect chains. Applying it to every ordinary destination in `navigationReason` incorrectly treated all boundary exits as unavailable. Weakening that shared function would weaken sign-in and deep-link safety.

The local helper instead recognizes existing registered public/admin routes, rejects external/unknown/traversal/encoded/query/hash/whitespace/control-character targets, and requires canonical UUIDs on admin user routes. It permits only query-free boundary paths; it introduces no auth callback/query contract. Existing participant and social target validation remains in place. Destination permission is still checked by the unchanged router and AdminBoundary.

Navigation then applies the existing current-lease and unresolved-command checks. At an actual shell exit it also guards post/reply/edit drafts, media selections and intro changes relative to their saved value. Restoring or explicitly cancelling those drafts permits the exit. Participant panel transitions retain their previous draft-retention and target-change semantics. No mutation, replacement idempotency key, recovery replay or disposal is triggered by a pure `canNavigate` check. Actual route unmount disposes the existing owner; canonical Auth remains owned by the application.

## Validation

Evidence root: `frontend/src/app/social-composition/__tests__/fe07-boundary-exits/evidence/`. Commands below ran from frontend with pinned Node 24.22.2 and retained dependencies unless stated otherwise.

| Check | Command or API | Result |
| --- | --- | --- |
| Independent unit baseline | `node node_modules/vitest/vitest.mjs run src/app/social-composition/__tests__/fe07-boundary-exits/boundaries.test.ts` | Exit 1; 19 failed, 23 passed. `initial/unit.txt` |
| Independent native baseline | `BOUNDARY_RUN=initial BOUNDARY_WIDTH=390 node src/app/social-composition/__tests__/fe07-boundary-exits/native.mjs` | Exit 1; clean and restored-profile public exits fail |
| Targeted owner/provider and new regressions | `node node_modules/vitest/vitest.mjs run src/app/social-composition/__tests__/fe07-boundary-exits/boundaries.test.ts src/app/social-composition/__tests__/owner.test.ts src/app/social-composition/__tests__/provider.test.tsx` | Exit 0; 87 tests / 3 files. `corrected/unit.txt` |
| Actual-router native repair | `node src/app/social-composition/__tests__/fe07-boundary-exits/native.mjs` | Exit 0; 6/6 at 360/390/430px. `corrected/native.json` |
| TypeScript, including peer reconciliation | `node node_modules/typescript/bin/tsc --project tsconfig.json --pretty false` | Exit 0 |
| Exact source/new-test lint | `node node_modules/oxlint/bin/oxlint src/app/social-composition/owner.ts src/app/social-composition/__tests__/fe07-boundary-exits` | Exit 0 |
| Reconciled application bundle | Vite `build({configLoader:'runner',publicDir:false,build:{outDir:'/tmp/mcr-fe07-boundary-build',emptyOutDir:true}})` | Exit 0; existing large-chunk warning |
| Planning provenance, repository root | `python3 scripts/mcr/check-plan.py` | Exit 0; 13 archive checksums match; application/release acceptance is separate |
| Assignment and whitespace | Exact changed-path/import inspection; `git diff --check` | Exit 0; only assigned paths |

The 44 new regression cases cover allowed public/admin boundaries, unchanged participant return-target policy, unsafe/malformed/private targets, composer/reply/edit/intro/media draft protection, exact intro restoration, sending/uncertain and genuine PENDING original-key retention, identity/event/policy cleanup with ignored-abort late reads, and private child/subscription disposal without disposing canonical Auth. The initial baseline had 42 cases; explicit PENDING and prepared-media cases were added afterward. Existing owner/provider tests retain participant target, cancellation, policy, command and native boundary behavior. No broad Vitest run or unrelated test fix was performed.

Native Chrome 156.0.8078.80 runs actual browser history and the canonical router in StrictMode. At every width, both clean World and restored-profile cases prove:

- A query-bearing admin target stays blocked with Continue disabled.
- Public exit removes the world DOM, destroys the game and closes the active socket: zero games/sockets.
- Native Back creates one replacement world: two known games across the intentional unmount/remount, peak one game/socket and one active game/socket.
- Admin navigation reaches the unchanged Access denied gate, with zero world games/sockets and no private shell DOM.
- No profile save, hidden mutation or command replay occurs; only login and world-ticket writes are present. There are no uncaught browser errors.

Native reports pin owner, provider, blocker, route registry, router, WorldShell, AdminBoundary, own fixture/runner and retained lockfile. The corrected capture identifies the original base plus edited source hashes; every pin was checked against the reconciled delivered tree and remained identical. Unit logs normalize only the local checkout prefix and trailing blank lines. Before/after 390px screenshots were visually inspected. The runner uses a reserved loopback port, task cache and explicit browser/server disposal; disk stayed above the 8GiB floor. Temporary dependencies and build output are excluded from delivery.

## Disposition and limits

The bounded boundary-exit repair is `ACCEPTED` within its source/fixture/native scope and is integrated/published. It does not complete original FE07: its original criteria still include persistent shell/socket behavior across panels and direct gift entry, and the shell-journey owner independently qualifies that whole task on the published candidate. This document claims no original task or IA scope cut.

The shell-journey owner explicitly clarified that its safe-draft case concerns retention through same-shell panel/history/resizing. The exit cases here use fresh World or a profile restored through its real editor. Cross-owner unsaved Me-profile exit guarding is neither added nor claimed; no account/root lease was expanded. Existing social draft/uncertain-command protection is retained and exercised.

Private controller observations and subscriptions are cleared on owner disposal in tests, while native routing proves actual shell/game/socket teardown. These receipts do not certify arbitrary application-wide cache eviction. Real backend/provider administration permission, real server multiuser flows, physical devices, production admission and release/IN decisions remain `NOT_RUN` by this repair. The synthetic canonical session and C05 producers are identified in the fixture and native report.

## Publication receipt

On 2026-11-04 the designated integrator published the increment normally to `julymeltdown/mcr:master`, checkpoint `4052d709fef976d14653351cd8bf78ef85bd1579`. Mapping: source `daa1c757` → `a572b101b67c07c75fda1fa9be359c264ff7d3e8`, documentation `08510287` → `c049ddbc7bdf1c95762dd31fbb3b297ae188e49b`, handoff manifest `c705ccb9` → `77115679ad2f2657891065ea7e4840828b9d8eca`. The integrator reported no source conflict and reran all 87 affected tests successfully after peer reconciliation.

The implementation owner freshly fetched and independently verified all four published commits as ancestors of `origin/master`. All 27 payload hashes from the historical handoff manifest and every native input pin matched checkpoint `4052d709`. The manifest records the original committed handoff, not subsequent documentation receipts. The whole-FE07 owner received the actual source checkpoint directly for its independent final journey. This receipt adds no production change or further review gate.
