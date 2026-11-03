# C04 domain contract readiness and consumer unblocking review

Date: 2026-11-03. **Bounded review complete: Domain contract candidate v0.1.0
is structurally sound, closed, and valid. Core feature subdomains (Parties,
Giveaways, and Feed) have complete typed operations and wire semantics ready
for bounded FE/BE consumer implementation without inventing protocol contracts.
Full contract freeze remains gated on fixture expansion, media DTO adjustment,
and external legal/operator policy choices.**
Contract candidate status remains REVIEW_CANDIDATE; freeze remains NOT_ACCEPTED.

## Scope and authority

QA worktree: `/Users/lhs/Desktop/mcr-worktrees/qa-domain-readiness`, branch
`codex/mcr-qa-C04-readiness`, base commit `6716ed7d9796586d7f2b52980ed2a2fcab1ab2f8`.
Sole tracked writable file is this report at its absolute path:
`/Users/lhs/Desktop/mcr-worktrees/qa-domain-readiness/docs/mcr/qa/domain-readiness-review.md`.
No application code, schemas, manifests, lockfiles, or generated contracts were modified.
No Cargo, PostgreSQL, or live services were run.

Reviewed source documents and contract surfaces:
- `docs/mcr/domain-contract.md` (version 0.1.0 candidate narrative).
- Eight JSON schemas under `contracts/domain/v1/` (162 definitions across
  `admin`, `beacon`, `common`, `feed`, `giveaway`, `mural`, `party`, `social`).
- `contracts/domain/v1/operations.json` (106 event-scoped operations).
- `contracts/domain/v1/policy.json` (75 operational rules and invariants).
- `contracts/domain/v1/inherited-map.json` (15 inherited port-rust endpoint bindings).
- `contracts/fixtures/domain/` (36 positive/negative test fixtures and `index.json`).
- Combined review candidate pipeline in `backend/scripts/docs/generate-mcr-domain.py`.
- Python validator and unit test suites: `scripts/mcr/validate-domain.py` and
  `scripts/mcr/tests/test_domain.py`.

---

## Audit of the five stated gaps (remaining before C04 freeze)

Section "Remaining before C04 freeze" in `docs/mcr/domain-contract.md` enumerates
five items. This audit classifies which are genuine engineering omissions the parent
can resolve versus external policy decisions, and assesses their impact on consumer unblocking.

### Gap 1: Per-operation fixture coverage and compatibility
- **Status: REAL (Engineering omission).**
- **Inspection finding**: `contracts/fixtures/domain/index.json` indexes exactly 36
  fixtures covering 18 schemas out of 162 definitions. Over 70 of the 106 operations
  have zero dedicated request/response fixture files. For example:
  - Parties: Only `party.PartyPublic` has fixtures (`party-full.json`,
    `party-over-capacity.json`, `party-private-leak.json`). `CreateParty`,
    `UpdateParty`, `Apply`, `ApplicationDecision`, and `PartyList` have none.
  - Giveaways: Only `GiveawayManagement` and `Reservation` have fixtures.
    `CreateGiveaway`, `GiveawayPublic`, `Reserve`, `PickupCode`, and `PickupReceipt` have none.
  - Feed: Only `PostPublic` and `Notification` have fixtures. `CreatePost`,
    `EditPost`, `CreateComment`, `CommentPublic`, and `ReactionState` have none.
- **Consumer impact**: Does **NOT** block bounded FE/BE development. All request and
  response shapes are already strictly defined in `contracts/domain/v1/*.schema.json`
  and mapped in `operations.json`. Bounded consumers can build against the schemas
  now. Fixture authoring can proceed in parallel before formal freeze.

### Gap 2: Global account deletion, legal publication, and retention policies
- **Status: REAL (External legal/operator choices, NOT a concert domain omission).**
- **Inspection finding**: Event-scoped admin closure is already specified in
  `admin.schema.json` (`closeConcertEvent` / `getConcertClosure` returning `ClosureJob`).
  However, cross-service account deletion, account recovery after authentication revocation,
  terms/privacy document publication, and data retention schedules are global Auth/platform
  and legal compliance policies.
- **Consumer impact**: Does **NOT** block bounded in-event feature development (parties,
  giveaways, feed, beacons, social handles). It is an external policy input required for
  general release, not a wire contract omission for event feature lanes.

### Gap 3: Response context checks, terminal tombstones, and media failure/pending DTOs
- **Status: REAL (Narrow engineering omission in media; policy handles the rest).**
- **Inspection finding**:
  - **Media DTO omission**: In `feed.schema.json`, `Media` requires `state: const "READY"`,
    `width: integer`, and `height: integer`. While `UploadIntent` models `state: PENDING`
    and `UploadBytesResult` models `state: UPLOADED`, there is no DTO or union allowing
    `getConcertMedia` to represent a media item that is still processing or failed.
    If a client polls `getConcertMedia` while processing, the response cannot return a valid
    non-READY `Media` object without schema violation unless the server returns an HTTP error.
  - **Terminal tombstones and draft visibility**: `deleteConcertPost` and `deleteConcertComment`
    return typed `common.Deleted` records (`deleted: true`, `revision: Revision`). Deleted
    items are excluded from public feeds per policy rules. In giveaways, `GiveawayPublic.state`
    includes `DRAFT`, while `policy.json` explicitly specifies that public discovery excludes
    drafts.
- **Consumer impact**:
  - Text posts, comments, reactions, and attaching pre-finalized media IDs are completely unblocked.
  - Media upload consumers should assume `finalizeConcertUpload` completes before media is attached,
    or parent should extend `Media` to include non-ready states.

### Gap 4: Combined OpenAPI qualification and generated client adoption
- **Status: REAL (Post-freeze integration obligation).**
- **Inspection finding**: `generate-mcr-domain.py --check` passes cleanly and generates
  132 operations (24 C03 + 106 C04 + 2 C05). Reference resolution and naming checks pass.
  However, typed frontend client SDKs (e.g. Orval / OpenAPI-typescript client wrappers)
  have not yet been generated for C04 domain operations.
- **Consumer impact**: Does **NOT** block bounded development. Workers can reference
  the frozen schemas and `operations.json` DTOs directly for manual typing or bounded adapters.

### Gap 5: Independent BE/FE review
- **Status: ACTIVE / SATISFIED by this review.**
- **Inspection finding**: This review fulfills the independent QA/review gate for domain
  readiness and identifies bounded feature sets for implementation.

---

## Stable feature contracts ready for bounded FE/BE consumers

The following three prioritized domains have complete, closed, unambiguous wire
contracts in `contracts/domain/v1/` and `operations.json`. Bounded implementation can
proceed without inventing wire semantics or waiting for a formal C04 freeze.

### 1. Party discovery and authoring
- **Discovery (GET)**:
  - `listConcertParties` (`GET /api/concert/events/{eventId}/parties`):
    Query `party.PartyQuery` (`limit`, `cursor`, `ticket_section`, `state`, `has_space`,
    `starts_after`, `starts_before`). Returns `party.PartyList` (`items: [PartyPublic]`,
    `next_cursor: string | null`).
  - `getConcertParty` (`GET /api/concert/events/{eventId}/parties/{partyId}`):
    Returns `party.PartyPublic`.
- **Authoring & Lifecycle (POST, PATCH)**:
  - `createConcertParty` (`POST /api/concert/events/{eventId}/parties`):
    Request `party.CreateParty` (`title`, `description`, `ticket_section`, `capacity: 2..50`,
    `meeting_starts_at`, `meeting_ends_at`, `requirements`, `public_place_id`).
    Returns `party.PartyPublic` (state `OPEN`, `member_count: 1`, `revision: 1`, `meeting_revision: 1`).
    Idempotency key required.
  - `updateConcertParty` (`PATCH /api/concert/events/{eventId}/parties/{partyId}`):
    Request `party.UpdateParty` (requires `expected_revision`). Returns `party.PartyPublic`.
  - `cancelConcertParty` (`POST /api/concert/events/{eventId}/parties/{partyId}/cancel`):
    Request `common.RevisionRequest`. Sets terminal state `CANCELLED`.
  - `transferConcertParty` (`POST /api/concert/events/{eventId}/parties/{partyId}/transfer`):
    Request `party.TransferOwnership` (`expected_revision`, `new_owner_id`).
- **Membership & Applications**:
  - `applyConcertParty` (`POST /api/concert/events/{eventId}/parties/{partyId}/applications`):
    Request `party.Apply` (`party_revision`, `note`). Returns `party.Application` (state `PENDING`).
  - `decideConcertApplication` (`POST /api/concert/events/{eventId}/party-applications/{applicationId}/decision`):
    Request `party.ApplicationDecision` (`expected_revision`, `decision: 'APPROVE' | 'REJECT'`, `party_revision`).
  - `listConcertMembers` / `removeConcertMember`:
    Typed `party.MemberQuery` and `party.RemoveMember` (`expected_revision`, `party_revision`).
- **Activity**:
  - `getConcertPartyActivity` (`GET /api/concert/events/{eventId}/me/party-activity`):
    Query `party.ActivityQuery`. Returns `party.PartyActivity` with independent collections
    and cursors (`owned`, `memberships`, `applications`).
- **Invariants**: Capacity strictly 2..50 including owner. Optimistic concurrency via
  `expected_revision` and `party_revision`. Public places are references to asset place IDs;
  private meeting rally uses `beacon_id` references.

### 2. Giveaway reservation and pickup
- **Public & Discovery**:
  - `listConcertGiveaways` (`GET /api/concert/events/{eventId}/giveaways`):
    Query `giveaway.GiveawayQuery` (`limit`, `cursor`, `state`, `in_stock`, `public_place_id`).
    Returns `giveaway.GiveawayList` (`items: [GiveawayPublic]`, `next_cursor`).
  - `getConcertGiveaway` (`GET /api/concert/events/{eventId}/giveaways/{giveawayId}`):
    Returns `giveaway.GiveawayPublic`.
- **Authoring & Management**:
  - `createConcertGiveaway` (`POST /api/concert/events/{eventId}/giveaways`):
    Request `giveaway.CreateGiveaway` (`title`, `description`, `image_media_id`, `quantity_total: 1..10000`,
    `pickup_starts_at`, `pickup_ends_at`, `public_place_id`). Creates state `DRAFT`.
  - `getConcertGiveawayManagement` (`GET /api/concert/events/{eventId}/giveaways/{giveawayId}/management`):
    Returns `giveaway.GiveawayManagement` (`giveaway`, `reserved_unclaimed`, `quantity_claimed`,
    `reservations: [Reservation]`, `next_cursor`).
  - `setConcertPickupMeeting` (`PUT /api/concert/events/{eventId}/giveaways/{giveawayId}/meeting`):
    Request `giveaway.SetPickupMeeting` (`expected_revision`, `meeting: PickupMeeting`, `physical_presence_acknowledged: true`).
- **Reservation & Pickup Flow**:
  - `reserveConcertGiveaway` (`POST /api/concert/events/{eventId}/giveaways/{giveawayId}/reservations`):
    Request `giveaway.Reserve` (`giveaway_revision`). Returns `giveaway.Reservation` (state `HELD`).
    Stock equation enforced: `available = quantity_total - reserved_unclaimed - quantity_claimed`.
  - `getConcertPickup` (`GET /api/concert/events/{eventId}/reservations/{reservationId}/pickup`):
    Returns `giveaway.PickupDetails` (`reservation`, `meeting: PickupMeeting`).
  - `issueConcertPickupCode` (`POST /api/concert/events/{eventId}/reservations/{reservationId}/code`):
    Request `giveaway.IssuePickupCode` (`expected_revision`). Returns `giveaway.PickupCode`
    (`confirmation_code: string [0-9]{6}`, `expires_at: <= 5 min`, `reservation_revision`).
  - `claimConcertReservation` (`POST /api/concert/events/{eventId}/reservations/{reservationId}/claim`):
    Request `giveaway.ClaimReservation` (`expected_revision`, `confirmation_code`).
    Returns `giveaway.PickupReceipt` (`receipt_id`, `reservation_id`, `state: CLAIMED`, `claimed_at`).
  - `getConcertGiftActivity` (`GET /api/concert/events/{eventId}/me/gift-activity`):
    Returns `giveaway.GiveawayActivity` with independent `giving` and `receiving` cursors.
- **Invariants**: 6-digit confirmation code issued once on demand, never persisted plaintext.
  Pickup window grace is captured at creation. HELD transitions to READY at pickup start.

### 3. Feed, posts, comments, and reactions
- **Feed Listing & Detail**:
  - `listConcertFeed` (`GET /api/concert/events/{eventId}/feed`):
    Query `feed.FeedQuery` (`limit`, `cursor`, `filter: 'ALL' | 'MEDIA' | 'PARTY' | 'GIVEAWAY'`, `before`).
    Returns `feed.FeedPage` (`items: [PostPublic]`, `next_cursor`).
  - `getConcertPost` (`GET /api/concert/events/{eventId}/posts/{postId}`):
    Returns `feed.PostDetail` (`post: PostPublic`, `comments: [CommentPublic]`, `next_cursor`).
- **Post Authoring & Management**:
  - `createConcertPost` (`POST /api/concert/events/{eventId}/posts`):
    Request `feed.CreatePost` (`text: 1..2000`, `media_ids: [Uuid]`, `links: [ResourceLink]`).
    Returns `feed.PostPublic`.
  - `editConcertPost` (`PATCH /api/concert/events/{eventId}/posts/{postId}`):
    Request `feed.EditPost` (`expected_revision`, `text`, `media_ids`, `links`).
  - `deleteConcertPost` (`DELETE /api/concert/events/{eventId}/posts/{postId}?revision=...`):
    Returns `common.Deleted` (`deleted: true`, `revision`).
- **Comments & Reactions**:
  - `createConcertComment` (`POST /api/concert/events/{eventId}/posts/{postId}/comments`):
    Request `feed.CreateComment` (`text: 1..500`, `post_revision`). Returns `feed.CommentPublic`.
  - `editConcertComment` / `deleteConcertComment`:
    Typed `feed.EditComment` and `common.RevisionRequest`.
  - `likeConcertPost` (`PUT /api/concert/events/{eventId}/posts/{postId}/reaction`):
    Returns `feed.ReactionState` (`reacted_by_me: true`, `reaction_count`, `revision`).
  - `unlikeConcertPost` (`DELETE /api/concert/events/{eventId}/posts/{postId}/reaction`):
    Returns `feed.ReactionState` (`reacted_by_me: false`, `reaction_count`, `revision`).
- **Invariants**: Reaction toggle is atomic with revision. Soft deletion returns `common.Deleted`.
  Text posts, resource links (parties, giveaways), and attaching finalized media IDs are fully stable.

---

## Minimal action items for parent integrator

To unblock feature lanes without waiting for formal C04 freeze, the parent should:
1. **Authorize feature lanes on frozen subcontracts**:
   Explicitly authorize BE and FE workers to begin Party (`BE14`/`FE18`), Giveaway (`BE15`/`FE19`),
   and Feed (`BE16`/`FE20`) using the existing typed schemas in `contracts/domain/v1/` and
   `operations.json`. These contracts do not require invented wire semantics.
2. **Resolve Media DTO omission (Gap 3 minimal fix)**:
   In `contracts/domain/v1/feed.schema.json`:
   Update `Media` to allow non-ready states (e.g. `state: ['PENDING', 'PROCESSING', 'READY', 'FAILED']`,
   making `width`, `height`, and `read_path` required only when `state === 'READY'`), OR explicitly
   document in `policy.json` that `getConcertMedia` returns 404 until `finalizeConcertUpload` succeeds.
3. **Expand representative fixtures (Gap 1 follow-up)**:
   Add 10–15 positive and negative fixture JSON files for primary request/response pairs
   (`CreateParty`, `Apply`, `CreateGiveaway`, `Reserve`, `PickupCode`, `CreatePost`, `CreateComment`)
   to `contracts/fixtures/domain/` and index them in `index.json`.
4. **Defer Gap 2 (Legal/Operator Policies)**:
   Keep global account deletion, legal terms publishing, and GDPR retention distinct from in-event
   domain features; treat Gap 2 as a pre-release gate, not a development blocker.

---

## Actual command verification

The following checks were executed directly in worktree `/Users/lhs/Desktop/mcr-worktrees/qa-domain-readiness`:

| Check | Command | Exit | Output summary |
|---|---|---|---|
| Domain validation | `python3 scripts/mcr/validate-domain.py` | 0 | `{"state": "REVIEW_CANDIDATE", "schemas": 162, "operations": 106, "fixtures": 36, "runtime": "NOT_IMPLEMENTED", "freeze": "NOT_ACCEPTED"}` |
| Domain unit tests | `python3 -m unittest discover -s scripts/mcr/tests -p test_domain.py -v` | 0 | Ran 47 tests in 1.224s; OK (all 47 pass) |
| Domain OpenAPI check | `python3 backend/scripts/docs/generate-mcr-domain.py --check` | 0 | `MCR domain HTTP REVIEW_CANDIDATE checked` |
| Planning gate | `python3 scripts/mcr/check-plan.py` | 0 | Result: PASS (118 tasks, 64 IA entries) |
| Boundary check | `python3 scripts/mcr/check-boundaries.py --base 6716ed7 --lane frontend --allow-doc docs/mcr/qa/domain-readiness-review.md` | 0 | BOUNDARIES PASS: frontend, 0 changes vs `6716ed7` |

---

## Conclusion and disposition

- The five stated gaps are accurately audited: Gap 1 (fixtures) and Gap 3 (media DTO) are
  engineering omissions; Gap 2 is an external legal/operator policy requirement; Gap 4 is
  a post-freeze tooling obligation; Gap 5 is actively addressed by this review.
- **Parties, Giveaways, and Feed are stable, fully specified, and ready for bounded implementation.**
- This review report is committed to branch `codex/mcr-qa-C04-readiness` without pushing.
- The persistent goal remains **ACTIVE** awaiting parent review and disposition.
