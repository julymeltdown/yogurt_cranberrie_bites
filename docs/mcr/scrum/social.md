# Social delivery tracker

Checkpoint: 2026-11-04, Asia/Seoul. Coordination base:
`f0e44bd1bba943b02319f9d9a42433e253e8de6f`.

This tracker records observed work and dependencies, not task acceptance. The
planning task states in `generated/` remain a baseline. Main and scrum leads
track progress; implementation owners send validated commits directly to
delivery without coordinator approval. See `../parallel-delivery.md`.

## Ownership and goals

- Social SL: `01a1a1fe-ca12-7cc3-9a1d-62bbb2689498`, checkout
  `/Users/lhs/Desktop/mcr-worktrees/sl-social`, branch `codex/mcr-sl-social`.
  Sole writable path: this file. Own unbudgeted persistent goal verified ACTIVE
  through `get_goal`; no application-code or shared-file ownership.
- Delivery: `01a1a1fc-7f3d-7ff1-9416-e3435c8a6f08`; sole master publisher and
  root/shared composition owner. Exact module exports require its coordination.
- Backend: `01a1a1fe-16e1-7ee3-a9dc-8b5f037cc24c`, isolated
  `be-social-policy`, branch `codex/mcr-be-social-policy`, base `f0e44bd`.
  Owns relationship directories in concert-domain, concert-application and
  concert-service; relationship tests/support/scripts; lane evidence
  `docs/mcr/backend/relationship-policy.md`; expressly delegated relationship
  exports and `0006_relationship_policy.sql`. No frontend or Auth/Profile/
  Gateway/proto/contract/root-startup ownership. Worker reported actual `get_goal` ACTIVE with the unbudgeted BE37 plus
  BE28–30 implementation/validation/documentation/handoff/fix objective. The
  committed API and subsequent service work independently demonstrate activity.
- Frontend: `01a1a1fe-1ab7-79b0-adbc-d4c025701acb`, isolated
  `fe-social-feed`, branch `codex/mcr-fe-social-feed`, base `f0e44bd`.
  Current source activity is under assigned `frontend/src/features/event-feed/`
  and `frontend/src/features/feed-thread/`; no backend or root/shared ownership.
  Worker reported actual `get_goal` ACTIVE with the unbudgeted FE27/28
  implementation/validation/documentation/handoff/fix objective. Required FE28
  image work and browser qualification remain in scope.

- FE04: `01a1a202-375e-7401-bfa1-ff8721e90ed6`, isolated
  `/Users/lhs/Desktop/mcr-worktrees/fe-media-upload`, branch
  `codex/mcr-fe-media-upload`, base
  `48458d597af73bd695c8d5813b123c6a96e55321`. Delivery reserved
  `frontend/src/shared/media/**`, `frontend/src/features/media/model/**`
  including local tests, and `docs/mcr/frontend/media-upload.md`. No existing
  shared API/root/manifests/contracts ownership. Worker reports an active goal;
  actual result requested at milestone. Typed port already sent directly to FE28
  and community FE `01a1a1fd-6ec9-71e2-a2c6-b5dddca7542e`.

## Assigned delivery

| Tasks | Status | Observed deliverable / validation | Commit / published ancestry | Next action and blocking owner |
|---|---|---|---|---|
| BE37 | IN_PROGRESS | Typed API committed and sent directly to community, admission, feed and delivery. Pinned Rust1.88 rustfmt and planning check exit 0. Cargo / owned PostgreSQL NOT_RUN. | `bcd2cd25ea6e80c1d4263934f632bd4dc428e103`, six paths verified through Git metadata; published ancestry NOT_RUN. | Backend implements Pg policy, receipt-backed block producer and migration0006; delivery schedules Cargo and real authority composition. |
| BE28 | NOT_STARTED | Private encrypted versioned handles remain required; no implementation evidence yet. | None observed. | Same backend writer follows BE37; managed crypto secret/real owner composition cannot be fabricated. |
| BE29 | NOT_STARTED | Explicit reciprocal, platform-bound decisions, receipt/outbox, expiry/revoke remain required. | None observed. | Same backend writer; reuse BE15 transaction seam and fresh BE37 policy. |
| BE30 | NOT_STARTED | Current grant/version/expiry/block/suspension recheck and no-store reveal remain required. | None observed. | Same backend writer; delivery owns external route/composition delegation. |
| FE27 | MOCK_READY | First controller/UI slice: chronological filters/cursors, notices, strict contract/context decoding, withdrawn target eviction, synchronous privacy cleanup. 10 controller tests, typecheck, plan and focused lint/diff checks exit 0 per owner. Browser/live NOT_RUN. | `97ef9fb39d72e4dfdffbc294759e4510aa874653`, nine paths verified via Git metadata; published ancestry NOT_RUN. | Delivery wires shell-scoped controller with existing auth/bootstrap AccessPort and reauthorized target callbacks; FE owner qualifies browser journey. |
| FE28 | IN_PROGRESS | Thread/composer source preparation; media explicitly unavailable rather than fake success. Feature tests NOT_RUN. | No feature commit observed; published ancestry NOT_RUN. | Frontend continues independent thread/comment/draft flows; FE04 image transport writer is active; consume qualified typed port when committed. |
| FE04 | IN_PROGRESS | Typed controller API sent directly to FE28 and community FE; contract/transport mismatch explicitly routed to delivery. Feature validation NOT_RUN. | No commit observed. | FE04 implements/validates lane-local adapter, image lifecycle and uncertain-finalize denial; delivery resolves transport semantics. |
| IN03 | NOT_RUN | No two-account real-service social/privacy journey established. | No candidate-bound gate evidence. | Delivery qualifies HTTP, WS and cache denial/revocation with real owners after required flows exist. |

The policy API is committed at `bcd2cd2`; it is not a frozen public HTTP
contract or runtime-qualified handoff. It names
`concert_domain::relationship::{PolicyQuery, Field, PolicyStamp}` and
`concert_application::relationship::RelationshipPolicy::authorize`, with fields
PublicProfile, Presence, Interaction, SafetySelf and PrivateHandle.
SafetySelf supports actor=subject safety operations in READ_ONLY; ordinary
Interaction requires OPEN/LIVE. Resource owners still enforce ownership. Decisions bind actor,
subject, event, query and revisions; generic policy does not substitute for
transactional reciprocal-grant reveal authorization. Production authority must
be real and fail closed when unavailable. Consumers received the API directly;
no SL code review is needed to proceed.

## Concrete dependency routing

1. Main allocated FE04 after delivery confirmed no incumbent. Its typed port is
   `createImageUploadController` with read/subscribe/select/upload/recover/retry/
   cancel/dispose and purpose POST/GIVEAWAY/REPORT. Only validated READY Media
   appears as an attachment. FE28 and community FE receive it directly. The
   canonical HTTP transport supports JSON/form only, but C04 specifies an
   authenticated, bounded same-origin byte PUT; its UploadIntent has no external
   presigned URL/header fields. FE04 proposed lane-local authenticated XHR for
   that actual contract plus a separate credential-free presigned helper not
   selected by C04. Delivery decides the implementation boundary; no existing
   shared API edits were implicitly acquired. Tests must distinguish these
   transports, reject failed/unknown finalize as READY, fence identity changes
   and free preview/worker/network resources. Text/thread work continues.
2. Delivery confirmed no active Friend/Auth relationship-adapter writer.
   Backend supplied exact authority requirements in the committed lane document:
   request-bound current Auth session/account and subject restrictions, both
   Friend block directions, exact query binding, checked_at=DB_now, positive
   authority_revision, bounded validity and fencing against owning-service
   revocation through the protected transaction. Delivery can allocate a narrow
   adapter patch against these concrete requirements. Existing
   Auth/Profile reads do not prove real block authority. No test-only authority
   may be silently used in production.
3. Delivery confirmed migration 0006 reservation and the expensive Cargo queue:
   bootstrap, admission, delivery, community party, BE37. Only expensive build
   phases serialize; source work continues. Shared target remains
   `/Users/lhs/Desktop/mcr-build/backend`; no duplicate target is authorized.
4. With BE37 seam commit `bcd2cd2` available, main can provision a
   separate BE31/32 writer for `concert-application/src/feed/**`, dedicated
   feed tests, Gateway `concert_feed.rs` / `concert_comments.rs` and lane
   evidence. Delivery owns exports/routes/manifests/proto and explicitly
   delegates any needed inherited Post-owner paths after reconnaissance.
   Feed/content implementation does not wait for BE28–30. Main received this
   proposed scope and required event/cursor/privacy/linked-target checks.

## Remaining IN03 scope and limits

IN03 directly requires BE27, BE30, BE32, BE33, BE37, FE28, FE30, FE31, FE35 and
FE36. The two current assignments do not cover that whole gate. Introduction /
age policy, feed-owner integration, notifications, sharing UI, and safety/cache
propagation require their own concrete delivery evidence. No other writer or
completion is inferred from an absent local implementation or the static plan.

Useful next disjoint frontend opportunities are FE31
`frontend/src/features/social-sharing/**`, FE35 `frontend/src/features/inbox/**`
and FE36 `frontend/src/features/safety/**` / `account/**`, after delivery checks
current ownership. These can use pinned contracts and deterministic fixtures;
real backend availability is not an implementation prerequisite. They need
explicit isolated writer assignments before source edits.

No acceptance count, scope cut, release readiness, managed-secret provisioning,
production Friend/Auth fencing, image upload, or real multiuser evidence is
claimed here. Worker results will record commands/exit codes and feature commits
before publication; delivery verifies the actual master ancestry. Remote-tracking
master was observed at `920631d64c85355df33163a51cd4d1643f4fa109` while both
frontend branch subsequently advanced to `97ef9fb` and backend to `bcd2cd2`.
Neither feature commit was an ancestor of the observed remote-tracking ref
(`git merge-base --is-ancestor` exit 1 for each); this is a bounded local-ref
observation, not a fresh remote fetch or evidence of live qualification.

## Tracker verification

- `python3 scripts/mcr/check-plan.py`: exit 0, planning provenance/structure/
  exporter/delivery-routing PASS; application build/tests/deployment NOT_RUN.
- `git diff --check`: exit 0 before the tracker checkpoint.
- Only `docs/mcr/scrum/social.md` is staged for the SL handoff. The delivery
  integrator publishes this branch commit; the SL does not push master.

Tracker handoff: initial commit `22f6063d7d81c66ee8663a5672b40a98b9eeabc1`
sent directly to delivery. This follow-up records first feature commits and the
newly allocated FE04 writer without adding any application-code approval gate.
