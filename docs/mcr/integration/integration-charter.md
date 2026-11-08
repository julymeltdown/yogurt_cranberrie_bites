# Integration charter

Status: **PLANNING_COMPLETE / IMPLEMENTATION_NOT_STARTED**.

## 1. Authority and routing

| Question | Answer |
| --- | --- |
| Who decides? | The user. No king, scrum lead, or reviewer gate exists for this integration. The user's 2026-11-04 direction removed manager review and approval stages; implementation owners validate their own code, queries, tests, runtime evidence and fixes. |
| Where does code land? | `julymeltdown/yogurt_cranberrie_bites`, branch `master`. |
| Where does the foundation come from? | `julymeltdown/mcr`, branch `master`, commit `a2b10bf92d716bdb5d0ca85b481de6265b466bf7`. |
| Where does reused social logic come from? | `julymeltdown/bigyeting`, pinned at `dd47ed502edd5d4f9bc4b726e9088b1ee0f0547b` for the documents in this set; the local audit checkout is `5eecdcaffd1a9d505b9ab2a974f8d5623217e90a`. See [source-register.md](source-register.md) for the reconciliation rule. |
| Is MCR still a delivery target? | No. MCR `master` stays as published history. New work goes to the yogurt repository. Do not push integration work to MCR. |

The MCR working agreement in [AGENTS.md](../../../AGENTS.md) still governs
language, evidence truthfulness, frontend/backend separation, worktree isolation,
and the prohibition on decorative left accent bars. Where it says "delivery
repository is `julymeltdown/mcr`", this charter supersedes that single sentence
for the integration: the delivery repository is
`julymeltdown/yogurt_cranberrie_bites`. Every other clause remains in force.

## 2. Language and naming

- All chat, commits, documentation, code identifiers, UI copy, and test names are
  English.
- The Korean term 트친소 is a user-facing concept name. In code and documents it is
  `mutuals-intro post` (entity `intro_post`). Do not introduce a second name.
- The repository name is `yogurt_cranberrie_bites`; the product name in planning
  text is "Yogurt Cranberrie Bites". Neither appears in product UI.

## 3. Baselines

| Role | Repository | Revision |
| --- | --- | --- |
| Integration foundation | `julymeltdown/mcr` | `a2b10bf92d716bdb5d0ca85b481de6265b466bf7` |
| Seed commit on delivery | `julymeltdown/yogurt_cranberrie_bites` | `03b673224aaa77842d763f817a660eb2d6f634d6` (empty commit) |
| Social logic donor | `julymeltdown/bigyeting` | `dd47ed502edd5d4f9bc4b726e9088b1ee0f0547b` (documented), `5eecdcaffd1a9d505b9ab2a974f8d5623217e90a` (audited checkout) |
| Rust service foundation | `julymeltdown/port-rust` | `704f42149d5e8e35e1d578676d8ae2dac4f0f9c2` (already imported into MCR) |

Reused source is never copied wholesale. Each reused symbol is named in
[migration-plan.md](migration-plan.md), rewritten to the destination's identity
model, and covered by a destination test. The C08 guard
(`scripts/mcr/check-ui-origin.mjs`) rejects exact unreviewed copies of Bigyeting
frontend files and rejects Bigyeting-only rendering dependencies.

## 4. What the foundation already provides

Verified by reading the seed tree, not assumed:

| Capability | Where it lives today | Integration consequence |
| --- | --- | --- |
| React 19 + Vite + Phaser 3.90 frontend, one router, one shell | `frontend/src/app/`, `frontend/src/world/` | Add profile/social/room screens into the existing router and shell. Do not mount a second root. |
| Six-service Rust backend (auth, profile, friend, post, notification, gateway) plus a concert service | `backend/services/` | New social/profile/room data belongs to the concert service and the profile service, not to a new service. |
| Canonical identity: one Auth UUID, session context version, CSRF, browser-origin checks | `backend/services/api-gateway/src/adapters/in/http/` | Reuse the existing session/CSRF/context validation. Do not add a second login. |
| Mutuals-intro post entity with paired age bounds (no gender) | `contracts/domain/v1/social.schema.json`, `concert-service/migrations/0049_intro_native_witness.sql`, `concert-domain/src/intro_policy/` | Extend with gender + audience summary; keep the existing paired-bounds and fail-closed semantics. |
| Age verification claims, canonical claim ordering, Seoul-date birthday arithmetic | `concert-domain/src/age_policy/`, `concert-application/src/intro_policy/` | Reuse the arithmetic unchanged. The claim store itself moves to the profile service and gains the NAVER gender bucket; see §6.1. |
| Media upload with normalization, dimension and pixel limits, owner-scoped read | `concert-application/src/media/`, `concert-service/src/media_runtime/`, `contracts/domain/v1/operations.json` media operations | Reuse for profile photo/artwork posts and mutuals-intro attachments. |
| Feed posts with `media_ids`, comments, reactions | `post-service/` (`post_concert_posts`, `post_concert_comments`) | Reference implementation for independent profile posts; do not overload event feed semantics. |
| Notifications with deduplicated delivery | `notification-service/`, `concert-service/src/notification_live/` | Add an `INTRO_LIKED` kind through the existing pipeline. |
| Avatar persistence keyed by `(event_id, user_id)` | `backend/services/concert-service/src/adapters/out/repository/avatar.rs`, `concert-domain/src/avatar/mod.rs` | Must be re-keyed to profile ownership. This is a real migration, not a rename. |
| Pixel-art world rendering (Phaser scene, illustrated scenery, depth ordering) | `frontend/src/world/`, `frontend/src/entities/avatar/runtime/motion-adoption/` | Reuse rendering primitives for a personal room; do not reuse concert maps or murals. |
| Relationship blocks and moderation | `concert-service/src/relationship/`, `moderation` migrations | Apply to every new visibility decision. |

## 5. Scope boundary

### In scope for the integrated release

1. Profile: nickname, public introduction, profile image, emo character, favorites
   across three categories (bands/artists excluding K-pop, anime, games).
2. Personal room belonging to a profile: fixed room, furniture placement, up to
   four pixel-art posters converted from user images, saved layout.
3. Independent mutuals-intro posts with heart likes, author-only liker list,
   notifications, comments and replies, and author-configured age/gender audience.
4. Independent profile photo/artwork posts with their own visibility.
5. People discovery by genuinely overlapping catalog items, separate from the
   mutuals-intro list.
6. Activity: likes received, comments, replies, follow notifications.
7. Blocking, reporting, content deletion, and account deletion for the new entities.

### Explicitly excluded from the integrated release

| Excluded | Source | Reason |
| --- | --- | --- |
| Daily matching, application/acceptance flows, external-account reveal approval, interest scores, event rewards | Bigyeting | The user's product plan excludes them; they contradict the heart-only interaction model. |
| Concert schedules, event participation gates, gifts and pickup, shared mural | MCR | Event-specific; the concert is over and the profile product must outlive it. |
| Direct messaging, real-time room visiting, voice chat | Both | Not in the initial release; adding them beside comments would fragment the first-conversation path. |
| Multiple rooms per user, furniture trading, automatic music recommendation, collaborative streaming | Product plan | Out of initial scope. |
| Video attachments | Bigyeting | Only still images are required; do not port the video pipeline. |
| A second authentication system, a second router root, a second Phaser instance | MCR rule | One canonical identity, one shell, one world connection. |

Excluding a feature means **not integrating it**, not deleting it from MCR's
history. MCR's 112-task register keeps its own status; this charter does not
change it.

## 6. Decisions

The user answered the three open questions on 2026-11-08. They are recorded in
§7 and implemented as follows.

### 6.1 How NAVER supplies age and gender (verified in the donor)

The claim is **not** entered by the user and it is **not** a free-form profile
field. Bigyeting obtains it from the NAVER Login profile API
(`https://openapi.naver.com/v1/nid/me`) during a dedicated verification flow:

| Step | Source (audited checkout `5eecdcaf`) |
| --- | --- |
| Authorization start | `POST /v1/naver-verification/start` → `begin_naver_verification` in `application/use_cases/oauth_start.rs` |
| Callback | `GET /v1/naver-verification/callback` → `application/use_cases/naver_verification.rs` |
| Profile fetch | `adapters/out/oauth_naver_provider.rs` `load_naver_profile` → `oauth_profiles.rs` `NaverProfile` |
| Raw fields | `id`, `gender` (`F`/`M`/`U`), `birthyear` (`YYYY`), `birthday` (`MM-DD`) |
| Age derivation | `domain/privacy.rs` `derive_exact_age_claim(birthyear, birthday, seoul_date)` → `age_at_verification` + `next_birthday_at` |
| Storage | `naver_verifications(member_id, gender_bucket, age_at_verification, next_birthday_at, verified_at, expires_at, status)` |
| Current age | `current_age.rs` `current_verified_age_sql_at`: `age_at_verification + (today >= next_birthday_at)` in the Seoul time zone |
| Read model | `repository/naver.rs` `naver_verification_response_at` → `{ status, verified_at, expires_at, claims: { age, gender_bucket } }` |

Consequences that the integration must preserve:

- Gender is a **verified provider attribute**, bucket `female | male | unknown`.
  It is not self-declared, and it must never be entered by the user.
- A missing or unrecognized `gender` produces `profile_incomplete` and writes no
  claim row. It does not default to `unknown`.
- Explicit provider `U` becomes `unknown`, and `unknown` passes only an audience
  whose author selected `unknown`.
- Claims expire (90-day maximum). An expired claim yields no attributes, so a
  restricted post becomes invisible again until the user re-verifies.
- The donor requires a supported claim for **every** post, including unrestricted
  ones (`exact_age_intro_policy_contract.rs`
  `unrestricted_intros_require_a_supported_naver_claim_across_every_surface`).
  The integration removes that requirement for unrestricted posts and keeps it
  for restricted ones — this is the change the user asked for, and it is recorded
  as D-03.

### 6.2 What the destination must change

| Donor behavior | Destination behavior | Reason |
| --- | --- | --- |
| NAVER claim required for every intro post | Required only when `minimum_age` is set or `gender != any` | User: restriction is for choosing who may see the post, not a global gate |
| Gender compared as `allowed_gender = 'any' OR allowed_gender = viewer.gender_bucket` | Same comparison, but `unknown` is a real audience value and a viewer with no claim never matches `female`/`male`/`unknown` | Fail-closed on unknown attributes |
| Age verification bound to `(user_id, event_id)` | Bound to `user_id` only, with the event column retained as an optional audit reference | A personal profile must outlive a concert |
| Claim tables owned by the concert service | Owned by the profile service, with the concert service reading through `IdentityClaimsPort` | Profile-owned data must not depend on an event |
| `age_at_verification` + `next_birthday_at` arithmetic | Reuse MCR's existing `age_policy` arithmetic unchanged | Already adapted and tested |

### 6.3 Remaining decisions

| ID | Question | Default | Blocks |
| --- | --- | --- | --- |
| D-04 | Room default visibility | `public`, owner toggle to `private` | WP-07 |
| D-05 | Profile photo post default visibility | `public`, owner toggle to `private` | WP-06 |
| D-06 | Do events remain? | Yes, optional grouping only; never required for profile, room, or intro features | — |
| D-07 | Is following in scope? | Yes, independent follow with no approval step | WP-09 |
| D-08 | Age range limits | Keep `14..=100` and `15..=101` as configuration, exposed as inclusive input | — |
| D-09 | Should a user be able to see and disconnect the NAVER claim, as the donor allows? | Yes, mirroring the donor's connect/disconnect/reconnect flow | WP-04 |

D-01, D-02, and D-03 are resolved by the user's 2026-11-08 answers.

## 7. Decision log

| Date | ID | Decision | Decided by |
| --- | --- | --- | --- |
| 2026-11-08 | — | Delivery repository is `yogurt_cranberrie_bites`; foundation is MCR `a2b10bf9`; seed is an empty commit on top of it. | User instruction |
| 2026-11-08 | — | All work is document-driven; documents precede code. | User instruction |
| 2026-11-08 | D-01 | Gender comes from the verified NAVER Login profile (`gender` field), not from a self-declared field. `IdentityClaimsPort` reads it from the claim store. | User instruction |
| 2026-11-08 | D-02 | Provider-verified gender is acceptable for access control. It is never presented as self-declared, and a user cannot edit it. | User instruction |
| 2026-11-08 | D-03 | Age/gender restrictions must exist. NAVER linking is required only for posts that carry an age or gender restriction, not for every post. | User instruction |
| 2026-11-08 | D-04 | Room visibility defaults to `public` with an owner toggle. | Lead, per user delegation ("you decide") |
| 2026-11-08 | D-05 | Profile photo posts default to `public` with an owner toggle. | Lead, per user delegation |
| 2026-11-08 | D-06 | Events remain an optional grouping; no profile-owned feature may require one. | Lead, per user delegation |
| 2026-11-08 | D-07 | Following is in scope as an independent relationship with no approval step. | Lead, per user delegation |
| 2026-11-08 | D-08 | Age bounds stay `14..=100` / `15..=101` as configuration, shown as inclusive input. | Lead, per user delegation |
| 2026-11-08 | D-09 | The user can connect, inspect, and disconnect their NAVER claim from their own profile settings. | Lead, per user delegation |
| 2026-11-08 | D-10 | The concert world leaves the primary navigation and stays reachable from a secondary menu. | Lead, per user delegation |

## 8. Delivery discipline

- One writer per file scope. Backend writers own `backend/`, frontend writers own
  `frontend/`, contract writers own `contracts/` and `backend/docs/api/`. A
  shared-file change (root router, lockfile, workspace manifest, contract
  registry) needs an explicit transfer recorded in the work package.
- Each work package lands as its own commit or commit series on `master`, with
  the verification evidence named in [verification-plan.md](verification-plan.md).
- Never force-push. Reconcile remote history normally.
- Status vocabulary is exactly `NOT_STARTED`, `IN_PROGRESS`, `CONTRACT_READY`,
  `INTEGRATED`, `ACCEPTED`, `BLOCKED`, `NOT_RUN`. A unit test never certifies a
  real multiuser flow.
- A document that describes intent is marked `PLANNING_COMPLETE`; only a document
  that cites executed commands with exit codes may claim a task is `ACCEPTED`.
