# Reviewed Customize production adapters

Status: CONTRACT_READY for the scoped production adapters. Source qualification
PASS at `f23344e7df7bcf472786b55a2ebbecd52561a7ee` under accepted round
`AR03-PRODUCTION-SOURCE-202611041141Z`. Production registry/Account/World/Profile
adoption and public release activation are NOT_RUN by this increment. The original AR03 goal remains
unfinished; its historical BLOCKED metadata is preserved without reset.

## Source and exclusive ownership

Source base: `01d604080670c967e1ce194df5c3d1827df11228` (`origin/master` fetched
2026-11-04). Existing exclusive checkout: `fe-ref-map`, new in-place branch
`codex/mcr-fe-AR03-production-adapter`. The prior branch ending at `5be584ad`
is preserved. Root153's exact source grant supersedes the earlier new-file-only
allocation for the additional necessary authority/controller hunks below.

Owned files: new `reviewed-production.ts` and `production-persistence.ts`;
`runtime.ts` authority discrimination/lifecycle only; `reviewed-manifest.ts`
verified-stage operation helpers only; `controller.ts` catalog-backed Random and
its necessary shared selection function; `MotionEditor.tsx` functional Random;
new `entities/avatar/runtime/__tests__/reviewed-production/**` and
`features/avatar/__tests__/reviewed-production/**`; this document and
`docs/mcr/evidence/customize-reviewed-production/**`.

Root retains all registry, provider, Account, World and Profile composition seams.
No CSS, original/derived art, compositor/draw/UV/anchor code, contract, generated
type, backend, manifest, lockfile or old whole-character file is changed.
Developer validation and fixes belong to the implementation owner. Historical
king/SL/parent review or acknowledgment clauses do not create an approval gate.

## Typed authority and persistence

`PublicReviewedConfig` is extracted from the existing public `AvatarConfig`
wire union. It is not another enum, schema or HTTP client.

`createReviewedProductionAuthority({ auth, scope, pin, signal })` is async. It
performs exactly one protected C03 bootstrap GET through the supplied canonical
Auth owner, copies the synchronous public DTO, runs the existing asynchronous
bootstrap/bundle decoder and exact reviewed-manifest hash/pin check, verifies the
public avatar belongs to that manifest, and fences the response against identity,
event, generation, privacy policy and abort changes. It never changes an operator
registry or migrates a static appearance. `auth` borrows `session`, `privacy` and
`request` from the existing `SessionController`.

The returned `ReviewedProductionAuthority` has `qualification:
'REVIEWED_PRODUCTION'`, immutable scope and manifest pins, closed config/selection
operations, `canSave`, `signal`, `isCurrent()`, `onInvalidate(fn)` and `dispose()`.
Only factory-minted objects pass the private WeakMap proof. Spreading a genuine
object, supplying a staged manifest or changing a TEST_ONLY qualification string
cannot manufacture authority. The existing staged decoder stays
`STAGED_NOT_ADMITTED`; the fixture factory stays TEST_ONLY. Shared verified-stage
operations carry no production grant.

The supplied `signal` is the **registry owner's lifetime**, including pending
initialization. It must not be a view or peer-row signal. Abort or canonical
privacy cleanup synchronously revokes authority and subscribers. Runtime disposal
also disposes its authority. Root owns one factory/runtime per matching pin/scope;
views, self and peers borrow that same owner. A production runtime rejects injected
art loaders, keeps the existing hash-verified asset loader and compositor unchanged,
and revokes on resource-load/hash failure. No fixture fallback is attempted.

`createReviewedMotionPersistence({ auth, authority })` returns
`MotionPersistence<PublicReviewedConfig> & { dispose(): void }`. The Auth object
must be the same owner that minted authority. Reads reuse the existing strict
`decodeParticipant(value, scope)` decoder and scope checks; saves send exactly
`{ expected_revision, config }` to the protected avatar PUT. The exact submitted
config and incremented revision must return. Regressed or inconsistent same-revision
reads fail. Transport conflict/denial/uncertainty is preserved, with no automatic
mutation retry. Disposing persistence cancels its own work, not the borrowed
runtime/authority. Authority revocation cancels pending work and prohibits new
requests. No token, private cache, session or QueryClient is created.

The existing controller retains explicit save/reset/reentry/reconciliation behavior.
Random uses only the actual catalog, keeps preset and layered modes explicit and
does not save. A repeated layered sample gets a different admitted hair choice
without unbounded retries. The editor adds a working Random action alongside Reset
and Save. Existing CSS is frozen; exact native layout with three actions remains
to be measured in the Root-supported route.

## Frozen coverage and pixels

Original specification §5.3 requires six presets, six hairstyles, four eye options,
six outfits, four accessories and selectable skin tones. Existing reviewed v2
provides these counts plus eight body tones. The source pack is
`mcr_user_characters_20261103` revision 3; 314 sheet groups are not 314 runtime items.

The unchanged reviewed family is `mcr_selected_motion_reviewed_20261104`, revision
2, contract 2.0.0, canonical content hash
`sha256:7f2891c81ffa1d5850107118898a7ef9377b222ab8639b3f2c9daf7908cf9018`.
The unchanged atlas SHA-256 is
`1cd8f05af8d76d58243e2f6e143961953b4d4291fdd553df8c8d33aabd700dbd`.
Visual provenance is the existing pinned receipt at
`docs/mcr/contracts/avatar-motion-v2/review/visual-review.json`; source art revision
`bb7d21b3`, art receipt `145d0e21`.

These reviewed derivatives disclose authored reconstruction for body, hair,
outfit, accessories and side/rear poses, supplied eye cutouts and existing semantic
skin/hand palette composition. They are not exact original JPEG pixels and are
not pixel-identical to the admitted static 1.1 family. No new reconstruction,
recoloring, dye controls, synthetic C06 art, legacy fallback or silent static-family
conversion is added. Root retains compatibility and actual public admission.

## Validation and remaining integration

New tests exercise real canonical memory-session/privacy/HTTP implementations
against deterministic public C03 responses, full bundle/hash negatives, copied
proof rejection, stale/foreign identity/policy/abort races, runtime withdrawal,
actual tampered resource bytes, every preset/item ID, nullable slots, independent
coordinates, server read/save/reentry, conflict/uncertainty, Random and cleanup.
Controller-only tests retain explicit TEST_ONLY fixture authority. Original and
frozen atlas hash checks establish byte preservation, not cross-family identity.
The existing motion/public-admission/Auth suites remain included.

The bounded supervisor is `entities/avatar/runtime/__tests__/reviewed-production/qualify.py`.
It requires an accepted named round, exact committed HEAD, absolute exclusive cwd,
Node24 path, >=16 GiB start / 8 GiB stop floors, <=600 seconds including at least
60 seconds cleanup, <=2.5 GiB sampled aggregate RSS and <=128 MiB whole charged
source growth/carry/output/cache/TMP. One Node child runs at a time; Vitest uses
one worker thread. No browser, server, native journey, install, dependency link,
application build or new checkout is started. Existing dependencies are read only.

### Qualified source result

The retained Node runtime was exactly v24.22.2. All six sequential checks exited 0:
15 Vitest suites / 346 tests; application typecheck over 855 files / 0 diagnostics;
scoped oxlint; `git diff --check 01d60408`; and `python3 scripts/mcr/check-plan.py`.
The final `checks-114553.json` contains exact argv, cwd, source pins and cleanup.
No application build or live service/device claim follows from these results.

Three earlier attempts remain recorded separately. The first correctly denied
writes because positive fixtures inherited unavailable mutation capability
(333 pass / 13 fail); the second rejected an inconsistent unavailable/null-reason
fixture (345 pass / 1 fail). Both fixtures now use canonical capability states.
The third passed all 346 tests but found one application typing error: the Auth
schema decoder does not expose PrivateParticipant. Persistence now reuses the
existing strict, session-scoped participant decoder; the final round passed.
No decoder schema widening or unsafe cast was introduced.

The output round was released at 11:46:10.571551 UTC, before its 11:51:41 UTC
hard deadline. Across all attempts peak aggregate RSS was 1,470,464,000 bytes and
charged output/source growth was 80,036 bytes. Release found zero owned processes,
TMP paths, cache or dependency links; the borrowed `.vite-temp` remained empty and
the retained lockfile hash stayed unchanged. Raw reports, failed logs, allocation
and release accounting are preserved under this increment's evidence directory.

After qualified handoff, Root must implement async registry pending initialization
and cancellation, exact discriminated factory/borrowed types, Account controller
and avatar/Profile dispatch, and World family dispatch without changing the old
static path. The actual supported route requires native 360/390/430 checks and
shared self/peer/Profile/public persistence evidence under a fresh output lease.
The prior IN01 static cohort proof is not reviewed-v2 activation or layered-option
coverage. This source increment cannot complete FE12/FE13/AR03 or cut their scope.
