# FE31 private social sharing

Status: **IN_PROGRESS**. C04 contract binding is **CONTRACT_READY**; isolated controller and component coverage is **MOCK_READY**. Root composition, real gateway, multiuser privacy, device, QA04 and IN03 acceptance are **NOT_RUN**. This document does not accept FE31 or the social gate.

## Ownership and provenance

Assigned checkout: `/Users/lhs/Desktop/mcr-worktrees/fe-social-sharing`, branch `codex/mcr-fe-FE31`, base `5c95ed4ddb243853b27fb625f726341b9771dadd`. Feature ownership is only `frontend/src/features/social-sharing/**` and this document. All UI, state and validation are newly authored; no legacy rendering or Bigyeting source was copied. Existing canonical Auth, HTTP and privacy owners and the pinned frontend lockfile are reused without adding dependencies. `docs/source/` remains untouched.

Product authority: `MCR_IMPLEMENTATION_PLAN_V2_EN.md` FE31, U04 `/me/social`, U05 `/me/requests`, introduction sharing entry, QA04 and IN03; implementation sequencing follows `docs/mcr/project-assessment.md` and `docs/mcr/execution-plan.md`.

Pinned local contract inputs at the base:

| Source | SHA-256 |
| --- | --- |
| `contracts/domain/v1/social.schema.json` | `b275b36f2ad8eee90922f7ad27cfcbece943773a94831720b6dfca588d0201d2` |
| `contracts/domain/v1/operations.json` | `42ed48ee0cad846b94f9f7a5d39a38beeeb5f5b41e0129f157e51ff31c635223` |
| `contracts/domain/v1/policy.json` | `508d22af964b718cdcbd5a25b32b9a1e3f7b651a5b8c25c87d56f1f6b68d0ebe` |
| `contracts/fixtures/domain/approved-social-request.json` | `f4c55a2fb56c7086a03984cb345df491d753c27ceca31b6c1c2545d6cd4c295a` |

The approved-request fixture supplies synthetic metadata for tests only. Backend BE37/BE28–30 handoff `a5e84da` confirms normalization, per-platform versions, whole-request invalidation after handle changes and reciprocal reveal. It is compatibility evidence, not proof that a public gateway or fenced authority is running.

## Behavior and privacy

`model/contract.ts` strictly decodes public metadata, reciprocal consents, private reveal and command receipts. Unknown private fields, duplicate platforms, unrelated actors/events, wrong owners/versions and invalid expiry are rejected. Removed handles retain monotone version tombstones. Handle input is lowercased and trimmed; platform syntax is validated before sending. Plaintext is never returned by the handle-list endpoint.

`model/controller.ts` owns transient input, metadata and one ephemeral revealed account name. Requester consent is opt-in for each platform and binds both current configured versions and the current introduction revision. Recipient approval requires an explicitly selected offered subset and a separate reciprocal consent acknowledgement. Reject, cancel, revoke and expiry display server metadata without manufacturing grants or age eligibility. No follow action exists. Revoking cannot erase names already seen or copied, and unblocking does not restore consent.

Sensitive reveal is explicit, single-attempt, protected and `no-store`. Nothing prefetches, retries, persists or inserts the result into Query caches. The controller validates counterpart identity, platform and version, checks expiry before and after dispatch, and clears the name on hide, unmount, another read, account/event/policy invalidation, handle mutation, revoke, expiry and disposal. Late responses cannot restore it. Canonical privacy cleanup also clears handle drafts and consent choices. Access loss clears visible data and drafts; an unresolved command identity remains available only within its unchanged canonical scope.

Every domain mutation freezes a new idempotency key with its original body and captured scope. A lost result or pending202 locks new writes. Explicit status reads bind actor, event, operation, key and resource/revision; no uncertain mutation is automatically replayed and no success is fabricated from a pending/retired/malformed receipt. Confirmed success reloads authoritative metadata. No plaintext handles enter status queries or public receipt state. Normal writes stop in read-only mode; remove/cancel/revoke remain safety actions. Backend authorization and reciprocal grant enforcement remain authoritative.

## Composition handoff

Create one `createSocialSharingController({ auth, access, sensitiveRead: auth.requestSensitive })` per active canonical shell. `access` supplies current membership/event availability and read-only state; route-local mock authority is prohibited. Start the controller once, dispose at shell teardown, and notify canonical privacy of block, handle, consent and membership changes. The production clock defaults to `Date.now()`; align with canonical server-time handling at composition if required. The test clock does not establish real authority.

- U04: `loadHandles()` and render `HandleSettings`.
- U05: `loadRequests()` and render `SharingRequests`; its `onRequest(id)` routes and calls `loadRequest(id)` before rendering `SharingRequest`.
- Current introduction entry: `loadHandles()` plus `loadOffer(introId)`, then `SharingOffer`. Reload its offer explicitly after stale-revision failure.
- Detail: use a route-specific retry callback for `loadRequest(id)`. Unmount clears the revealed name; do not copy it into route state, caches, analytics, logs, storage or persistence.

Without the qualified sensitive port, reveal fails closed. The canonical session single-attempt dependency was separately delegated and committed as `c5a23cb` (delivery equivalent `7e7179b`); it protects both `requestSensitive` and generic request entry paths. The separately delegated enrollment fixture repair is `b5c7468`: cancellation asserts the actual fetch Request signal while payload clones remain for body inspection. Pinned Node24 native GC reproduction demonstrated actual-signal=true/clone-signal=false; this is a fixture repair, not a production HTTP change.

No feature commit modifies root routes/composition, shared contracts/manifests, backend, world, avatar, generated assets or CI. The integrator owns wiring and real vertical-flow checks.

## Validation and delivery

Commands use pinned Node24.22.2 and pnpm11.7.0, with dependencies installed offline from the retained frozen lockfile (exit0).

| Check | Result |
| --- | --- |
| `vitest run src/features/social-sharing --maxWorkers 1` | exit0, 26 tests:21 controller and5 component |
| `vitest run src/features/auth/__tests__/sensitive-read.test.ts src/features/auth/__tests__/session.test.ts src/features/auth/__tests__/enrollment.test.ts --maxWorkers 1` | exit0,80 tests after narrow cancellation fixture repair |
| `python3 scripts/mcr/check-plan.py` | exit0; planning only |
| `node scripts/mcr/check-ui-origin.mjs` | exit0; source-origin only |
| Feature lint and `git diff --check` | exit0 at first controller milestone |
| Browser360/390/430, feature build, final typecheck | Pending |

Committed ancestry before the first feature milestone: base → `c5a23cb` single-attempt Auth dependency → `b5c7468` cancellation-test repair. Delivery receives feature commits directly and integrates serially; this worker never pushes master. Browser evidence, final checks, subsequent feature commits and concrete review fixes will be recorded here before bounded completion.
