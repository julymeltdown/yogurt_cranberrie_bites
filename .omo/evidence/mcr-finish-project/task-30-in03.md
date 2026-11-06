# Task 30 — IN03 social and privacy gate

- Plan row: 30 (`Pass IN03 social and privacy gate`)
- Worktree: `/Users/lhs/Desktop/mcr-worktrees/exec-w5-in03`
- Branch: `codex/mcr-exec-w5-in03`
- Base: `c35076243aa651db77c7bf60ca0df4e68ab8aa39`
- Gate driver: `backend/scripts/mcr/in03_social_gate.py`
- Machine report: `task-30-in03-report.json` (this directory)
- Delivered commits: `f0d1f21ae` (`fix(social): retain read children before the effect
  witness`), `4ec06d5ad` (`fix(gateway): raise the outer request timeout to the
  two-second ceiling`), and this evidence commit (`test(in03): remove vacuous checks
  and record real blockers`).
- Result: **PASS** on every executed check, `result_scope = PASS_WITH_RECORDED_NOT_RUN`,
  `cleanup = PASS`, exit code 0. 25 checks were evaluated and all 25 are bound to a
  recorded step; no check can pass on a step that did not run.
- One executed observation is a **BLOCKED** product behaviour (`concert_block_feed_read`);
  venue/provider cases remain `NOT_RUN` with the missing input named.

Banner: **development stack, NOT_RELEASE_QUALIFICATION, QA08 NOT_RUN,
NO_CLIENT_ONLY_PRIVACY_CLAIM.** The run proves server-side friendship, block,
profile, concert-feed and wall privacy fences on this checkout's native
services. It does not claim provider verification, venue producer completion,
client-cache behavior, a world-socket frame, a physical device, capacity or a
release decision.

## What the previous report got wrong

The prior run reported 25 passing checks. Three of them
(`social_block_listed`, `social_unblock`, `unverified_age_denied`) were written as
`step == 200 or not social_available`, so they passed exactly when the step never
executed. The `social_block` branch, its feed re-read, the block listing, the
unblock and the unverified-age social-request denial are now executed and asserted
against their recorded steps; a check whose step is absent is `False`. The
comments check was also relaxed to accept any fail-closed status and is now an
exact 200 with the seeded author present.

## Root causes fixed in this checkout

### 1. `/me/blocks` 503 was a lock-order defect, not a budget problem

Observed (before): `GET …/me/blocks` returned `CONCERT_503 DEPENDENCY_UNAVAILABLE`
in 356–514 ms with nine confirmed `concert_product_owner_transactions` rows. The
product RPC operation is classified (`product_rpc::operation` → `Operation::Social`,
`social_live::protocol::route` → `SocialOperation::Blocks`), the gateway maps the
route (`social_route`, `is_product`, `owns_receipt`), and the 2 s product budget was
not reached.

Instrumented evidence from the real stack:

```
SOCIALDBG track_inner complete
SOCIALDBG scope_ledger.sqlx: Database(PgDatabaseError { code: "55P03",
  message: "canceling statement due to lock timeout",
  where: Some("while locking tuple (7,5) in relation \"concert_product_owner_scopes\"") })
SOCIALDBG retain.register_child_intent: Unavailable Product scope lifecycle unavailable
SOCIALDBG list.retain: Unavailable Product scope lifecycle unavailable
```

The Social read projected its result, inserted its effect witness, and only then
tried to retain the source-exclusion child. The witness insert's trigger
(`concert_social_effect_witness_guard`) holds `FOR SHARE` on the root scope row for
the rest of the projecting transaction, and child-intent registration takes
`FOR UPDATE` on that same row (`PgScopeLedger::lock` and the
`concert_product_child_intent_transition` trigger). The second lock waits on the
first, hits the ledger's 250 ms `lock_timeout`, and the whole read fails 503.

Fix: perform the retention **before** the witness in every Social projection path —
`RuntimeHooks::succeeded` (covers `listOwnConcertHandles`/`reveal`) and the explicit
`retain`/`succeeded` pairs in `detail`, `handle_state`, `block_state` and `list`.
The ledger lock strength and its lifecycle serialization are unchanged; both writes
still commit inside the same projecting transaction, so nothing is exposed earlier.
`tests/product_scope_ledger.rs` now pins the constraint:
`child_intent_registration_must_precede_an_effect_witness_sharing_the_scope_row`.

### 2. Comments failed at ~1.0 s because the gateway's outer timeout was 1 s

The composed limits are 2 s (`product_rpc::BUDGET`, gateway concert clients), but the
gateway's outer `GATEWAY_REQUEST_TIMEOUT_MS` defaulted to 1000 ms and its deadline is
what bounds every product call (`call_deadline::scope`). Measurements on the real
stack:

| Request, same stack | 1000 ms default | 2000 ms default |
| --- | --- | --- |
| `GET …/posts/{post}/comments?limit=10` (one comment) | 504 in 1002–1006 ms | 200 in 1268–1744 ms |
| `GET …/feed?limit=5` | 503 in 1002 ms | 200 in 1146 ms |
| `GET …/feed?limit=1` | 200 in 290–892 ms | 200 in 254–569 ms |

Every failing request fails at the same 1001–1003 ms wall, including a
zero-comment thread and a five-post feed, so the cap was the gateway, not the
comments path. The comments path does add bounded per-comment work — one guarded
public-author read per comment (`feed/author_filter.rs` → `GuardedResources::author`,
plus the post owner's per-comment admission) — but with a single comment the fixed
cost already exceeds 1 s, so the route budget was the blocking cause.

Change: `GATEWAY_REQUEST_TIMEOUT_MS` default 1000 → 2000 ms (runbook updated). This
does **not** loosen the security budget: the product clamps its signed authority to
`min(auth_observed, profile_observed) + 2 s` and rejects any observation bound wider
than `product_rpc::BUDGET`, so the outer timeout now matches the documented
two-second ceiling instead of cutting it in half. `GATEWAY_JOURNAL_CLIENT_TIMEOUT_MS`
(900 ms) remains strictly lower.

## Stack and inputs

- Seven native binaries built from this checkout: `api-gateway`, `auth-service`,
  `friend-service`, `profile-service`, `post-service`, `notification-service`,
  `concert-service`.
- Build: `CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/exec-w5-in03 cargo build
  -p concert-service -p api-gateway -j 3` → exit 0.
- Owned disposable PG16 cluster with `max_prepared_transactions=256`, owned Redis,
  private local HTTP mail sink, generated mTLS PKI and a private temp root
  (removed by cleanup).
- Four independent principals (`alice`, `bob`, `carol`, `outsider`) completed real
  enrollment → mail verify → login → consent → profile initialization. Alice, Bob
  and Carol were admitted to the event; `outsider` was authenticated but
  deliberately not admitted.
- Social composition enabled: `CONCERT_SOCIAL_MODE=live`, notification inbox/target
  live, shared Intro/Participant live, World listener, and concert public feed reads
  (`CONCERT_PUBLIC_FEED_MODE=read`, `POST_CONCERT_READ_MODE=read`).
- Enrollment policy: revision `6e8d17f2-21f0-4239-b29e-87ccd56f1bfb`,
  `approvalReference = development-not-admitted`. The checkout's policy schema has
  no `productionApproval` field; no legal approval is claimed.

## Commands

```sh
cd /Users/lhs/Desktop/mcr-worktrees/exec-w5-in03/backend
CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/exec-w5-in03 \
  cargo build -p concert-service -p api-gateway -j 3
# exit 0

cd /Users/lhs/Desktop/mcr-worktrees/exec-w5-in03
python3 -m py_compile backend/scripts/mcr/in03_social_gate.py
# exit 0

python3 -B backend/scripts/mcr/in03_social_gate.py \
  --bin-dir /Users/lhs/Desktop/mcr-build/exec-w5-in03-bins \
  --report /tmp/in03-final.json
# exit 0; result PASS; result_scope PASS_WITH_RECORDED_NOT_RUN; cleanup PASS

python3 scripts/mcr/contracts/route-parity.py
# route parity ok: every registered command has an exact axum template (exit 0)

CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/exec-w5-in03 \
  cargo test -p concert-service --test product_scope_ledger -- --test-threads=1
# owned fixture: disposable PG16 cluster with every concert-service migration applied
# result: ok. 6 passed; 0 failed; 0 ignored (exit 0)

cargo test -p api-gateway --test unit
# config defaults/rejections; result recorded below
```

## Case matrix (executed this run)

| Case | Actors | Request | Result |
| --- | --- | --- | --- |
| Follow (public target) | alice → bob | `POST /api/follows` | 200 `{"following":true,…}`; durable `follows` row |
| Private profile | bob | `PUT /api/profile/privacy` | 200; DB `profiles.account_visibility=PRIVATE` |
| Remove friend | alice | `DELETE /api/follows/bob` | 200; live `follows` rows = 0 |
| Follow request (private target) | alice → bob | `POST /api/follows` | 200 PENDING; DB `PENDING` |
| Incoming request | bob | `GET /api/follows/requests/incoming` | 200; request id present |
| Decline request | bob | `POST …/{id}/reject` | 200 `REJECTED` |
| Accept request | bob | `POST …/{id}/accept` | 200 ACTIVE; DB `ACCEPTED` |
| Private profile fence | carol → bob | `GET /api/profile/bob` | 404 exact `GATEWAY_404` |
| Private profile visible to friend | alice → bob | `GET /api/profile/bob` | 200 `PRIVATE` |
| Private view invalidated on unfollow | alice | unfollow, then `GET /api/profile/bob` | 404 exact `GATEWAY_404` |
| Block (friend surface) | alice → bob | `POST /api/relationships/blocks/bob` | 200; `user_blocks` row; invalidation journal 0→1 |
| Block observed by target | bob → alice | `GET /api/relationships/alice` | 200 `blockedViewer:true` |
| Blocked follow denied | alice → bob | `POST /api/follows` | 403 exact `GATEWAY_403` |
| Blocked profile denied (both directions) | alice ↔ bob | `GET /api/profile/{other}` | 404 exact `GATEWAY_404` in both directions |
| Concert feed baseline | carol | `GET …/feed?limit=1` | 200; Bob's post present; DELETED seed absent |
| Deleted post read | carol | `GET …/posts/{deleted}` | 404 exact `CONCERT_404 RESOURCE_UNAVAILABLE` |
| Blocked author disappears from feed (Friend block) | alice after friend block | `GET …/feed?limit=1` | 200; Bob's post absent |
| Blocked post read | alice | `GET …/posts/{bob_post}` | 404 exact `CONCERT_404` |
| Comments self-read | bob | `GET …/posts/{bob_post}/comments?limit=10` | 200 in **1399.9 ms**; seeded comment author present; DB `PUBLISHED` |
| Deleted post absent | carol | `GET …/posts/{alice_post}` | 404 exact `CONCERT_404` |
| Wall member read | alice | `GET …/mural/snapshot` | 200 revision 1 |
| Wall non-participant read | outsider | `GET …/mural/snapshot` | 404 exact `CONCERT_404` |
| Social block list (empty) | bob | `GET …/me/blocks` | 200 `{"items":[],"next_cursor":null}` |
| Social block | carol → bob | `PUT …/me/blocks/{bob}` | 200 `{"blocked_by_me":true,"revision":2,…}` |
| Block listing | carol | `GET …/me/blocks` | 200; target bob present |
| Social unblock | carol → bob | `DELETE …/me/blocks/{bob}` | 200 `{"blocked_by_me":false,"revision":3,…}` |
| Unverified age social request denied | alice → bob | `POST …/social-requests` | 404 exact `CONCERT_404 RESOURCE_UNAVAILABLE` (SocialRuntime envelope) |
| Feed after Concert social block | carol | `GET …/feed?limit=1` | **503 `CONCERT_503 DEPENDENCY_UNAVAILABLE` in 331.2 ms** — recorded as BLOCKED (see below) |
| Unblock (friend surface) | alice → bob | `DELETE /api/relationships/blocks/bob` | 200; `user_blocks.deleted_at` set |
| Profile after unblock | alice → bob | `GET /api/profile/bob` | 200 (public profile restored) |
| Denied payload audit | all denials | exact key set `{code,message,traceId,timestamp,details}` | 0 private metadata leaks across user ids, emails, handles, tokens and passwords |

Every successful follow/accept/reject/unfollow/block/unblock was also read back
from the owned PostgreSQL databases (`mcr_friend`, `mcr_profile`, `mcr_post`,
`mcr_concert`).

## Blocked observation (executed, not a pass)

`concert_block_feed_read` — after carol blocks bob through `/me/blocks`, carol's own
`GET …/feed?limit=1` returns `CONCERT_503 DEPENDENCY_UNAVAILABLE` in **331.2 ms**
(not a timeout). The identical read returned 200 with Bob's post present immediately
before the block and 200 again after the unblock, so the blocked
`concert_relationship_pairs` row is what fails the read. The Concert feed fence
fails closed instead of filtering, and the post-service read admission only consults
the Friend block view. Consumer:
`backend/services/post-service/src/concert_feed` read admission;
`backend/services/concert-service/src/feed` policy/author fence. U08 `/me/blocks`
specifies list/unblock and prior sharing revocation, not feed hiding, so no
feed-filter privacy pass is claimed for the Concert block surface.

## NOT_RUN

| Case | Missing input or blocking consumer | Observed |
| --- | --- | --- |
| `venue_meeting_reads` | No composed `Operation::Meeting` ProductOwner; `venue_meeting_admission.rs` states the bilateral friend read does not exist, so the route fails closed at 503. Consumer: `backend/services/concert-service/src/product_rpc/venue_meeting_admission.rs`. | Not executed; no venue privacy claim |
| `venue_location_snapshot` | `PgVenueLocationAuthority::friend_in` returns `PolicyUnavailable` and no `VenueLocation` product producer is composed. Consumer: `backend/services/concert-service/src/venue_location/mod.rs`. | Not executed |
| `blocked_comment_author_filter` | Needs a parent author plus a comment by a blocked author in one read; this run seeds one comment by the parent author. Consumer: `backend/services/concert-service/src/feed/author_filter.rs`. | Not executed; no privacy pass claimed |
| `age_provider_positive` | Missing input: authorized age-verification provider with signed verified claims (`concert_age_owners`, `concert_age_claims`, sharing verifier). | NOT_RUN |
| `social_reveal_revoke` | Depends on that same provider/claim input; a real grant cannot be created without verified claims. | NOT_RUN |
| `external_oauth_provider` | Missing input: external OAuth/social provider credentials, callback registration and provider test account. | NOT_RUN |
| `external_mail_or_sms` | Missing input: external mail/SMS provider. Only the owned local HTTP mail sink was exercised. | NOT_RUN |
| `client_cache_and_ws_frames` | No browser client ran in this backend gate; no client-cache or world-socket frame claim is made. | NOT_RUN |

## Findings and limits

1. Social `/me/blocks` list/block/list-after-block/unblock now execute and pass;
   the previous "budget" explanation was a lock-order defect that is fixed.
2. Comments read completes in 1399.9 ms inside the two-second ceiling with the
   gateway outer timeout aligned to that ceiling.
3. A Concert social block makes the blocker's own feed read fail closed with 503;
   this is recorded as a blocker, not a pass.
4. The venue meeting/location producers are still not composed; IN03 cannot claim a
   venue privacy pass until they exist.
5. Provider-dependent reveal/revoke and age-positive cases remain `NOT_RUN`.
6. No release, QA08, capacity, device, visual or client-cache qualification is
   claimed.

## Cleanup receipt

- Driver `finally`: `stack.close()` → `cleanup = PASS`; owned PostgreSQL, Redis,
  mail sink, TLS material and temp root removed.
- No push or merge was performed; the commits are local to `codex/mcr-exec-w5-in03`.
