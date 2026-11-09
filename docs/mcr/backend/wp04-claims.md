# WP-04 — claims and the visibility decision

**Status:** `INTEGRATED`
**Branch:** `codex/yc-wp04-claims`
**Delivered commits:** `41ef11065` (core, merged as `3f49da8e2`), `3c11b14cd` (T-302), `4fec50b16` (T-309/310/307), `d7f551669` (T-308/311)
**Base at delivery:** `620ef5851`

## What this work package owns

WP-04 owns exactly one thing: **the decision about who may see an Introduction
post**, and the claim it depends on. Every surface that shows, hides, likes,
comments on, shares or serves an image for an intro post must call that decision
rather than re-deriving it.

The decision is written once, as a pure function, and once more as SQL — and the
two are proven identical on a shared case table compiled into both test binaries.
There is no third implementation.

| Task | Deliverable | Where |
| --- | --- | --- |
| T-301 | `IdentityClaimsPort`, `Attribute<T>`, redacting `Debug` | `crates/concert-application/src/identity_claims.rs` |
| T-302 | the claim adapter over the profile service | `services/concert-service/src/intro_posts/claims_adapter.rs` |
| T-303 | `IntroAudience`, `GenderCondition`, paired bounds | `crates/concert-domain/src/intro_audience.rs` |
| T-304 | `can_view_intro` — the twelve-row table | `crates/concert-application/src/intro_visibility.rs` |
| T-305 | the SQL predicate | `services/concert-service/src/intro_posts/visibility_sql.rs` |
| T-306 | page and detail queries that embed it | `services/concert-service/src/intro_posts/repository.rs` |
| T-307 | media authorized through the attached post | `services/concert-service/src/profile_media/resolver.rs` |
| T-308 | shared link resolution | `services/concert-service/src/intro_posts/links.rs` |
| T-309 | like create/remove under the real policy | `services/concert-service/src/intro_posts/likes.rs` |
| T-310 | the liker list | `services/concert-service/src/intro_posts/likes.rs` |
| T-311 | `INTRO_LIKED`, deduped | migration `0087` (WP-05), proven by `tests/intro_like_notifications.rs` |
| T-312 | evidence | `docs/mcr/integration/evidence/v01-*.json`, `v02-*.json`, `v03-*.json`, `v13-*.json` |

## The decision, in one place

`can_view_intro(post, viewer) -> Verdict` returns the decision **and the row of
the table that produced it**. That is not decoration: it is what makes "row 6
reads no claim" a checkable property rather than a comment, because the caller
can assert which row fired.

The SQL form (`visibility_sql::predicate`) is one boolean fragment over the
`intro_posts` alias with seven typed placeholders. It is composed into a `where`
clause, never applied after a query. Rows 1–5 are a conjunction; rows 6–12 are a
single `CASE`, so the unrestricted arm is visibly claim-free.

`tests/visibility_cases/mod.rs` is the shared case table. It is included by both
`crates/concert-application/tests/intro_visibility.rs` and
`services/concert-service/tests/intro_visibility_contract.rs`, so a case cannot be
added to one side and forgotten on the other.

## The claim port, and why it is shaped this way

The concert service owns the audience decision but not the claim store:
`identity_claims` lives in the profile service's database. T-302 adds a narrow
internal RPC (`profile/v1/identity_claim_reader.proto`) rather than a
cross-database join.

Three properties are structural:

1. **The subject is a profile, not an Auth UUID.** The caller names a
   `profile_id`; the profile service resolves it to an account in its own
   database. Neither side trusts the other's mapping.
2. **The evaluation instant is the caller's.** The profile service reports
   whether the claim is live *at the instant the decision uses*, so two clocks
   cannot disagree about an expiry.
3. **No attribute is rendered.** The reply carries the age at verification and
   the next birthday, never a current age and never a birth date. The caller
   computes the current age with the Seoul-date arithmetic it already imports.

The mapping is closed:

| Stored state | Attribute |
| --- | --- |
| live `VERIFIED` with an age | `Known(ExactAgeClaim)` |
| live `VERIFIED` with `female` / `male` | `Known(Gender)` |
| live `VERIFIED` with `unknown` | `Unspecified` |
| `REPLACED`, `REVOKED`, expired, unsupported | `Unavailable` |
| no row at all | `Unavailable` |
| transport error, timeout, malformed reply | `Unavailable` |

There is no `unwrap_or_default` in the adapter. A transport failure is "we do not
know", and the decision denies on unknown.

## The Visibility port

WP-05 defined a `Visibility` port so the intro producer would never re-derive the
decision, and shipped the fail-closed `DenyRestricted` as its only implementation.
WP-04 supplies the real one:

* `ComposedPolicy` resolves the audience and the block state, reads the claim
  **only when `needs_claims(audience)` says so**, and calls `can_view_intro`.
* The port is `async` because the claim read is I/O. A synchronous port could only
  have been implemented by never reading claims — which is the bug this task
  exists to fix.
* `DenyRestricted` remains the default. An unconfigured deployment denies every
  restricted post rather than admitting them.

`DenyRestricted` is still what a caller gets if it constructs a store without
`with_visibility`. That is deliberate: the safe default is never silently
upgraded.

## Limitations, recorded

1. ~~**Block state is per request, not per row.**~~ **Fixed.** The predicate's
   block term is now a correlated `not exists` over `discovery_blocks`, resolved
   through the local `discovery_profiles` mirror, so rows 4 and 5 are evaluated
   against the actual (viewer, author) pair. The old per-request boolean meant a
   viewer who had blocked anyone lost the whole page. The term is fail-closed in
   both directions: an active block either way denies, and a pair that cannot be
   resolved denies. The one exception is a **nil viewer** (an anonymous shared-link
   resolver), which no block can name; treating it as blocked would deny every
   anonymous read of an unrestricted post. See `predicate`/`block_term` in
   `intro_posts/visibility_sql.rs` and `tests/intro_blocks_per_row.rs`.
2. **The page query and the detail query resolve author standing differently.**
   The page evaluates it through `concert_world_account_deletion_fences`; the
   detail query uses the scope's `author_standing`. Both are concert-local.
3. **The room-poster branch of the media resolver** matches the layout JSON's
   poster list and is exercised only by construction, not by a test with a real
   room layout.
4. **The claim adapter's gRPC transport** is covered by the profile-service server
   compiling under its method policy and by unit tests over a fake port; this
   suite does not run an end-to-end RPC between the two services.
5. **The page tests share one fixture database** and are serialized by a mutex
   for that reason. They assert real query behaviour, not a private database.

## Verification

All commands and exit codes are recorded in the four evidence files. Summary:

| Command | Result |
| --- | --- |
| `cargo +1.88.0 check --locked --workspace` | 0 |
| `cargo test -p concert-domain --test intro_audience` | 10 passed |
| `cargo test -p concert-application --test identity_claims` | 9 passed |
| `cargo test -p concert-application --test intro_visibility` | 14 passed |
| `cargo test -p concert-service --test intro_visibility_contract` | 8 passed |
| `cargo test -p concert-service --test intro_posts_page` | 15 passed |
| `cargo test -p concert-service --test intro_cursor` | 7 passed |
| `cargo test -p concert-service --test claims_adapter` | 11 passed |
| `cargo test -p concert-service --test intro_likes_policy` | 7 passed |
| `cargo test -p concert-service --test intro_likers_policy` | 7 passed |
| `cargo test -p concert-service --test media_authorization_policy` | 9 passed |
| `cargo test -p concert-service --test intro_links` | 7 passed |
| `cargo test -p concert-service --test intro_blocks_per_row` | 9 passed |
| `cargo test -p concert-service --test intro_like_notifications` | 5 passed |
| `cargo test -p concert-service --test intro_likes` (WP-05, unchanged) | 9 passed |
| `corepack pnpm@11.7.0 run typecheck` | 0 |
| `python3 backend/scripts/mcr/wp02_migration_ledger.py` | PASS |

No migration was added by WP-04. The ledger reports the existing heads.
