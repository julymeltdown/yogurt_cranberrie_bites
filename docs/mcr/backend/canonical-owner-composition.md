# CF01 canonical owner composition

Status: IN_PROGRESS (implementation and scoped native qualification PASS; integration/master delivery pending).

## Contract and ownership

Base: `591d908a85a01fe0a494e04c0109bb0bec64a3e1`. The change owns only
`config/canonical_runtime.rs`, `tests/canonical_owner_composition.rs`, its new
support directory, and this document. Existing `CanonicalRuntimeCore` APIs are
preserved. No manifest, lockfile, protocol, migration, router or main changes.

`CanonicalOwnerComposition::compose(pool, release, config)` accepts exactly one
supplied `PgPool`, a real `Arc<ValidatedProductRelease>`, and typed
`CanonicalOwnerConfig`. Configuration supplies the producer UUID, retained child
keys, Gift pickup keys, mandatory Auth/Friend/Profile/Notification endpoints,
Concert service credential, TLS material and relationship signer. There are no
successful defaults or optional recovery owners.

Construction order is the cycle break: `PgNotificationBindingReader` receives a
clone of that pool and the same instance **before** any ledger exists. The real
PublicProfileReader (including selected release artwork validation),
GiftPrivateRuntime, SocialReadSettler and NotificationClient feed the closed
CompositeChildIntentSettler; one CanonicalRuntimeCore creates the ledger,
registry and delivery publisher; the actual CanonicalOwnerFence uses that same
registry. Returned fields retain typed owned adapters/core/fence for later
producer composition. Credential/key holders have no Debug implementation.

Composition creates no positive scope, does not reacquire expired authority,
starts no worker and activates no route. Root owns main mounting, the native
resolver union, producer composition and supervised recovery. Notification
binding reconstruction remains completion-only; its query must match the
original finished-root row for this exact instance.

## Native evidence

The exact target `canonical_owner_composition` has two ignored native cases,
run explicitly by `canonical_owner_composition_support/qualify.py` using only a
frozen committed executable and source pins. Each uses a new marker/inode-owned,
socket-only PostgreSQL 16 cluster, five isolated databases and actual mTLS
Auth/Friend/Profile/Notification services. Fixture copies and source hashes are
recorded in `canonical_owner_composition_support/provenance.json`.

1. Real canonical scopes and actual Profile/Local native children retain all
   four prepared exclusions through inner serialization and unread outer-body
   Drop; real retained body EOF invokes authenticated typed delivery, releases
   children/owners and remains idempotent.
2. Notification execution without a real target provider refuses. Its actual
   client has already persisted the private recovery reference. A negative-only
   completion fault retains the finished root and pending intent. A fresh
   factory, after the original expiry, uses the actual same-instance reader and
   closed composite worker to complete the real Notification durable
   close-before-dispatch NoEffect proof. Wrong instance/reference/secret and
   missing fresh-registry authority remain negative. Original delivery success
   still requires the original registry and exact terminal capability.

Both cases passed at the exact source below. They do not
certify Gift/Social business flows, public Notification target-policy/inbox
acceptance, external Gateway HTTP, independent owner process survival or full
production main composition. Missing target policy must remain unavailable.

## Executed checkpoint and review

Tested source: `0138dbe5f94997c6393c3b4f0851ace441afe42e`, comprising
`6a6fa6f75d96b3ef23c70cd2d8edef054c60cbef` (implementation/native fixture)
and `0138dbe5f94997c6393c3b4f0851ace441afe42e` (qualifier child termination).
Final provenance/receipt/documentation edits do not change compiled Rust.
A read-only peer review of the factory, ownership cycle and fixture found no
actionable issues. It ran no build or native resources and is not a runtime gate.

Executed commands and exit codes:

- `rustfmt +1.88.0 --edition 2021` over the assigned Rust paths: 0.
- `git diff --check`: 0.
- `python3 scripts/mcr/check-plan.py`: 0 (planning/provenance only).
- Exact package-only `cargo +1.88.0 clean -p concert-application` against the
  authorized shared target: 0; 9 files / 28.1 MiB removed. Previous writer's
  IntroError::Expired source SHA `2a6f1d0a58dca05fe749da25bcc684b4c3c153ccde22bd8ca476399ea0723f23`
  differs from this source's `d3f518330e277242a16ff8173b6c7b62ff7a3a805e4a0aaf61548ec3c44ba3d4`.
  This was an exact cache repair, not an intake of unrelated source.
- `cargo +1.88.0 test --manifest-path backend/Cargo.toml --locked --offline -j 2
  --config profile.dev.package.contracts-build.opt-level=2 -p concert-service
  --test canonical_owner_composition --test canonical_runtime_config --no-run
  --message-format=json`: 0 / 38.17 s. Debug 0, incremental disabled.
- Frozen `canonical_runtime_config-tests --test-threads=1`: 0; **3 passed**.
- `canonical_owner_composition_support/qualify.py` with the frozen committed
  checkpoint, explicit PG16 binary directory and new private report: 0;
  **2 actual native cases passed**, 1.217 s and 6.919 s respectively.
  The entire guarded runtime completed in 13.76 s.

Exactly 3,521 build/runtime/art/release source pins match Git blobs at the tested
commit, with zero mismatches. The frozen binary contains this worker's actual
baked Concert manifest directory. Live SQL migration checksums matched all
Auth **18**, Friend **18**, Concert **37**, Profile **9**, Notification **6** files
in both owned clusters. Every owner database had zero prepared transactions at
the end of each case, before cleanup.

Frozen factory-test executable: 49,871,728 bytes,
SHA-256 `82e22c20a3d62c99b20b8c083d2a110c353b3042415fc98b5edeb9794cd30e34`.
Frozen legacy core test executable: 14,194,336 bytes,
SHA-256 `00e5a451cf5c71d2edfa13ddf786a468e2931f34dc1449205d582dcdbb1fec5d`.
Portable qualification metadata is retained in
`tests/canonical_owner_composition_support/native_qualification.json`.
Raw checkpoints/logs/private native references remain outside the repository.

The target was released with zero open handles at
**2026-11-04T07:08:03.725063Z**, before any copied test or PostgreSQL launch.
There was no later target access. Full copied workflow release at
**2026-11-04T07:09:17.411420Z** confirmed both owned roots absent and the runtime
process group gone. Maximum whole additional allocation was **246,136,832 bytes**,
including **104,558,592 bytes** runtime growth, within one **2 GiB** hard bound.
Owned-cluster removal is cleanup evidence, not a substitute for the zero-prepared
native settlement assertions made before removal.

## Delivery boundary

Root may integrate the serial committed handoff without a lead approval gate.
The assignment changes no main mounting, producer/native resolver union or
supervised child worker startup. Full canonical-owner authority completion and
master readback remain separate obligations; this scoped factory pass must not
close that larger persistent goal. The missing real Notification root target
policy continues to return Unavailable, including for an empty inbox/count.
