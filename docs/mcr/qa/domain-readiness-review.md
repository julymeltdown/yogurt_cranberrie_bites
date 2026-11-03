# C04 domain contract readiness and consumer unblocking review

Date: 2026-11-03. **Bounded review complete: Domain contract candidate v0.1.0
is structurally closed and locally validated. Core feature subdomains (Parties,
Giveaways, and Feed) have defined operations, typed DTOs, and documented wire rules
suitable for parent-authorized bounded FE/BE consumer implementation.
Formal C04 contract freeze remains pending independent BE and FE lane reviews,
fixture expansion, media state adjustments, and global deletion orchestration.**
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
- `MCR_IMPLEMENTATION_PLAN_V2_EN.md` and `PLAN_TABLES_EN.md` (authoritative plan and task mappings).
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
five items. This audit classifies their engineering and policy realities based on
direct inspection of schemas, operations, and fixtures.

### Gap 1: Per-operation fixture coverage and compatibility
- **Status: REAL (Engineering omission).**
- **Authoritative artifact evidence**:
  - `contracts/fixtures/domain/index.json` indexes exactly 36 fixture files covering
    18 distinct schema definitions out of 162 total definitions in `contracts/domain/v1/`.
  - Of the 106 operations in `operations.json`, exactly **25 operations** have at least
    one DTO (request or response) represented among those 18 indexed schemas.
  - Exactly **81 operations** have neither their request nor their response represented
    in `index.json`.
  - Exactly **zero operations** have both their request and response covered by dedicated
    fixture files (for instance, `createConcertParty` has `party.PartyPublic` covered,
    but lacks a fixture for `party.CreateParty`).
  - Core subdomains lack operation-level positive and negative request/response fixtures:
    - Parties: Lacks fixtures for `CreateParty`, `UpdateParty`, `Apply`, `ApplicationDecision`,
      `PartyList`, `MemberList`, and `PartyActivity`.
    - Giveaways: Lacks fixtures for `CreateGiveaway`, `UpdateGiveaway`, `GiveawayPublic`,
      `Reserve`, `PickupCode`, and `PickupReceipt`.
    - Feed: Lacks fixtures for `CreatePost`, `EditPost`, `CreateComment`, `CommentPublic`,
      and `ReactionState`.
- **Consumer impact**: Does **NOT** block bounded consumer implementation. All request
  and response shapes are strictly defined in `contracts/domain/v1/*.schema.json` and
  mapped in `operations.json`. Bounded consumers can develop against these candidate
  schemas while fixture expansion proceeds.

### Gap 2: Global account deletion, auth revocation recovery, legal publication, and retention policies
- **Status: REAL (Split between concrete engineering omissions and external policy choices).**
- **Engineering omissions**:
  - **No domain deletion coordinator wire**: There is no domain-level operation or event
    schema defining how an Auth-service account deletion cascades to Concert entities
    (scrubbing participant records, deleting private handles, reassigning/cancelling
    owned parties, or releasing held reservations).
  - **No post-revocation recovery wire**: There is no contract defining gateway error
    recovery or session-invalidation handling when an auth token is revoked mid-flow.
  - Event closure is specified in `admin.schema.json` (`closeConcertEvent` /
    `getConcertClosure` returning `ClosureJob`), but this is Concert-local and does not
    coordinate global Auth erasure.
- **External legal/operator choices**:
  - Legal terms and privacy policy publication.
  - Mandatory data retention schedules, statutory erasure deadlines, and operational
    archive retention periods.
- **Consumer impact**: The lack of a global deletion wire does **NOT** block in-event
  feature workflows (parties, giveaways, feed). It is an integration obligation for
  account lifecycle (BE42) and release gates, not an in-event feature contract omission.

### Gap 3: Response context checks, terminal tombstones, media DTOs, and archive scope
- **Status: REAL (Engineering omissions across several schema/context boundaries).**
- **Specific inspection findings**:
  1. **Media DTO state omission**: In `feed.schema.json`, `Media` requires `state: const "READY"`,
     along with `width`, `height`, and `read_path`. `UploadIntent` models `state: PENDING`
     and `UploadBytesResult` models `state: UPLOADED`. However, there is no DTO allowing
     `getConcertMedia` (`GET /api/concert/events/{eventId}/media/{mediaId}`) to represent
     media that is currently processing or has failed validation/re-encoding. An in-flight
     query will either violate schema or require an undocumented HTTP error code.
  2. **Operation-context validation**: `policy.json` rules (e.g. ticket-section matching,
     phase restrictions, and owner invariants) must be enforced by backend business logic.
     Static JSON schema validation alone cannot enforce multi-entity contextual preconditions.
  3. **Terminal tombstones and feed collections**: Deletions return `common.Deleted`
     (`deleted: true`, `revision: Revision`), but `FeedPage` and `PostDetail` schemas
     only allow `PostPublic` with `state: const "PUBLISHED"`. The contract prunes deleted posts
     from feeds rather than emitting tombstones. References to deleted posts in resource links
     must be handled by clients as absent entities.
  4. **Archive media scope**: In `admin.schema.json`, `Archive` provides read-only
     historical projections of feed, mural, and attendance, but does not define media
     lifecycle or storage retention guarantees for attached assets after event closure.
- **Consumer impact**: Text posts, comments, reactions, and attaching pre-finalized media IDs
  are unblocked. Media upload pipelines must treat media as unusable until `finalizeConcertUpload`
  succeeds.

### Gap 4: Combined OpenAPI qualification and generated client adoption
- **Status: REAL (Post-freeze tooling and integration gate).**
- **Inspection finding**: `generate-mcr-domain.py --check` passes cleanly and generates
  132 operations (24 C03 + 106 C04 + 2 C05). However, generated TypeScript client wrappers
  (e.g. Orval / OpenAPI-typescript client interfaces) have not been produced or qualified
  for C04 domain operations in `frontend/src/`.
- **Consumer impact**: Does **NOT** block bounded development. Workers can write typed
  adapters directly against the candidate OpenAPI operations and schemas.

### Gap 5: Independent BE and FE review
- **Status: ACTIVE / ONGOING.**
- **Inspection finding**: The specification requires independent reviews from both backend
  and frontend lanes. This QA audit provides cross-cutting contract analysis, but does
  **not** substitute for the independent BE and FE development lane reviews required prior
  to formal C04 freeze.

---

## Stable feature contracts ready for bounded FE/BE consumers

The candidate contracts for the three prioritized feature sets are structurally closed,
typed, and internally consistent in `contracts/domain/v1/` and `operations.json`.
With parent authorization of bounded contract pinning, implementation in the assigned
feature tasks can proceed without workers inventing wire semantics.

### 1. Party discovery, authoring, and management (BE16–BE19 / FE21–FE23)
- **Relevant Plan Tasks**:
  - Backend: `BE16` (Party aggregate & CRUD), `BE17` (Applications & atomic approval),
    `BE18` (Leave, remove & cancel), `BE19` (Rally & real arrival).
  - Frontend: `FE21` (Party discovery & application), `FE22` (Party authoring form),
    `FE23` (Party management & rally).
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
    Requires `X-Idempotency-Key`.
  - `updateConcertParty` (`PATCH /api/concert/events/{eventId}/parties/{partyId}`):
    Request `party.UpdateParty` (requires `expected_revision`). Returns `party.PartyPublic`.
  - `cancelConcertParty` (`POST /api/concert/events/{eventId}/parties/{partyId}/cancel`):
    Request `common.RevisionRequest`. Sets terminal state `CANCELLED`.
  - `transferConcertParty` (`POST /api/concert/events/{eventId}/parties/{partyId}/transfer`):
    Request `party.TransferOwnership` (`expected_revision`, `new_owner_id`).
- **Applications & Membership**:
  - `applyConcertParty` (`POST /api/concert/events/{eventId}/parties/{partyId}/applications`):
    Request `party.Apply` (`party_revision`, `note`). Returns `party.Application` (state `PENDING`).
  - `decideConcertApplication` (`POST /api/concert/events/{eventId}/party-applications/{applicationId}/decision`):
    Request `party.ApplicationDecision` (`expected_revision`, `decision: 'APPROVE' | 'REJECT'`, `party_revision`).
  - `listConcertMembers` / `removeConcertMember`:
    Query `party.MemberQuery` and `party.RemoveMember` (`expected_revision`, `party_revision`).
- **Activity & Rendezvous**:
  - `getConcertPartyActivity` (`GET /api/concert/events/{eventId}/me/party-activity`):
    Returns `party.PartyActivity` with independent collections and cursors (`owned`,
    `memberships`, `applications`).
  - `getConcertRally` / `setConcertRally`:
    Uses `party.Rally` and `party.SetRally` (requires `physical_presence_acknowledged: true`,
    binds to `beacon_id` in `beacon.schema.json`).

### 2. Giveaway reservation and pickup journey (BE20–BE23 / FE24–FE26)
- **Relevant Plan Tasks**:
  - Backend: `BE20` (Giveaway aggregate & catalog), `BE21` (Atomic reservation command),
    `BE22` (Cancellation & expiry worker), `BE23` (Pickup confirmation & receipts).
  - Frontend: `FE24` (Giveaway discovery & own activity), `FE25` (Giveaway authoring & owner tools),
    `FE26` (Reservation & pickup journey).
- **Public Discovery**:
  - `listConcertGiveaways` (`GET /api/concert/events/{eventId}/giveaways`):
    Query `giveaway.GiveawayQuery` (`limit`, `cursor`, `state`, `in_stock`, `public_place_id`).
    Returns `giveaway.GiveawayList` (`items: [GiveawayPublic]`, `next_cursor`).
  - `getConcertGiveaway` (`GET /api/concert/events/{eventId}/giveaways/{giveawayId}`):
    Returns `giveaway.GiveawayPublic`.
- **Authoring & Owner Management**:
  - `createConcertGiveaway` (`POST /api/concert/events/{eventId}/giveaways`):
    Request `giveaway.CreateGiveaway` (`title`, `description`, `image_media_id`, `quantity_total: 1..10000`,
    `pickup_starts_at`, `pickup_ends_at`, `public_place_id`). Creates `DRAFT`.
  - `getConcertGiveawayManagement` (`GET /api/concert/events/{eventId}/giveaways/{giveawayId}/management`):
    Returns `giveaway.GiveawayManagement` (`giveaway`, `reserved_unclaimed`, `quantity_claimed`,
    `reservations: [Reservation]`, `next_cursor`).
  - `setConcertPickupMeeting` (`PUT /api/concert/events/{eventId}/giveaways/{giveawayId}/meeting`):
    Request `giveaway.SetPickupMeeting` (`expected_revision`, `meeting: PickupMeeting`,
    `physical_presence_acknowledged: true`).
- **Reservation & One-Time Pickup Flow**:
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
    Returns `giveaway.PickupReceipt` (`receipt_id`, `state: CLAIMED`, `claimed_at`).
  - `getConcertGiftActivity` (`GET /api/concert/events/{eventId}/me/gift-activity`):
    Returns `giveaway.GiveawayActivity` with independent `giving` and `receiving` cursors.

### 3. Event feed, posts, comments, and reactions (BE31–BE32 / FE27–FE28)
- **Relevant Plan Tasks**:
  - Backend: `BE31` (Event feed and post adapter), `BE32` (Comments, reactions & content controls).
  - Frontend: `FE27` (Event timeline and notices), `FE28` (Thread, composer & comments).
- **Feed Listing & Detail**:
  - `listConcertFeed` (`GET /api/concert/events/{eventId}/feed`):
    Query `feed.FeedQuery` (`limit`, `cursor`, `filter: 'ALL' | 'MEDIA' | 'PARTY' | 'GIVEAWAY'`, `before`).
    Returns `feed.FeedPage` (`items: [PostPublic]`, `next_cursor`).
  - `getConcertPost` (`GET /api/concert/events/{eventId}/posts/{postId}`):
    Returns `feed.PostDetail` (`post: PostPublic`, `comments: [CommentPublic]`, `next_cursor`).
- **Post Authoring & Moderation**:
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
  - `likeConcertPost` / `unlikeConcertPost`:
    Atomic toggle on `/api/concert/events/{eventId}/posts/{postId}/reaction` returning `feed.ReactionState`.
- **Boundaries**: Text posts, linked entity cards (parties, giveaways), and attaching
  already-finalized media IDs are fully ready. As noted in Gap 3, the media upload pipeline
  itself (`initConcertUpload` -> `putConcertUploadBytes` -> `finalizeConcertUpload`)
  should be treated as synchronous-to-READY until the non-ready media DTO is resolved.

---

## Minimal action items for parent integrator

To unblock feature development lanes without waiting for formal C04 freeze:
1. **Authorize bounded pinning for prioritized feature tasks**:
   Authorize backend and frontend workers to begin implementation on the agreed candidate
   contracts for Party (`BE16–BE19` / `FE21–FE23`), Giveaway (`BE20–BE23` / `FE24–FE26`),
   and Feed (`BE31–BE32` / `FE27–FE28`). Workers pin these candidate schemas locally
   without modifying shared contract roots.
2. **Resolve Media DTO omission (Gap 3 minimal fix)**:
   In `contracts/domain/v1/feed.schema.json`:
   Update `Media` to allow non-ready states (e.g. `state: ['PENDING', 'PROCESSING', 'READY', 'FAILED']`,
   making `width`, `height`, and `read_path` required only when `state === 'READY'`), OR document
   in `policy.json` that `getConcertMedia` returns 404 until `finalizeConcertUpload` succeeds.
3. **Specify domain account-deletion wire (Gap 2 engineering fix)**:
   Define the internal command or event payload for Concert to consume when Auth initiates
   account deletion (handling cascading cleanup of participant data, handles, and reservations).
4. **Expand representative fixtures (Gap 1 follow-up)**:
   Author request/response fixture JSON pairs for key operations (`CreateParty`, `Apply`,
   `CreateGiveaway`, `Reserve`, `PickupCode`, `CreatePost`, `CreateComment`) under
   `contracts/fixtures/domain/` and index them in `index.json`.
5. **Conduct lane-specific BE and FE reviews (Gap 5 follow-up)**:
   Ensure the assigned BE and FE implementation lanes review their respective endpoint
   mappings and concurrency controls prior to formal freeze.

---

## Precommit boundary check limitation

The boundary verifier `scripts/mcr/check-boundaries.py` inspects changes using `git diff base`
and `git ls-files --others` within the repository root discovered from `Path.cwd()`.
- **Limitation**: When executed from a lane worktree, `check-boundaries.py` only inspects
  the index and untracked files of *that specific worktree*. If a tool or process executes
  from a default session directory pointing to the integration checkout or another lane,
  accidental writes in those external checkouts are completely invisible to the lane's
  boundary checker.
- **Rule**: All file write operations in assigned worker and QA chats must use explicit
  absolute filesystem paths targeting only the assigned worktree.

---

## Actual command verification

The following checks were executed directly in worktree `/Users/lhs/Desktop/mcr-worktrees/qa-domain-readiness`:

| Check | Command | Exit | Output summary |
|---|---|---|---|
| Domain validation | `python3 scripts/mcr/validate-domain.py` | 0 | `{"state": "REVIEW_CANDIDATE", "schemas": 162, "operations": 106, "fixtures": 36, "runtime": "NOT_IMPLEMENTED", "freeze": "NOT_ACCEPTED"}` |
| Domain unit tests | `python3 -m unittest discover -s scripts/mcr/tests -p test_domain.py -v` | 0 | Ran 47 tests in 1.224s; OK (all 47 pass) |
| Domain OpenAPI check | `python3 backend/scripts/docs/generate-mcr-domain.py --check` | 0 | `MCR domain HTTP REVIEW_CANDIDATE checked` |
| Planning gate | `python3 scripts/mcr/check-plan.py` | 0 | Result: PASS (118 tasks, 64 IA entries) |
| Boundary check | `python3 scripts/mcr/check-boundaries.py --base 6716ed7 --lane frontend --allow-doc docs/mcr/qa/domain-readiness-review.md` | 0 | BOUNDARIES PASS: frontend, 1 changes vs `6716ed7` |

---

## Conclusion and disposition

- The five stated gaps are audited with exact counts: Gap 1 (81 operations lack fixtures; 0 operations have both request/response covered) and Gap 3 (Media DTO and context validation) are engineering omissions; Gap 2 is split between missing internal deletion wires and external legal policies; Gap 4 is post-freeze tooling; Gap 5 requires separate BE/FE lane reviews.
- **Party (BE16–BE19 / FE21–FE23), Giveaway (BE20–BE23 / FE24–FE26), and Feed (BE31–BE32 / FE27–FE28) candidate contracts are stable and ready for parent-authorized bounded consumer implementation.**
- This report is committed to branch `codex/mcr-qa-C04-readiness` without pushing.
- The persistent goal remains **ACTIVE** awaiting parent review and disposition.
