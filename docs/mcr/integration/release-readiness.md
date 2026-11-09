# Release readiness analysis

Status: **NOT RELEASABLE**. This document states what is proven, what is broken,
and what must be true before the integration can be called complete.

| Field | Value |
| --- | --- |
| Snapshot | 2026-11-09 19:20 Asia/Seoul |
| Head | `421a4e620` |
| Tasks integrated | 106 / 110 |
| Release gate | **FAIL** — 1 PASS, 3 FAIL, 1 BLOCKED, 2 sweeps incomplete |

---

## 1. The honest summary

The feature code is essentially complete. Every work package landed, every
security decision is implemented and tested, and 106 of 110 tasks are merged.

**The product still does not work end to end.** The integration gate — seven
real services, three real identities, real HTTP — passes one of five steps.

The gap is not missing features. It is that several pieces were built and tested
against fixtures that hid the way they compose. Each work package was correct in
isolation and each verification was honest about its own scope; the defects live
in the seams between them.

That is the normal failure mode of parallel development, and it is what the gate
exists to catch. The gate is doing its job.

---

## 2. What is proven

| Capability | Evidence |
| --- | --- |
| 43 profile operations frozen with schemas | 35 core + 61 domain + 17 bundle contract tests |
| Schema applies from empty and from previous head | Migration ledger, both services, both scenarios |
| NAVER verification produces a real claim | 85 tests; live, expired, revoked, absent, wrong-gender |
| The 12-row visibility decision | 14 pure + 8 SQL parity, one shared case table |
| The decision is one implementation | Pure function and SQL predicate proven identical |
| Likes, likers, media, links share that decision | 35 tests across five suites |
| Per-row block resolution | 9 tests, including unresolvable-pair denial |
| Profile posts and media | 61 tests; attachment isolation proven |
| Personal room and posters | 62 backend + 64 frontend tests |
| Avatar re-keyed to profile ownership | 34 backend + 27 frontend tests |
| Discovery projection with worker | 19 + 12 + 11 + 5 tests |
| Three-section navigation and profile screens | 112 tests |
| Profile identity created at registration | 3 tests, id matches the backfill derivation |

**Step 1 of the gate passes**: two photographs uploaded, normalized, published,
and listed through real HTTP against seven real services.

---

## 3. What is broken, in dependency order

### B-1 — The discovery projection is never populated

**Symptom**: `discovery_profiles` is empty in a running stack.

**Cause**: `main.rs:891-901` builds the projection worker's channel with
`Channel::from_shared(endpoint)?.connect_lazy()`. That is a plaintext channel
with no TLS material and no service identity. The profile service listens on
HTTPS and admits only the Concert workload, so every round fails before it
reaches the source.

The worker starts, records no failure, and produces nothing. The failure is
silent because a round failure is logged and retried by design — a stale
projection is better than a stopped service — so nothing surfaces.

**Blast radius**: this is the single root cause behind three gate failures.

| Depends on it | Why |
| --- | --- |
| Intro detail (step 2) | The response needs the author's nickname; the producer refuses a detail it cannot render rather than showing it blank |
| Liker list rows (step 3) | `shared_with_author` and the author summary come from the projection |
| Discovery people list | Empty projection, empty list |

**Why it was not caught**: the visibility tests mirror their own profiles into
`discovery_profiles` as fixture setup. That is correct for testing the
predicate, and it means the predicate never ran against an empty projection.

### B-2 — The intro delete returns 400

**Symptom**: `DELETE /api/me/intro-posts/{id}?expected_revision=N` returns
`INVALID_INPUT`.

**Cause**: not yet diagnosed. The publish returns 200 and the gate now sends the
revision the publish reported. Either the delete parser rejects the query shape
or the revision it receives is not the one the row holds.

**Blast radius**: step 4, and the "delete an intro post leaves the room intact"
half of the lifecycle sweep.

### B-3 — The avatar read returns 503

**Symptom**: `GET /api/me/avatar` returns `DEPENDENCY_UNAVAILABLE`.

**Cause**: not yet diagnosed. The avatar producer is served by the concert
transport and the route exists, so this is either a missing dependency in the
composition or a store that was never wired.

### B-4 — Room visibility returns 409

**Symptom**: `PATCH /api/me/room/visibility` returns `VERSION_CONFLICT`.

**Cause**: the gate sends a revision the room does not hold. Either the gate
reads the revision from the wrong field or the room's initial revision is not
what the read reports.

### B-5 — The lifecycle sweep's durable half errors

**Symptom**: `JSONDecodeError` reading a durable response.

**Cause**: a probe returned a non-JSON body where the sweep expected JSON. This
is a gate defect, not a product defect — but it must be fixed before the sweep
can report anything.

### B-6 — The cross-surface sweep uses invented paths

**Symptom**: eight surfaces return three different statuses.

**Cause**: the sweep still probes `/api/intro-posts/{id}/attachments` and
`/api/intro-posts/resolve?token=...`, neither of which is in the contract, and
uses a hardcoded introduction id. Same class of defect as the one already fixed
in step 2.

---

## 4. Why "106 of 110 tasks" overstates readiness

The task count measures delivered work, not working product. Three specific
reasons it is misleading here:

1. **A task's acceptance was scoped to its own package.** "Media authorization
   through the attached post" passed 9 tests and was true. It did not mean media
   was reachable end to end — that needed the transport, the identity, and the
   projection, none of which the package owned.
2. **Fixtures supplied what production must supply.** The projection mirror in
   the visibility fixtures, the mirrored profiles in the discovery fixtures, and
   the pre-seeded identities in the intro fixtures each hid a seam.
3. **The four remaining tasks are the integration itself.** T-951..T-956 are not
   cleanup; they are the work of making the composition real, and they are the
   hardest part.

---

## 5. What must be true to call this complete

Ordered by dependency. Each item is a gate step returning PASS.

| # | Condition | Depends on |
| --- | --- | --- |
| 1 | The projection worker reaches the profile service over TLS with the Concert identity, and a round writes rows | B-1 |
| 2 | Intro detail returns 200 for an authorized viewer and 404 for an outsider | 1 |
| 3 | A like returns 200, the author's liker list contains the liker, and the notification exists | 1 |
| 4 | The intro delete succeeds and leaves the room, avatar, and posts byte-identical | B-2 |
| 5 | The avatar read returns 200 | B-3 |
| 6 | The room visibility change succeeds | B-4 |
| 7 | The cross-surface sweep probes contract paths and returns eight identical denials for the outsider | B-6 |
| 8 | The lifecycle sweep's durable half parses and passes | B-5, then 4 |

Not required for the gate, still required for release:

| Item | State |
| --- | --- |
| Browser captures at 360/390/430 | Not run — no browser session against a live stack |
| Live NAVER authorization | Blocked — no console-issued application in this environment |
| Discovery against a populated projection | Blocked on 1 |
| Performance | Deferred by the earlier MCR decision |

---

## 6. The plan

**Wave 1 — unblock the projection (B-1).** One owner, one change: give the
worker the TLS material and service identity the other edges already use. This
is the highest-leverage fix in the project because three gate steps depend on
it. Verify by asserting a row appears in `discovery_profiles` in a running
stack, not by asserting the worker started.

**Wave 2 — the three small defects (B-2, B-3, B-4).** Independent of each other
and of wave 1. Each is a diagnosis plus a small fix. They can run in parallel
with wave 1 on disjoint files.

**Wave 3 — the gate's own defects (B-5, B-6).** Fix the sweep paths and the
durable probe. These are gate corrections, not product changes, and they must
not weaken an assertion.

**Wave 4 — re-run the gate unchanged.** All five steps PASS, both sweeps PASS.
Only then is the integration complete.

**Wave 5 — browser captures.** The one acceptance item that needs a real
browser.

---

## 7. What this analysis changes about the process

The gate found six defects that 106 merged tasks and roughly 1,400 tests did
not. The lesson is not that the tests were wrong — they were each honest about
their scope — but that **a package-level green is not evidence of a working
product**, and the composition must be exercised continuously rather than at the
end.

Concretely, for the remainder of this work:

1. The gate runs after every merge, not at milestones.
2. A fix is not accepted on a unit test alone when the defect was found by the
   gate; it is accepted when the gate step moves.
3. Fixtures that supply what production supplies are labelled as such, so the
   next reader knows which seams they hide.
