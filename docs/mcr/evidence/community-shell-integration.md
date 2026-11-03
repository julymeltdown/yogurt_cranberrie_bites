# Giveaway routes and shell ownership

Status: **MOCK_READY**. The default application router now opens giveaway
discovery, detail and own activity at `/gifts`, `/gifts/:giveawayId` and
`/gifts/mine`. This is production composition code qualified against deterministic
HTTP fixtures, not a functioning Concert backend or a release gate.

## Reviewed inputs

FE24 worker `5f1005d` and receipt `595c07f` are integrated as `3cb3f9c` and
`8c33ba3`. All 51 changed paths match the assigned frontend/document boundary.
The parent verified all 50 receipt file sizes/hashes and 177 candidate source
hashes in the worker's native report. Independent review `1e85b25`, corrected
by `1a1bbd0`, is integrated as `fe2caab`/`9921759`: no implementation findings.
The report distinguishes host Node 26 checks from pinned Node 24 checks.

## Composition behavior

- One shell-owned giveaway controller survives panel navigation and closing back
  to World. Entering World alone does not start a gift request.
- Components use the canonical Auth session, FE02 privacy coordinator and shared
  transport. There is no second identity store or request client.
- Bootstrap capabilities retain the exact privacy generation that authorized
  their read. Policy changes immediately clear feature state and release world
  resources, then require a fresh bootstrap before restoring the shell.
- Suspended/closed access stays closed. Retry cannot manufacture revalidation.
  Late bootstrap and feature responses cannot restore another identity or an
  obsolete permission decision.
- Effect-owned controllers tolerate React StrictMode setup/cleanup. World and
  feature resources are released on exit. Public sign-in does not load the
  community or Phaser bundle.

## Executed validation

Retained Node **24.22.2**, pnpm **11.7.0**; commands from repository root unless
specified. All final listed commands exited **0**.

- `pnpm --dir frontend test src/app/__tests__/community.test.tsx
  src/app/__tests__/shell.test.tsx
  src/features/gift-discovery/__tests__/contract.test.ts
  src/features/gift-discovery/__tests__/controller.test.ts
  src/features/gift-discovery/__tests__/ui.test.tsx`: **105 tests**, five files.
  Six composition cases cover actual default routes, deep links, navigation,
  StrictMode, refresh, revocation, obsolete bootstrap, suspension and replacement.
- `pnpm --dir frontend build`: strict TypeScript and Vite pass. The first run
  found an exact-optional-property mismatch in the test setup; it was corrected.
  Existing main and new community chunks exceed the 500 kB uncompressed warning
  threshold; optimization remains required before release.
- `pnpm --dir frontend lint`: pass.
- From `frontend`, `node src/app/__tests__/community-browser.mjs` with
  `MCR_COMMUNITY_EVIDENCE` pointing at `docs/mcr/evidence/community-shell`:
  installed Chrome **156.0.8078.80**, **390×844** and **1280×900**, pass.
  [Report](community-shell/report.json) records source hashes and resource counts.
  Native checks exercise actual default router composition and FE01 transport,
  focus/Back, own activity, one instrumented world owner, no extra list fetch on
  return, synchronous revocation, fresh disabled capability, cleanup and built
  public-page bundle isolation. Parent inspected mobile list and desktop own
  activity screenshots; no decorative left accent is present.
- `python3 scripts/mcr/check-plan.py`: pass; source archive remains unchanged.

## Limits and ownership correction

The browser data is synthetic and the world lease is instrumented. These runs
do not prove real Phaser, WebSocket, backend giveaway endpoints, reservations,
approved character art, physical devices or multiplayer. No FE24 whole-task or
IN01–IN05 acceptance follows. Required accepted tasks remain **11/112**.

During this integration the former BE04 writer resumed in the main checkout and
created `1de15ca` (boundary tests) and `18635e0` (identity handoff). The integrator
stopped that writer and reviewed both commits before publication. The dependency
allowlist matches already integrated manifests; the Auth-only local Profile
test dependency is now checked with complete package equality, preserving all
other package fields. The selected admission architecture remains unchanged and
explicitly unimplemented. Correction `c66eb8a` passed all **12** focused Concert
core helper tests. No backend application behavior or frontend files were
changed by that writer. Shared Rust/native validation remains BE08-owned.
