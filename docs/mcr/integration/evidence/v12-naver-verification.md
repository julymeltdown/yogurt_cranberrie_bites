# WP-04b — NAVER verification evidence (V-12)

Status: **INTEGRATED**. Tasks T-201 … T-212 of
[task-breakdown.md §7](../task-breakdown.md#7-wp-04b--naver-verification-tasks)
are implemented, tested against a real PostgreSQL 16 cluster and a fake provider
on loopback, and committed on `codex/yc-wp04b-identity`.

Base commit: `b5926f288` (merge of `codex/yc-wp02-schema`).

## 1. What was built

| Task | Deliverable | Path |
| --- | --- | --- |
| T-201 | Provider profile decoding, `normalized_gender`, size bound | `backend/services/profile-service/src/identity/naver_profile.rs` |
| T-202 | Provider client: token exchange, profile fetch, typed errors | `backend/services/profile-service/src/identity/naver_provider.rs` |
| T-203 | Provider configuration with production refusal | `backend/services/profile-service/src/identity/naver_config.rs` |
| T-204 | Verification start: one-use state, 10-minute expiry, return-path allowlist | `backend/services/profile-service/src/identity/start.rs` |
| T-205 | Verification callback: consume once, validate in order, supersede | `backend/services/profile-service/src/identity/callback.rs` |
| T-206 | Status read: canonical ordering, Seoul current age | `backend/services/profile-service/src/identity/status.rs` |
| T-207 | Disconnect: `REVOKED`, idempotent | `backend/services/profile-service/src/identity/disconnect.rs` |
| — | Claim store (start/callback/status/disconnect SQL) | `backend/services/profile-service/src/identity/repository.rs` |
| — | Module root and runtime composition | `backend/services/profile-service/src/identity/mod.rs` |
| — | Internal callback transport (`profile.identity.v1`) | `backend/proto/profile/v1/identity_verification.proto`, `backend/services/profile-service/src/adapters/in/grpc/identity_verification.rs` |
| T-208 | Gateway identity routes and callback | `backend/services/api-gateway/src/adapters/in/http/identity_routes.rs`, `backend/services/api-gateway/src/adapters/out/identity_callback_client.rs`, `backend/crates/gateway-domain/src/identity_verification.rs` |
| T-209 | Identity status UI | `frontend/src/features/identity/IdentityStatus.tsx`, `frontend/src/features/identity/api/identity.ts` |
| T-210 | Start and callback UI | `frontend/src/features/identity/VerificationStart.tsx`, `frontend/src/features/identity/VerificationCallback.tsx` |
| T-211 | Gated-post prompt | `frontend/src/features/identity/VerificationPrompt.tsx` |
| T-212 | This evidence package | `docs/mcr/integration/evidence/v12-naver-verification.json`, this file |

The three authenticated operations (`getOwnIdentityVerification`,
`startIdentityVerification`, `disconnectIdentityVerification`) travel through the
existing profile product transport, under the same owner scope and delivery
discipline as every other profile operation. The callback does not: it is
anonymous, so it travels through a separate, narrow service
(`profile.identity.v1.IdentityVerificationService/Complete`) that carries no
actor and no claim value and is callable only by the Gateway workload.

## 2. Hard constraints and the test that proves each

Every case runs against the real `identity_claims` and `identity_verifications`
tables in the runner-owned cluster, so the schema's own CHECK constraints
participate in every assertion.

| Constraint | Test |
| --- | --- |
| Missing provider gender → `profile_incomplete`, **no claim row**, never coerced to `unknown` | `identity_callback::a_missing_gender_is_profile_incomplete_with_no_claim_row` |
| Unrecognized provider gender → `profile_incomplete`, **no claim row** | `identity_callback::an_unrecognized_gender_is_profile_incomplete_with_no_claim_row` |
| Explicit provider `U` → bucket `unknown` (the only producer of it) | `identity_callback::an_explicit_unspecified_gender_writes_the_unknown_bucket` |
| Derived age outside 14–100 → `age_not_supported`, **no claim row** | `identity_callback::an_age_outside_the_supported_range_is_age_not_supported` |
| The 14 and 100 boundaries are inclusive | `identity_callback::the_supported_age_boundaries_are_inclusive` |
| Duplicate provider subject already backing another user → rejected, no second claim | `identity_callback::a_duplicate_provider_subject_is_rejected_with_no_second_claim` |
| Re-verification supersedes: old row `REPLACED`, expiry capped at the replacement instant | `identity_callback::a_re_verification_supersedes_and_caps_the_old_expiry` |
| A supersede never extends an already-earlier expiry | `identity_callback::a_supersede_of_an_already_expired_claim_keeps_the_earlier_expiry` |
| Callback state is one-use; reuse rejected, no claim | `identity_callback::a_reused_state_is_rejected_with_no_claim` |
| Callback state expiry rejected, no claim | `identity_callback::an_expired_state_is_rejected_with_no_claim` |
| The raw state is stored only as a digest | `identity_start::the_state_is_stored_only_as_a_digest` |
| Return path outside the closed allowlist refused (`https://evil.example`, `//evil.example`, …) | `identity_start::a_return_path_outside_the_allowlist_is_refused` |
| Bounded in-flight states per user | `identity_start::a_second_in_flight_state_beyond_the_bound_is_refused` |
| No provider token, subject, birth date or age in logs or `Debug` | `naver_provider::the_access_token_and_profile_render_no_value_in_debug`, `naver_profile::the_decoded_profile_redacts_every_value_in_debug`, `identity_callback::the_callback_outcome_carries_no_claim_value`, `identity_callback::the_stored_rows_hold_no_provider_token_subject_or_birth_date`, `identity_status::the_claims_debug_renders_no_age` |
| The flow is EVENT-FREE: no `event_id` anywhere | `identity_start`, `identity_callback`, `identity_status`, `identity_disconnect` all bind only a `user_id`; the schema is WP-02's `0013_identity_claims.sql`, which has no event column; `identity_routes::the_callback_is_not_a_profile_product_route` and the profile producer's event-path rejection |
| Production config with placeholder client id/secret refuses to start | `naver_config::a_production_start_with_the_local_client_id_refuses`, `…_secret_refuses`, `naver_config::a_production_start_with_placeholders_refuses_to_start` |
| The redirect URI must be the console-registered callback | `naver_config::a_redirect_uri_that_is_not_the_registered_callback_refuses` |
| The callback is anonymous and answers `303` with a constant `Location` | `identity_routes::the_callback_is_anonymous_and_does_not_require_browser_intent`, `identity_routes::a_location_is_a_constant_path_plus_a_constant_marker` |
| A callback without a valid state is a closed `400` | `identity_routes::a_callback_without_a_valid_state_is_a_closed_400` |
| The callback cannot reach another operation | `identity_routes::the_callback_cannot_reach_another_operation` |
| The callback has its own admission class | `identity_routes::the_callback_has_its_own_admission_class` |
| Disconnect is idempotent and touches nothing else | `identity_disconnect::a_second_disconnect_returns_the_same_state`, `identity_disconnect::a_disconnect_touches_no_other_table` |
| The user's own restricted posts keep their audience | `identity_disconnect::a_disconnect_leaves_the_users_own_restricted_posts_untouched` |
| A revoked claim yields no attributes, so no restricted audience matches | `identity_disconnect::a_disconnect_removes_the_attributes_the_audience_decision_needs` |
| An older usable row is never selected over a newer expired one | `identity_status::an_older_usable_row_is_never_selected_over_a_newer_expired_one` |
| The current age moves on the Seoul birthday with no stored mutation | `identity_status::the_current_age_advances_on_the_seoul_birthday_without_a_stored_mutation` |

## 3. Verification commands and exit codes

The PostgreSQL cluster is the private, marker-owned `mcr-pg-*` cluster created by
`backend/scripts/mcr/wp04b_identity_cluster.py` (the
`migration_smoke.py`/`wp02_migration_ledger.py` harness pattern: `/tmp` root,
Unix socket only, `listen_addresses=''`, no inherited credentials). Server
version: **16.14 (Homebrew)**. The fixture database is created empty and the
test binaries apply the service's own migration files.

Run from `backend/` unless stated.

| # | Command | Exit | Result |
| --- | --- | --- | --- |
| 1 | `cargo +1.88.0 check --locked --workspace` | **0** | workspace compiles |
| 2 | `PROFILE_TEST_DATABASE_URL=… cargo +1.88.0 test --locked -p profile-service --test naver_profile` | **0** | 9 passed |
| 3 | `… --test naver_provider` | **0** | 9 passed |
| 4 | `… --test naver_config` | **0** | 15 passed |
| 5 | `… --test identity_start` | **0** | 8 passed |
| 6 | `… --test identity_callback` | **0** | 25 passed |
| 7 | `… --test identity_status` | **0** | 11 passed |
| 8 | `… --test identity_disconnect` | **0** | 8 passed |
| 9 | `cargo +1.88.0 test --locked -p api-gateway --test identity_routes` | **0** | 12 passed |
| 10 | `cargo +1.88.0 test --locked -p profile-service --test unit` | **0** | 47 passed |
| 11 | `cargo +1.88.0 test --locked -p profile-service --test profile_product_transport` | **0** | 7 passed |
| 12 | `cargo +1.88.0 test --locked -p api-gateway --test unit` | **0** | 218 passed |
| 13 | `cargo +1.88.0 test --locked -p api-gateway --test profile_product_routes` | **0** | 10 passed |
| 14 | `cargo +1.88.0 test --locked -p api-gateway --test browser_auth_http` | **0** | 14 passed |
| 15 | `cd frontend && corepack pnpm@11.7.0 run typecheck` | **0** | no type errors |
| 16 | `cd frontend && corepack pnpm@11.7.0 exec vitest run src/features/identity` | **0** | 20 passed (3 files) |

**Total new tests: 85 backend + 12 gateway + 20 frontend = 117.**

## 4. V-12 case disposition

| Case | Input | Expected | Status | Evidence |
| --- | --- | --- | --- | --- |
| V-12-a | `birthyear=2000`, `birthday=07-12`, Seoul date 2026-07-12 | `age_at_verification=26`, `next_birthday_at=2027-07-12` | **PASS** | `identity_callback::a_complete_profile_writes_one_claim_with_the_derived_age` |
| V-12-b | Same claim read on 2027-07-12 | current age 27, no stored mutation | **PASS** | `identity_status::the_current_age_advances_on_the_seoul_birthday_without_a_stored_mutation` |
| V-12-c | `gender=F` | bucket `female` | **PASS** | `naver_profile::the_donor_gender_mapping_is_preserved_exactly`, `identity_callback::a_complete_profile_writes_one_claim_with_the_derived_age` |
| V-12-d | `gender=M` | bucket `male` | **PASS** | `identity_callback::the_male_gender_writes_the_male_bucket` |
| V-12-e | `gender=U` | bucket `unknown` | **PASS** | `identity_callback::an_explicit_unspecified_gender_writes_the_unknown_bucket` |
| V-12-f | `gender` missing | `profile_incomplete`, no claim row | **PASS** | `identity_callback::a_missing_gender_is_profile_incomplete_with_no_claim_row` |
| V-12-g | `gender=X` | `profile_incomplete`, no claim row | **PASS** | `identity_callback::an_unrecognized_gender_is_profile_incomplete_with_no_claim_row` |
| V-12-h | `birthyear` missing | fails, no claim row | **PASS** | `identity_callback::a_missing_birthyear_is_profile_incomplete_with_no_claim_row` |
| V-12-i | Derived age outside 14–100 | `age_not_supported`, no claim row | **PASS** | `identity_callback::an_age_outside_the_supported_range_is_age_not_supported` |
| V-12-j | Same provider subject already backs another user | duplicate, no second claim | **PASS** | `identity_callback::a_duplicate_provider_subject_is_rejected_with_no_second_claim` |
| V-12-k | Re-verify after expiry | new claim supersedes; old row capped | **PASS** | `identity_callback::a_re_verification_supersedes_and_caps_the_old_expiry`, `…a_supersede_of_an_already_expired_claim_keeps_the_earlier_expiry` |
| V-12-l | `DELETE /api/me/identity/naver` | `REVOKED`; own posts keep audience; own viewing denied | **PASS** | `identity_disconnect::a_disconnect_makes_the_read_return_revoked`, `…a_disconnect_leaves_the_users_own_restricted_posts_untouched`, `…a_disconnect_removes_the_attributes_the_audience_decision_needs` |
| V-12-m | Callback state reused | second use rejected; no claim | **PASS** | `identity_callback::a_reused_state_is_rejected_with_no_claim` |
| V-12-n | Callback for expired state (>10 min) | rejected; no claim | **PASS** | `identity_callback::an_expired_state_is_rejected_with_no_claim` |
| V-12-o | Any of the above | no token, subject, birth date or age in logs or state outside the claim row | **PASS** | `naver_provider::the_access_token_and_profile_render_no_value_in_debug`, `naver_profile::the_decoded_profile_redacts_every_value_in_debug`, `identity_callback::the_stored_rows_hold_no_provider_token_subject_or_birth_date`, `identity_status::the_claims_debug_renders_no_age` |
| V-12-p | Production config with placeholder client id/secret | service refuses to start | **PASS** | `naver_config::a_production_start_with_placeholders_refuses_to_start` |
| NAVER-CONSOLE | A test NAVER application registered with the callback URL | — | **BLOCKED** | No test NAVER application exists in this environment. See §6. |

**15 of 16 V-12 cases PASS. NAVER-CONSOLE is BLOCKED for the reason in §6.**

The V-12 evidence is recorded in machine-readable form in
[v12-naver-verification.json](v12-naver-verification.json).

## 5. Deviations from the task document

1. **T-203 lives in `src/identity/naver_config.rs`, not `src/config/`.**
   The task says `backend/services/profile-service/src/config/`. The provider
   settings are resolved *by* `AppConfig::load` (a production placeholder refuses
   to start there), but the type itself belongs with the flow it configures, and
   putting it in `config/` would have made the identity module depend on the
   service's whole configuration surface. `AppConfig` holds a
   `naver_oauth: NaverOAuthConfig` field and calls `NaverOAuthConfig::load`.

2. **The callback is not a profile product operation.**
   The task says T-208 writes `identity_routes.rs` and registers "the four
   identity operations". Three of them are profile product operations and are
   registered in the existing closed route table. The callback cannot be: it is
   anonymous, and every profile product message is bound to a verified actor.
   Relaxing that binding for one route would be the vulnerability. The callback
   therefore has its own route module, its own gRPC service
   (`profile.identity.v1`), and its own admission class
   (`RouteClass::VerificationCallback`). This is recorded here rather than
   silently implemented.

3. **The supersede cap is `verified_at - 1µs`, not `verified_at`.**
   The schema's `identity_claims_lifetime` CHECK requires `expires_at >
   verified_at` for **every** row, including a replaced or revoked one. Capping
   at exactly the replacement instant violates it. One microsecond is below the
   instant's precision, so the row still ends at the replacement instant as the
   domain contract requires. The same adjustment applies to revocation. The
   tests assert the one-microsecond value and say why.

4. **A replacement row is inserted with a domain-separated subject hash.**
   `identity_claims_subject` is a global unique index on
   `(verifier, subject_hash)`, and `identity_claims_immutable_evidence` refuses
   to rewrite an earlier row's evidence. A user re-verifying their own NAVER
   account would therefore collide with their own superseded row and could never
   verify again. The duplicate-subject rule is a *live-claim* rule, which the
   probe in `store_claim` enforces authoritatively; the insert uses a hash no
   provider subject can produce whenever a supersede happened or a non-live row
   already holds the subject. The live row always holds the real hash, so global
   uniqueness of a live subject is preserved exactly. This is the one place where
   the implementation works around a WP-02 schema decision rather than
   implementing the document literally, and it is called out for the schema
   owner.

5. **`RpcAction::VerificationCallback` was added to `platform`.** The callback
   needed an authorization action that admits only the Gateway workload and
   declares no actor. Adding a variant to the closed `RpcAction` enum is the
   platform's own extension point; no existing behaviour changed.

6. **`RouteClass::VerificationCallback` was added to `gateway-domain`.** The
   admission arrays grew from 8 to 9 entries. The new class has a budget of 20 per
   network per minute and a concurrency ceiling of 8.

7. **`AppConfig` gained `naver_oauth` and `identity_subject_salt`.** Every
   `AppConfig` literal in the tests was updated. `PROFILE_IDENTITY_SUBJECT_SALT`
   is required in production (≥32 bytes) and has a fixed parity default.

8. **The database name in the harness is `mcr_wp_four_b_identity`.** The
   `migration_smoke.Cluster::url` validator accepts only `mcr_[a-z_]+`, which
   excludes digits; the name is spelled out to satisfy it rather than weakening
   the validator.

9. **The V-12 "real services with a test NAVER application" step is BLOCKED.**
   T-212 step 1 asks for execution against real services with a test NAVER
   application. Everything else in T-212 was executed: every case ran against a
   real PostgreSQL 16 cluster and the real provider client, with only the far end
   of the provider faked on loopback. See §6.

## 6. Blocked: NAVER-CONSOLE

T-212 step 3 asks to "record the console approval reference as the external
prerequisite evidence". No NAVER developer application exists in this environment:

- there is no `NAVER_OAUTH_CLIENT_ID` / `NAVER_OAUTH_CLIENT_SECRET` pair for a
  test application;
- the console registration of
  `{origin}/api/identity/naver/callback` has not been made, so a real
  authorization round trip cannot complete;
- consequently V-12 cannot be executed "against real services with a test NAVER
  application" as T-212 step 1 describes.

What *was* executed is strictly stronger than a unit test for every case except
the provider itself: the real `NaverProviderClient` performed a real token
exchange and a real bearer profile fetch over HTTP against a fake provider, and
the real claim store wrote to the real schema. The one thing that cannot be
proven without a console application is that NAVER's own `/v1/nid/me` response
matches the shape this decoder accepts — which is the donor's documented shape,
unchanged.

**To unblock**: register a test NAVER application with the callback URL, set
`NAVER_OAUTH_CLIENT_ID`, `NAVER_OAUTH_CLIENT_SECRET` and
`NAVER_OAUTH_REDIRECT_URI`, and run the flow once end to end. Record the
application id as the console approval reference.

## 7. Known pre-existing failures (not caused by this work)

These fail identically on `master` (`b5926f288`) before this branch and are
recorded so a reader does not attribute them to WP-04b.

| Suite | Failing | Cause |
| --- | --- | --- |
| `profile-service --lib` | `config::tests::external_migration_mode_rejects_schema_damage_without_owner_privileges` | `container runtime socket /var/run/docker.sock does not exist` |
| `profile-service --test contract` | 6 of 10 | the contract binary shares one fixture database and its cases mutate the seeded `profiles` rows; verified failing on `master` with the same fixture |
| `frontend vitest` (whole suite) | 18 tests in 4 files | `location-registration`, `c07-social-client/compatibility`, `map-presentation-v1/input-readiness`, `meet-beacon/photo-client`; verified failing on `master` with the same counts |

The frontend numbers are exact: before this branch the whole suite is
**4 failed files / 18 failed tests / 5750 passed**; after it is
**4 failed files / 18 failed tests / 5770 passed**. The 20 added tests are the
identity feature's own.

## 8. Limitations

- The client-side identity feature has no route mounted in `src/app/router.tsx`.
  Route composition is integrator-owned (`frontend/AGENTS.md`: "root router
  composition ... remain integrator-owned"). The three components and the
  controller are complete and tested; mounting them is a one-line change for the
  integrator, and the handoff names the exact paths.
- `display_on_profile` is always `false`. The frozen contract requires the field;
  a profile display setting does not exist yet, and the value never affects an
  audience decision.
- The 14–100 age window is configuration (charter D-08). It is enforced by the
  schema and by `concert_domain::age_policy::arithmetic`, and it is not presented
  as the final global age policy.
- `identity_verifications` rows are never pruned by this work package. The
  account-deletion saga (WP-02 `0015_profile_account_deletion.sql`) erases them
  with the account; a retention purge for abandoned states is not part of T-201…
  T-212.
