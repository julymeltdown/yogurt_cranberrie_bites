# C04 domain contract readiness review

Date: 2026-11-03. **Bounded review complete: Domain candidate v0.1.0 is structurally
closed in `contracts/domain/v1/`. Core subdomains (Parties, Giveaways, Feed) have
complete DTO definitions suitable for parent-authorized bounded pinning. Full freeze
remains NOT_ACCEPTED pending independent BE/FE lane reviews and fixture expansion.**
Contract candidate status remains REVIEW_CANDIDATE.

## Scope and authority

QA worktree: `/Users/lhs/Desktop/mcr-worktrees/qa-domain-readiness`, branch
`codex/mcr-qa-C04-readiness`, base commit `6716ed7d9796586d7f2b52980ed2a2fcab1ab2f8`.
Sole tracked writable file is this report at its absolute path:
`/Users/lhs/Desktop/mcr-worktrees/qa-domain-readiness/docs/mcr/qa/domain-readiness-review.md`.
No contracts, schemas, manifests, application source, or Cargo/PG processes were touched.

## Audit of the five stated gaps (remaining before C04 freeze)

1. **Fixture coverage (Engineering omission)**: `contracts/fixtures/domain/index.json`
   indexes 36 fixtures covering 18 schemas out of 162. Of 106 operations in
   `operations.json`, exactly **25 operations** have at least one DTO represented;
   **81 operations** have zero DTOs represented; and **zero operations** have both
   request and response covered. Does not block bounded consumer typing because all
   schemas in `contracts/domain/v1/*.schema.json` are closed and typed.
2. **Account deletion & recovery (Split omission vs policy)**:
   - *Engineering omission*: No domain deletion coordinator wire defines how Auth
     account deletion cascades to Concert entities (scrubbing participant records,
     private handles, reservations). No wire contract exists for recovering or
     polling a background deletion job after credential revocation (generic session
     invalidation is already qualified by C03 `SessionController`).
   - *External policy*: Legal terms publication and statutory data-retention schedules
     are external operator requirements, not in-event feature blockers.
3. **Response context & DTO omissions (Engineering omissions)**:
   - `feed.schema.json`: `Media` requires `state: const "READY"`; lacks DTOs for
     in-flight (`PENDING`/`PROCESSING`) or `FAILED` media during background ingestion.
   - Deletions return `common.Deleted`, but `FeedPage` and `PostDetail` schemas omit
     tombstones, requiring clients to treat deleted linked entities as pruned.
   - `admin.schema.json`: `Archive` does not bound retention/lifecycle for attached media.
   - Operation-context rules in `policy.json` (e.g. ticket section matching) require
     backend enforcement beyond static schema validation.
4. **OpenAPI qualification & client SDK (Post-freeze integration)**: Assembly passes
   cleanly (132 ops in `generate-mcr-domain.py --check`); client SDK generation follows freeze.
5. **Independent BE/FE review (Active gate)**: This QA audit does **not** substitute for
   the required independent backend and frontend lane reviews before formal freeze.

## Core feature subdomains and central consumer prerequisites

### Prioritized operation mapping

| Domain | Plan tasks | Key operationId | Method & path | Request / Response schema |
|---|---|---|---|---|
| Party | BE16–BE19 / FE21–FE23 | `listConcertParties` | GET `/api/concert/events/{eventId}/parties` | `party.PartyQuery` / `party.PartyList` |
| Party | BE16–BE19 / FE21–FE23 | `createConcertParty` | POST `/api/concert/events/{eventId}/parties` | `party.CreateParty` / `party.PartyPublic` |
| Party | BE16–BE19 / FE21–FE23 | `applyConcertParty` | POST `/api/concert/events/{eventId}/parties/{partyId}/applications` | `party.Apply` / `party.Application` |
| Party | BE16–BE19 / FE21–FE23 | `getConcertPartyActivity` | GET `/api/concert/events/{eventId}/me/parties` | `party.ActivityQuery` / `party.PartyActivity` |
| Giveaway | BE20–BE23 / FE24–FE26 | `listConcertGiveaways` | GET `/api/concert/events/{eventId}/giveaways` | `giveaway.GiveawayQuery` / `giveaway.GiveawayList` |
| Giveaway | BE20–BE23 / FE24–FE26 | `createConcertGiveaway` | POST `/api/concert/events/{eventId}/giveaways` | `giveaway.CreateGiveaway` / `giveaway.GiveawayPublic` |
| Giveaway | BE20–BE23 / FE24–FE26 | `reserveConcertGiveaway` | POST `/api/concert/events/{eventId}/giveaways/{giveawayId}/reservations` | `giveaway.Reserve` / `giveaway.Reservation` |
| Giveaway | BE20–BE23 / FE24–FE26 | `issueConcertPickupCode` | POST `/api/concert/events/{eventId}/reservations/{reservationId}/code` | `giveaway.IssuePickupCode` / `giveaway.PickupCode` |
| Giveaway | BE20–BE23 / FE24–FE26 | `claimConcertReservation` | POST `/api/concert/events/{eventId}/reservations/{reservationId}/claim` | `giveaway.ClaimReservation` / `giveaway.PickupReceipt` |
| Giveaway | BE20–BE23 / FE24–FE26 | `getConcertGiftActivity` | GET `/api/concert/events/{eventId}/me/giveaways` | `giveaway.ActivityQuery` / `giveaway.GiveawayActivity` |
| Feed | BE31–BE32 / FE27–FE28 | `listConcertFeed` | GET `/api/concert/events/{eventId}/feed` | `feed.FeedQuery` / `feed.FeedPage` |
| Feed | BE31–BE32 / FE27–FE28 | `createConcertPost` | POST `/api/concert/events/{eventId}/posts` | `feed.CreatePost` / `feed.PostPublic` |
| Feed | BE31–BE32 / FE27–FE28 | `createConcertComment` | POST `/api/concert/events/{eventId}/posts/{postId}/comments` | `feed.CreateComment` / `feed.CommentPublic` |
| Feed | BE31–BE32 / FE27–FE28 | `likeConcertPost` | PUT `/api/concert/events/{eventId}/posts/{postId}/reaction` | `common.EmptyRequest` / `feed.ReactionState` |

### Central frontend client prerequisites (parent-owned)

Before frontend feature workers (FE21–FE28) can execute mutations and error handling:
1. `frontend/src/shared/api/http.ts:133`: Header allowlist permits only `x-correlation-id`,
   `x-request-id`, and `x-client-id`. Sending required mutation idempotency headers
   (`X-Idempotency-Key` / `Idempotency-Key`) triggers `TransportError('invalid-request')`.
2. `frontend/src/shared/api/http.ts:114`: `errorFamily` strictly permits `'auth' | 'core'`,
   rejecting any domain error family.
3. `frontend/src/shared/api/errors.ts:2`: `CORE_REASONS` contains only 18 reasons. C04
   domain errors (`IDEMPOTENCY_CONFLICT`, `DUPLICATE_APPLICATION`, `PICKUP_CODE_INVALID`,
   `INVALID_TRANSITION`, `MEDIA_NOT_READY`) are rejected by `parseCoreDetails`.

## Boundary verifier limitation and executed checks

`scripts/mcr/check-boundaries.py` discovers its repository root via `git rev-parse --show-toplevel`
executed on `Path.cwd()`. Consequently, running the checker inside a lane worktree inspects
only that worktree's discovered root and index, and is completely blind to accidental writes
occurring in external checkouts when tools execute with a default session cwd. Absolute
paths inside assigned worktrees are required.

Executed verification checks:
- `python3 scripts/mcr/validate-domain.py`: Exit 0 (162 schemas, 106 ops, 36 fixtures).
- `python3 -m unittest discover -s scripts/mcr/tests -p test_domain.py -v`: Exit 0 (47 tests).
- `python3 backend/scripts/docs/generate-mcr-domain.py --check`: Exit 0.
- `python3 scripts/mcr/check-plan.py`: Exit 0 (118 tasks, 64 IA entries).
- `python3 scripts/mcr/check-boundaries.py --base 6716ed7 --lane frontend --allow-doc docs/mcr/qa/domain-readiness-review.md`: Exit 0 (1 change).

## Disposition

Candidate contracts for Parties, Giveaways, and Feed are structurally ready for
parent-authorized bounded pinning. Shared frontend HTTP/error client updates are required
prerequisites for frontend mutations. Full freeze remains NOT_ACCEPTED.
Report committed; goal remains **ACTIVE** awaiting parent disposition.
