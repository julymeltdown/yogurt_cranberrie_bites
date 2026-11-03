# Social composition — bounded frontend handoff

Date: 2026-11-04. Status: **MOCK_READY**. The provider/owner/route seam is implemented and exercised with actual canonical MCR Auth, HTTP, privacy and feature controllers. Product root installation, real server qualification, real Phaser/socket continuity, approved avatar/media producer wiring, verification provider, sharing/reveal flows and IN03 remain **NOT_RUN by this worker**. No product scope is removed by this handoff.

## Ownership and provenance

Worker branch: `codex/mcr-fe-social-composition`; original base `f1dee537af728475b3e47300de20de34777e05c2`. Exact implementation scope is `frontend/src/app/social-composition/**` and this document. The separately transferred inbox full-count follow-up owns only its named controller/contract/UI/count test/fixture/browser/evidence paths and `docs/mcr/frontend/inbox.md`. There are no root/router/shared contract/manifest/lockfile/CI/backend implementation edits in this worker's patch relative to published master.

All composition rendering and orchestration are newly authored. Existing MCR headless controllers and UI are reused by import, never copied from legacy Bigyeting rendering. Dependencies are the retained React 19, React Router data router, canonical Auth/transport/privacy, existing feature modules and generated public contracts. No new dependency, install or lockfile resolution was introduced during the refresh; the existing ignored checkout-local dependency directory was reused.

Authorized prerequisite ancestry:

| Source revision | Worker ancestry / purpose |
| --- | --- |
| `fb44efbe83fb9557f8ef0214ee1109265c1fcaab` | `7a8c345`: initial inbox |
| `021a191adc1fbde7e5b01e71a2ae5406c2ae9fc9` | `db5b867`: required receipt/read-all recovery fixes |
| `08bbfb175c387628dc9b498ee27eaf9305af6b1f` | `1732ace`: people/profile dependency |
| `3477041257187806392119a25a8c15218d2a84e5` | `59ee7e8`: introduction/age dependency |
| `a25b85592b906209f4a9d20f9c7279ced8abab02` | `47bebae`: additive C04 full-count contract dependency |
| `e677f7929f14ab7aa8547ca94145cc68b4ae23b3` | canonical full unread-total consumer; retained through refresh |
| `fe2167f` | preserved all 22 composition WIP paths before refreshing dependencies |
| `e869480395b70b4aae6cfcf50e904b079a59a0e7` | published master merged as `9e62f70`; authoritative current C04/Unicode/visual prerequisites |

Main authorized exactly 16 dependency-only conflict resolutions: domain operations/policy, domain fixture index and two mural fixtures, generated OpenAPI/operation registry, recovery-reads document, intro editor/contract, people UI/contract/controller, and three domain validation/test scripts. Each was verified byte-identical to `e869480`; no semantic dependency edit or regeneration occurred. Owned inbox conflicts retained `e677f79`. Earlier screenshots/reports remain in committed ancestry; fresh reports are source-hashed observations on the refreshed base. `docs/source/` remains byte for byte unchanged.

The final handoff is the first committed version of this document plus subsequent concrete review fixes. Delivery should integrate the final owned-path diff from `e869480` rather than replaying older prerequisite cherry-picks already present on master. Only the designated delivery integrator publishes master.

## Public seam and lifetime contract

`index.ts` exports `SocialFeatures`, `SocialNavigationBoundary`, `SocialRoutePanel`, `SocialPanel`, `useSocialOwner`, `useSocialUnreadTotal`, `createSocialOwner`, route parsing and typed ports. `owner.ts` creates one child-controller set per authorized shell lease; it never constructs another session, QueryClient, game or world connection. Reads start lazily when their panel is entered. Returning to a panel refreshes authorized observations without replacing drafts or unresolved command identities.

FE07ROOT owns the root insertion and all application routes. Mount `SocialFeatures` **once inside the existing authenticated data-router shell, outside route-switched elements**. Supply the same canonical `SessionController` already supplied to the shell/world. Pass the **original privacy scope captured with the accepted bootstrap**, not a newly captured scope that would bless stale bootstrap data. Preserve exact bootstrap/scope object identities and stable port/factory/navigation references across panel renders. A new auth/bootstrap/privacy lease intentionally replaces and disposes the old owner.

The composition shape for the root owner is:

```tsx
<SocialFeatures
  auth={canonicalAuth}
  bootstrap={acceptedBootstrap}
  scope={bootstrapPrivacyScope}
  availability={socialAvailability}
  navigate={navigateSocial}
>
  <SocialNavigationBoundary>
    {/* Keep the existing world host and its scoped ports in this stable shell. */}
    <WorldHost {...existingWorldProps} />
    <Outlet />
  </SocialNavigationBoundary>
</SocialFeatures>
```

This is an insertion contract, not an additional root implementation. The existing `PortsProvider` and data router must already enclose it. `SocialNavigationBoundary` uses the data router's single blocker facility; combine it with any existing root blocker instead of registering competing blockers. Controlled feature navigation calls `owner.navigate(path)`. Ordinary Links and POP/Back are covered when the boundary is mounted. The boundary retains the current panel while a command or target-specific edit needs recovery; it never overrides a command lock or silently discards a draft. Hard document unloads are outside this seam's router guarantee.

`navigateSocial(destination)` must recheck `canonicalAuth.privacy.isCurrent(destination.scope)` immediately before calling the existing router's navigate function. Do not cache a successful authorization preflight as permission for the destination. Its actual feature performs a fresh canonical read. Do not mount feature controllers separately alongside this owner.

Install `SocialRoutePanel` (or `SocialPanel` with the exact path) for:

| Routes | Real feature used |
| --- | --- |
| `/feed`, `/notices/:noticeId` | EventFeed / EventNotice |
| `/feed/new`, `/feed/:postId`, `/feed/:postId/edit` | PostComposer / FeedThread; direct edits wait for current authorized ownership |
| `/meet/people`, `/people/:userId` | People / PublicProfile |
| `/meet/intro/edit`, `/me/age` | IntroEditor / AgeStatus |
| `/notifications` | Inbox including canonical full-count observation |
| `/me/requests` | Explicit optional requests module; unavailable when absent |

Use canonical UUID route IDs. Route parsing rejects unsafe/unknown/encoded/query-bearing social destinations. Other safe application routes remain root-owned. Do not send `/world` through `SocialRoutePanel`; keep the existing world visible/covered/blocked according to root behavior.

`SocialAvailabilityPort` is an explicit **producer readiness and authorization projection**, not a new bootstrap wire capability. `read()` returns null or `{ scope, bootstrap, services }`, using the exact accepted bootstrap object and all five privacy scope fields. Service keys are `feed`, `people`, `intro`, `age`, `inbox`, `requests`; absent/false stays unavailable. Feed additionally requires the existing feed capability. Mutations respect the current bootstrap mutations capability and READ_ONLY phase through existing feature access ports; inbox SAFETY commands retain their existing rules. A ready test fixture is never evidence for a production availability flag.

`useSocialUnreadTotal()` observes the independent canonical count only while the owner and inbox producer are current and the count is ready. It returns null for unknown/stale/loading/error/unavailable state, never a loaded-row count or fabricated zero. This hook does not start background inbox polling; root may explicitly start the owner's existing inbox controller if its badge needs a server observation before the inbox is visited. Do not create a second badge controller.

## Privacy, navigation and optional producers

The owner registers parent cleanup before starting child lifetimes. Privacy/logout/identity/event/policy withdrawal disposes all child controllers, cancels preflights and media/optional requests, clears private observations and withdraws context. Delayed non-cooperative reads cannot restore previous private DOM. StrictMode creates a fresh owner after its discarded effect, leaving one active set. The root continues to own Auth and world disposal; composition never disposes the canonical Auth itself.

`notifyPolicy({ scope, reason })` accepts only the exact captured lease and reviewed policy reasons (`block`, consent revoke/expiry, handle replacement, suspension or event closure). Extra/private payload fields are rejected. Trusted root policy integration must reauthorize a new bootstrap/privacy lease after withdrawal; do not reuse the prior bootstrap with a new scope.

Unresolved writes preserve their existing operation, idempotency key and drafts. The owner blocks panel/target navigation while sending/pending/uncertain/retired commands, age verification or uploads need resolution. Target-specific reply/edit drafts cannot be silently overwritten by another post. There is no mutation replay or automatic replacement key. Inbox preflight and feed resource preflight both use canonical authorized HTTP; stale or denied results do not navigate. Denied resource links are removed from both feed and thread projections.

Optional producer boundaries deliberately remain explicit:

- `requests.create(context + access)` owns a scoped module with `start`, `dispose`, `read().locked`, `subscribe` and `Panel`. No FE31 implementation or sharing target/request body is inferred here.
- Profile sharing/block callbacks are absent and disabled until their concrete owners supply reviewed root behavior. This seam does not claim reveal/revoke/block flow acceptance.
- `media.create(context)` supplies the existing upload and preview interfaces plus cleanup. Absent media leaves text posting available and image upload unavailable.
- `avatars.resolve(context + signal)` supplies reviewed avatar resources under the original bootstrap scope. No character image, fallback, generated design or legacy art is supplied by this seam.
- `verification.allowOrigin` must come from a trusted reviewed provider configuration. Completion is `owner.completeVerification(originalScope, proof)`, which rechecks the lease and existing controller rules. No provider/callback route is fabricated.

## Validation and evidence

Node 24.22.2 and retained dependencies were used. The following commands completed with exit 0 on the refreshed base:

```text
node frontend/node_modules/vitest/vitest.mjs run src/app/social-composition src/features/inbox --root frontend
node frontend/node_modules/typescript/bin/tsc -p frontend --noEmit --incremental false
frontend/node_modules/.bin/oxlint frontend/src/app/social-composition frontend/src/features/inbox
node frontend/src/app/social-composition/__tests__/browser-smoke.mjs
node frontend/src/features/inbox/__tests__/browser-smoke.mjs
python3 scripts/mcr/check-plan.py
git diff --check
```

Focused suite: **126/126** (43 composition and 83 inbox cases). Composition coverage includes safe/direct routes, exact bootstrap/scope readiness, preserved drafts, optional producer failure, scope-bound verification/policy rejection, target denial/late preflight cancellation, command identity retention, actual data-router navigation/POP blocking, StrictMode and delayed response withdrawal after identity/policy changes.

Native Chrome `156.0.8078.80` production-build fixtures passed at composition widths **360/390/430**, inbox **360/390/430/1280**. Source-hashed reports and screenshots are under each module's `__tests__/evidence/`. The 390 px inbox/pending screenshots were visually inspected: readable controls, retained draft/recovery affordance, neutral symmetric borders and no horizontal overflow. Assertions cover keyboard focus, at least 44 px buttons, no browser errors, total37 with one loaded unread row, stable existing WorldHost element with peak one **instrumented test** game/connection lease, draft return, fresh inbox target preflight plus destination reread, denied profile eviction, absent-producer controls, and privacy clearing. Link and router POP attempts retain the exact pending route/key/draft with only one POST. Inbox proof covers positive full count with an empty loaded page, read-all server cutoff/new arrivals and status recovery without mutation replay.

These are deterministic HTTP and instrumented world fixtures built with Vite, not real multiuser, Phaser, socket, physical-device, live privacy/provider or release proof. The current root Vite production build also exits 0 into a temporary directory removed after the check; this proves the merged application builds, not that root has installed this seam. Planning check verifies provenance/structure only. FE07ROOT must install and qualify the seam; backend and QA owners must run the required live vertical gates. No new acceptance or release claim follows from this document.

The combined feature suite also completed with exit 0, **218/218 across 15 files**, from `frontend/`:

```text
node node_modules/vitest/vitest.mjs run src/app/social-composition src/features/inbox src/features/event-feed src/features/feed-thread src/features/people src/features/profile src/features/intro src/features/age-status
```

A first broad invocation from the repository root failed one existing Unicode oracle case because that test resolves its Python helper relative to the working directory. Running the same suite from `frontend/` passed; no dependency file was edited. An intermediate newly added count test also failed TypeScript because the fetch spy's input union was not narrowed to Request; the test was corrected and the final typecheck passed. These failed attempts are retained here rather than presented as clean initial runs.

## Targeted peer review fixes

A separate read-only review reproduced three concrete defects on merge checkpoint `9e62f70`. All are corrected in the final owned patch:

- Direct-edit route effects restarted an edit immediately after Discard. Initialization is now once per route entry; ending/discarding exposes a route back to the post and permits a fresh composer. A related guard prevents the thread UI's edit action from overwriting a retained new-post draft before its navigation callback. StrictMode tests and native edit/discard/draft-return checks pass.
- A delayed inbox target lookup could navigate after the user chose another panel. Both accepted navigation intent and direct route entry now cancel inbox and resource preflights immediately; separate delayed-result regressions cover both boundaries.
- Same-scope producer loss cleared an uncertain inbox command's original binding and allowed a replacement key. Private rows/count/targets still clear immediately, but command binding, receipt and attempt restrictions remain in memory for the same valid lease. Interrupted sending/checking becomes uncertain. Explicit recovery after availability returns uses the original key; it does not replay PATCH. Sending/uncertain tests and the native pending-availability-recovery sequence pass. Policy or identity withdrawal still destroys the old command binding.
