# CR07 shared canonical runtime core

## Delivery and evidence status

Task CR07 is the reusable composition portion of BE-CANONICAL-OWNER-AUTHORITY. Its source base is Root commit `be56bbbc9b97ea7eed60fec483310e790f3cab89`. The implementation is isolated in branch `codex/mcr-be-CR07`. This patch adds a core constructor and tests; it does not activate production Product modes, replace Participant composition or supervise a worker in main.

Compilation and test execution are `NOT_RUN`, pending a fresh coherent target allocation. Rustfmt and source/API checks are recorded separately below. The physical CR01–CR06 cohort remains frozen in its separate checkout at `a65c05c7ed96100a191620d765f7cc059216a0dd` with 3053 source/runtime pins. This patch cannot be treated as part of that older executable, source manifest or pending build request.

## Construction and integration contract

`CanonicalRuntimeCore::new(pool, instance, keys, settler)` constructs one actual PgScopeLedger, one ScopeRegistry using a clone of that ledger, and one DeliveryPublisher using that ledger and registry. Core clones and the `ledger()`, `registry()` and `delivery()` accessors preserve their existing configuration and state. Call the constructor once for the configured producer instance and pass clones into Participant, Product and child-producing feature composition. A new core per feature would violate this integration contract.

The `owner_fence(auth_endpoint, friend_endpoint, credential, tls, signer)` helper uses the actual CanonicalOwnerFence constructor with that same registry. The helper neither acquires an owner lease nor parses configuration. Existing endpoint, credential, TLS and signer checks are retained. No fake identity, fallback producer, successful default settler, new deadline or environment policy is introduced.

`settlement_service(resolver)` uses the same ledger and requires the caller's actual NativeTransactionResolver. Producer-specific COMMITTED receipt proof remains the resolver's responsibility. `child_recovery_worker(settings)` clones that same ledger, including its retained keyring and typed settler, and delegates bounded setting validation to the production worker constructor. It starts no task and grants no content authority.

Root retains all production main and existing configuration ownership. Root must supply the actual configured typed child-settler composition (including public, Gift and explicitly enabled Social adapters as applicable), the closed native resolver union for active producers, and one shared registry across handlers. Root must mount and guard the shared Delivery and producer-settlement services once, retain the production HTTP terminal middleware, and supervise the worker through the existing shutdown mechanism. Helper construction alone proves neither adapter presence nor production activation. In particular, ParticipantOnlyChildren remains insufficient for public/Gift/Social recovery; this helper never silently substitutes for it.

## Scoped paths and provenance

| Destination | Ownership and source |
| --- | --- |
| `backend/services/concert-service/src/config/canonical_runtime.rs` | New helper; composes existing production constructors at the recorded Root base |
| `backend/services/concert-service/src/config/mod.rs` | Exact grant: add only `pub mod canonical_runtime;` |
| `backend/services/concert-service/tests/canonical_runtime_config.rs` | New constructor identity and negative boundary tests using existing public production seams |
| `docs/mcr/backend/canonical-runtime-composition.md` | New lane-specific implementation, provenance and validation evidence |

The constructor signatures were traced in `product_rpc/scope_ledger.rs`, `adapters/out/relationship_authority/guarded/registry.rs` and `scope.rs`, `product_rpc/delivery.rs`, `product_rpc/settlement.rs` and `product_rpc/child_recovery.rs`. Their implementation files are read-only in this assignment. No code from historical rendering, source documents or external repositories is copied. Existing dependencies and retained resolutions suffice; no Cargo manifest, lockfile, migration, proto, frontend, source artwork or other shared composition file is changed.

## Validation and remaining acceptance

The three authored tests use closed lazy SQLx pools, with explicit dedicated local connection options and no connection attempt. They verify rejection of a nil producer instance without child dispatch, registry identity and preserved instance across feature clones, and actual service rejection of raw caller metadata without a VerifiedCaller. The service checks use the actual PgNativeAbortResolver and DeliveryService. The injected negative-only child adapter can return only Unavailable and is never dispatched by these tests. No successful owner/native double or public test-only authority constructor is added.

These tests deliberately do not certify database persistence, owner acquisition, typed native settlement, strict mTLS startup, concurrent recovery or main integration. The existing physical recovery cohort and Root's combined actual runtime validation retain those separate obligations. Scope checks require the config module delta to remain exactly one declaration, and all other changed paths to match this assignment.

Executed source checks: `rustfmt +1.88.0 --edition 2021 --check` on the two new Rust files passed with exit 0; `git diff --check` passed with exit 0. Receiving-owner checks confirmed the four exact assigned paths, byte-exact preservation of config/mod.rs apart from its single granted declaration, and one construction of each ledger/registry/publisher in the core. These are source checks only; neither the three Rust tests nor a compiler/native command has run.

Planned fresh validation after an explicit target allocation: Rust 1.88.0, locked/offline dependencies, at most two jobs, debug information and incremental compilation disabled; compile `cargo +1.88.0 test --manifest-path backend/Cargo.toml --locked --offline -j 2 -p concert-service --test canonical_runtime_config --no-run --message-format=json`. Freeze the exact executable outside the allocated target and release the target before running its three tests. A later combined native run must use Root's actual shared live composition; a constructor-only PASS cannot certify that gate. Record the final source commit, complete source pins, Cargo exit code, frozen executable hash, actual test results and release receipt without relabeling NOT_RUN work.

Required validation, production integration, master delivery and the full persistent goal remain open.

## Root integration follow-up

Root imported CR07 as `c7edc116` and refactored the Participant factory to use
the core for its existing registry, delivery publisher, Auth/Friend fence and
Participant-specific settlement service. The returned runtime exposes that
same core and fence for later feature composition. Combined compilation and
serving validation of this refactor are still NOT_RUN. The C03-only child
settler remains explicit and rejects every other kind; no public/Gift/Social
recovery, native resolver union or child worker activation is claimed.

## Focused constructor execution at Root d0f063d1

Root's seven-artifact cohort includes the byte-identical CR07 core and constructor test entrypoint at `d0f063d18a0549935f80a536077c30962d89f9e9`. Independent Git/hash comparison confirmed the core SHA-256 `41eae42a5e3d75edfd5190939fb7a56596148bfb6f8fb25f2785186499a6cabd` and test SHA-256 `280fb46ee48154a9adf5360f2b1713ef54c0588e4818a8648ce9a6ea159dd1a2` are identical to the original `73082c9b` handoff. A separate historical source rebuild is unnecessary.

After the named MEDIA target was unconditionally released with zero handles and no compiler processes, Root executed all three copied constructor tests: exit 0, 3 passed, 0 failed, 0 ignored. The frozen `canonical-runtime-tests` executable is 14,194,240 bytes with SHA-256 `8c67132526dc7f80303134e3d9b011b71ee1e0f27d6edb166c768bbd4caa92b2`. Exact focused-test log SHA-256: `c55d4afd84a8686fceda77534d9958a0e31f830a9436003f5b5f5855831c7a04`. Target release receipt SHA-256: `86d25739efa00dcbe6ba9bde7aeac0f6780163c069219c037335764352a66556`. The implementation owner independently rehashed the copied executable and test log and checked their linkage to the released artifact manifest.

This is actual focused constructor/negative-caller evidence under Root's revision, not a claim that `73082c9b` was compiled or that production recovery is activated. The same cohort's first actual cancellation native case failed before producer work on a reserved owner-policy validation mismatch; its exact owned fixture cleanup passed. Read-only source inspection found the actual Auth/Friend owner policies already include Notification in the same order as the platform policy, and the fixture delegates the production guarded server rather than declaring its own caller list. Frozen Cargo JSON marks the test-profile platform library fresh while rebuilding both actual owner libraries; the production-main build uses a different platform artifact. Stale linked dependency metadata is a concrete repair candidate, not yet a proven cause at this checkpoint. No production authentication rule, owner caller permission, or fixture allowlist is weakened.

A repaired coherent cohort and its actual native receipts remain required. Record any fresh constructor rerun as a new exact artifact/receipt rather than upgrading this first cohort's failed native result. The full canonical objective still requires the typed cross-producer composition, native resolver union, main worker supervision and scoped validated master delivery described above.


## Fresh constructor rerun after platform repair

The repaired seven-artifact cohort qualifies the three constructor tests at unchanged Root source `d0f063d18a0549935f80a536077c30962d89f9e9`. Earlier NOT_RUN authoring statements and first-cohort failure discussion above remain historical checkpoints. The core and test entrypoint remain byte-identical to the original `73082c9b06f1aef7481d9069eab563a8ece77b09` handoff, with the source hashes recorded above.

Root used the named `ROOT-SEVEN-CACHE-REPAIR-D0F-20261104` allocation and cleaned only the `platform` package. Source revision and bytes were unchanged. The repaired first test compile records platform and the actual Auth/Friend libraries as `fresh: false`. Its observer raced deletion of an ephemeral compiler file; that interrupted attempt receives no validation credit. Root stopped only its owned process group 7571, preserved the failure, repaired the observer and retried within the same allocation. The successful retry reused artifacts rebuilt during that same repair attempt, exited 0 in 22.5 seconds and did not reuse the original stale platform artifact. Debug information and incremental compilation were disabled. No owner policy or fixture permission changed.

All seven artifacts compiled successfully and were copied outside target. Target release at `2026-11-04T05:34:09.719291+00:00` recorded zero open handles and no compiler processes before copied tests or PostgreSQL startup. No resource abort occurred. Independent implementation-owner readback compared all 2,151 frozen source hashes to the exact Git tree, checked the 1,515 core checkpoint hashes as its subset, and rehashed all seven executables. This required no target access or separate allocation.

The original combined cohort's pre-producer owner-policy failure is preserved. Its test-profile platform artifact was marked fresh while rebuilding the owners; main used a different platform artifact. Package-only repair and the later nine-case native PASS resolve that exact mismatch without weakening production policies. Source pins alone do not prove every cached dependency's compiled revision. See `canonical-cancellation.md` for the exact command and native case/clean/observer receipt digests.

After release, `canonical-runtime-tests --test-threads=1` exited 0: three passed, zero failed, zero ignored. The actual tests were `core_factories_preserve_actual_verified_caller_requirements`, `feature_clones_share_the_existing_registry_and_instance`, and `nil_instance_is_rejected_without_database_or_child_dispatch`. The copied executable is 14,194,240 bytes, SHA-256 `466ad9550b0968729970b23d05fc7782ce0d17ec74f11885f18128377c31b635`. Exact focused log SHA-256: `f08ac9455bd18b2e277457fc937fbe166c0ca082869bef9763b3e7ef266d7d48`. Both were independently rehashed and matched the released artifact manifest and execution receipt. No separate historical CR07 rebuild was needed.

Portable receipt identities from cohort `mcr-root-seven-repair-d0f063d1-5fp4vkh7`:

| Receipt | SHA-256 |
| --- | --- |
| `source-pins.json` | `da1057e2e3a5d77292e91926cc006f3aba6a8c6e2fdf3b0561e888383ff08692` |
| `concert-tests-2-receipt.json` | `8375a8c94f2a84e03d429f4917ffc207d1a2f31c1970176d78f95a61776aa7bf` |
| `target-release.json` | `2e7185253fd539348ebb89c4256cff724407bcf8513ab6916c266d8cdd3f49db` |
| `pure-test-receipts.json` | `9192dc66934e5d5d7fb05d29862d20dc0d16d214b079a96d23ef49375aa314da` |

The repaired cohort also passed all nine actual PG16/mTLS core cancellation cases, each with owned cleanup PASS. The repaired Participant C03 attempt stopped during migration preflight: retained expectations used Auth 17 while the current manifest contains 18. Actual protected HTTP cancellation was NOT_RUN, prepared count was zero, and owned cleanup passed. These core results do not qualify C03. Typed cross-producer composition, native resolver coverage, supervised child recovery in production main, browser flow and scoped validated master delivery remain open.
