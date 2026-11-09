# WP-10 release gate re-run — findings

Status: **GATE INCOMPLETE (honest measurement)**. The gate script was **not**
modified. Two real integration defects were found and fixed; a third is
diagnosed and reported rather than papered over.

Branch `codex/yc-refactor-1`, base `3f49da8e2`.

## 1. The gate tally

| Run | PASS | PARTIAL | FAIL | BLOCKED | Result |
| --- | --- | --- | --- | --- | --- |
| Before (master `3f49da8e2`) | 0 | 2 | 0 | 5 | INCOMPLETE |
| After the two fixes | 0 | 2 | 1 | 4 | INCOMPLETE |

Per step, before → after:

| Step | Before | After |
| --- | --- | --- |
| step1_profile_posts | BLOCKED (401 on first upload) | **FAIL** (all media/post calls 200; author list count 0) |
| step2_intro_post | BLOCKED (400/404) | BLOCKED (400/404) |
| step3_liker_navigation | BLOCKED (401) | BLOCKED (401) |
| step4_poster_and_delete | PARTIAL (401) | PARTIAL (401) |
| step5_outsider_denial | BLOCKED (404/401) | BLOCKED (404/401) |
| sweep cross_surface | BLOCKED | BLOCKED |
| sweep lifecycle | PARTIAL | PARTIAL |

The step-1 status moved from BLOCKED to FAIL, which is **progress, not
regression**: BLOCKED meant "the product cannot reach this surface at all"; FAIL
means "the surface works and one assertion about it does not". Every media and
post call in step 1 now returns `200`.

## 2. Defect 1 (fixed) — the concert-hosted profile transport had no parent authority

**Symptom.** Every concert-owned profile operation (`initProfileMediaUpload`,
`getOwnRoom`, `listProfilePosts`, …) returned `401 PROFILE_401
AUTHENTICATION_REQUIRED` at the Gateway, while the Gateway's own telemetry
recorded the route as `SUCCESS` and the Gateway log showed the request entering
`dispatch` and never reaching the producer.

**Cause.** `crates/platform/src/actor_delegation.rs` grants
`VerifiedParentAuthority` to a closed allowlist of methods. It listed
`/profile.product.v1.ProfileProductService/Dispatch` (the profile service's
transport) but **not** `/concert.profile_product.v1.ProfileProductService/Dispatch`
(the concert service's transport, added later by the profile-owner routing work).
Both producers re-decode the signed parent and refuse the request without it, so
the concert-hosted one answered `Unauthenticated: Verified original profile
parent required`.

**Fix.** Added the concert-hosted method to the allowlist, with a comment naming
both transports and why both are needed. A new test,
`both_profile_product_transports_receive_the_verified_parent`, asserts that both
methods receive parent authority and that a neighbouring method on the same
service does not.

## 3. Defect 2 (fixed) — the concert profile producer had no media store

**Symptom.** With defect 1 fixed, `initProfileMediaUpload` returned `503
DEPENDENCY_UNAVAILABLE`.

**Cause.** `ProfileProductProducer::new` leaves `media: None`, and
`with_media` was never called by `profile_product::transport::runtime`. The
producer therefore answered every media operation with "Profile media is
unavailable".

**Fix.** `profile_product::runtime` now takes the media store and attaches it;
`main.rs` builds a `ProfileMediaStore` over the same bounded
`BoundedImageSanitizer` the concert upload uses, so a profile photograph is
normalized by identical code and the shared `concert_application::media` limits
apply. `None` remains expressible for a composition that genuinely has no
sanitizer, and then the refusal is truthful rather than silent.

## 4. Defect 3 (diagnosed, NOT fixed) — the Gateway sends the account id as `profile_id`

**Symptom.** After both fixes, step 1 creates both media assets and both posts
(`200` each), but `GET /api/profiles/{authorProfileId}/posts` returns `200` with
`items: []`.

**Cause.** The Gateway's profile clients send `profile_id: actor.to_string()`,
where `actor` is the **Auth account** id. `profile_posts.author_profile_id` is
keyed by the **profile** id from `profile_identities`, which is a different UUID.
Verified directly against the gate's database:

```text
POSTS        a5ebf1e1-9dd0-481a-8828-955fafb767ed   (stored author_profile_id)
AUTHOR_USER  a5ebf1e1-9dd0-481a-8828-955fafb767ed   (Auth account id)
AUTHOR_PROFILE 8722c367-73d3-4389-8c73-44802d345dd8 (real profile id)
```

So `createProfilePost` writes under the account UUID and the list reads under the
profile UUID, and the two never meet.

**Why it is not fixed here.** Resolving account → profile id is a design change,
not a patch: the Gateway would need the profile id from the identity composition
(the `ConcertProfileObservation` does not carry it today), or the concert service
would need a local account→profile mapping. `discovery_profiles` carries both
columns and is in the same database, so it is a viable local source — but its
projection is not driven in the gate stack, so it is empty there. Choosing
between "extend the identity observation" and "drive the projection" is a
decision for the routing/integration owner, and either choice is outside a gate
re-run. **No migration was added**, so the ledger is unaffected.

This defect also blocks step 5 (outsider denial) and the cross-surface sweep,
which both need a real profile id to address.

## 5. Step 2 and step 3 remain blocked for their own reasons

* **step2_intro_post** — `POST /api/me/intro-posts` returns `400 INVALID_INPUT`
  and `GET /api/discover/intro-posts` returns `404`. The intro routes are
  registered on the concert transport but the Gateway's profile route table does
  not mount them, so the request never reaches the producer. This is the
  "11 of 13 intro operations served" gap: served by the producer, not yet
  mounted by the Gateway.
* **step3_liker_navigation** — `/api/intro-posts/{id}/likes` returns `401`,
  which is the same unmounted-route symptom as step 2.

## 6. Verification

| # | Command | Exit |
| --- | --- | --- |
| 1 | `python3 backend/scripts/mcr/wp10_profile_social.py --bin-dir …/backend/target/debug --report docs/mcr/integration/evidence/wp10-gate-report.json` | **1** (INCOMPLETE — see §1) |
| 2 | `cd backend && cargo +1.88.0 check --locked --workspace` | **0** |
| 3 | `cargo test -p platform --lib` (includes the new parent-authority test) | **0** (210 passed) |
| 4 | `cargo test -p concert-service --test concert_profile_product_routes` | **0** (12 passed) |
| 5 | `python3 backend/scripts/mcr/wp02_migration_ledger.py --report /tmp/gate-ledger.json` | **0** (PASS / PASS) |

The gate exits 1 by design when the result is not PASS; that is the honest
measurement, not a failure of the harness.

## 7. Not done

* **Step 2 of the assignment (the visitor profile read) was not started.** It
  depends on `getProfile` producing `shared_items`, `room_available` and
  `gender_bucket`, and all three are keyed by the profile id — the same value
  defect 3 shows the Gateway does not currently resolve. Building the read on top
  of an unresolved identity would produce three fields that are correct in a unit
  test and empty in the product. It should follow the account→profile resolution
  decision.
* **Step 3 (switch the visitor route) was therefore not done.** It was
  conditional on step 2 producing all three fields, and step 2 was not built.
* The gate script is unmodified. No migration was added. No evidence document was
  deleted.
