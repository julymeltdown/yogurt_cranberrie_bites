# Verification plan — mandatory cases and evidence format

Status: **PLANNING_COMPLETE / IMPLEMENTATION_NOT_STARTED**. Every verification
below is `NOT_RUN`.

Machine-readable companion: [verification-matrix.json](verification-matrix.json).

## 1. Evidence rules

- A verification is `PASS` only with: the exact command, its exit code, the
  output path under `docs/mcr/integration/evidence/`, the commit tested, and the
  timestamp.
- A unit test never certifies a real multiuser flow. The end-to-end scenario
  (V-11) requires real services and three distinct identities.
- A failed or unexecuted case is recorded as `FAIL` or `NOT_RUN`. Never
  paraphrase a failure into a pass.
- Screenshots are captured at 360, 390, and 430 px width for any UI case.
- A verification that needs a decision recorded as `OPEN` is `BLOCKED` with that
  decision named.

## 2. Mandatory verifications

### V-01 — Age boundary

**Claim**: With audience "women, ages 18–29", a viewer aged 18 and a viewer aged 29
are allowed; a viewer aged 17 and a viewer aged 30 are denied.

**Method**: Domain-level case table plus SQL-level case table, same inputs.
Birthday changes and claim-validity changes are evaluated against the same server
timestamp.

| Case | Age | Expected |
| --- | --- | --- |
| V-01-a | 17 | Denied |
| V-01-b | 18 | Allowed |
| V-01-c | 29 | Allowed |
| V-01-d | 30 | Denied |
| V-01-e | birthday is today | Age increments on the Seoul date, not UTC |
| V-01-f | claim expires at `now` | Denied (expiry is inclusive-exclusive: `expires_at <= now` fails) |
| V-01-g | claim expires at `now + 1µs` | Allowed |

**Owner**: WP-04. **Test paths**:
`backend/crates/concert-domain/tests/intro_audience.rs`,
`backend/services/concert-service/tests/intro_visibility_contract.rs`.

### V-02 — Unknown attributes fail closed

**Claim**: If age or gender cannot be determined, the viewer never bypasses a
restriction. `allowed_gender = unknown` does not imply an unverified age is
acceptable, and an explicit provider `unknown` is not the same as "no claim".

| Case | Viewer state | Audience | Expected |
| --- | --- | --- | --- |
| V-02-a | no claim at all | age 18–29, gender any | Denied |
| V-02-b | claim exists but expired | age 18–29, gender any | Denied |
| V-02-c | claim exists, provider gender `U` (bucket `unknown`), age 25 | gender `unknown` | Allowed |
| V-02-d | provider `unknown`, no age bound | gender `unknown` | Allowed |
| V-02-e | provider `unknown`, age 25 | gender `unknown`, age 18–29 | Allowed |
| V-02-f | provider `unknown`, age 25 | gender `female` | Denied |
| V-02-g | claim exists, age 25, gender `female` | gender `female` | Allowed |
| V-02-h | claim exists, age 25, gender `male` | gender `female` | Denied |
| V-02-i | no claim | `any`, no bounds | Allowed without any claim read |
| V-02-j | claim expired, was `female` | gender `female` | Denied |
| V-02-k | claim revoked by the user | gender `female` | Denied |

**Owner**: WP-04.

### V-12 — NAVER verification produces a real claim

**Claim**: Age and gender come from the verified provider profile, not from a
form, and every failure mode leaves the user unverified rather than guessing.

| Case | Input | Expected |
| --- | --- | --- |
| V-12-a | `birthyear=2000`, `birthday=07-12`, Seoul date 2026-07-12 | `age_at_verification=26`, `next_birthday_at=2027-07-12` |
| V-12-b | Same claim read on 2027-07-12 | current age 27, no stored mutation |
| V-12-c | `gender=F` | bucket `female` |
| V-12-d | `gender=M` | bucket `male` |
| V-12-e | `gender=U` | bucket `unknown` |
| V-12-f | `gender` missing | verification fails `profile_incomplete`, no claim row written |
| V-12-g | `gender=X` (unrecognized) | verification fails `profile_incomplete`, no claim row written |
| V-12-h | `birthyear` missing | verification fails, no claim row written |
| V-12-i | Derived age outside 14–100 | verification fails `age_not_supported`, no claim row written |
| V-12-j | Same provider subject already backs another user | verification fails as duplicate, no second claim row |
| V-12-k | Re-verify after expiry | new claim supersedes; old row capped at the replacement instant |
| V-12-l | `DELETE /api/me/identity/naver` | status becomes `REVOKED`; restricted posts authored by this user keep their audience; this user's own viewing of others' restricted posts is denied |
| V-12-m | Callback state reused | second use rejected; no claim written |
| V-12-n | Callback for an expired state (>10 min) | rejected; no claim written |
| V-12-o | Any of the above | no provider token, subject, birth date, or exact age appears in logs or persisted state outside the claim row |
| V-12-p | Production config with placeholder client id/secret | service refuses to start | 

**Owner**: WP-04b.

### V-13 — Unrestricted posts need no verification

**Claim**: The donor's global gate is removed. A user with no NAVER claim can
publish and view an unrestricted intro post.

| Case | Expected |
| --- | --- |
| V-13-a | Unverified user publishes an intro post with `gender=any`, no age bounds | succeeds |
| V-13-b | Unverified user views another user's unrestricted post | allowed |
| V-13-c | Unverified user views a restricted post | `404 RESOURCE_UNAVAILABLE`, and the UI offers verification without revealing content |
| V-13-d | Unverified user likes an unrestricted post | succeeds |
| V-13-e | Unverified user likes a restricted post | `404` |

**Owner**: WP-04 + WP-05.

### V-03 — One decision across every surface

**Claim**: List, search, detail, attachment, thumbnail, shared link, and like
creation produce identical decisions for the same (viewer, post) pair, and a
changed audience is re-evaluated on the next request.

| Case | Surface | Expected |
| --- | --- | --- |
| V-03-a | list page | Post absent from the page; page size may be short |
| V-03-b | detail | `404 RESOURCE_UNAVAILABLE` |
| V-03-c | attachment list | `404` |
| V-03-d | media content bytes | `404` |
| V-03-e | shared link resolution | `404`, no preview bytes |
| V-03-f | like creation | `404` |
| V-03-g | author widens the audience, viewer re-requests | Post now visible |
| V-03-h | author narrows the audience after the viewer loaded the card, viewer re-requests | Post now `404` |
| V-03-i | pagination | Server filters inside the query; a page is never post-filtered to fewer than `limit` while reporting `next_cursor` from the unfiltered set |

**Owner**: WP-04 + WP-05. **Note**: V-03-i is the specific defect the product
plan calls out ("do not paginate on the server and then filter the returned page
on the client").

### V-04 — Like removal without disclosure

**Claim**: A user who has lost access can still remove their own like, and the
response reveals neither the current like count nor any restricted content.

| Case | Expected |
| --- | --- |
| V-04-a | `DELETE /api/intro-posts/{id}/like` by a viewer who no longer satisfies the audience → `200 {"liked": false, "like_count": null}` |
| V-04-b | The like row is actually removed |
| V-04-c | The response body contains no body text, no attachment IDs, no author data |
| V-04-d | `PUT` by the same viewer → `404 RESOURCE_UNAVAILABLE` |

**Owner**: WP-05.

### V-05 — Idempotent likes, one notification, no side effects

| Case | Expected |
| --- | --- |
| V-05-a | Two `PUT` requests from the same viewer → one `intro_likes` row |
| V-05-b | Two `PUT` requests → exactly one `INTRO_LIKED` notification |
| V-05-c | No follow edge, no follow request, no application, no approval, no reward row is created |
| V-05-d | `DELETE` twice → second returns `{"liked": false, "like_count": <current>}` if still visible, or `like_count: null` if not |
| V-05-e | Self-like is rejected |
| V-05-f | Liker list is `403` for a non-author and `200` for the author |

**Owner**: WP-05.

### V-06 — Attachment isolation

**Claim**: Photographs attached to a restricted intro post never appear in the
profile gallery or in room posters, and nobody can attach another person's media.

| Case | Expected |
| --- | --- |
| V-06-a | Visitor's profile gallery does not contain the intro attachment |
| V-06-b | The author's own profile gallery does not contain it either (it was never published there) |
| V-06-c | `POST /api/me/posts` with a `media_id` owned by another profile → `400`/`404`, no post created |
| V-06-d | `POST /api/me/posts` with a `media_id` owned by the author but attached to a restricted intro post → rejected unless the media purpose is `PROFILE_POST` |
| V-06-e | `POST /api/me/room/posters` referencing an intro attachment → rejected |
| V-06-f | Reading the media by ID as an unauthorized viewer → `404` |

**Owner**: WP-06 + WP-07.

### V-07 — Ordinary posts are independent and unconverted

| Case | Expected |
| --- | --- |
| V-07-a | A profile with no intro post and an undecorated room can publish a photo post |
| V-07-b | The published image bytes are the normalized photograph, not a pixel conversion |
| V-07-c | No poster record is created as a side effect |
| V-07-d | The photo post appears in the profile gallery for an authorized visitor |
| V-07-e | Deleting the photo post leaves the room and intro posts intact |

**Owner**: WP-06.

### V-08 — Lifecycle independence

| Case | Expected |
| --- | --- |
| V-08-a | Delete an intro post → room layout, room revision, avatar, photo posts unchanged |
| V-08-b | Change intro audience → room and photo post visibility unchanged |
| V-08-c | Change room visibility → intro audience and photo post visibility unchanged |
| V-08-d | Change room visibility → the intro post is still visible to viewers who satisfy its audience |
| V-08-e | Publish an intro post without ever opening the room → succeeds |

**Owner**: WP-05 + WP-07.

### V-09 — Ownership and account switching

| Case | Expected |
| --- | --- |
| V-09-a | `PUT /api/me/room` from a session that does not own the room → `403`, no write |
| V-09-b | `PUT /api/me/avatar` cannot target another profile |
| V-09-c | Switching accounts clears drafts, pending uploads, avatar cache, and room draft |
| V-09-d | After switching back, the previous account's draft is not visible to the new account |
| V-09-e | An `account_context_version` mismatch returns `ACCOUNT_CONTEXT_STALE` and writes nothing |

**Owner**: WP-08 + WP-09.

### V-10 — Failure handling and resource cleanup

| Case | Expected |
| --- | --- |
| V-10-a | Photo upload cancelled mid-flight → no orphan post, staged media cleaned |
| V-10-b | Photo upload fails → the composer keeps the draft and offers retry |
| V-10-c | Two concurrent room saves with the same `expected_revision` → one succeeds, one gets `409` with the current layout |
| V-10-d | Leaving the room screen destroys the Phaser scene and releases textures |
| V-10-e | Re-entering the room restores the saved layout exactly |

**Owner**: WP-06 + WP-07.

### V-11 — End-to-end scenario

The scenario in [product-plan.md](product-plan.md) §9, executed with three real
identities against real services. B and C complete the NAVER verification with
test accounts that have the required provider attributes; A publishes the
restricted post.

1. A publishes a selfie and an artwork post; room stays default.
2. A publishes a restricted intro post (women, 18–29); B, verified as a woman
   aged 18–29, can view it and leaves a heart.
3. A opens the liker notification → B's profile → follows or comments.
4. A places a poster; the intro post and its like records are unchanged.
5. A deletes the intro post; room and photo posts intact.
6. C, either unverified or verified outside the audience, cannot see the post,
   its attachments, or its bytes.

**Owner**: WP-10.

## 3. Non-functional checks that still apply

| Check | Source | Applies to |
| --- | --- | --- |
| `scripts/mcr/check.sh guards` (boundaries, UI origin) | MCR AGENTS | Every package |
| `scripts/mcr/check.sh contracts` | MCR AGENTS | WP-01 and any contract change |
| Accessibility: keyboard reachable, focus visible, `aria-pressed` on the heart, labelled controls | MCR AGENTS | Every UI package |
| No decorative left accent bars in any new UI | MCR AGENTS | Every UI package |
| 360/390/430 px capture comparison | MCR AGENTS | Every UI package |
| No prohibited dependency added | C08 guard | Every frontend package |

## 4. Evidence file format

Each verification records one JSON file under
`docs/mcr/integration/evidence/`:

```json
{
  "verification": "V-04",
  "status": "PASS",
  "commit": "<sha>",
  "recorded_at": "2026-11-09T00:00:00Z",
  "commands": [
    { "command": "cargo test -p concert-service --test intro_likes_contract", "exit_code": 0 }
  ],
  "cases": [
    { "id": "V-04-a", "result": "PASS", "note": "200 with like_count null" }
  ],
  "artifacts": ["docs/mcr/integration/evidence/v04-liker-removal.txt"],
  "limitations": []
}
```

`status` is `PASS` only when every listed case is `PASS` and every command exited
0. Otherwise it is `FAIL`, `NOT_RUN`, or `BLOCKED` with `limitations` naming why.
