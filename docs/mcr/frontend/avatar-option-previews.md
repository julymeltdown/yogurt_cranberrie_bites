# FE13-OPTIONS / UIV04 full-outfit option previews

Status: **MOCK_READY** for this bounded frontend increment. Integration, approved
character-art presentation, live multiuser flows and FE13 acceptance remain
**NOT_RUN**. Delivery base is `e869480395b70b4aae6cfcf50e904b079a59a0e7` on
`codex/mcr-fe-FE13-option-previews`. This document and its sibling implementation
are delivered together in the commit directly following that base; resolve its
exact SHA with `git log --format=%H --all -- docs/mcr/frontend/avatar-option-previews.md`.
Only the delivery integrator pushes to `julymeltdown/mcr:master`.

## Behavior and ownership

`OptionStrip.tsx` adds three full-outfit candidates per page below the existing
large preview and right-hand category toolbar. Every candidate is a complete
current-manifest config: a preset, one replaced layer, accessory `null`, or one
replaced tone. The labels explicitly describe full outfits. No isolated hair/eye
thumbnail, randomizer, reset control, invented swatch, thumbnail URL or new DTO is
introduced. The existing preset buttons and native ID selects remain available.
Selection calls the existing `selectPreset`, `selectItem` or `selectTone`; only
the existing explicit Save writes. Category/page browsing preserves the draft,
selected outfit and main canvas.

The controller's new `acquireOptionPreviews` uses the existing owner's
`supports` and `render` methods. It rejects batches over three and serializes
option renders across batch/owner replacements. It creates no atlas, renderer,
Auth identity, socket, resource fetcher or cache. The main lease remains separate.
There can be three delivered option leases and one main lease; at most one
additional option render runs at a time. The existing main-render flight remains
independent. The shared compositor's existing 50-entry cache cap is unchanged.

Category/page cleanup runs in layout effects. Draft changes, non-ready phases,
scope/identity/policy changes, owner replacement, stop and disposal abort and
release the batch. Epoch, scope, owner, draft and current-batch checks reject
stale completion. A render that ignores abort keeps its serial slot until it
settles; its eventual lease is disposed before notification. This deliberately
bounds work but can delay new option previews if an owner never settles. Native
ID alternatives remain usable according to the existing controller rules.

Only option lifetime and publication cleanup were added to `controller.ts`.
Existing GET, save receipt validation, explicit reconciliation, draft/cancel,
conflict, uncertain-write and retained hidden-draft behavior are preserved.
No backend imports or implementation dependencies were added.

## Source and contract provenance

All product UI and option-lifetime code in this increment is newly authored.
The inspected CUSTOMIZE reference is
`docs/mcr/design/ui-reference-audit-2026-11-04/user-reference.jpg`, SHA-256
`2b3988d546f87b813d78ca13bf267730a6dd19118bd6535a2d3c3032b6f12d23`.
Its large preview, category rail and compact choice row informed the layout.
Symmetric borders and ordinary focus outlines are used; no left accent bar.

| Existing input at the delivery base | Use / destination | Evidence |
| --- | --- | --- |
| C03 `contracts/core/v1/operations.json`, `contracts/fixtures/core/{private-participant,error-version-conflict}.json` | Existing GET `/me`, PUT `/me/avatar`, revision and failure fixtures | Existing HTTP controller unchanged; new tests verify explicit writes/reconciliation |
| C06 `contracts/assets/v1/`, `contracts/fixtures/assets/avatar-manifest.json` | Existing manifest/config only; synthetic test metadata in `__tests__/option-previews/data.ts` | Strict manifest/config validation and shared-compositor pixel equality |
| `frontend/src/features/avatar/preview.ts`, `contract.ts` | Imported existing port/strict config; no copied adapter | Native report pins SHA-256 of both read-only files |
| `frontend/src/entities/avatar/runtime/compositor.ts` | Existing same-owner thumbnail implementation, imported only | Native report pins SHA-256 and compares RGBA data |
| FE13 fixture scaffolding from `1fdf411a52ec3b8e3e6d0a4a71f7ba86715135c9`, `__tests__/fixtures/{harness,preview,data}.ts[x]` | Adapted test HTTP/privacy/compositor setup in new `option-previews/{harness,preview,data}.ts[x]` | Original files remain unchanged; dependencies are retained React, Query, Vite, Playwright and Vitest |
| FE12 `1db1680e6a5041250b79ca0ffb1dfd965a279edc`, `world/avatar/__tests__/fixtures/png.mjs` | Imported geometric PNG generator in new `native.mjs` | Synthetic PNG SHA-256 is in the native report; no PNG copied into product assets |

`docs/source/`, originals, source catalog, manifests, lockfile, routes, shared
contracts, root composition and backend are unchanged. The test metadata adds
preset combinations of the existing synthetic fixture, alternating accessory
presence to obtain distinguishable full-outfit pixels. It does not approve art,
expand product inventory or claim distinct synthetic hair artwork.

The only additional transferred existing test path is
`__tests__/visual-adoption/presentation.test.tsx`. Main explicitly authorized its
obsolete zero-total-render title/assertion to become main-canvas preservation,
bounded option leases, unchanged draft/selection and no-PUT checks. Every other
assertion remains. The original 26/27 result is retained in
`option-previews/evidence/initial-presentation-failure.json`.

## Validation — 2026-11-04

Use retained Node `24.22.2` at
`/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node` for each `node`
command below. Dependencies were temporarily linked from the integration
checkout; no installation or lockfile change occurred. The link is removed for
handoff. All final commands exited **0**; JSON evidence is under
`frontend/src/features/avatar/__tests__/option-previews/evidence/`.

From `frontend/`:

```text
node node_modules/vitest/vitest.mjs run src/features/avatar/__tests__/option-previews.test.tsx src/features/avatar/__tests__/visual-adoption/presentation.test.tsx --configLoader runner --no-cache
node node_modules/typescript/bin/tsc --noEmit --incremental false --pretty false
node node_modules/oxlint/bin/oxlint src/features/avatar/AvatarEditor.tsx src/features/avatar/OptionStrip.tsx src/features/avatar/controller.ts src/features/avatar/__tests__/option-previews.test.tsx src/features/avatar/__tests__/option-previews src/features/avatar/__tests__/visual-adoption/presentation.test.tsx
```

From the worktree root:

```text
node frontend/src/features/avatar/__tests__/option-previews/native.mjs
node frontend/src/features/avatar/__tests__/option-previews/production.mjs
python3 scripts/mcr/check-plan.py
git diff --check
```

- **17/17** final focused tests pass: option port/UI lifetime plus all three
  presentation tests. The initial retained controller/UI/adapter run passed
  **24/24**, with its accompanying presentation run failing only the obsolete
  render assertion. Tests cover invalid/unsupported/failed options, a 40-batch
  ignored-abort churn, draft/privacy/identity/stop/owner/dispose teardown,
  recipient exceptions/reentrant stop, StrictMode, native alternatives, no
  implicit PUT and conflict/uncertain explicit revision-bound reconciliation.
- Native **Chrome 156.0.8078.80**, widths **360/390/430**, passes actual editor,
  controller, HTTP and shared-compositor canvas checks. Every displayed candidate
  and main preview matches the existing owner's thumbnail RGBA data. Option
  canvases differ from the main canvas, and accessory variation changes pixels.
  Keyboard activation, touch page/category selection, no browsing PUT, one
  explicit Save, 44px controls, no horizontal overflow, one owner/atlas read,
  bounded late ignored-abort results and policy disposal pass. Asset requests
  carry no Authorization, cookie or referrer. Reports contain no browser errors.
- The real reviewed entry is exercised separately without a reviewed binding:
  unavailable UI, zero canvas and zero asset reads. Screenshots for loaded,
  hair and unavailable states at all three widths are retained beside the report.
  Loaded 360 and unavailable 390 captures were visually inspected.
- Strict project types and scoped lint pass. The production bundle compiles this
  explicit test entry only, to `/private/tmp`, using runner config loading and
  retained dependencies (resolved Vite 8.1.3). Source/bundle hashes are recorded
  in `production.json`; native product-source hashes are in `native/report.json`.
- The planning checker passes its source/provenance/export checks. It certifies
  planning only. Source/evidence hashes and command outcomes are in `checks.json`.
- A separate read-only peer reviewed lifecycle, privacy/owner/stop handling,
  reentrancy, queue bounds, main lease, save invariants and accessibility. It
  reported no actionable findings and confirmed selection focus/Tab behavior
  with a focused Chrome probe. It edited no files and did not run a broad suite.

Initial fixture work exposed a missing hash prefix, overlapping synthetic atlas
regions and a widened JSON type; these were fixed in test scaffolding. An early
new test also waited for renderer activity rather than ready delivery; it now
waits for the observed result. These failed attempts do not qualify the feature.

## Delivery limits

This patch delivers the option-preview increment; it does not claim full FE13,
UIV04, IN05 or release acceptance. Approved character-art binding/presentation,
root/shell/Me/world integration, real backend multiuser behavior, physical mobile
devices and final user/reference acceptance remain **NOT_RUN**. The loaded
fixtures are visibly synthetic and never act as a product fallback. No model or
reasoning override, art generation, source replacement or new dependency is used.
