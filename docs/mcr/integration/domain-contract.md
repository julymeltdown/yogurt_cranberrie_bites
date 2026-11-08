# Domain contract — identity, audience, visibility, and data model

Status: **CONTRACT_READY (proposed)**. Freezing this document is work package
WP-01. Until WP-01 is `ACCEPTED`, no feature code may depend on these names.

This document defines the semantics every implementation must agree on. The
transport shapes are in [http-contract.md](http-contract.md); the SQL is named
here but written by the owning backend work package.

## 1. Identity

### 1.1 Profile identity

- `profile_id` is the canonical internal profile identifier. It is a UUID.
- The MCR foundation has no `profiles` table. Today a "profile" is the canonical
  Auth user UUID plus the Profile service's self-profile record
  (`backend/services/profile-service/src/concert_self_profile/`). The integration
  therefore introduces `profiles(id UUID PRIMARY KEY, account_id UUID UNIQUE NOT
  NULL, ...)` in the **profile service**, where `account_id` is the canonical Auth
  user UUID.
- `profiles.id` is generated once at profile creation and never reused or
  rewritten. Unlinking an external account does not change it.
- Every new table in the concert service references `profile_id` and reads
  ownership through the profile service's owner API. No cross-database join.
- MCR's `user_id` (Auth UUID) and Bigyeting's `member_id`/`x_account_id` are
  **not** interchangeable. Do not align them by string shape. Bigyeting's
  `member_id` is a donor concept only; nothing in the destination stores it.

### 1.2 Session and mutation context

Reuse the existing gateway contract unchanged:

- Browser origin check plus `X-MCR-Intent` header before any mutation.
- Active canonical Auth session; the actor is never taken from the request body.
- `account_context_version` must match the session's version; a mismatch is
  `ACCOUNT_CONTEXT_STALE` and the client clears account-scoped caches.
- CSRF token required for mutations on browser sessions.
- Author-side ownership is verified server-side on every request, never inferred
  from a client-supplied `profile_id`.

### 1.3 IdentityClaimsPort

The audience decision needs attributes that the profile table does not own: age
and gender. Both come from the verified NAVER Login claim described below.
The MCR foundation has an event-scoped age claim store
(`concert_age_claims`, keyed by `user_id` and `event_id`) but no gender field
anywhere; the integration moves the claim store to the profile service and adds
the provider gender attribute to it.

```rust
// backend/crates/concert-application/src/identity_claims.rs (proposed)
pub trait IdentityClaimsPort: Send + Sync {
    /// Returns the attribute snapshot used for one audience decision.
    /// Implementations must fail closed: any unavailable source yields
    /// `Attribute::Unavailable`, never a default value.
    fn read(
        &self,
        profile: ProfileId,
        now: DateTime<Utc>,
    ) -> impl Future<Output = Result<IdentityClaims, ClaimsError>> + Send;
}

pub struct IdentityClaims {
    pub profile: ProfileId,
    pub age: Attribute<ExactAge>,      // years, Seoul-date arithmetic
    pub gender: Attribute<Gender>,
    pub source: ClaimSource,           // provider + revision + verified_at + expires_at
    pub evaluated_at: DateTime<Utc>,
}

pub enum Attribute<T> {
    Known(T),
    Unspecified,   // the provider returned an explicit "not specified" value
    Unavailable,   // no claim, expired claim, revoked claim, or source failure
}

pub enum Gender { Female, Male }

pub struct ClaimSource {
    pub verifier: String,              // e.g. "naver"
    pub source_revision: String,
    pub verified_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

pub enum ClaimsError { Unavailable }   // never carries the attribute value
```

Rules:

- **Age and gender both come from the verified NAVER Login claim.** The donor
  flow is the authority: `GET https://openapi.naver.com/v1/nid/me` returns
  `gender` (`F`/`M`/`U`), `birthyear`, and `birthday`; `derive_exact_age_claim`
  turns the date of birth into `age_at_verification` plus `next_birthday_at`; the
  current age is `age_at_verification + (seoul_today >= next_birthday_at)`.
  Reuse MCR's already-adapted arithmetic in
  `backend/crates/concert-domain/src/age_policy/arithmetic.rs` unchanged.
- Gender is a **verified provider attribute**, never a user-entered field. The
  profile UI shows it read-only with its verification date and expiry, and offers
  connect / disconnect, mirroring the donor's `NaverAccountStatus` and
  `DELETE /v1/naver-verification`.
- Missing or unrecognized provider gender is `profile_incomplete`: no claim row
  is written, and the user stays `Attribute::Unavailable`. It must never be
  coerced to `unknown`.
- Explicit provider `U` maps to `Attribute::Unspecified` (the donor's
  `gender_bucket = 'unknown'`). `Unspecified` matches only an audience whose
  author selected `unknown`.
- Every status other than a live `Verified` claim maps to
  `Attribute::Unavailable`, never to a numeric age or a guessed gender.
- The port never returns raw claim rows, provider tokens, or exact age to a
  viewer-facing DTO. Only `can_view` results and the author's own summary leave
  the server. The owner may read their own derived age and gender bucket, exactly
  as the donor's `NaverAccountStatus` does.
- The claim store moves from the concert service to the profile service and is
  keyed by `user_id` only. The concert service reads it through this port. The
  donor's `naver_verifications` table shape is reused (§4.3).

### 1.4 Claim lifecycle

| Event | Effect |
| --- | --- |
| User connects NAVER | Provider callback writes a claim with `verified_at` and a 90-day maximum `expires_at` |
| User disconnects | Claim becomes `REVOKED`; every restricted post the user authored keeps its audience but this user's own attributes become unavailable |
| Claim expires | `Attribute::Unavailable`; the user is offered re-verification |
| User re-verifies | A new claim supersedes the old one; the old row's `expires_at` is capped at the replacement instant |
| Account deletion | Claims are deleted with the account |

A user without a claim can still use the whole product: profile, photos, room,
favorites, and unrestricted intro posts. They simply cannot view, or be matched
by, a post that carries an age or gender restriction. The UI must say this
plainly rather than showing an empty screen.

## 2. IntroAudience

### 2.1 Value

```rust
pub struct IntroAudience {
    pub gender: GenderCondition,
    pub minimum_age: Option<ExactAge>,          // inclusive, 14..=100
    pub maximum_age_exclusive: Option<MaximumAgeExclusive>, // 15..=101
}
```

- Both bounds are present or both are absent. `minimum_age < maximum_age_exclusive`
  is a hard invariant.
- The wire keeps the half-open upper bound. The interface shows an inclusive
  upper bound and converts: "Ages 18–29" → `minimum_age: 18,
  maximum_age_exclusive: 30`.
- The 14–100 / 15–101 window is configuration (charter D-08), not policy. It must
  not be presented as the final global age policy.

### 2.2 GenderCondition

```rust
pub enum GenderCondition { Any, Female, Male, Unknown }
```

| Value | Meaning for the author | Effect on a viewer |
| --- | --- | --- |
| `any` | No gender restriction | Anyone whose other conditions pass |
| `female` | Only women may view | Viewer must have `gender = Known(Female)` from a live claim |
| `male` | Only men may view | Viewer must have `gender = Known(Male)` from a live claim |
| `unknown` | Only viewers whose provider reported an explicit "not specified" | Viewer must have `gender = Unspecified` |

- `unknown` is **not** "any". It maps to the donor's `gender_bucket = 'unknown'`,
  which only exists when the provider explicitly returned `U`. It must never be
  treated as a wildcard.
- `Unavailable` (no claim, expired claim, provider failure, unrecognized value)
  never satisfies `female`, `male`, or `unknown`. Only `any` ignores gender.
- The author's UI offers `any`, `female`, `male`, and `unknown`. It does not offer
  a "verified only" toggle: a restricted audience already implies verification,
  because an unverified viewer can never satisfy it.

### 2.3 Audience summary

The author sees a human summary: `Women · Ages 18–29`, `Anyone`,
`Gender not specified · Ages 18+`. The summary is derived from the value on the
server and returned only on the owner projection. Viewer-facing DTOs never carry
the numeric bounds.

## 3. Visibility decision

### 3.1 Pure decision function

```rust
pub fn can_view_intro(
    post: &IntroPostState,      // state, audience, author, timestamps
    viewer: &ViewerContext,     // profile, relationship, claims, now
) -> Decision
```

`Decision` is `Allowed` or `Denied(reason)` where reason is one of
`NotPublished`, `AuthorBlocked`, `ViewerBlocked`, `AuthorInactive`,
`AuthorRestricted`, `AudienceAge`, `AudienceGender`, `AttributeUnavailable`.
Reasons are server-internal; the HTTP response is a uniform
`RESOURCE_UNAVAILABLE` for every denial except the author's own management path.

### 3.2 Decision table

Evaluated in this order; the first failure denies.

| # | Condition | Result |
| --- | --- | --- |
| 1 | Viewer is the author | `Allowed` (management/preview path) |
| 2 | Post state is not `PUBLISHED` | Deny `NotPublished` |
| 3 | Post is expired (`expires_at <= now`) | Deny `NotPublished` |
| 4 | Author account is not active, or author is under an active sanction | Deny `AuthorInactive`/`AuthorRestricted` |
| 5 | A block exists in either direction between viewer and author | Deny |
| 6 | `audience.minimum_age` is `None` and `audience.gender == Any` | `Allowed` |
| 7 | Age bound present and `viewer.age != Known` | Deny `AttributeUnavailable` |
| 8 | Age bound present and `!(min <= age < max)` | Deny `AudienceAge` |
| 9 | `gender == Any` | `Allowed` |
| 10 | `gender == Female/Male` and `viewer.gender != Known(matching)` | Deny `AudienceGender` |
| 11 | `gender == Unknown` and `viewer.gender != Unspecified` | Deny `AudienceGender` |
| 12 | Otherwise | `Allowed` |

Notes that the implementation must not soften:

- A viewer whose attributes cannot be evaluated never bypasses a restriction.
  Row 7 is the whole point: unknown age fails closed.
- An unrestricted post (row 6) requires no identity claim at all. This is the
  change introduced by this redesign: the global NAVER-linking requirement is
  removed for posts without age or gender restrictions. The donor required a
  claim for every post; that requirement is narrowed, not deleted.
- A restricted post always requires a live claim from the viewer. There is no
  path where a missing claim is treated as satisfying a restriction.
- The author's own path bypasses rows 2–12 but still requires an active session
  and ownership.

### 3.3 Surfaces that must apply the same decision

Every one of these is a separate enforcement point and each has its own test:

| Surface | Enforcement |
| --- | --- |
| Intro list (discovery) | SQL predicate inside the page query; never post-filter a page |
| Intro detail | Same predicate re-evaluated on the single row |
| Attachment list and thumbnail | Join to the post's audience predicate |
| Media content bytes | Re-evaluate on every request; never cache across users |
| Shared link / preview resolution | Same predicate; no restricted bytes in the preview |
| Like creation | Same predicate before insert |
| Like removal | Allowed even after access is lost, but the response carries no count |
| Comment creation and listing | Same predicate |
| Notification target resolution | Re-check on open; a notification grants nothing |
| Profile-level intro list | Same predicate per row |

### 3.4 Caching

- Restricted responses are per-user and `no-store`. They must not enter a shared
  cache.
- The client invalidates discovery, intro, media, and liker queries when the
  author changes the audience or when the account context version changes.
- Changing a policy affects subsequent requests only. Already-delivered
  photographs and user-saved copies cannot be recalled; the design must say so
  rather than imply recall.

## 4. Data model

All new tables are additive migrations. Do not edit applied migrations. New
concert-service migrations start at `0080_`; verify the highest existing number
before authoring.

| Table | Columns (minimum) | Invariants |
| --- | --- | --- |
| `profiles` (profile service) | `id`, `account_id`, `nickname`, `bio`, `profile_image_media_id`, `visibility`, `created_at`, `updated_at` | `account_id` unique; `id` immutable; nickname 1–40 chars |
| `profile_favorites` (profile service) | `profile_id`, `category`, `catalog_item_id`, `position` | `category IN ('music','anime','game')`; `UNIQUE(profile_id, category, catalog_item_id)`; at most 5 per category; music excludes K-pop by catalog policy |
| `identity_claims` (profile service) | `claim_id`, `user_id`, `verifier`, `source_revision`, `subject_hash`, `gender_bucket`, `age_at_verification`, `next_birthday_at`, `verified_at`, `expires_at`, `status` | See §4.3. Keyed by `user_id` only |
| `identity_verifications` (profile service) | `verification_id`, `user_id`, `verifier`, `state`, `state_digest`, `created_at`, `expires_at`, `completed_at` | One-use OAuth state, ≤10 minute expiry |
| `profile_rooms` (concert service) | `owner_profile_id UNIQUE`, `layout jsonb`, `visibility`, `revision`, `updated_at` | No `intro_id`, no `event_id`; revision monotonic |
| `profile_avatars` (concert service) | `owner_profile_id UNIQUE`, `config jsonb`, `revision`, `updated_at` | No participant key; validated by the existing avatar catalog |
| `intro_posts` | `intro_id`, `author_profile_id`, `body`, `favorites_snapshot jsonb`, `audience_gender`, `audience_min_age`, `audience_max_age_exclusive`, `state`, `revision`, `published_at`, `expires_at` | No `room_id`; bounds paired; `state IN ('DRAFT','PUBLISHED','DELETED')` |
| `intro_attachments` | `intro_id`, `media_id`, `position` | `UNIQUE(intro_id, media_id)`; max 4; media must be owned by the author |
| `intro_likes` | `intro_id`, `actor_profile_id`, `created_at` | `PRIMARY KEY(intro_id, actor_profile_id)`; no reward trigger; no self-like |
| `intro_comments` | `comment_id`, `intro_id`, `author_profile_id`, `parent_comment_id`, `body`, `state`, `created_at` | Visible only under the parent post's audience |
| `profile_posts` | `post_id`, `author_profile_id`, `description`, `visibility`, `state`, `created_at`, `deleted_at` | No `intro_id`; independent visibility |
| `profile_post_attachments` | `post_id`, `media_id`, `position` | Same ownership rule as intro attachments |
| `media_assets` | `media_id`, `owner_profile_id`, `purpose`, `state`, `width`, `height`, `bytes`, `checksum` | Purpose distinguishes `PROFILE_POST`, `INTRO_POST`, `PROFILE_IMAGE`, `ROOM_POSTER` |
| `notifications` (existing service) | add kind `INTRO_LIKED` | Dedupe on `(recipient, kind, intro_id, actor_profile_id)` |

### 4.3 Identity claim store

The donor's `naver_verifications` table is the model. The destination renames it
to a provider-neutral `identity_claims` and removes the event dependency.

```sql
create table identity_claims (
  claim_id            uuid primary key,
  user_id             uuid not null,
  verifier            text not null check (verifier ~ '^[a-z][a-z0-9._-]{0,63}$'),
  source_revision     text not null check (source_revision ~ '^[A-Za-z0-9._-]{1,100}$'),
  subject_hash        bytea not null check (octet_length(subject_hash) = 32),
  gender_bucket       text not null check (gender_bucket in ('female','male','unknown')),
  age_at_verification smallint check (age_at_verification between 14 and 100),
  next_birthday_at    date,
  verified_at         timestamptz not null,
  expires_at          timestamptz not null,
  status              text not null check (status in ('VERIFIED','REPLACED','REVOKED')),
  constraint identity_claims_verified_requires_attributes
    check (status <> 'VERIFIED'
           or (age_at_verification is not null and next_birthday_at is not null)),
  constraint identity_claims_lifetime
    check (expires_at > verified_at and expires_at <= verified_at + interval '90 days'),
  constraint identity_claims_next_birthday_future
    check (next_birthday_at is null
           or next_birthday_at > (verified_at at time zone 'Asia/Seoul')::date)
);
create unique index identity_claims_one_verified
  on identity_claims (user_id) where status = 'VERIFIED';
create index identity_claims_canonical
  on identity_claims (user_id, verified_at desc, expires_at desc, claim_id desc);
```

- `subject_hash` is a salted hash of the provider subject, so the same NAVER
  account cannot back two users. Reuse the donor's `stable_hash` approach.
- The canonical row is selected exactly as the donor does: newest `verified_at`,
  then newest `expires_at`, then newest `claim_id`. Never fall back to an older
  usable claim.
- Current age is computed in the Seoul time zone from `age_at_verification` and
  `next_birthday_at`. There is no stored "current age" column: it would go stale
  between birthdays.
- `event_id` is deliberately absent. The donor's `concert_age_claims` table is
  event-scoped and its event cleanup can erase a user's verification when a
  concert ends; a personal profile must not inherit that.

### 4.4 Deletion semantics

| Delete | Also removes | Must leave intact |
| --- | --- | --- |
| Intro post | Its likes, comments, attachments (association only) | Room, avatar, profile posts, favorites |
| Profile post | Its attachments (association only) | Room, intro posts |
| Media asset | Nothing until reference count reaches zero | — |
| Profile / account | All owned rows, in the existing account-deletion saga order | — |

Before physically deleting a shared media file, verify no remaining reference.

### 4.5 Revision and concurrency

- `intro_posts.revision`, `profile_rooms.revision`, `profile_avatars.revision`,
  `profile_posts` use the existing MCR revision discipline: a mutation carries
  `expected_revision`, is checked inside the command transaction, and returns
  `409 REVISION_CONFLICT` with the current revision on mismatch.
- Room layout saves are the highest-conflict operation (drag, save, drag, save).
  The client must keep the unsaved layout on conflict and offer an explicit
  resolution; it must not silently overwrite.

## 5. Room layout value

```rust
pub struct RoomLayout {
    pub version: 1,
    pub wall: WallStyleId,
    pub floor: FloorStyleId,
    pub furniture: Vec<FurniturePlacement>,   // grid-aligned
    pub posters: Vec<PosterPlacement>,        // max 4
    pub character: CharacterPlacement,
}

pub struct FurniturePlacement { pub item: FurnitureId, pub cell_x: u16, pub cell_y: u16, pub layer: u8 }
pub struct PosterPlacement { pub poster_media_id: MediaId, pub slot: u8, pub x: u16, pub y: u16, pub width: u16, pub height: u16 }
```

- The room grid is fixed. Coordinates outside the grid, overlapping furniture in
  the same layer, more than four posters, and unknown style IDs are rejected
  server-side.
- The server validates the layout structurally and by bounds. It does not try to
  judge aesthetics.
- Poster images are stored as produced by the browser conversion (PNG, nearest
  neighbor). The server validates size and dimensions and re-encodes only for
  safety, never re-pixelates.

## 6. Poster conversion contract

Input: a user image (JPEG/PNG/WebP), cropped to the poster frame.
Output: a PNG at the poster's native pixel size.

| Preset | Target pixel size | Character |
| --- | --- | --- |
| Fine pixels | 64 × 64 | More detail, still visibly pixel art |
| Chunky pixels | 32 × 32 | Bold, poster-like |

- Conversion is deterministic: same input, crop, and preset produce identical
  bytes.
- The default preset must be immediately usable; the user is never asked about
  color count, dithering, or layers.
- Faces, logos, and album art must stay recognizable at mobile display size. The
  verification case renders a face and a logo at 360 px and inspects them.
- Conversion runs in the browser. The server never receives the source photo for
  poster creation.

## 7. What this contract deliberately does not include

- No compatibility score, closeness score, or ranking weight.
- No application, approval, or pending-request state on likes.
- No contact or external-account disclosure tied to a like.
- No room reference inside an intro post.
- No pixel conversion for ordinary photographs.
- No event or concert dependency in any profile-owned table.
