# CF02 shared Participant and typed Product owner assembly

Status: IN_PROGRESS. Source implemented; build/native validation NOT_RUN.
This component does not activate production mounting or accept full C03/C04/IN01.

## Ownership and base

Task: `CF02-SHARED-PARTICIPANT`. Isolated branch
`codex/mcr-be-CF02-shared-participant`, worktree basename
`be-shared-participant-composition`, exact base
`f4349d5a1884c451cabdefc6462fe8e82e6d84fd`. Root transferred exclusive existing
writes to `config/canonical_runtime.rs` and `config/participant_runtime.rs`.
The only additive paths are `tests/shared_participant_composition.rs`,
`tests/shared_participant_composition_support/{mod.rs,fixture.rs,environment.rs,owners.rs}`
and this document. Existing CF01 and restore worktrees stay read-only. No root
main/config module, native-resolver dispatch, proto, SQL, registry, manifest,
lockfile, frontend or central recovery script is changed.

The original persistent canonical-owner goal is aligned and reused. Its actual
metadata remains historically BLOCKED because the tool cannot resume it; the
explicit assignment resumes source execution. This component does not falsely
complete or replace the larger production goal.

## Origin-bound API

```rust
ParticipantRuntimeSettings::compose_shared(
    self,
    pool: PgPool,
    release: Arc<ValidatedProductRelease>,
    additional: ParticipantProductOwners,
) -> anyhow::Result<SharedParticipantRuntime>
```

`ParticipantProductOwners` contains only explicit `profile_endpoint`,
`notification_endpoint` and typed `Arc<PickupKeys>`. Auth/Friend endpoints,
producer UUID, active plus retained `Arc<ChildRecoveryKeys>`, credential, TLS
and relationship signer come from the same private validated Participant
settings. Public Profile must be the exact configured SELF Profile endpoint;
additional endpoints require explicit HTTPS host/port without credentials,
query or path. Release digest must match the validated actual product release.

The factory calls the existing CF01 `CanonicalOwnerComposition::compose`, not
an alternate child implementation. Participant then receives clones of that
exact core/registry/ledger/delivery and `Arc<CanonicalOwnerFence>`. Thus its
actual PublicProfile, GiftPrivate, SocialRead and Notification adapters and
composite settler remain shared with Product callers. There is no public API
for injecting an unrelated core/fence/keyring/resolver/pool bundle.

The single returned `participant.settlement` uses the existing closed
`NativeResolverUnion`, constructed from the exact same pool/producer UUID.
Its constructor rejects closed or non-dedicated Concert pools. The base union
routes actual Participant, Social, Operations and Age evidence; unmatched
valid operations use abort-only native proof. That fallback is not a committed
witness. New Intro dispatch, production mounting, one-time settlement/delivery
registration and worker supervision remain Root's separately owned work.

`SharedParticipantRuntime` exposes `participant: ParticipantRuntime` and
`owners: CanonicalOwnerComposition`. Root mounts the Participant services with
the existing Gateway verifier; Decision uses the separate decision verifier.
Root mounts `participant.settlement` and one shared Delivery service exactly
once. The factory starts no listeners or workers.

## Standalone and key-role preservation

Existing `compose(pool, release)` retains the standalone core,
`ParticipantOnlyChildren` quarantine and `ParticipantNativeResolver` behavior.
Both modes call one private assembly method for actual canonical Auth reads,
SELF Profile client, AvatarPersistence, ProfileJournal, Decision service,
retained delivery and ParticipantRecovery. No fake successful owner is added.

Environment `load` and explicit `from_validated` use the same private validation
path. The explicit constructor takes closed parsed ParticipantSettings, typed
TLS/Gateway/relationship/SELF material and dedicated decision public keys.
It constructs ParticipantDecisionVerifier against that exact Gateway verifier;
a prevalidated decision verifier from a different Gateway cannot bypass key
role separation. Outbound relationship/SELF keys also cannot overlap Gateway
keys or each other. Parity identities remain refused. Construction changes no
environment variable and does not connect to a database or start a server.

## Native fixture plan and honest limits

The new ignored native integration host is
`shared_participant_core_and_real_profile_native`. Its controller must supply
one exact marker/inode-owned socket-only PG16 cluster and dedicated local
addresses, certificates and dedicated fixture keys. It applies actual owner
migrations, seeds explicit test identities/admissions/selected avatars, and
starts actual Auth relationship/owner, Friend owner, Profile SELF/public/settlement
and Concert decision/closed-union settlement transports with production guards.
The source-bound selected release constructor is reused read-only from CF01.

The host is designed to check:

- Actual Participant Get, canonical Auth/SELF reads and retained inner EOF;
  exact pointer identity of its actual scope in both consumer registries.
- A test-owned additional callback using the real typed PublicProfile reader
  on that exact Participant scope; real Auth/Friend/Local/Profile prepared work.
- Inner serialization alone retains ownership. Actual retained outer body EOF
  uses guarded typed CompleteResponse. Negative Profile Complete retains its
  exact pending intent after earlier owner/Local settlement.
- Original expiry refuses fresh read authority. A different configured keyring
  cannot decrypt/ACK the original intent even while Profile is available. The
  actual shared composite recovery settles it idempotently, preserves original
  ciphertext and permits original delivery retry without inventing a new scope.
- Actual coordinated Profile PATCH exercises distinct SELF/Decision signatures,
  ProfileJournal/native decision and the existing closed union. Standalone C03
  Get and terminal delivery still work after the shared assembly stops.
- Wrong release, mismatched SELF/public owner, unsafe Notification endpoint,
  foreign/closed pool and invalid decision-role keys fail construction.

The test-owned additional public-profile callback is composition evidence,
not a claim that the public C03 response now exposes arbitrary-subject content.
Gateway HTTP/browser, independent owner processes, deployment activation, all
Product operations, complete Intro dispatch and broad production acceptance
remain NOT_RUN. No fallback provider or positive fake settlement is used.

## Planned finite validation and provenance

No Cargo/shared target/PG/browser/runtime launch is authorized until a separate
named resource offer from the allocator is accepted. Only the new integration
target and existing `participant_runtime_config` target are proposed, Rust1.88,
locked/offline, at most two jobs, debug 0/incremental off and retained
contracts-build opt-level2. Freeze exact executables, source/runtime pins and
live baked migration inputs; honor the offer's target-release, byte/time/free
space and owned-cluster cleanup boundaries. Commands/exits/evidence will be
recorded after actual execution, without converting static checks into runtime
acceptance.

Adapted/read-only source provenance follows. All source revisions below are
the exact base `f4349d5a1884c451cabdefc6462fe8e82e6d84fd`; unchanged existing
modules are compiled by path without modifying them. Dependencies are only the
existing locked Concert dev dependencies and standard-library private runner
plus existing owned-PG helpers. No manifest/dependency installation is needed.

| Source under `tests/` | SHA256 | Destination under `tests/` | Changes / reuse |
| --- | --- | --- | --- |
| `canonical_owner_composition_support/environment.rs` | `7d1b8a3a51be7c63a6aaeaa0a82a25dbb62f9e7ae46737a89ea0905166dd49f6` | `shared_participant_composition_support/environment.rs` | Exact owned identity guards and migration inputs; CF02 dedicated database names/addresses |
| `canonical_owner_composition_support/owners.rs` | `80a792190a04f984f1abcd7be3310d6f94b7c6c07e6a1ebfd454dcbee723f3c7` | `shared_participant_composition_support/owners.rs` | Actual owner guards; added real Auth relationship and Profile SELF/settlement services |
| `canonical_owner_composition.rs` | `10488ad03bfbe5466dfd7b93a73eebf27966d77cd5edb8195eb7e02a4760dce8` | `shared_participant_composition.rs` | Typed retained-body terminal helpers; new native same-scope and standalone scenarios |
| `canonical_owner_composition_support/capture.rs` | `8226908b6566e22896e254dcd626071364e6df8757c9c8ba6ae4db238e488385` | `shared_participant_composition_support/mod.rs (read-only path module)` | Actual signed Gateway parent/gateway key helper |
| `canonical_owner_composition_support/release.rs` | `6e0ca735e4dcddc07b7ca60114c20e520823e9338877cd55cfacea0decdb2df8` | `shared_participant_composition_support/mod.rs (read-only path module)` | Exact source-bound selected release constructor |
| `process_child_recovery_support/fault.rs` | `6b6f48654e82427875ae5e2f433e038ba7fd824de94eddd18a0f8b9c9468293b` | `shared_participant_composition_support/mod.rs (read-only path module)` | Unchanged exact negative-only Profile Complete wrapper |
