# CF02 shared Participant and typed Product owner assembly

Status: CONTRACT_READY. Source and bounded component/native validation complete.
Normal Root publication and independent publication readback remain pending.
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
`ParticipantOnlyChildren` quarantine and dedicated `ParticipantNativeResolver`
mutation witness behavior. A private closed standalone dispatcher sends only
the exact `GetParticipant`/`getConcertParticipant`/`GET`/event-`me` tuple to the
same-store `PgNativeAbortResolver`. The prior standalone adapter refused that
read transaction, leaving actual terminal delivery unresolved even though its
read work rolled back. The abort-only adapter cannot certify COMMIT; all other
tuples still go to the dedicated Participant resolver. Shared mode continues
to use the existing full closed union.
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

## Native fixture and honest limits

The new ignored native integration host is
`shared_participant_core_and_real_profile_native`. Its controller must supply
one exact marker/inode-owned socket-only PG16 cluster and dedicated local
addresses, certificates and dedicated fixture keys. It applies actual owner
migrations, seeds explicit test identities/admissions/selected avatars, and
starts actual Auth relationship/owner, Friend owner, Profile SELF/public/settlement
and Concert decision/closed-union settlement transports with production guards.
The source-bound selected release constructor is reused read-only from CF01.

The passing Round3 host checked:

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
  Get, terminal delivery and coordinated PATCH retain actual abort/commit proof
  after the shared assembly stops. The fixture gracefully stops listeners and
  restarts actual owners before the standalone instance, ensuring fresh owner
  settlement channels rather than cached dispatch into the old producer.
- Wrong release, mismatched SELF/public owner, unsafe Notification endpoint,
  foreign/closed pool and invalid decision-role keys fail construction.

The test-owned additional public-profile callback is composition evidence,
not a claim that the public C03 response now exposes arbitrary-subject content.
Gateway HTTP/browser, independent owner processes, deployment activation, all
Product operations, complete Intro dispatch and broad production acceptance
remain NOT_RUN. No fallback provider or positive fake settlement is used.

## Finite validation and provenance

Three separately named resource rounds were used. Each froze exactly two test
executables outside the shared target before zero-handle TARGETRELEASE; no
copied test or owned PostgreSQL process ran before that release. There was no
after-release target access. The executable/source freeze for the successful
round is `2d48127b8c629b133b967455bb6729e21db95358`. The following documentation
commit changes no compiled source or dependency input.

| Round | Frozen source | Build / regular | Actual native result |
| --- | --- | --- | --- |
| 1 | `063186f31db4a08b688ba1f45458f7c43de047b3` | Exit 0 / 8 PASS | FAIL. First private URL helper rejected dedicated names containing digits, cleaned up; corrected private adapter then launched host, which lacked inbound caller deadline and failed gRPC 3. Both attempts retained. |
| 2 | `f318582a860957e463b66372169a50bb75a8982b` | Exit 0 / 8 PASS | FAIL. Shared identity, typed Profile child, expiry/key refusal, common recovery, idempotent EOF and coordinated PATCH assertions passed; final standalone Get terminal assertion failed. No whole-native acceptance claimed. |
| 3 | `2d48127b8c629b133b967455bb6729e21db95358` | Exit 0 / 8 PASS | PASS. One actual native test, exit 0, including shared and standalone Get/terminal and coordinated PATCH. |

Round1's missing deadline was fixed by the actual `CallDeadlineLayer` and
propagated remaining-time header in `f318582a`. Round2 led to the narrow
standalone native-read abort dispatcher and explicit graceful owner/listener
restart in `2d48127b`; dedicated mutation witness validation remains unchanged.
Targeted source review of the final production/lifecycle fix reported no
actionable findings. Review was source-only, not runtime evidence or an approval
gate. All corrections stay within the exact assigned eight paths.

Round3's first admission failed the fresh 16 GiB gate before any target/job
access; an incorrectly premature coordination ACCEPT message was immediately
corrected. The same offer was actually accepted at
`2026-11-04T09:53:15.082865Z`, with 17,405,906,944 bytes free. Its clock and byte
baseline were not reset. Carry was conservatively charged at 363,147,264 bytes,
including the entire earlier measured peak plus source/helpers/evidence.

Actual build command (existing lockfile, offline Rust 1.88, two jobs;
`MCR_BACKEND_TARGET` identifies the allocator-leased backend target):

```sh
CARGO_TARGET_DIR="$MCR_BACKEND_TARGET" \
CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 \
cargo +1.88.0 test --manifest-path backend/Cargo.toml --locked --offline -j 2 \
  --config profile.dev.package.contracts-build.opt-level=2 -p concert-service \
  --test shared_participant_composition --test participant_runtime_config \
  --no-run --message-format=json
```

Cargo exited 0 in 24.52 seconds; full executable freeze took 25.29 seconds.
All 3,608 tracked source/runtime/migration inputs matched their Git blob IDs,
with zero mismatches. Copied artifact SHA256 values:

- `shared_participant_composition`: `5f2e76bd7bbb3cd15be388fe57c60e387395ffa59dbcb10836688a6849fe9cb2`.
- `participant_runtime_config`: `5c23139a7a26ddfe0c3daf060ec04835fc3251594e3b12c715eaaad6447c4ced`.

The copied configuration executable ran with `--test-threads=1 --nocapture
--format terse`: exit 0, eight tests passed. The native executable ran with
`--ignored --exact shared_participant_core_and_real_profile_native
--test-threads=1 --nocapture --format terse`: exit 0, one test passed in 3.95
seconds (host 4.165 seconds). The private controller supplied one socket-only,
marker/inode-owned PostgreSQL 16.14 cluster with four exact databases and real
mTLS service transports. Migration ledger counts/hashes matched Auth 18,
Friend 18, Concert 38 and Profile 9. Both actual coordinated writes produced
`COMMIT` decisions and `COMMITTED` Profile terminal receipts. All four databases
had zero prepared transactions before cleanup; teardown is not settlement proof.

TARGETRELEASE was `2026-11-04T09:54:18.866939Z`, lsof exit 1 with empty output
and no open handles. FULLRELEASE was `2026-11-04T09:55:10.807360Z`: owned host,
postmaster, controller, process group and cluster root independently absent.
Measured whole-round peak, including all conservative prior carry, was
519,348,224 bytes versus the 2 GiB cap; native runtime peak was 96,485,376 bytes.
The copied-test/native guard completed in 8.42 seconds. Acceptance through
FULLRELEASE took 115.73 seconds, within the 1,200/750/450 second windows and
reserved cleanup time.

Private local receipts are under `/private/tmp/mcr-cf02-round3-ce0dymlk`:
`checkpoint.json`, `source-blob-readback.json`, `native.json`,
`runtime-guard.json`, `full-release.json`, bounded logs and
`evidence-sha256.json`. Earlier failures remain under
`/private/tmp/mcr-cf02-k28emvai` and `/private/tmp/mcr-cf02-round2-ntdvlzaq`.
No private credentials, keys, SQL data or generated native binary are committed.
Key evidence SHA256 values:

- `native.json`: `f97bd5046e46b569288b222c2601a38689468c043062660ece0f97462c8203c8`.
- `native-host.log`: `6b7e0287c5b679376807bd8e2ee88664da1559b86bc00ec5645063e9d2df9b85`.
- `full-release.json`: `320d1942aea94cc8cc43e79a491da90691a67e20f532d4f7be7c07a83ea1aea0`.

Final `rustfmt +1.88.0 --check --config skip_children=true` for all seven
assigned Rust files, `git diff --check`, and `python3 scripts/mcr/check-plan.py`
exited 0. The planning check confirms original archive/provenance preservation;
it does not certify application runtime or deployment.

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
