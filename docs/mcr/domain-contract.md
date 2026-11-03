# C04 domain HTTP and transition candidate

State: **IN_PROGRESS / REVIEW_CANDIDATE**, version 0.1.0. This is an offline
contract candidate, not a freeze, generated SDK, server implementation, provider
qualification or task acceptance. The original eight untested schemas were
preserved from the stopped writer and revised by the integrator in
`/Users/lhs/Desktop/mcr-worktrees/contracts-C04`, advanced to base `c654507`.
No backend or frontend implementation is changed by this candidate.

## Delivered contract surface

Eight Draft 2020-12 schema documents contain 162 definitions across common
receipts/errors, parties/applications/membership/rally, physical beacons,
giveaways/reservations/pickup, introductions/age/private social sharing,
feed/media/notifications/blocks, mural and operations. Objects are closed. Core
public profiles and asset places are referenced from their existing local IDs;
there is no second canonical identity or parallel copy of those contracts.

`contracts/domain/v1/operations.json` inventories 106 event-scoped operations.
Each records method/path, request location/schema, success and pending schema,
actor, a named precondition/result rule, write policy, idempotency/retry behavior,
no-store policy, and status/reason matrix. GET/DELETE structured arguments use
query parameters; POST/PUT/PATCH use JSON except the explicitly bounded image
byte PUT. The register is a target interface, not runtime route coverage.

The combined [browser HTTP review artifact](browser-contract.md) now reproduces
through the retained final/core pipeline. It contains 132 HTTP operations: 24
unchanged C03, 106 C04 and two C05. The new assembler preserves all source schema
assertions, namespaces domain/realtime definitions, resolves references locally,
and emits explicit path/query/body parameters, error status/reason restrictions,
no-store headers, idempotency headers, pending receipts and binary image content.
The frozen core artifact and FE09's input are unchanged. Its 14 integrity tests
are assembly/schema evidence, not a full OpenAPI conformance certification or
complete producer/consumer semantics. See [assembly evidence](evidence/domain-assembly.md).

All 106 operations now declare exact path parameter types and a closed query
schema, including an empty query for body operations. Public discovery has bounded
ticket-section, capacity/stock, time and feed filters; own/admin lists use explicit
state filters. Pagination defaults to 20 and is capped at 100 per collection.
Party activity now returns three independently paged collections; giveaway
activity uses distinct giving/receiving cursors. No ambiguous shared cursor can
silently skip one of those collections. Cursors are opaque authenticated encrypted
or server-stored random tokens bound to actor/event/operation/collection/filter;
their authenticity and current authorization remain producer obligations.

The target parsing oracle accepts percent-encoded UTF-8 exactly once, rejects
duplicate/unknown fields, malformed encoding, noncanonical integers/booleans,
unsafe identifiers and mismatched events, and enforces query bounds. It does not
implement an HTTP router, database filtering, authorization or cursor signing.
DELETE revision arguments are query fields; GET and DELETE have no JSON body.
Filter ranges are lower-inclusive/upper-exclusive UTC intervals. Details live
in `policy.json.request_targets` and the per-operation query schemas.

`policy.json` carries 75 reusable operation rules, common authorization and
browser guards, phase-aware write categories, state transitions, paging,
receipt retention/recovery, pickup and age invariants. The offline validator
rejects unknown schema references, duplicate routes/IDs, undocumented errors,
status drift and mutations missing either browser guard. Every referenced
schema resolves locally; network schema retrieval is disabled.

`inherited-map.json` binds 15 target adapters to exact operation IDs/methods/paths
from the retained final `generate-openapi.py.build_document()` and verifies both
inherited receipt-status queries. Event membership, DTO projection, canonical
identity, safe notification payloads and asynchronous 202 recovery still need
adapters. Editing posts, listing/editing/deleting comments, event association and
the image pipeline are explicitly new capabilities. An upstream 202 is never
converted into an already-completed action.

## Decisions under review

- New commands use canonical UUID idempotency keys scoped by actor/event/operation.
  Method, canonical target and body/query digest belong to the request hash, so
  reusing a key for another target conflicts. A 24-hour result window is a
  candidate default; retained key tombstones prevent expired commands running
  again. Final operational retention still needs the closure/deletion policy.
  Current authorization is rechecked before reading a saved result. Receipts
  contain status metadata, never raw request bodies or one-shot secrets.
- All personalized responses and errors are no-store. Mutations require exact
  allowed Origin **and** `X-MCR-Intent: browser` before downstream effects. Domain
  reason names retain C03's `INVALID_INPUT`, `AUTHENTICATION_REQUIRED`, and
  `INVALID_MEDIA_TYPE`; new domain-specific reasons extend them. Generic
  `RESOURCE_UNAVAILABLE` hides inaccessible private resources.
- Capacity includes the party owner; active parties always have exactly one owner.
  CLOSED may reopen, while CANCELLED and COMPLETED are terminal. Cancellation
  does not transition to completion. Approval, membership and capacity updates
  must share a database transaction. Ownership transfer is explicit; leaving
  cannot produce an ownerless active party.
- Public place references do not disclose private physical instructions. Beacons
  require explicit acknowledgement, reviewed open places, revisions, current
  authorized audiences and DB-time expiry. Removed members lose descriptions and
  subscriptions. Virtual coordinates are not accepted in beacon DTOs.
- Giveaway CLOSED means the UI's Completed label; CANCELLED is separate. Sold-out
  is derived. `available = total - reserved_unclaimed - claimed`. A reservation
  holds one unit; HELD becomes READY at pickup start, and expiry is pickup end
  plus captured configured grace, not ten minutes after booking. Existing
  reservations prevent silent pickup-window/public-place changes. Private meeting
  updates are versioned and notify recipients that the instructions moved.
- Pickup reads never mint confirmation codes. Explicit recipient code issuance
  returns six digits once, an exclusive expiry of at most five minutes and the
  new reservation revision. Only a keyed hash is retained. Owner claim checks the
  reservation, code, current policy and shared throttling, then atomically moves
  reserved to claimed stock. Duplicate confirmation returns the same safe receipt.
  A lost code response requires explicit rotation, never a plaintext receipt.
- Age bounds are either both null or both numeric with `min <= age < max`. The
  source's current validator supports both-or-neither; older independently-null
  SQL is not copied. Canonical claim order is verified time DESC, expiry DESC,
  claim ID DESC, selected **before** validity. There is no fallback to an older
  valid claim. Claims are at most 90 days; one birthday transition uses Seoul
  midnight. February 29 becomes February 28 in a non-leap year. An immediately
  replaced same-tick claim may have zero validity. Unknown/expired/unsupported
  claims fail restricted policy without deciding basic world eligibility, event
  admission or legal signup age.
- Introduction drafts allow empty text; publishing does not. Create-if-absent
  private intro/handle commands use expected revision 0; existing resources use
  a positive revision and handle tombstones keep versions monotone.
- A separate authorized sharing-offer query exposes platform/version metadata,
  so the client can bind consent without discovering a plaintext handle. It is
  not public-profile metadata. Approval names a nonempty subset of requested
  platforms with explicit reciprocal consent. Reveal requires both current
  reciprocal grants, exact user/event/request/platform/version bindings, valid
  expiry and freshly evaluated membership, age, block and suspension policy.
  Replacement/removal of either handle invalidates old access. Unblock never
  resurrects consent. Reveal is explicit, without prefetch or automatic replay.
- Only sanitized, actor-owned READY image media can be attached. The candidate
  uses same-origin bounded PUT for its first transport; optional presigned upload
  ports do not imply a qualified external object store. Decode/reencode,
  signature/MIME checks, single frame, dimensions/bytes/pixel limits and metadata
  stripping remain real server obligations. REPORT media remains private.
- Mural operations omit author IDs. Deltas are consecutive from a known revision;
  a reset contains no replayable operations. Restore occurs only while paused,
  creates a greater revision and forces resync. FROZEN cannot resume. A closure
  job cannot claim COMPLETED while any required step failed. Public archives use
  separate projections with no live profiles, personal reaction flags, private
  meetings, handles or recipient/applicant lists.

## Source fidelity and verification

Age arithmetic/boundary behavior was rechecked against pinned Bigyeting commit
`5eecdcaffd1a9d505b9ab2a974f8d5623217e90a`, specifically
`backend/services/bigyeting-api/src/domain/privacy.rs` and
`src/adapters/out/repository/current_age.rs`. The pure Python oracle is newly
authored from those documented semantics; no source UI or provider credentials
are copied. Raw date-of-birth fields are fixture inputs only, not HTTP DTO fields.
The inherited backend API map is checked against the already imported port-rust
source `704f42149d5e8e35e1d578676d8ae2dac4f0f9c2` final assembler.

Commands from the isolated contract worktree:

| Command | Result |
|---|---|
| `python3 scripts/mcr/validate-domain.py` | Exit 0; 162 definitions, 106 operations, 35 indexed positive/negative fixtures; exact inherited map resolves |
| `python3 -m unittest discover -s scripts/mcr/tests -p test_domain.py -v` | Exit 0; 46 tests, including target checks across all 106 operations |

Regressions cover canonical claim replacement/ties, no fallback, supported age
limits, birthdays/Seoul year boundary/leap-day behavior, expiry boundaries,
reciprocal consent and handle replacement, foreign actor/event/platform denial,
private-field leaks, capacity/stock equations, reservation timing, terminal-state
rules, mural gaps/resets, unsafe numeric/Unicode data, duplicate JSON keys,
receipt identity/status fences and browser-guard/error-registry drift.
Target regressions additionally cover path metadata drift, strict scalar decoding,
invalid/double UTF-8 percent encoding, duplicate encoded keys, event isolation,
independent activity cursors, revision-bearing DELETE queries and command-only
receipt/audit filters. The first target pass exposed a chained timestamp schema
reference and a test using forbidden mural revision zero; both were corrected.

These are schema and semantic examples. They do not prove locks, concurrent stock
or capacity correctness, durable receipts/outbox, provider authenticity, request
origin enforcement, actual privacy revocation, image processing, API availability
or two-user flows. The first inherited-map check failed because the final generator
needs its sibling import directory; the validator now provides that directory
only for the assembly call and restores the search path afterward.

## Remaining before C04 freeze

1. Complete per-operation request/success/error fixture coverage and contextual
   producer/consumer compatibility, including cursor authenticity/authorization.
   All path/query schemas are registered, but their target tests and the current
   35 payload examples are not complete 106-operation HTTP coverage.
2. Complete global account deletion, safe recovery after auth revocation, public
   legal/document publication and retention/closure policies. The preserved
   draft deletion DTOs are not registered as a pretend working flow. Align the
   C03 Auth account-delete adapter with the eventual global cleanup coordinator;
   an event-only delete cannot certify global erasure.
3. Tighten aggregate-aware response context checks, explicit terminal tombstones,
   media failure/pending DTOs, owner/public draft distinctions, private activity
   ownership and public archive eligibility. Source-derived semantics take
   precedence over convenience in this candidate.
4. Review the combined retained-pipeline candidate and qualify generated client
   adoption after the full contract freezes. Assembly/collision/drift checks now
   pass; they do not establish per-operation producer/consumer compatibility.
5. Obtain independent BE/FE review and fix their findings before marking C04
   CONTRACT_READY/ACCEPTED. Runtime implementations and integration gates remain
   separate obligations even after that contract freeze.
