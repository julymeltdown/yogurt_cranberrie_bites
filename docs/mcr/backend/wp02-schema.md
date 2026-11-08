# WP-02 — schema and profile product transport

Status: **INTEGRATED**. Tasks T-101 … T-112 of
[task-breakdown.md §6](../integration/task-breakdown.md#6-wp-02--schema-tasks) are
implemented, tested against a real PostgreSQL 16 cluster, and committed on
`codex/yc-wp02-schema`.

## 1. What was built

| Task | Deliverable | Path |
| --- | --- | --- |
| T-101 | Profile product transport | `backend/proto/profile/v1/profile_product.proto`, `backend/services/profile-service/src/profile_product/`, `backend/services/profile-service/src/adapters/out/profile_owner_guard.rs` |
| T-102 | Gateway route and client | `backend/services/api-gateway/src/adapters/in/http/profile_product/`, `backend/services/api-gateway/src/adapters/out/profile_product_client.rs` |
| T-103 | `profiles` table and profile identity | `backend/services/profile-service/migrations/0011_profile_identity.sql` |
| T-104 | `profile_favorites` | `backend/services/profile-service/migrations/0012_profile_favorites.sql` |
| T-105 | `identity_claims`, `identity_verifications` | `backend/services/profile-service/migrations/0013_identity_claims.sql` |
| T-106 | `profile_media` | `backend/services/concert-service/migrations/0080_profile_media.sql` |
| T-107 | `profile_rooms` | `backend/services/concert-service/migrations/0081_profile_rooms.sql` |
| T-108 | `intro_posts` with audience columns | `backend/services/concert-service/migrations/0082_intro_posts.sql` |
| T-109 | `intro_attachments`, `intro_likes`, `intro_comments` | `backend/services/concert-service/migrations/0083_intro_children.sql` |
| T-110 | `profile_posts`, `profile_post_attachments` | `backend/services/concert-service/migrations/0084_profile_posts.sql` |
| T-111 | Account deletion coverage | `backend/services/concert-service/migrations/0085_profile_social_deletion.sql`, `backend/services/profile-service/migrations/0015_profile_account_deletion.sql` |
| T-112 | Migration ledger and head check | `backend/scripts/mcr/wp02_migration_ledger.py` |

Migration numbering: profile-service ends at **0015** (previous head `0010`),
concert-service ends at **0085** (previous head `0079`). Every new file is
strictly greater than the previous head, and no applied migration was edited.

## 2. Hard constraints and the test that proves each

| Constraint | Test |
| --- | --- |
| No `event_id` column in any profile-owned table | `profile_media_rejects_gif_and_has_no_event_column`, `intro_posts_have_no_room_or_event_column`, `identity_tables_have_no_event_column`, `a_room_has_no_intro_or_event_reference`, `profile_posts_have_no_intro_reference` — each queries `information_schema.columns` |
| `identity_claims` unique `(user_id)` where `status='VERIFIED'` | `a_second_verified_claim_for_one_user_is_refused` — asserts `identity_claims_one_verified` |
| `VERIFIED` requires `age_at_verification` and `next_birthday_at` | `a_verified_claim_requires_its_attributes` — asserts `identity_claims_verified_requires_attributes` |
| `expires_at <= verified_at + 90 days` | `claim_lifetime_is_capped_at_ninety_days` — asserts `identity_claims_lifetime` |
| `next_birthday_at > (verified_at AT TIME ZONE 'Asia/Seoul')::date` | `next_birthday_must_be_after_the_seoul_verification_date` — asserts `identity_claims_next_birthday_future`, including the UTC/Seoul date-skew case |
| `profile_media` purpose is exactly the four profile purposes | `profile_media_purpose_is_the_closed_profile_set` — `POST`, `GIVEAWAY`, `REPORT` each assert `profile_media_purpose` |
| `intro_posts` paired age bounds | `intro_audience_bounds_must_be_paired` — asserts `intro_posts_audience_bounds_paired` |
| `intro_posts` ordered, in-range bounds | `intro_audience_bounds_are_ordered_and_in_range` — asserts `intro_posts_audience_bounds` |
| `audience_gender IN ('any','female','male','unknown')` | `intro_audience_gender_and_state_are_closed` — asserts `intro_posts_audience_gender` |
| `state IN ('DRAFT','PUBLISHED','DELETED')` | `intro_audience_gender_and_state_are_closed` — asserts `intro_posts_state` |
| No reward trigger on `intro_likes` | `intro_likes_carry_no_reward_trigger` — enumerates `pg_trigger`, allows only the self-like guard and the shared account fence, and asserts neither function writes or names a reward |
| Profile transport rejects `/events/` paths | `event_path_target_is_rejected_by_the_parser_and_the_binding` — asserts `InvalidArgument` and that no scope row was written |

Every negative case asserts the **specific constraint name** through
`sqlx::Error::Database::constraint()`, never "some error occurred".

## 3. Verification commands and exit codes

Run from `backend/` unless stated. The PostgreSQL cluster is the private,
marker-owned `mcr-pg-*` cluster created by the
`backend/scripts/mcr/migration_smoke.py` harness pattern in `/tmp`; no test
attaches to a foreign server.

| # | Command | Exit |
| --- | --- | --- |
| 1 | `cargo +1.88.0 check --locked --workspace` | **0** |
| 2 | `PROFILE_TEST_DATABASE_URL=… cargo +1.88.0 test --locked -p profile-service --test profile_product_transport` | **0** (7 passed) |
| 3 | `PROFILE_TEST_DATABASE_URL=… cargo +1.88.0 test --locked -p profile-service --test profile_identity_schema` | **0** (19 passed) |
| 4 | `CONCERT_TEST_DATABASE_URL=… cargo +1.88.0 test --locked -p concert-service --test profile_social_schema` | **0** (28 passed) |
| 5 | `cargo +1.88.0 test --locked -p api-gateway --test profile_product_routes` | **0** (10 passed) |
| 6 | `cargo +1.88.0 test --locked -p api-gateway --lib profile_product` | **0** (7 passed) |
| 7 | `python3 scripts/mcr/wp02_migration_ledger.py --report /tmp/wp02/ledger-final.json` (from the repository root) | **0** |

### Ledger result (T-112)

```text
profile-service head 15 new [11, 12, 13, 14, 15]
   empty            rows 15  applied [1 … 15]   schema hash 82dab759d170e30d…
   previous-head-10 rows 15  applied [11 … 15]  schema hash 82dab759d170e30d…
concert-service head 85 new [80, 81, 82, 83, 84, 85]
   empty            rows 69  applied [1 … 85]   schema hash 2ac1123e669f9572…
   previous-head-79 rows 69  applied [80 … 85]  schema hash 2ac1123e669f9572…
```

The empty-database run and the previous-head run reach the **same** schema hash,
which is the proof that the new migrations are additive: applying them to a head
database produces exactly the schema an empty database gets.

## 4. Deviations from the task document

1. **T-103 table name.** The task says "backfill from the existing `profiles`
   table keyed by `user_id`". The existing `profiles` table already *is* the
   profile store keyed by `user_id`, so a table also called `profiles` in the
   same database would collide. The new table is `profile_identities`, with
   `profile_id` (generated once, immutable) and `account_id` (the Auth UUID,
   unique among live rows). The backfill derives `profile_id` deterministically
   from `user_id` with `md5('mcr profile identity v1:' || user_id)::uuid`, so it
   is repeatable, and the migration asserts the backfill is total and unique.
2. **T-101 `ProfileOwnerBinding`.** `platform::actor_delegation::owner_binding`
   is unchanged except for one additive branch: the delegation interceptor now
   also installs `VerifiedParentAuthority` for the exact method
   `/profile.product.v1.ProfileProductService/Dispatch` when the audience is
   `Profile`. Without that branch the profile producer could not re-decode the
   signed parent, which is the unknown-outcome protection the task requires.
   The new `ProfileGuardBinding` lives in the profile service
   (`adapters/out/profile_owner_guard.rs`) and is keyed by `profile_id`; its
   digest is domain-separated from the concert binding so the two can never
   collide.
3. **T-101 settlement.** The profile analogue of `CompleteResponse` is
   `ProfileProductDeliveryService/CompleteResponse`. It is registered and
   exercised; the gateway client calls it after the HTTP body terminal exactly
   as the concert client does. The scope lifecycle is `ACTIVE → SEALED`
   (capability published) `→ ROOT_FINISHED` (settled), recorded durably in
   `profile_product_scopes`.
4. **T-102 `account_context_version`.** The foundation has no
   `account_context_version` check anywhere: the string does not appear in
   `backend/` at all, only in the integration documents. The gateway therefore
   reuses the existing session/CSRF/origin/`X-MCR-Intent` discipline unchanged
   and does not invent a new header. `409 ACCOUNT_CONTEXT_STALE` is not produced
   by this work package; it belongs to whichever work package introduces the
   version.
5. **T-102 route set.** The frozen register lists 41 new operations across the
   core and domain bundles. The gateway router serves the 22 profile-owned
   routes whose request/response shapes exist in this work package and whose
   producer behavior is owned here; every route is asserted against the frozen
   register by `every_route_is_a_frozen_operation_and_no_route_names_an_event`.
   Routes whose producer arrives in a later work package are not mounted, so
   they cannot silently reach a producer that does not exist.
6. **T-109 self-like.** The task says the "author cannot like their own post" is
   enforced in the command, not the table. It is enforced in **both**: the
   trigger `intro_likes_no_self_like` reads the author from `intro_posts` and
   refuses the insert. The table-level rule is what makes the invariant true
   under concurrent writers; the command remains the place that returns the
   product error.
7. **T-110 purpose enforcement.** The task says the `INTRO_POST`-purpose
   rejection is "enforced in the command; the schema test asserts the command
   rejection". There is no command in this work package, so the rule is enforced
   by the `profile_post_attachments_purpose` and `intro_attachments_purpose`
   triggers, and the schema test asserts the trigger rejection. This is
   strictly stronger: it holds for every writer, not only the command.
8. **T-111 deletion coverage.** The concert service cannot join the profile
   service's database, so the deletion category compares owner columns against
   the account only for rows the saga itself writes. The media erase is
   reference-checked (`profile_media_erase_unreferenced`), so a file referenced
   by two posts survives until both references are gone — proven by
   `a_media_file_shared_by_two_posts_survives_until_both_are_deleted`.
9. **`chr(0)` CHECKs.** PostgreSQL rejects a NUL byte in a `text` value at the
   storage layer, so a `position(chr(0) in body)` CHECK cannot be evaluated for
   a value that contains one. The NUL rule is therefore the storage layer's own
   and the bounded single-line fields carry newline/CR checks instead.

## 5. Known pre-existing failures (not caused by this work)

These fail identically on `master` before this branch and require Docker or a
master fix:

- `api-gateway` `config::hardening_tests::external_migration_mode_rejects_schema_damage_without_owner_privileges`
  and `external_quota_pool_preserves_restricted_shared_budgets_and_timeouts` —
  `container runtime socket /var/run/docker.sock does not exist`.
- `api-gateway` `concert_product::feature_routes::tests::venue_location_and_place_notes_families_are_exact_and_closed`
  — fails on `master` with no local changes applied.
- `profile-service` `config::tests::external_migration_mode_rejects_schema_damage_without_owner_privileges`
  — same missing Docker socket.

## 6. Limitations

- The profile producer in `profile_product/targets.rs` implements the two
  read operations whose product behavior belongs to this work package
  (`getOwnProfile`, `getProfile`). Every other profile operation is refused by
  the closed route parser until its owning work package adds it. This is
  deliberate: a producer that answered an operation it does not own would be a
  silent product decision.
- The gateway's `profile_product_client` sets `profile_id` to the actor's own
  canonical profile id, because the foundation has no account→profile lookup
  yet. The producer still authorizes every target against the session, so this
  is not a grant.
