# Integrated authenticated Auth admission read

Date: 2026-11-03. Bounded BE04 prerequisite; whole BE04 remains IN_PROGRESS.
The generated Auth server now exposes the real minimal owner-facts read behind
its registered Gateway/actor/body/replay guard and original-deadline layer.
Concert's production `UnavailablePolicy` is unchanged. This is not browser
admission, an accepting Concert adapter or release acceptance.

## Source and review

Parent prerequisites `a44f8fa` and `e79fd177` integrated as `1df8a96` and
`4d8c01f`; worker implementation `9bf3233` as `9dd8bc6`, followed by reviewed
test correction `5c59f1f` as `4bc75bb`. The worker's duplicate prerequisite
cherry-pick `e774b1a` was not integrated again. All nine delivered worker paths
match final `5c59f1f`: [inventory](auth-admission-rpc/hashes.json).

The parent reviewed the full handler, generated-server tests, native harness,
four minimal composition exceptions, method policy, private authority context
and actual owner adapter. The separate prerequisite/event review is retained in
[its QA record](../qa/admission-contract-review.md). Independent actual-handler
review is recorded in [the handler QA record](../qa/auth-admission-rpc-review.md).

QA found one P2 test setup race: a proof with almost zero remaining live time
could expire before the owner read, missing the intended during-read test. R1
now reserves 1.25–1.5 seconds of signed live time before one request, confirms
owner dispatch while live, holds the owner pending, and requires actual proof
expiry to win before both the two-second resource ceiling and five-second
original deadline. No denial retry or weaker assertion was substituted. The
runtime handler was unchanged by this correction.

## Qualified behavior

- Only a registered Gateway with a verified exact request/actor proof can call
  the method. Canonical nonnil user/family IDs and optional exact provider row,
  provider and positive owner generation are required; shape is not ownership.
- The production Pg adapter reads current account, exact live family and optional
  binding facts in one database snapshot. Email-only requests do not touch the
  provider relation. No current negative fact becomes a positive fallback.
- The guard-entry start, original caller stop, captured proof stop and current
  signed wall expiry bound the complete await. Rechecks reject late success;
  the cancellation flag signals dropped work without promising immediate DB
  termination. Locked-query timeout and one-connection recovery are exercised.
- The response contains only observation time and account/session/binding facts,
  with no-store metadata. Credentials, subjects, email, roles, Profile content
  and portable admission authority are excluded. Errors have fixed safe text.

## Combined verification

Clean final candidate `97a698a43e502a73ff00a489b3155b370cb9d681` was rebuilt
with locked/offline Rust 1.88.0 and two jobs. Cargo-selected artifacts passed
**11 controlled generated-server cases**, **five real PostgreSQL RPC cases**
and **22 retained owner cases**, with zero ignored native tests. Both fresh
PG16.14 clusters passed all thirteen SQLx migrations, exact ledger/reapplication,
restart persistence and cleanup. All 178 receipt source hashes match; the parent
independently confirmed both owned directories were absent.

The initial combined `9dd8bc6` also passed 103 platform authority, two live guard,
48 scoped Auth library, 23 application library, eight value and thirteen Gateway
transport tests. Their owning runtime sources remain unchanged by R1. These
results, final command exits, native receipts and resolved failures are retained
in [auth-admission-rpc/](auth-admission-rpc/). Final build/test-build, controlled
and native commands each exited zero. The helper/contract follow-up at `d197379`
passed **98 helper tests**, **138 contract regressions** and planning checks.

Two integration issues were resolved explicitly: an invocation named a nonexistent
`live_grpc_guard` test target (the correct network target then passed), and BE15
retained stale contract pins after `5c02a31` (the parent audited/refreshed them and
added a negative drift check). The [pin review](receipt-contract-pin-refresh.md)
preserves the unchanged command/receipt semantics; neither failure is hidden by
the final passing checks.

The worker's full Auth library attempt remains **48 passed / one failed** because
the unchanged schema/privilege fixture requires unavailable Docker. The explicit
filtered command is **48 passed / one filtered**, not all-library PASS. Its
original failure and command receipts are retained. Native migrations and owner
reads do not certify that separate restricted-role/schema-damage test.

## Disposition and limits

The bounded implementation and independent review deliveries are approved. Both
actual goals are tool-confirmed COMPLETE and their source/build checkouts stopped. Real Gateway JWT-to-read orchestration, Profile and
event eligibility, signed Concert admission, production mTLS/least privilege,
backup/restore epoch safety, replica replay and revocation/stream invalidation
remain unqualified. A completed snapshot is not a lease against later revocation.
No BE04, C04, IA or integration acceptance count changes. Full commands and
provenance remain in the [worker record](../backend/auth-admission-rpc.md).
