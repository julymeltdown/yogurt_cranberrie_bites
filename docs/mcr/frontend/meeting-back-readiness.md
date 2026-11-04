# FE07 direct-beacon Back test readiness

The named legacy test now waits for the visible, committed Meet heading before clicking Back again. Its final `/world`, same world-host and exactly one mount assertions are unchanged. This is a bounded test-readiness correction, with no product routing or meeting-authority change.

## Source and exact ownership

- Initial/reproduction base: `d73296f582a459eb94d55123f8480e78b52a0d21`; isolated branch `codex/mcr-fe-FE07-meeting-back-readiness`.
- Source correction: `c2887c9a4442fd25958e4ca6c909764959605c39`, one added line in `frontend/src/app/meeting-entry/__tests__/root-registration.test.tsx`, only the case `preserves direct-beacon back navigation through Meet to World without creating a second world lease`.
- The original test was last changed at `d80631e5eff52bb9e03198585261d463611dbf63`. The existing readiness proof was published at `fe0c3359b53bf928fd5d43fe7fdb646f9cdf03de` in `frontend/src/app/meeting-entry/__tests__/back-readiness/navigation.test.tsx`.
- The only additional write is this new document. No production source, other case, import, timeout, skip, fixture, routing policy, manifest, lockfile, historical failure receipt or QA10 harness was changed. The designated integrator resolved a crossed allocation before the edit and confirmed this worker as the sole named-case writer; the other worker confirmed zero edits.

## Independent reproduction and diagnosis

At the pinned base, the exact focused case failed with exit 1:

```text
Expected: "/world"
Received: "/meet"
Test Files  1 failed (1)
Tests       1 failed | 6 unselected (7)
```

The six other cases were excluded by the diagnostic `-t` filter, not disabled in source. The failure is timing-dependent: QA10 reported the same failure on `f30f7871e8dafb0a0cac6d04857bc49b9cbf5d72` (3416/3417), and a subsequent unchanged-test candidate `68c5f48d` passed 3417/3417. Those QA10 results are independent owner reports, not full-suite runs by this worker. Their historical FAIL remains unchanged.

`usePanelBack` in `frontend/src/app/navigation.tsx` captures the rendered location. The first click publishes router state `/meet` before React necessarily commits the new panel and Back handler. Waiting only on router state permits the second click to invoke the previous beacon handler, whose parent is still Meet. The existing diagnostic test demonstrates this ordering with real router subscription observations and a held React `act` commit boundary. Its normal/StrictMode cases already await the committed heading successfully.

The correction adds that same observable readiness check after the existing `/meet` wait and before the second real DOM click:

```tsx
expect(await screen.findByRole('heading', { name: 'Meet', level: 1 })).toBeVisible()
```

No blind sleep, suppressed failure, shortened journey or weaker final assertion is introduced.

## Validation and input pins

Commands ran from frontend using retained dependencies and pinned Node 24.22.2; no install occurred.

| Command | Exit/result |
| --- | --- |
| `node node_modules/vitest/vitest.mjs run src/app/meeting-entry/__tests__/root-registration.test.tsx -t 'preserves direct-beacon back navigation through Meet to World without creating a second world lease'` before the patch | 1; exact failure above |
| `node node_modules/vitest/vitest.mjs run src/app/meeting-entry/__tests__/root-registration.test.tsx src/app/meeting-entry/__tests__/back-readiness/navigation.test.tsx src/app/meeting-entry/__tests__/bridge.test.ts src/app/meeting-entry/__tests__/routes.test.tsx` after the patch | 0; 32 tests / 4 files passed, including the original seven registration cases and both normal/StrictMode committed-panel cases |
| `node node_modules/oxlint/bin/oxlint src/app/meeting-entry/__tests__/root-registration.test.tsx` | 0 |
| `python3 scripts/mcr/check-plan.py` from repository root | 0; planning/source archive provenance only |
| `git diff --check` and exact case/path inspection | 0; one test line plus this document |

SHA-256 pins at the reproduction/fix checkpoint:

| Input | SHA-256 |
| --- | --- |
| Registration test before correction | `1321636a31ff4e4e6730374fb029cbfd6ef0c45de966ef97619970c4db66457b` |
| Registration test after correction | `fc117dc9f32ebe79bf1c974d4fdea1f77742918450fe12aa9b2a894b837a0422` |
| Existing back-readiness proof | `2259cd67490ffa185b1ce8cf28a0bdc3121bedea77980087ab862e9bce6cafcc` |
| Product navigation | `2efd2d1236536e663f3fae00f74ecaacaea091a3474c45c47c845afa0a2ff1cc` |
| WorldShell | `9ab07b46c077bde4381ea2ec5918c146e2d05dfe7dc355bafad7448d9ff2d5f9` |
| Retained lockfile | `47ca44459819ad0e887d2f4f93394fdf3b1053b8f45edc9ceb0236981feee1f6` |

## Delivery and limits

The source commit and following documentation commit form the exact two-path handoff to designated frontend integrator Root153 for serial integration and normal master publication. Published candidate ancestry must be confirmed before this bounded repair goal closes. QA10 owns full-candidate verification after publication; this worker did not run the full 3417-test suite or a new native suite.

Independent historical context only: the whole-FE07 owner reported actual browser-router/canonical Auth/default Phaser/StrictMode direct beacon `015` → Meet → World passing at `8cd2046601b21e957e73a099d3975aa740a2a1bd`, with its 11-case native journey at 360/390/430px and 155 focused tests, published through `19ad4ca0`/`59de1ac9`. That evidence is not rerun or newly certified here. The present checks use jsdom, canonical Auth fixtures, an instrumented world port and the real fail-closed meeting bridge. They make no live beacon HTTP, reviewed place/audience/expiry, physical arrival, real backend, physical-device or release claim.

This regression correction does not add an original task closure, change whole FE07 disposition, or withhold its independently delivered journey. Existing routing, private-state cleanup, world continuity and meeting authorization remain their owners' responsibilities.
