# QA-BACK-READINESS: direct-beacon Back observation

**READY diagnostic handoff. MOCK_READY test evidence only. The reserved original test remains unchanged and its intermittent failure remains unresolved in source.**

Base/parent: `adb73ce109d5f67ef7c0234a730d1cd0459f219a`. Branch: `codex/mcr-qa-back-readiness`. Isolated checkout: `qa-back-readiness`. New test/evidence subtree: `frontend/src/app/meeting-entry/__tests__/back-readiness/`. Delivery commit is the commit adding this report and that subtree, discoverable with `git log -1 --format=%H -- docs/mcr/qa/back-navigation-readiness.md`. Main/Delivery publishes; no additional approval gate.

## Finding and owner action

The controlled evidence supports a **test observation error**. Router publication of `/meet` does not prove React has committed the Meet panel or replaced the accessible Back button's handler. The original test waits only on `router.state.location.pathname`, then immediately reuses the same accessible button name. A still-committed beacon handler correctly selects the beacon's parent `/meet` again. No independent production defect is established by these probes.

The exact minimal proposed correction belongs to Delivery / the owner of the reserved `frontend/src/app/meeting-entry/__tests__/root-registration.test.tsx`: insert the following after original line 69, before its second unchanged Back click:

```tsx
expect(await screen.findByRole('heading', { name: 'Meet', level: 1 })).toBeVisible()
```

Keep both existing pathname assertions, the identical Back accessible-name queries, the same-host assertion and `mounts === 1`. This proposal is documented only; **this branch does not apply it to the reserved test or any production file**. The new normal/StrictMode probes use the same original global Back queries with this added commit observation, and also check visible World tools, no leftover Back button, peak lease counts and disposal.

## Source trace and controlled schedule

- `frontend/src/app/routes.ts:31` gives Meet parent `/world`; line 37 gives the beacon parent `/meet`.
- `frontend/src/app/navigation.tsx:35` captures the rendered `useLocation` in `usePanelBack`; lines 40-46 derive and navigate to that rendered panel's safe parent when no known parent-history key applies.
- `frontend/src/app/WorldShell.tsx:114` obtains both title and Back handler for the current rendered panel, attaches the handler at line 141 and its heading at line 142.
- The untouched original test at lines 68-71 clicks beacon Back, waits for public router pathname `/meet`, clicks the same globally named button, then expects `/world`.

The new controlled test uses **actual `createAppRoutes`, `RouterProvider`, `WorldShell`, canonical Auth fixture, existing world fixture ports, actual meeting bridge and actual DOM clicks**. It has no route composition overrides, route/bridge/navigation mocks or fake navigation results. A public asynchronous React `act` boundary holds the React commit queue while two ordinary `fireEvent.click` calls execute. The real router remains free to publish. A public `router.subscribe` observer records only published pathname and the current DOM heading, not private router internals.

The controlled test asserts this sequence:

| Observation | Public pathname | Committed visible heading | Result |
| --- | --- | --- | --- |
| First beacon Back inside the commit boundary | `/meet` | Find each other | Meet panel not yet committed |
| Second actual Back click before boundary release | `/meet` | Find each other | Beacon handler reissues its parent |
| Boundary released; visible Meet panel awaited | `/meet` | Meet | Destination's Back handler committed |
| Click Back in committed Meet panel; await World tools | `/world` | World | Same host, one mount, peak one lease |
| Unmount | — | — | Zero games/connections; one disposal |

This intentionally characterizes why the original observation can fail; it does not pretend the original sequence reaches World. Two further tests observe the actual destination heading before their second click (normal and StrictMode). They preserve one live world owner, peak games/connections of one, no navigation remount, zero beacon requests and complete disposal. StrictMode setup/cleanup counts are compared to the captured pre-navigation mount count; the normal case explicitly retains `mounts === 1`.

The shell's real meeting release remains `null`, so its actual fail-closed guidance is shown at the initial beacon. No meeting grant, source, venue revision, clock or arrival is fabricated. Broad bootstrap capabilities are not used to create meeting authority. The setup reuses the existing canonical Auth fixture and disposes its unrelated gift controller before shell ownership, matching the original test's arrangement.

## Reproduction and focused validation

Original UIV04 `frontend/src/features/me/__tests__/visual-adoption/evidence/validation.json` is preserved byte-for-byte. Its historical base `11d7cb00165f78b2ec9c564dd93669edad104e88` recorded expected `/world`, observed `/meet` both in the full-suite summary and the baseline comparison. A fresh untouched-original run here exited **1**, six passed / one failed with that same assertion at original line 71. Full failed output is retained in the new evidence subtree.

Pinned runtime: Node **v24.22.2**, Vitest **4.1.9**. Prepend `/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin` to PATH. Retained integration dependencies were consumed through a temporary `frontend/node_modules` symlink; no install, lockfile change, cache-enabled run or TypeScript build-info generation. The link is removed before committed handoff.

| Check | Actual result | Exit |
| --- | --- | --- |
| Untouched original root-registration file alone | 6 passed, 1 failed: expected `/world`, got `/meet` | 1 |
| New controlled/normal/StrictMode tests plus original, repeat 1 | New 3 pass; original 6 pass / 1 fail | 1 |
| Same two files, repeat 2 | New 3 pass; original 6 pass / 1 fail | 1 |
| Same two files, repeat 3 | New 3 pass; original 7 pass | 0 |
| Combined five focused navigation files | 82 passed, zero failed or skipped | 0 |
| Strict combined frontend TypeScript | No diagnostics | 0 |
| Strict combined frontend lint | No diagnostics | 0 |

The three repetitions were selected because the original failure was already intermittent in UIV04. They explicitly retain both failing and passing original outcomes; the controlled probes passed every repeat. The combined green run does **not** fix or supersede the failures. No full 96-file suite or application/release acceptance is claimed.

Commands from checkout root unless stated otherwise:

```sh
# Untouched original
node frontend/node_modules/vitest/vitest.mjs run --root frontend --configLoader runner --no-cache src/app/meeting-entry/__tests__/root-registration.test.tsx

# Exactly three finite repetitions of these same two files
node frontend/node_modules/vitest/vitest.mjs run --root frontend --configLoader runner --no-cache src/app/meeting-entry/__tests__/back-readiness/navigation.test.tsx src/app/meeting-entry/__tests__/root-registration.test.tsx

# Combined affected navigation tests
node frontend/node_modules/vitest/vitest.mjs run --root frontend --configLoader runner --no-cache src/app/meeting-entry/__tests__/back-readiness/navigation.test.tsx src/app/meeting-entry/__tests__/root-registration.test.tsx src/app/meeting-entry/__tests__/routes.test.tsx src/app/__tests__/routes.test.ts src/app/__tests__/shell.test.tsx

node frontend/node_modules/typescript/bin/tsc --noEmit --incremental false --project frontend/tsconfig.json
# From frontend/:
node node_modules/oxlint/bin/oxlint src vite.config.ts
```

Repeat orchestration collected subprocess return codes without rewriting failed results; its wrapper exited 0 after recording the three nested test exits **1, 1, 0**. Strict/lint checks used the final new test source. The existing tests' default collection and assertions were not altered, skipped or quarantined. There are no longer sleeps, forced state updates or weakened lease checks.

## Provenance, source and log bindings

`navigation.test.tsx` is newly authored diagnostic test logic. Its setup pattern is adapted from the read-only root-registration setup at this exact base; source and destination are recorded above. Existing fixture logic is imported rather than copied or modified. Dependencies are the retained React, Testing Library, React Router and Vitest dependencies; no new package.

New test SHA-256: `2259cd67490ffa185b1ce8cf28a0bdc3121bedea77980087ab862e9bce6cafcc`.

Read-only source/contract-input pins, verified equal to the exact base:

| Path | SHA-256 |
| --- | --- |
| `frontend/src/app/meeting-entry/__tests__/root-registration.test.tsx` | `1321636a31ff4e4e6730374fb029cbfd6ef0c45de966ef97619970c4db66457b` |
| `frontend/src/app/router.tsx` | `6626b51bd8bfa3589dff42ab0166ec05bd07438b83cb83e5bffe21b7d32d1eb2` |
| `frontend/src/app/WorldShell.tsx` | `9418ecaa41689bcdf2d1a15872097bb2672d29786602a14e5251fa308f4ed27b` |
| `frontend/src/app/navigation.tsx` | `2efd2d1236536e663f3fae00f74ecaacaea091a3474c45c47c845afa0a2ff1cc` |
| `frontend/src/app/routes.ts` | `e49ceffdc2e6e52bfba8e361b727504268fab264763c124878a556ee7f677829` |
| `frontend/src/app/WorldHost.tsx` | `8438ad02ad8d21b3fc0e64d76ea2e47fdfb89d8f7ca5db86e57516678170a491` |
| `frontend/src/app/ports.ts` | `71741772296f3590ef3d8e04b7c5e462d4410f7e18f575e44cf0775c379b2be7` |
| `frontend/src/app/__tests__/fixtures/ports.ts` | `ff8b01aa247d6c8ca3afc3e5ed92fa7d1fecabab2dadc79e586e4ce14f3daf10` |
| `frontend/src/features/gift-discovery/__tests__/fixtures/data.ts` | `79993cf295f0f398a508ca6b3f70a3d10cbd1b992b9ac72a98db14d8730a35be` |
| `frontend/src/app/meeting-entry/routes.tsx` | `6d1972f2fab6536e1ae5a0eb7d39247ed1adf19d527f551fcc2b29b6c65cc2a9` |
| `frontend/src/app/meeting-entry/bridge.ts` | `ecb3adf14cac50f11457f8a6868b84753c967ae7c60fe38c25e43b813f4befdb` |
| `frontend/src/features/me/__tests__/visual-adoption/evidence/validation.json` | `41c19562b1cfe2cc05b99f90401ecad3f1bc20c885d41264f7e5b552a0c92890` |
| `frontend/src/app/meeting-entry/__tests__/routes.test.tsx` | `0f6aa8692e42b800698d9f6ed5a1231ef7b37818be439ec852ae4545c1ce8788` |
| `frontend/src/app/__tests__/routes.test.ts` | `5ca28a98aa0964eb53903c5c0e0abd7df3f87e813b4e43d742ec61c8b8588ead` |
| `frontend/src/app/__tests__/shell.test.tsx` | `243a2e96c505883b400e6cdd898b7b3f708c108d41f753fc1e8506a39a7162ec` |
| `frontend/package.json` | `0f7a05587bcf1af5f4d5d077a0c2d90011adc87e1036a61db9ef3acf37c8b7d3` |
| `frontend/pnpm-lock.yaml` | `47ca44459819ad0e887d2f4f93394fdf3b1053b8f45edc9ceb0236981feee1f6` |

Retained logs in `frontend/src/app/meeting-entry/__tests__/back-readiness/evidence/`:

| File | SHA-256 |
| --- | --- |
| `original-first.log` | `d1edb8be32cc49494d063fda58a206ac9af179728a3accd1af07da7bbecc109b` |
| `combined.log` | `660d8df7c6a9f613035ca3759e0e82f82e308df0db7dba3d263bf9921d9dabf6` |
| `repeat-1.log` | `bc610c207438c2aa47b27229e35335a5ea0b9669984f3c3c937411c2486d015d` |
| `repeat-2.log` | `3e90dfa56285378644652d040518d4f5b4de6cbaff15dc53a2e302667fd414da` |
| `repeat-3.log` | `751c7b788f70f9c6734787d5330f8c070f66bd03e5aba48c967ce5899af00c58` |
| `types.log` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `lint.log` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |

`evidence/validation.json` contains machine-readable commands, nested exits, repeat results, source/log hashes, asserted observations, the proposed owner correction and limitations. Evidence logs retain exact synthetic DOM/error text. No historical source or evidence is rewritten to change its language.

## Delivery boundary and remaining work

Only this new report and the new `back-readiness/` subtree are writable. All existing files, including original root-registration, WorldShell, navigation, router, meeting implementation/fixtures, manifests/locks, backend/contracts/art, `docs/source/` and original UIV04 evidence remain unchanged. Exact changed-path and source-pin checks are performed before commit; authored files pass `git diff --cached --check` with raw `.log` evidence excluded. Full raw logs intentionally retain their original trailing blank lines (the default whole-diff whitespace check reports those only); no cross-directory backend implementation imports are added.

The original test correction remains **NOT_RUN / NOT_APPLIED in its reserved file**. Its owner can apply the one added committed-heading observation and rerun this focused set. Production navigation changes are not justified by this diagnosis. Live world/Phaser connections, native devices, server authorization, real venue guidance and multiuser behavior are outside this fixture proof.

Persistent goal verification: initial actual `get_goal` returned null; a new aligned unbudgeted QA-BACK-READINESS goal was created. Fresh actual reads returned **active**, with the objective covering source-bound controlled diagnosis, new actual-route regression probes, focused validation, precise owner correction, clean committed handoff and scoped fixes; `remainingTokens=null`. Stop this writer on the committed READY handoff; Main/Delivery owns integration and publication.
