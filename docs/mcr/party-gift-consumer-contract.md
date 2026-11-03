# Party and gift consumer contract — revision 1

Status: **CONTRACT_READY for the nine operations below**, by integrator decision
after independent frontend review and contextual qualification. Whole C04 remains
REVIEW_CANDIDATE; all remaining operations and required task/IA gates stay in scope.
This permits FE21/FE24 development against deterministic fixtures without waiting
for matching backend features. It does not enable a production endpoint.

| Consumer | Frozen operation IDs |
|---|---|
| FE21 discovery/detail/activity | `listConcertParties`, `getConcertParty`, `getConcertPartyActivity` |
| FE21 application/cancellation/recovery | `applyConcertParty`, `cancelConcertApplication`, `getConcertCommandStatus` |
| FE24 discovery/detail/own activity | `listConcertGiveaways`, `getConcertGiveaway`, `getConcertGiftActivity` |

The exact method/path/query/body/response/status/error definitions are the named
entries and their reachable closed schemas in `contracts/domain/v1/operations.json`,
`common.schema.json`, `party.schema.json`, `giveaway.schema.json`, C03 public
profile/avatar schemas, and `policy.json` at `ca9f955`. The browser artifact is
`contracts/generated/mcr-domain.openapi.json` at the same revision. No alternative
route, request DTO, legacy URL or second client is authorized by this decision.

Use canonical FE09 `auth.request`, the registered domain operation, typed UUID
idempotency key for commands, and scoped FE02 privacy cleanup. R1–R3 prerequisites
are reviewed at `2645b6f`/`5e829d0`; context rules at `ca9f955` with independent
tests/report integrated in `88fd8f3`. Combined domain checks: 93 passed; both
generated artifacts' drift checks pass. Independent reviews found no remaining
actionable defect in this bounded contract scope.

Consumer response decoders must enforce the request's captured actor/event,
requested resource ID, closed schema, count/time/state invariants, own collection
identity, duplicate IDs, requested page limit and public projection rules before
committing state. Every Member now requires `event_id`. Public gift DRAFT is
rejected; authorized own giving may contain drafts. Keep independent cursors and
empty continuations. A single page's absent row never proves eligibility. Server
authorization and linked-resource relationships remain authoritative.

For a final apply/cancel Application response, additionally bind event, applicant
to the actor and application/party IDs to the captured command target or already
validated selected application. A 202 is only a matching PENDING receipt. Receipt
queries bind command/key/actor/event and a known result target; SUCCEEDED uses
the command's registered final status and still needs authorized activity/resource
reread. 404, RETIRED, expiry, outage and lost responses never imply nonexecution,
success or permission to retry with a new key. No automatic command replay.

Retain all seven C03 capability decisions. NORMAL apply requires available meet
and mutation decisions plus known current eligibility. SAFETY cancellation in
READ_ONLY is not blanket-disabled by mutations=false/EVENT_READ_ONLY. Other deny
or unavailable reasons and unknown eligibility fail closed. Pending commands,
receipt presentation and private views are evicted on session/event/policy changes.

FE24 does not acquire FE25 authoring or FE26 reservation/pickup command ownership.
Actual application composition, browser journeys, authorization, SQL races,
multiuser recovery and full feature/task acceptance require separate evidence.
