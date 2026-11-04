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

## Renewed feature-side Account editor owner

Owner integration increment: CONTRACT_READY. Source qualification PASS at
`fb184de331b44fc1454115460fa32b44020d052d` under accepted round
`AR03-REVIEWED-OWNER-SOURCE-202611041229Z`. The earlier 346-test qualification
and owner-verified 31-file publication at `a4da2cb4` remain historical evidence
for the preceding adapter source.

`createReviewedMotionEditorOwner({ auth, scope, authority, runtime, onCommitted })`
borrows the same factory-minted authority and shared production runtime supplied
by Root's registry. It verifies the exact authority object, Auth identity, full
privacy scope and both owners' current status before creating the existing
production persistence and motion controller. It performs no bootstrap/factory,
Auth/session, game, socket or art-owner initialization of its own.

Account owns the returned owner. Call `owner.start()` for explicit initialization,
or let `MotionEditor` start its borrowed `owner.controller` idempotently. Read,
select, choose, Random, Reset, Save, reload and reconciliation remain the existing
controller API. The production controller carries `lifetime: 'owner'`; editor
view unmount does not stop its work, abort a save or discard the shared Account
draft. Existing plain controllers retain their view start/stop behavior. Save is
disabled when the authenticated production authority disallows mutations; reads
and drafts remain available. All catalog-backed preset/layer/body controls use
the existing functioning component, with no new art or CSS.

Call `owner.dispose()` when Account ownership ends. It removes its authority
subscription, disposes its controller and cancels its persistence work only;
it never disposes the borrowed runtime or authority. Authority withdrawal clears
the feature owner synchronously, and late receipts cannot publish committed
state. The optional committed callback receives only current matching-scope
receipts. The existing controller reports a callback failure as saved with a
refresh needed, without retrying the accepted mutation.

Root retains every registry/provider/Account/World/Profile composition path.
This feature-side owner does not activate a public release, migrate a static
appearance or establish actual native or shared-player acceptance. The original
AR03 goal remains unfinished with its actual BLOCKED metadata preserved.

### Owner qualification and delivery limits

The first allocated attempt passed all 365 tests across 16 suites, including
19 new owner/component cases. The full application type check covered 857 files
with zero diagnostics. Retained Node v24.22.2, scoped oxlint, source whitespace
and planning checks all exited 0. Exact commands, source hashes, scope, runtime,
start/finish times and cleanup are in `owner-integration/checks-122940.json`.
No implementation or test fix was required after the run.

Owner tests pass factory-minted public authority and the actual shared runtime
through canonical Auth/privacy/HTTP implementations against deterministic C03
responses. They read and hash-check the frozen descriptor and atlas bytes.
Native image decoding and canvas drawing primitives are mocked; these are
component/lifecycle tests, not pixel, browser, live backend or multiuser proof.
They exercise all six presets, all 28 independent items, nullable wardrobe slots,
Random/Reset/Save, capability-disabled Save, view remount during draft/save,
StrictMode initialization, conflict/uncertainty reconciliation, callback failure,
late read/save cancellation, proof/scope negatives and canonical privacy withdrawal.
A separately acquired runtime consumer lease remains usable in the unit test
after Account owner disposal.

The round accepted at 12:29:10.402821 UTC and fully released at
12:30:03.153592 UTC, before its 12:39:10 UTC hard deadline. Peak aggregate RSS
was 1,465,221,120 bytes; sampled entire charged carry/output peaked at 153,804
bytes and release accounting retained 155,844 bytes before its release record.
This conservatively includes all retained own source/tests/evidence and every
old attempt. Zero owned processes, TMP/cache, dependency links or borrowed
bytecode remained. The borrowed `.vite-temp` stayed empty and lock hash unchanged.
The raw Vitest log is preserved, including its EOF blank line. Qualified source
whitespace exit 0 predates the documentation/evidence-only delivery commit.

Root's renewed shared work owns one async registry authority/runtime per scope
and pin, static 1.1 compatibility, actual Account/Profile/World dispatch and the
renderer production proof guard. This leaf supplies only the Account-owned editor
adapter and functioning existing component. Production adoption and fresh
360/390/430 native/shared self-peer/Profile persistence acceptance remain open.
No Meet assignment, backend, public contract, CSS, art or shared entrypoint change
is included. Publication follows the named Root153 route without a review or
acknowledgment gate; the implementation owner's remote proof follows publication.

### Owner publication receipt

The 15-path owner handoff at `998b11a30fd6bed271ce9e855803c90c4128df15`
was published normally as `029b445cae93237b0e8736d40788171abfb46a4f`, parent
`e74712b6717d9394b93413d5dd4e0aa87379dc38`. The implementation owner independently
fetched master and checked `ls-remote` at 2026-11-04 21:51 KST. Both reported
`2bd046796acdf32a05e6ed04e4d95e12c9909481`; the publication is its ancestor.
All 15 published paths and blobs exactly match the committed source handoff and
remain identical at that fetched head. All three qualified frontend hashes match.
The original raw unit log remains unchanged: its EOF blank line is the sole
published whitespace warning (exit 2); non-log whitespace exits 0.

Exact receipt: `owner-integration/publication-receipt.json`. No new runtime work
or tests were started for this read-only remote proof. The leaf lease is delivered
and published, with no resource reservation held; its exact feature paths remain
available solely for concrete Root-discovered feature defects. Shared source
adoption is queued/NOT_RUN and remains Root-owned. This publication does not
activate the release or close native/shared-player acceptance. The original AR03
goal was freshly read as BLOCKED and unfinished. This receipt/document update is
metadata only and does not require another implementation or acknowledgment gate.
