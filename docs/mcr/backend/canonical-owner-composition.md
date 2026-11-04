# CF01 canonical owner composition

Status: IN_PROGRESS. Compilation and native qualification: NOT_RUN.

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

## Planned native evidence

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

These are source plans until commands and exit codes are recorded. They do not
certify Gift/Social business flows, public Notification target-policy/inbox
acceptance, external Gateway HTTP, independent owner process survival or full
production main composition. Missing target policy must remain unavailable.
