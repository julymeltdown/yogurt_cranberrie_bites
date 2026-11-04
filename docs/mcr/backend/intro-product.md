# BE27 Product Intro runtime

Status: **CONTRACT_READY** component handoff. Source `868d9db276b7f704ebeb675c7d2d96bc88b019a2` compiled successfully; two pure cases, all seven canonical native cases, four retained engine cases and PG restart persistence passed. Root integration/publication is pending. Public HTTP acceptance is **NOT_RUN**. These component results do not certify IN03 or product acceptance.

Base: `a7f3989d9c40c53a9c2982bced938f8b4853f627`; branch `codex/mcr-be-BE27-product`. The former consumer writer stopped before this isolated checkout began. Changes stay within BE27-PRODUCT ownership: `src/intro_live/**`, needed retained `repository/intro.rs` hooks, only `pub mod intro_live;` in `lib.rs`, exclusively reserved migration 0049, `tests/intro_product.rs`, `tests/intro_product_support/**` and this document. Root owns global Operation/HTTP/main and union resolver composition. No manifest, lockfile, contract, configuration, CI, old migration or `docs/source/` changes. Root explicitly transferred `backend/crates/concert-application/src/intro/mod.rs` for the single additive `IntroError::Expired` variant only, after the base lacked the historical adapter variant. This preserves the frozen 409 IDEMPOTENCY_EXPIRED reason; no broader application rewrite is included.

## Runtime and integration API

`IntroRuntime::new(PgPool, Arc<PublicProfileReader>, IntroCursor, Arc<FingerprintKeys>, Arc<AgeRuntime>)` receives actual typed dependencies. `dispatch_for_scope(Arc<OwnerGuardScope>, proto::ProductRequest) -> Result<ProductRepresentation, tonic::Status>` consumes Root's acquired scope. It implements `ScopedProductProducer`. `protocol::handles_operation(&str)` accepts exactly `listConcertIntros`, `getConcertIntro`, `getOwnConcertIntro`, `setConcertIntro`, `publishConcertIntro`, `closeConcertIntro`. Root must restore the original sealed parent **inside** the supervised future: Tokio spawning does not inherit task-local authority.

The facade compares the full raw request to the original sealed unary message and checks actor/event/family/request/operation/method/target/body/deadline before parsing. Closed command structs require nullable fields and reject duplicate/unknown fields. The retained PgIntro engine owns CAS, receipts, idempotency and BE15 outbox. Production policy registers the actual connection before effects and uses actual canonical Auth/Friend enrollment and current local admission. Public reads use actual held PublicProfile subjects and current authority; restricted reads borrow the same transaction for AgeRuntime's trusted PgClaimProjection and acknowledge pending values after known commit. Unknown, expired and untrusted claims cannot authorize restricted list/detail. Accepted persisted claims remain usable when new verification is unavailable. Public DTOs omit exact age, private bounds/expiry and private profile avatar URLs.

Migration 0049 registers the retained Intro schema and a private immutable same-transaction native witness. Both actual canonical registration ACKs and the confirmed full native identity must exist. Deferred commit requires the held deadline plus a witness from the current native transaction. Update/delete/truncate cannot rewrite witnesses. `IntroNativeResolver::new(pool, instance) -> Result<Self, Status>` uses actual native negative proof and requires the exact durable witness for COMMITTED; native commit alone remains UNKNOWN. The witness is settlement evidence, never new request authority. Feature code never marks serialization, releases scope or settles delivery.

The edit callback invokes the retained `revoke_intro_consent_in` inside the actual Intro command transaction, after event/Intro locking and mutation and before outbox commit. It creates no new ledger, receipt, consent grant, context or cross-service SQL. Cursor encryption/authentication binds actor/event/query and preserves the original fence and expiry across continuations.

## Validation and limits

The native harness has two pure protocol/cursor cases and seven ignored canonical cases. `tests/intro_product_support/qualify.py` requires a clean committed source checkpoint plus frozen binary SHA-256 identities. It creates a fresh private marker/inode-owned socket-only PG16 cluster with actual mTLS Auth/Friend/Profile services and four isolated owner databases, reset between cases. Central migration receipts must match source checksums. Fixture claims, Social records and selected source-bound quality-v6 release are explicit test metadata, not production grants or operator admission.

Cases cover all six operations, private-field omission, CAS/replay/precise expired-key reason, restricted list/detail across outage/trust/expiry, parent rebinding, block/departure/suspension, fatal Profile outage, original response disclosure ceiling, consent/outbox atomic rollback/replay, exact native witness, witness immutability, rollback and unknown committed effect quarantine. Other cases require zero prepared transactions. The final unknown-commit case deliberately retains owner exclusion, recorded separately; final owned-cluster destruction removes that quarantined fixture. Four retained Intro engine cases run separately on their unchanged explicit synthetic-authority prerequisite schema, followed by restart persistence. Those regressions are not canonical authority evidence.

Current checks:

- `rustfmt +1.88.0 --edition 2021` on owned Rust: exit 0, parsing/formatting only.
- Python compile of the owned qualifier: exit 0.
- `python3 scripts/mcr/check-plan.py`: exit 0, PASS; original source checksums remain intact.
- Rust build and the complete owned pure/native/retained/restart suite: exit 0, **PASS** at source `868d9db276b7f704ebeb675c7d2d96bc88b019a2`; exact evidence below.

Build request: Rust 1.88, `--locked --offline`, jobs <=2, debug 0, incremental 0, exact `cargo test -p concert-service --test intro_product --test intro --no-run --message-format=json`. Freeze source inventory/archive, Cargo JSON and exact executable copies/hashes outside the target, then release the target before private PG. No target or PG is currently held.

Public HTTP/main composition, browser/two-client IN03, client cache invalidation, actual operator admission and commercial provider activation remain **NOT_RUN** here. The existing unbudgeted goal is unfinished. Its API metadata still reports the old BLOCKED integration gate and old checkout; it was neither falsely completed nor replaced. The tools expose no active-status setter. The current authorized Product assignment resumes source work and remains unfinished through validation, committed handoff/fixes and normal Root publication plus fresh ancestry verification.

## Provenance

Audited headless backend code only. Existing migrations and supplied source archives remain unchanged. Historical source revisions establish provenance, not current runtime proof.

| Source revision | Source path | SHA-256 | Destination/adaptation |
| --- | --- | --- | --- |
| `fbe54712c3cb0bd95de937b001ee6a00eaefe900` | `backend/services/concert-service/src/adapters/out/repository/intro.rs` | `623b574db5183bad6f7517dd48c87075a1defce8e74d7bd80d08e0673f7cbb83` | `retained engine and relocated intro_live adapters` |
| `fbe54712c3cb0bd95de937b001ee6a00eaefe900` | `backend/services/concert-service/migrations/0039_intro_runtime.sql` | `1e14cb9bfb36dda6160bcd43eb0f0614b2c0f3841353b82bc673ec923b47b45b` | `reserved migration 0049` |
| `a7f3989d9c40c53a9c2982bced938f8b4853f627` | `backend/services/concert-service/src/adapters/out/public_profile/native/environment.rs` | `adde0dc16f21112e866bea96f11334d6033f6669fad408569de492f5c392c352` | `intro_product_support/environment.rs` |
| `a7f3989d9c40c53a9c2982bced938f8b4853f627` | `backend/services/concert-service/src/adapters/out/public_profile/native/owners.rs` | `0d9c52e710823bd936d36daf46da25dae08226efeb5db96dd4209dde2280a9ce` | `intro_product_support/owners.rs` |
| `a7f3989d9c40c53a9c2982bced938f8b4853f627` | `backend/services/concert-service/src/adapters/out/public_profile/native/capture.rs` | `438063134a950ef2d06a1f25a876f023552201ebab6304c8d77853ed70677806` | `intro_product_support/capture.rs` |
| `a7f3989d9c40c53a9c2982bced938f8b4853f627` | `backend/services/concert-service/src/adapters/out/public_profile/native.rs` | `3b276170b55f9e24d67841335b8ba3b8fe6d275831b8614f63bc8a0d0babbf3a` | `intro_product_support/fixture.rs` |
| `a7f3989d9c40c53a9c2982bced938f8b4853f627` | `backend/services/concert-service/tests/age_runtime_support/qualify.py` | `5d960e413cb54eff1ebaaafdacf0e64f231c70022098221b5fe2798226b23c68` | `intro_product_support/qualify.py` |

Pinned contract bytes at the base (no amendments):

- `contracts/domain/v1/operations.json`: `f801d703dfc5509bbe06a2f0fa07a170b6442a48b2204c7ead460fc9a3600112`.
- `contracts/generated/mcr-domain.openapi.json`: `905d6f5b6f66f95cb68499a5fbfb32802539fce515f421226d79cb375ecfb887`.

Exact build/native artifact identities and exit codes are recorded below. Component proof is not public HTTP acceptance.

Coordinated prerequisite intake before any accepted build: Root explicitly authorized serial `cherry-pick -x 591d908a85a01fe0a494e04c0109bb0bec64a3e1`, producing `d8a841acd1e05eef7af2de89c3cdb46a5317a985`. Its exact outside-assignment paths are Auth/Friend/Notification/Concert `build.rs` and `docs/mcr/backend/build-script-worktree-paths.md`. These replace compile-time protocol/include worktree paths with the existing runtime contracts-build helper to fix Root's observed cache/protoc shadowing. No target cleanup or additional artifact scope was granted. This prerequisite is already Root-owned delivery; the BE27 handoff must avoid duplicating it. Runtime migration inputs still require live source and baked-path pins.

## Historical failures and repairs

The following failures were preserved and corrected before the final coherent run. They do not supersede the final result or establish public HTTP acceptance.

- Committed source `c34e561513a3252339248024ec2941d760231e03` first failed compilation because the shared target reused a `concert_application` artifact as FRESH despite the committed Expired variant. Live source SHA-256 was `2a6f1d0a58dca05fe749da25bcc684b4c3c153ccde22bd8ca476399ea0723f23`. Under a separate explicit named repair lease, cleaning only that package removed 21 files/15.4 MiB. The identical-source rebuild passed, exit 0 in 23.46 seconds, establishing cache causality. Root's protocol-path prerequisite avoided the previous protoc failure.
- Two pure protocol/cursor cases passed. Early native attempts exposed an omitted actual Profile workload certificate, the unoptimized four-service test thread's default stack limit, and inconsistent fixture timestamps. Owned fixture fixes issue the real Profile mTLS certificate, specify `RUST_MIN_STACK=16777216` and derive the expired receipt's times from one SQL timestamp. These conditions are test evidence, not production HTTP or stack qualification.
- Source `12d0ddfbc7b24611873fdcb69bfa3ae659291b0d` rebuilt successfully, exit 0 in 3.25 seconds. Its frozen `intro_product` executable SHA-256 is `d94f221831974113520de5a1b94539af608b0986dd2ba7cf905bb28270b0a3b8`; retained `intro` SHA-256 is `7dc497209abad013a5f02de848a5a4eb59c9fc4b61224a2a191ac293abca7ee4`. Actual Cargo artifact source paths, live four-owner migration inventories and baked fixture checkout were pinned. Both source archive plus exact source delta/full manifest, Cargo JSON and binary copies were preserved outside the target. Fresh lsof scans found zero target handles before any copied executable or private PG run.
- The coherent `native-4.json` run passed both pure cases and three actual canonical native cases: all six Intro operations with CAS/replay/precise idempotency expiry, restricted Age list/detail parity across outage/trust/expiry, and full raw-frame/missing-parent rejection. Each passed case ended with zero prepared transactions. The next privacy case returned 503 where a hidden-author list must return 200 with no author. This remains a failed acceptance gate. The runner stopped; later native cases and the retained engine/restart stage were NOT_RUN. Its private cluster was stopped and removed successfully.
- The repair adds stage-specific native assertions and a negative-only current local admission/participant/block filter on the same registered transaction before PublicProfile acquisition. The shared reader intentionally uses generic unavailability for local missing policy; Intro must filter known absence without weakening generic owner/guard failures. Positive observations still require actual canonical enrollment and held Profile children. The outage harness also retains unknown child quarantine while Profile is down, restarts the same actual mTLS owner at the same endpoint/store, and retries only Test Root's system settlement. These repairs await the next explicitly allocated build/native window.

All failed attempts and immutable copies remain in the private `/tmp/mcr-be27-product-ffb4b491` evidence directory. The cumulative storage cap includes them; no byte or time budget is silently renewed. The first target window expired after explicit release. A separate bounded repair window was approved as queued, with no resources held while awaiting its named offer. No other service's implementation was edited to hide a failed assertion, and no partial run is labeled ACCEPTED.


## Final coherent component qualification

Run on 2026-11-04 under explicit named allocation `BE27-PRIVACY4-BACKEND-202611040727Z`: additional <=2 minutes target build/freeze plus <=6 minutes copied execution, with no cumulative byte-baseline reset. Previous target windows were explicitly released and their expiry recorded; no old allocation was reused. Clean committed source was `868d9db276b7f704ebeb675c7d2d96bc88b019a2`. Initial free space was 38.09 GiB, kernel handles 14721/368640; final free space was 38.02 GiB. All retained source archives, failed/current binary copies and logs totalled 1986 MiB under the 2 GiB archive/executable cap. The whole workflow retained its 8 GiB hard cap and 8 GiB free-space floor. No resource remains held.

Build command, from `backend/`, exit 0 in 4.44 seconds:

```sh
CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/backend CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo +1.88.0 test -p concert-service --test intro_product --test intro --no-run --locked --offline -j2 --message-format=json
```

Exactly two requested executable artifacts were frozen outside the shared target. Cargo reported `intro_product` newly compiled and retained `intro` FRESH; the latter's byte identity matched its previous committed-source copy. No clean was run in this allocation. The actual Cargo manifest/source paths and the baked fixture checkout were verified, not inferred from a tab or filename. A fresh `lsof -nP -F pn` scan found zero target handles, recorded in `target-release-6.json`; the target was unconditionally released before any copied executable or PG process and was never reread.

Copied execution command, exit 0 in 24.20 seconds:

```sh
python3 backend/services/concert-service/tests/intro_product_support/qualify.py --checkpoint /tmp/mcr-be27-product-ffb4b491/checkpoint.json --pg-bin-dir /opt/homebrew/opt/postgresql@16/bin --report /tmp/mcr-be27-product-ffb4b491/native-6.json
```

The qualifier supplies `RUST_MIN_STACK=16777216` for the unoptimized four-service test thread. This was PG16.14 (Homebrew), socket-only, with four isolated actual owner stores and actual Auth/Friend/Profile mTLS services. Each canonical case recreated all stores and verified source-bound central migration receipts: Auth 18, Friend 18, Concert 38, Profile 9. Live SQL migration SHA-384 inventories are frozen in the checkpoint, including migration 0049 SHA-384 `2c292352935615e7569206712ff88c001e6a64e35d32ac135b26b86999d710468492248df2a3338c08bbe2811bc05bca`.

| Exact test | Result | Prepared transactions afterward |
| --- | --- | --- |
| `closed_protocol_rejects_ambiguous_target_query_and_write_shape` | PASS, exit 0 | Pure case |
| `cursor_encryption_binding_and_original_fence_expiry` | PASS, exit 0 | Pure case |
| `canonical_authoring_discovery_detail_idempotency_and_close` | PASS, exit 0 | 0 |
| `canonical_restricted_age_outage_trust_and_expiry_parity` | PASS, exit 0 | 0 |
| `canonical_full_frame_rebinding_and_missing_parent_fail_closed` | PASS, exit 0 | 0 |
| `canonical_block_departure_suspension_and_profile_outage` | PASS, exit 0 | 0 |
| `canonical_claim_expiry_bounds_root_delivery_after_commit` | PASS, exit 0 | 0 |
| `canonical_intro_edit_revokes_consent_atomically_with_receipt_outbox` | PASS, exit 0 | 0 |
| `canonical_native_witness_requires_exact_registered_transaction` | PASS, exit 0 | 2, deliberately quarantined UNKNOWN |

The privacy case verified inherited Friend blocks, local admission departure and Auth suspension each yield an empty discovery list and 404 detail. Merely aborting the Tonic accept task had left HTTP/2 connections alive in the prior failed attempt; the final fixture sends graceful shutdown and awaits actual server termination. Real Profile outage now returns 503, retains unknown child exclusion while unavailable, and settles to zero prepared transactions after restarting the same mTLS owner at the same endpoint/store. No cached DTO replaces the failure.

The final native witness case verifies COMMITTED only with an exact registered same-transaction witness, IN_PROGRESS during a live actual transaction, ABORTED after rollback, UNKNOWN for wrong producer/reference or a committed effect without a witness, and immutable witness update/delete/truncate rejection. Its two prepared guards deliberately preserve owner exclusion for the last UNKNOWN effect. This is the sole nonzero case; deleting the entire marker/inode-owned disposable cluster removes the quarantined fixture afterward.

All four retained engine cases passed, exit 0: `empty_publish_close_expiry_and_event_isolation`, `foreign_profile_and_projection_failure_return_no_value`, `pagination_filter_profile_and_projection_fail_closed`, `receipts_revision_conflicts_and_invalidation_failure_roll_back`. They used the unchanged retained synthetic prerequisite schema in a separate database; they are regression evidence, not canonical authorization proof. Actual postmaster PID changed 61823 -> 63865 and startup time changed across restart; the retained Intro row stayed `PUBLISHED|1`. Final cleanup passed, and `/private/tmp/mcr-pg-ae_jg6e3` no longer exists.

### Immutable local evidence identities

All evidence resides in private `/tmp/mcr-be27-product-ffb4b491`; no sensitive fixture state or large binary/archive is added to Git. `source-b30b9e20.tar.gz` plus the binary Git delta `source-868.patch` reconstruct the final tested source; the full 2901-file inventory additionally pins actual releases, selected static assets, contracts, build inputs and fixture source. Source and copied binary hashes were rechecked after execution.

| Evidence | SHA-256 |
| --- | --- |
| Full source archive, base `b30b9e208d59d636a8198dee99657658f14632b8` | `87f5c0c7d7bad9d9e58811dda28cfb26ba9325e9fea19118f34ba5018974e5d3` |
| `source-868.patch` | `af47f3d4c70c15180c55857a39af42f718c3db77445f046354e5187f3aaf718e` |
| `source-manifest-868.json` | `982fd9babd66f494f5ec08663c182a5f01a00e3b1b9c26ac8d9c89dc5b66988b` |
| `cargo-6.jsonl` | `7aa8d4cc002c1eb21c1ae5a2f32a4f14dc5afb1bf02db3df8c1ad8f92f925436` |
| `v868/intro_product`, 49,929,696 bytes | `deff7cf58aa4bd43ceaf638f2c81c7193df5876f1270cf356ccffe912889c22b` |
| `v868/intro`, 24,109,248 bytes | `4fd10cc86c1f1b5e2535b026dcb03884442941bf0254a049e8ba8f4f343d8ed6` |
| `checkpoint.json` | `34518b188068ffb87e7011f72908329ad4a6d2546cb1d621a3e3b3b0d481489b` |
| `native-6.json` | `4e618d9e99c23797a48f508b3249ba319aabc189a60b6987bf0742846707cf1f` |

### Delivery ancestry and remaining owner

The base-to-source diff contains exactly 22 paths: 17 BE27-owned paths and five explicitly coordinated Root prerequisite paths. No application imports a frontend implementation; only the fixture reads approved source-bound static release assets. `docs/source/` is unchanged byte for byte. `git diff --check` and `python3 scripts/mcr/check-plan.py` passed, exit 0, after native execution. Planning provenance is separate from the runtime result.

Root should integrate the BE27 commits in order while excluding `d8a841acd1e05eef7af2de89c3cdb46a5317a985`, the already Root-owned `591d908a85a01fe0a494e04c0109bb0bec64a3e1` prerequisite. Own commits through the tested source: `296557ea`, `76dca41c`, `ffb4b491`, `c34e5615`, `b30b9e20`, `1473d345`, `12d0ddfb`, `d30ad9f0`, `8e14fc25`, `cf67c1a9`, `6828ad0f`, `868d9db2`; this final documentation commit follows them. The frozen source predates only documentation; no Rust/SQL/fixture changes follow qualification.

Single-writer ownership of the `lib.rs` export is released to Root upon this committed handoff. Root owns global Operation/HTTP/main composition and the union native resolver. Restore the original sealed parent inside the `run_owned` future, construct typed runtime dependencies, route the exact six closed operations, and dispatch the `intro_native_v1` reference kind to the Intro resolver. No feature code acquires authority, marks serialization or settles Root delivery. Root's combined checks, normal master publication and fresh delivered-ancestry verification remain required before this chat's assignment/goal is complete. Public HTTP, two-client IN03, browser cache invalidation, operator admission and commercial provider activation remain NOT_RUN here.
