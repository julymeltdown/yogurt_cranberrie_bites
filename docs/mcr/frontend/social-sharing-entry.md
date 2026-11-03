# FE31 default shell sharing entry

Status: implementation and deterministic default-shell qualification **MOCK_READY**; existing C03/C04 binding **CONTRACT_READY**. This handoff does not accept FE31, QA04 or IN03. Live gateway/database/provider, two-user invalidation, physical device and load checks remain **NOT_RUN**.

## Scope and authority

Base: `3f7be971e64d9c6ae422f8c85184e0ec464c07d4`, branch `codex/mcr-fe-social-sharing-entry`, isolated author checkout. Product authority is `MCR_IMPLEMENTATION_PLAN_V2_EN.md` FE31, U04 `/me/social`, U05 `/me/requests`, QA04 and IN03. The required project assessment and execution plan were read without changing the specification. The ten-screen reference was viewed directly: `docs/mcr/design/ui-reference-audit-2026-11-04/user-reference.jpg`, SHA-256 `2b3988d546f87b813d78ca13bf267730a6dd19118bd6535a2d3c3032b6f12d23`.

The assignment owns `app/social-composition/**`, new `app/__tests__/social-sharing-entry/**` and this document. Main explicitly granted `/me/social` registration in `app/router.tsx`, then the canonical RequestsModule import/wiring only in `app/world-panels/social.tsx`. That second exact grant avoids introducing another provider around the retained social owner. Granted `WorldShell.tsx` and `app/ports.ts` needed no changes. The qualified `features/social-sharing/**`, original completed worktree, backend, contracts, lockfiles, manifests, world implementation and `docs/source/**` remain byte-identical to the base.

## Resulting behavior

- The default `createAppRoutes` now mounts `/me/social`, in addition to existing `/me/requests`. Both use the existing root SocialFeatures lifetime. One lazy RequestsModule adapts the qualified FE31 controller; changing panels retains its private draft and command state in memory.
- The adapter starts only when an explicit trusted `services.requests === true` projection matches the canonical privacy scope and exact bootstrap object, and existing C03 `social_sharing.available` permits access. Event phase and mutations capability preserve read-only behavior. Auth success, module presence and C03 alone never establish service readiness. A producer arriving after route entry starts the same lazy module. Withdrawal clears private state; restoration rereads metadata without automatically revealing anything.
- Existing custom `social.requests` factories keep precedence. There is no added C03 capability, API route, detail URL or query contract. An authorized list row opens request detail inside `/me/requests`; its explicit Back returns to the list. Browser POP and shell Back retain their existing semantics.
- The canonical session's `requestSensitive` is the only supplied reveal reader. It makes one no-store attempt without token refresh or replay. Its absence disables reveal. No feature-owned Auth, Query cache, Phaser, world connection, storage copy or second navigation blocker is created.
- The existing blocker now sees FE31 `sending`, `pending`, `uncertain`, `verifying` and terminal `blocked` states. Links, shell Back and POP cannot abandon the original unknown command; inner detail Back is disabled. Explicit status reads use the original operation/key and the qualified controller's actor/event/resource/revision checks. A foreign receipt cannot unlock navigation; success requires the authorized result reread. No mutation is replayed.
- Reveal remains an explicit per-platform action. Hide, detail Back, route departure, readiness withdrawal, canonical policy/identity change and disposal abort/clear private output. Late responses cannot repopulate it. A reveal 401 withdraws the canonical identity and displays the shell's sign-in gate.
- Confirmed handle replacement advances canonical privacy policy and withdraws the old shell lease. Fresh, explicitly qualified bootstrap/readiness may reopen settings. The native fixture deliberately denies unreviewed policy reentry; its unavailable result is the expected fail-closed outcome. The jsdom fixture separately exercises successful fresh requalification.

Profile introduction offer entry is not part of this assignment. Its existing disabled default action remains unchanged; this patch does not invent an introduction target from a user ID.

## Presentation

The existing concert window supplies the frame, title, Back, size controls and icon navigation. The adapter adds compact Social links/Requests navigation and restrained spacing inside that window; it does not add another frame or decorative left accents. The qualified feature's consent, expiry, revocation limitations and 44px controls remain intact. Private drafts and revealed names in evidence are explicitly synthetic.

The native evidence covers 360×800, 390×844 and 430×932 CSS pixels, including the default half panel, expanded keyboard editing, requests, explicit reveal, uncertain navigation, read-only, empty, unavailable and error states. A full panel intentionally suspends the existing socket while retaining its Phaser/camera/owner; it does not create a second connection. The geometric test terrain and unavailable avatar art remain visible limitations. No approved character imagery was replaced, and no new or legacy fallback art was introduced. These captures establish usable composition, not complete visual fidelity or live-product acceptance.

## Provenance and dependencies

Product adapter, route mapping and scoped CSS are newly authored. The existing FE31 controller/UI are imported unchanged from the base, preserving the earlier qualified receipt/privacy fixes. `native.tsx` derives from base `app/__tests__/default-shell/qualified.tsx`, retaining its actual default routes, canonical login/HTTP, FE14 loader, production world composition, FE15 camera and C05 MockSocket instrumentation; only its local sharing fixture/control seam differs. `service.ts` derives its DTO metadata from existing `contracts/fixtures/domain/approved-social-request.json` and explicitly substitutes synthetic bound actors/event and long-lived test dates. It is never imported by a product entry point.

No dependencies were added or installed. Validation used retained dependencies via a temporary symlink to the integration checkout's `frontend/node_modules` and pinned Node 24.22.2. The temporary symlink is excluded from the handoff and removed on completion. Native Chrome is the existing local executable; no physical device or provider session is claimed.

## Validation and review

Exact commands, exit codes, source hashes and final results are recorded alongside [the evidence](../../../frontend/src/app/__tests__/social-sharing-entry/evidence/). Source pins distinguish author source from later evidence/documentation commits. The planning check remains a planning/provenance check; its result is separate from application validation.

The source review was a bounded read-only sidecar covering `sharing.tsx` and the exact owner/panel/route/root wiring diff against the base. Reviewer `01a1a335-1fa0-7da0-9115-51545e8572ec` reported no actionable findings and confirmed the qualified feature was untouched. It did not rerun tests or independently verify live behavior and was closed after reporting. During author validation, late trusted-readiness arrival exposed a concrete lazy-start defect; the owner now starts the same module upon that matching readiness notification. Dedicated default-route tests cover initial unavailability, arrival, withdrawal and restoration.

## Remaining qualification

The integrator may serially cherry-pick this committed handoff after comparing exact changed paths and base preimages; no Main or scrum-lead code approval is required. Only the designated integrator pushes master. Live service readiness must be supplied by an authoritative producer. Gateway/DB authorization, real reciprocal two-user grants, block/suspension/expiry propagation, physical mobile/browser, load, QA04 and IN03 remain separate acceptance work and **NOT_RUN** here.
