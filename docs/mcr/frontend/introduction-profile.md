# FE29–FE30 introductions and profiles

Status: **MOCK_READY** for the assigned feature implementation and fixture
qualification. **INTEGRATED / ACCEPTED: NOT_RUN**. Source base
`e6fef53d9f0841b3e16ce3baa4db01a834043957`; branch `codex/mcr-fe-FE29`;
checkout `/Users/lhs/Desktop/mcr-worktrees/fe-introduction-profile`.
Public checkpoint `08bbfb175c387628dc9b498ee27eaf9305af6b1f` precedes the
owner-command/age-status/evidence commit containing this record. Integrate both
commits in ancestry order. Remote master ancestry is delivery-owned and unverified
by this worker. This is a bounded handoff, not real IN03 or release acceptance.

## Ownership and provenance

Only `frontend/src/features/{people,intro,profile,age-status}/**` and this document
are assigned. No backend, root router, shared API, manifests, lockfile, artwork,
generated planning baseline or source archive changes. Source/archive checksums
pass. All UI and orchestration are newly authored. No old Bigyeting renderer,
component, style or external dependency was copied.

Read-only dependencies at the source base: C04 `contracts/domain/v1/{social.schema,
common.schema,operations,policy}.json`, C03 PublicProfile/core fixtures, C06
AvatarConfig; FE02 privacy/shared HTTP and FE27 cancellation discipline; FE13
`decodeAvatar`; FE12 `entities/avatar/runtime` production compositor. Frozen
contract operation IDs and error families are used explicitly. No FE03 shared
helper implementation exists at this base; scoped validation is authored from
frozen C04 semantics without claiming that prerequisite accepted.

Test-only fixtures reuse committed public-intro, canonical auth/capability data,
isolated cookie-policy helper and FE13 synthetic C06 metadata through imports.
Synthetic metadata exercises rejection by the production compositor; no synthetic
character artwork or fallback is rendered. No approved binding means visible
“Avatar unavailable.” Product thumbnails always enter the same reviewed FE12
compositor used by world/editor, never a new renderer.

## Implemented behavior

FE29 uses only `listConcertIntros`: bounded literal `q`, limit20, opaque cursor,
explicit pagination including empty pages, duplicate/cursor-loop checks and
`created_at DESC, intro_id DESC` ordering across pages. It requires no X account.
Closed public decoders reject ages, claims, handles and owner policy fields.
There is no public intro-by-ID endpoint in C04; profile entry uses C03
`GET /people/{userId}`. Denied direct IDs erase matching cached discovery and
cancel older list requests; later lists suppress that author until an authorized
profile read or a new privacy scope. Feedback does not reveal hidden/blocked/
restricted distinctions or raw server details. Profile report/block-management
links use fixed internal routes and canonical validated IDs.

FE30 owns the introduction draft, save, publish and close states. Conditions are
owner-only: both bounds null, or paired integers with `14 <= minimum <= 100`,
`15 <= maximum_exclusive <= 101`, `minimum < maximum_exclusive`. A boundary helper
only validates the form; it never authorizes discovery or computes a claim.
Expiry must be a future UTC instant locally; the server must also enforce the
event cutoff. Publication needs a saved nonempty DRAFT and no unsaved edits.
Close remains an explicit safety action in READ_ONLY and preserves an unsaved
draft. Route unmount does not destroy shell-owned drafts. Conflicts require an
explicit read and choice to keep the draft or adopt the server revision. Cancel
restores the saved draft without dispatching a write.

Each mutation has one original UUID idempotency key, operation, actor/event and
optional resource binding. Unknown transport/503/invalid success outcomes lock
new submissions. Only explicit command-status checks occur; no polling, replay,
replacement key or inferred success. Receipt binding/state invariants are checked;
SUCCEEDED metadata requires an authorized resource read at least at its revision.
404 status, stale reads, wrong actor, expired/retired/failed receipts stay locked.
Access interruption retains that original key and becomes uncertain, permitting
an explicit check when access returns. Privacy/identity changes synchronously
erase private state, cancel work, and fence late/reentrant acceptance.

Age status uses the canonical server status only: UNKNOWN, VERIFIED, EXPIRED,
UNSUPPORTED, PROVIDER_UNAVAILABLE, REVOKED. A local validity timer downgrades an
expired claim and never grants eligibility. Unsupported/unavailable/unknown age
never implies restricted access and does not itself block basic world use.
No age/self-declaration input or public verified badge exists. Verification starts
only explicitly with current writable server availability plus an injected HTTPS
origin allowlist; there is no default provider. Credentials, fragments, foreign
origins and sessions exceeding ten minutes reject. The trusted callback seam
accepts only verification ID/state/provider proof and requires the same pending
user/session and unexpired state. The backend must independently verify proof and
one-use state. Callback proof tokens are absent from controller snapshots; the pending provider
authorization URL is scoped to the owner and erased on invalidation. A receipt cannot
reconstruct a one-shot redirect; status reconciliation reads safe canonical age
status only.

## Delivery composition seams

Construct `createPeopleController({auth,access})`,
`createIntroController({auth,access})` and
`createAgeStatusController({auth,access,allowVerificationOrigin?})` once per active
shell. `auth` is the existing canonical session controller, not a new identity.
`access.read()/subscribe()` must project current server/bootstrap authorization,
feature availability and READ_ONLY. Feed policy/identity invalidations into the
existing `auth.privacy` coordinator. Dispose all feature controllers at shell
teardown before disposing auth; screen unmounts intentionally retain drafts.

Render `People` at `/meet/people`, `IntroEditor` at `/meet/intro/edit`,
`PublicProfile({controller: people,userId,...})` at `/people/:userId`, and
`AgeStatus` at `/me/age`. No world instance or connection is constructed here.
Pass avatar options with the **original authorized bootstrap PrivacyScope** and
resolver for that scope's pinned manifest, reviewed binding and asset resources.
Do not recapture a newer scope to bless old bootstrap assets. Missing/stale art
stays unavailable. PublicProfile accepts `sharing(userId)` and `blocking(userId)`
entry callbacks for the designated consent/block flows; buttons are disabled when
absent. Those callbacks do not represent successful consent or a completed block.
Fixed `/report?targetType=USER&targetId=...` and `/me/blocks` links are exposed.

Delivery owns root wiring and qualified provider callback routing. It must call
`completeVerification` with the bound callback proof, without URL/token logging,
and define a real allowlist from reviewed provider configuration. No callback
route or provider authority was fabricated inside this feature assignment.

## Validation and evidence

Pinned Node24.22.2, retained pnpm11.7.0; offline frozen install exit0 with no
manifest/lock changes. Commands below ran from repository root unless stated;
Node PATH was `/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin`.

| Command | Exit / evidence |
| --- | --- |
| `node frontend/node_modules/vitest/vitest.mjs run --config frontend/vite.config.ts frontend/src/features/people frontend/src/features/intro frontend/src/features/age-status frontend/src/shared/api frontend/src/shared/query` | 0; 252 tests / 8 files, including 40 feature tests |
| `node node_modules/typescript/bin/tsc -b --pretty false` (frontend) | 0; full strict TypeScript including owned fixtures |
| `node node_modules/oxlint/bin/oxlint src vite.config.ts` (frontend) | 0; no findings |
| `node node_modules/vite/bin/vite.js build` (frontend) | 0; existing large-chunk warning; root build alone does not prove feature routing |
| `node frontend/src/features/people/__tests__/browser-smoke.mjs` | 0; actual native Chrome156.0.8078.80, 360/390/430, zero page errors |
| `python3 scripts/mcr/check-plan.py` | 0; provenance/planning only; 13 archive checksums preserved |
| `git diff --check` | 0 |

Native report and six screenshots:
`frontend/src/features/people/__tests__/evidence/native-01/`. The report pins the
exact 25 source/test files by SHA256 and the pre-evidence candidate parent, so its
source evidence can be compared after cherry-pick. Test routes are isolated
MemoryRouter fixtures using real canonical Auth/shared HTTP with mocked server
responses, never product mock composition. Native tests cover no-X discovery,
search/profile/report navigation, denied direct-ID/list consistency, rejected
unreviewed art, owner-only conditions, draft retention across routes/READ_ONLY,
503-after-commit receipt/resource reconciliation without replay, publish/safety
close, provider unavailability, synchronous privacy cleanup, keyboard focus,
44px controls and no horizontal clipping at all three widths. Unit tests also
cover half-open/null/reversed/one-sided bounds, all age states/expiry, redirect
allowlists/callback binding, malformed private projections, ordering/cursor loops,
stale identity responses, throwing subscribers and reentrant cleanup.

## Remaining qualification

Real BE26/BE27 persistence/provider authority, root route/action integration,
approved production character binding, two-account HTTP/WS revocation behavior,
physical iOS/Android, load/performance and IN03 are **NOT_RUN**. An owner draft or
fixture suite cannot certify server enforcement, real age verification, a block,
mutual consent, production artwork or release readiness. No task scope has been
cut. Main/SL approval is not required to integrate a validated bounded handoff.
