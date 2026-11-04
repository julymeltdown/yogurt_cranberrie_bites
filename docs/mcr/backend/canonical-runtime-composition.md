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
