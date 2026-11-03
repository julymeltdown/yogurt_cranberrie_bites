# Participant producer — Phase A

Status: **IN_PROGRESS**. This bounded slice supplies an isolated guarded producer
and default-unavailable authority ports. It does not register a product listener,
Gateway route, real owner client, or mutation capability. Whole BE07 and IN01
remain open.

Base: `03fa2cdeeb70669ca35f8393a3532971c48fbccf`, branch
`codex/mcr-be-participant-producer`, isolated checkout
`/Users/lhs/Desktop/mcr-worktrees/be-participant-producer`. Its BE07 persistence
ancestry is the validated stack integrated as `04c928a` -> `5b7116b` -> `03fa2cd`.
Read the active specification (sections 5.2–5.3, C03/C06 and BE04/06/07), project
assessment, execution plan, AGENTS and parallel delivery authority before work.

## Producer and wire

The new internal package is `concert.participant.v1`. Its
`ConcertParticipantService` has exactly these methods:

| RPC | Exact public target bound into its signed request | C03 response |
|---|---|---|
| GetParticipant | GET `/api/concert/events/{canonical_event_uuid}/me` | PrivateParticipant |
| SetAvatar | PUT `/api/concert/events/{canonical_event_uuid}/me/avatar` | AvatarState |
| UpdatePreferences | PATCH `/api/concert/events/{canonical_event_uuid}/me` | PrivateParticipant |

`backend/proto/concert/v1/participant.proto` is an internal request envelope,
not a replacement public contract. ParticipantRequest field numbers are:
user_id=1, event_id=2, session_family_id=3, request_id=4, http_method=5,
http_target=6, request_json=7, release_digest=8,
authority_expires_unix_micros=9, auth_observed_at_unix_micros=10,
profile_observed_at_unix_micros=11. ParticipantResponse.response_json=1 contains
the frozen C03 JSON. The complete protobuf body and exact RPC path are signed
by existing actor delegation. No access token, secret, role or duplicated
identity store is added.

`ParticipantService::new(store, auth, profile, mutations)` accepts explicit
Arc ports. `ParticipantService::default()` makes every dependency unavailable.
`server()` returns the generated server with 16 KiB encode/decode bounds.
Registration must use `platform::grpc_method_policy::guard(&participant::POLICY,
service.server(), callers, actors)`. The complete policy permits only Gateway
for all three methods and binds actor field 1. Direct handler calls without the
sealed workload/signature context are denied. No listener is registered here.

The private-constructor VerifiedParticipantRequest checks canonical nonnil UUIDs,
exact actor/user match, session/request UUIDs, method/target equality without
queries, exact bounded raw JSON and current release identity. GET rejects any
body. Avatar JSON uses the existing strict C03/C06 decoder; preference PATCH
uses the exact preference-only decoder. Unknown fields, duplicate fields,
URLs, unsupported versions/types and payloads above 4096 bytes are refused
before owner/store dispatch. Raw request bytes remain unchanged and are exposed
with their SHA-256 to the mutation-authority adapter.

The signed observation and original deadline bounds can only shorten the
captured request authority. Auth, Profile, capability and persistence awaits
share this same monotonic stop through the existing deadline scope and timeout.
Checks run after every successful await, before persistence, through BE07's
per-await/precommit checks and before returning a response. Delayed errors and
successes never renew authority.

## Current owner and capability contracts

- CanonicalAuthPort returns CurrentAuth only after canonical Auth positively
  confirms an active account and the exact live session family. A JWT or signed
  read assertion alone is insufficient. Actor/session mismatches are refused.
- CanonicalProfilePort returns only the current canonical self user ID,
  display_name and optional bio. A failed Auth gate prevents its dispatch.
  Profile identity, C03 bounds and control characters are checked; no legacy
  avatar URL, handle, email, provider claim or inferred age is serialized.
- MutationAuthorityPort returns a separate MutationPermit. Every write requires
  its check against the exact verified actor/event/session/request, operation,
  target, raw body digest, release and current server mutation capability.
  Its check is repeated during native persistence. VerifiedReadAuthority is
  used only to preserve signed transport time; it never supplies this permit.
- ParticipantStore is implemented by the already validated AvatarPersistence.
  Reads do not create participant defaults. The store's release digest must
  match before any owner is called. BE07 retains event/admission locks,
  optimistic revisions, inventory validation and rollback behavior.

These ports are required integration contracts, not implemented live owner or
capability clients. CurrentAuth/Profile observations are internal server values,
not portable browser grants. Their owner freshness/expiry and the original
request bound are rechecked together. Replacing these ports with fixture grants
in product composition would violate this handoff.

Reads join persisted participant/avatar state with fresh canonical Profile and
return `age_status: UNKNOWN`; no age authority exists in this producer. Avatar
success must contain the exact submitted config and expected revision + 1.
Preference success must contain the exact submitted preferences and participant
revision + 1. Foreign store identities, malformed responses, extra/private
response fields and inconsistent successful writes fail closed. Actual response
JSON is validated against the bundled frozen C03 schema with format checks.
Responses carry `cache-control: no-store`.

Only the preference subset of UpdateParticipantRequest is supported. Valid
C03 requests containing display_name or bio are explicitly rejected by this
RPC rather than ignored. Full PATCH requires a separately owned canonical
Profile mutation/revision protocol; cross-service atomicity is not claimed.
Neither preference changes nor this producer automatically replaces an avatar.
World avatar-delta publication belongs to the separately assigned world owner.

## Errors and activation

Safe failures contain no SQL/owner diagnostics or private state. Handler errors
include static `x-mcr-core-reason` metadata for the later Gateway normalizer:

| gRPC status | C03 reason / HTTP mapping |
|---|---|
| InvalidArgument | INVALID_INPUT / 422 |
| Unauthenticated | AUTHENTICATION_REQUIRED / 401 |
| PermissionDenied | PERMISSION_DENIED / 403 |
| NotFound | RESOURCE_UNAVAILABLE / 404 |
| Aborted | VERSION_CONFLICT / 409 |
| FailedPrecondition | EVENT_READ_ONLY / 409 |
| Unavailable | DEPENDENCY_UNAVAILABLE / 503 |
| DeadlineExceeded | DEADLINE_EXCEEDED / 504 |
| Internal | INTERNAL_ERROR / 500 |

Failures generated before handler dispatch by the platform guard still require
Gateway normalization. Root transport must normalize its own body/media errors
and use the safe C03 envelope. A response/authority error after commit is
uncertain; no automatic write retry is permitted. The authorized GET can
reconcile a revision conflict without replaying the previous mutation.

Product activation requires a later exact grant for startup/config, configured
canonical owner and mutation authority adapters, generated Gateway client and
HTTP handlers. Existing BootstrapService, concert.proto, rpc.rs, Gateway,
configuration and main are unchanged by this assignment. Production art and
release qualification remain unavailable; metadata fixtures never enable them.

## Provenance, ownership and checks

All new producer, proto and tests are newly authored from the frozen contracts.
Dependencies are existing platform workload/actor/deadline guards, BE07
AvatarPersistence and C03/C06 decoders, and the bundled C03 OpenAPI schema.
There is no old rendering, UI, source asset or cross-directory frontend import.
The only shared changes granted by delivery are the service lib module export
and additive build.rs compilation of the new proto. Existing concert/post proto
compilation remains byte-for-byte unchanged; no manifest/lock migration changes.

Owned paths: new `backend/services/concert-service/src/participant/**`,
`backend/services/concert-service/tests/participant_producer.rs`, this document,
new `backend/proto/concert/v1/participant.proto`, service `src/lib.rs` export,
and the additive service `build.rs` step. The previous avatar checkout remains
stopped. The later user-authorized direct-delivery policy allows this owner to
non-force publish its qualified exclusive patch; kings and SLs are not review
queues. Shared-file ownership and physical build leases still apply.

Planning check and whitespace check: exit 0. Scoped Rust formatting: exit 0.
Exact eight-path/shared-change/import audit: exit 0. The retained vendored
`protoc-bin-vendored-macos-aarch_64-3.2.0/bin/protoc` compiled the new proto to a
descriptor and decoded its package, all eleven request fields and all three
methods successfully (exit 0), independently of Cargo.
At the original source checkpoint, compilation, guarded-RPC execution and the
native probe were **NOT_RUN**, pending the shared Cargo lease. The recovery
qualification below supersedes that historical state. Actual compile command:

```sh
env RUSTUP_TOOLCHAIN=1.88.0 \
  CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/media-combined-fa4b4b3/target \
  CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 \
  CARGO_INCREMENTAL=0 \
  cargo test --manifest-path backend/Cargo.toml --locked --offline -j 2 \
    -p concert-service --test participant_producer --no-run --message-format=json
```

The emitted executable is copied and SHA-pinned before releasing Cargo. The
native probe runs separately in the existing marker/inode-owned PG16.14 fixture,
using an explicit synthetic release and test-only owner/capability ports. It
exercises the real generated guarded RPC plus the real BE07 store, not live
Auth/Profile services or a browser network. This does not certify release,
whole BE07, the real multiuser world, or IN01/IN05.

## Recovery checkpoint — 2026-11-04

The preserved source checkpoints are `a2a60245` (producer), `f9e7323b`
(deadline/withdrawal probes), and `a4c4169e` (SQL-update rollback proof).
`dc11fe42` committed the previously dirty standalone-protoc evidence without
losing it. Merge `567d933f` imports fetched master `ce9e8e6c`; its shared-file
conflicts were resolved by preserving every upstream byte and adding only the
participant module export and participant proto compile step. Relative to that
merge parent, this slice still changes exactly the eight owned paths above.
The actual persistent Phase A goal remained ACTIVE through chat recovery.

Rechecked after the merge: `python3 scripts/mcr/check-plan.py`, scoped
Rust 1.88.0 `rustfmt --check --edition 2021`, `git diff --check`, and the exact
path/shared-content/import audit all exited 0. These do not establish Rust
compilation or executed RPC behavior. No participant compiler artifact, pinned
executable, or native result exists yet. The explicit shared-resource hold
reserves MEDIA for Friend RPC and BACKEND for World closure; no Cargo or private
PostgreSQL is started here until a named slot is released with at least 8 GiB
available. A process snapshot being idle does not transfer its lease.

Current C03 `AvatarConfig` is the layered 1.0.0 / whole-character 1.1.0 union.
The imported BE07 parser still supports only layered configurations at this
checkpoint. The selected-source owner retains avatar/domain/bootstrap and
migration 0025; this producer does not modify or duplicate their implementation.
Its complete current-union compatibility remains unqualified until their API is
available and the producer's actual guarded tests execute against it. Product
activation still requires qualified canonical authority and root composition
after this bounded Phase A assignment; default-unavailable ports do not satisfy
that remaining product scope.

The focused deadline matrix additionally covers slow mutation-capability awaits
and requires the exact `DEADLINE_EXCEEDED` reason after slow Auth/Profile calls,
including a noncooperative blocking Profile implementation. It must prove no
store dispatch. This source addition is also **NOT_RUN** until compilation.

The pending guarded whole-character projection test uses the exact selected
config golden at `contracts/fixtures/assets/selected-static/photo01_front_01.json`
and changes only its enumerated preset suffix 01..06. It checks PUT response,
authorized GET, preference-preserved avatar and structural mixed-field refusal.
Its in-memory store proves RPC/DTO compatibility only; it does not certify native
inventory validation or production-art admission. It intentionally depends on
the selected-source owner's canonical parser rather than a local replacement.

## First executed qualification

Source `08afce6c9a0b453d9c8748c3c7730fe099b4c445` compiled with the command above,
exit 0. The named MEDIA target was transferred by the Friend RPC owner and
confirmed by Backend king. Preflight found 32.64 GiB available, no target handles,
and no Cargo/rustc process. The exact `participant_producer` test executable was
selected from Cargo's `compiler-artifact` records, checking its target kind and
this checkout's service manifest path; `build-finished.success` was true. It was
copied before native execution and SHA-256 verified:

- Snapshot: `/tmp/mcr-participant-a2a6024/participant-producer-tests-08afce6c`.
- SHA-256: `332986c43ec7b68e63096bfe1cd6e5d10f24d1a524c4768e79b70803ab498dd3`.
- Artifact/pin receipts: `/tmp/mcr-participant-a2a6024/compiler-artifacts.jsonl`
  and `/tmp/mcr-participant-a2a6024/binary.json`.

| Executed command | Exit / result | Scope and limit |
|---|---|---|
| Copied test executable `--test-threads=1` | 101: 7 passed, 1 failed, 1 ignored | Seven existing guarded producer tests passed, including exact deadline/reason and slow capability checks. The whole-character matrix failed on preset 1 with INVALID_ARGUMENT because the canonical parser still accepts only layered input. No six-case success is claimed. |
| `python3 /tmp/mcr-participant-a2a6024/native_probe.py` | 0: native test PASS; cleanup PASS | Actual generated/guarded RPC with BE07 native persistence on PostgreSQL 16.14; synthetic release and explicit test owner/capability ports. |

The native test observed a nontransactional SQL sequence called by an AFTER
UPDATE trigger, then proved the avatar row remained revision 1 after permit
withdrawal. This demonstrates an executed update rolled back before commit.
It also passed saved revision 2, conflict -> authorized reread, preference-only
revision 2 with preserved avatar/current Profile, read-only write refusal with
read success, and suspended-admission read refusal. All current source SQL
migrations applied to a fresh private database; this driver does not qualify the
migration-runner ledger. The fixture used marker/inode ownership, Unix sockets
and no TCP. Cleanup removed its owned root; no participant PG process remained.

The complete result and migration hashes are preserved here:

```json
{
  "binary_sha256": "332986c43ec7b68e63096bfe1cd6e5d10f24d1a524c4768e79b70803ab498dd3",
  "cleanup": "PASS",
  "commands": {
    "0001_concert_core.sql": 0,
    "0002_command_receipts_outbox.sql": 0,
    "0003_places_map_releases.sql": 0,
    "0004_bootstrap_admission.sql": 0,
    "0005_world_admission.sql": 0,
    "0006_relationship_policy.sql": 0,
    "0007_community.sql": 0,
    "0008_meeting_beacons.sql": 0,
    "0009_receipt_domain_failures.sql": 0,
    "0010_feed.sql": 0,
    "0011_outbox_maintenance.sql": 0,
    "0012_media.sql": 0,
    "0016_meeting_deadline_guards.sql": 0,
    "0018_event_lifecycle.sql": 0,
    "0019_mural.sql": 0,
    "0020_mural_receipt_failures.sql": 0,
    "0021_moderation_operations.sql": 0,
    "0022_world_invalidation_receipts.sql": 0,
    "native_guarded_participant_persistence_and_reread": 0
  },
  "migration_sha384": {
    "1": "bf3d01697c1c6167fd999a92aa4323297f0e9c8b063176e2938f78dec8d1278f06cd84469d583a76314ac8aa520d8a5f",
    "10": "38cae9daee3e3cf739e231073b1dba3f3c7b5e7d1df5368aadbf0f98449dbf2d2b5c19012b660025fbd7c2215a6fd37c",
    "11": "f3d87a48a52e31d9da776de4bec34457b84c470488fc2ec7a78329c59442d7c57c9b70215413ca75d11008275551c779",
    "12": "26574dbb7376705544f63aaa0d0cf55e9208d6246217be62538b17588a9c6dbad95904adc69d3cc5fe338be10fba43cd",
    "16": "1f1828ec4601d862ad66738ffb2abe78b05b502930ef76e7449abf5fb1817cf245113385b3310690e1ef96f6ff23e3bc",
    "18": "194b87fd628249ea8be4c972c634857a8e266b8b9bc3f10cd3aa5ea6642f46dc2fcf0ac84c00127024b8514049f6b863",
    "19": "188f99ab9b21a516143afdc5bb7c81a77bb20571031635fbe2067a92586652ed9a6a0c9a6c326188a2c529210e7e4f67",
    "2": "6eae0492a1aedf4b2eaf30b44761a853376623d9cc65cc268941e416882998b40c46ff61f0acb8bd285be33a936f2e6b",
    "20": "b373bb67fe454a8ea3283e5ed66a2c3c201ebce1227b6493666b2858447c940d539a92910f79dbcafe77b2e276e27c36",
    "21": "ad14a764e258c16dcdc59e2ad7815ba38dd653f0710b1de2185cc624e277bc7eb22e56836b828100d618f43eacecda45",
    "22": "ff5b8a3141737b2f76f986018030d38c0e5550e4f835ca4568b98385f78c363c5731e0c9def50daa3e529165566602d4",
    "3": "cbf152183fd2bddecfdc5c15ffbcb94b70ae86bb94fe28cc5e6acbefd836dd74b4379b6ab1ccc0992217542b4020205a",
    "4": "cb60d4fbd67462ec3f6a899148dbcb6cf6d855171b6c1787f06c9e5274923b58b2fc7298c87353d8f513e85bf571d8dd",
    "5": "185a8f46a6defbc3a46f1c9495c857c32a3eec9faeaf6dae1c8e0c0592e88648d8b43d57020a84fc9fb99674d05494f6",
    "6": "be4ca65f89626dc0d6007a64dd151784800b3a47945e5b8bb3cd90e100904dd7c76bc95dbb32b0b63e7f391a64f040df",
    "7": "bfa9a46d76f127d3b7fe6ff381608bec2dedefc7405f7de735f01d9f7e104a267eef1f6e7102f3dc1abc41b6aec88c4b",
    "8": "f09cfb2513ae016ed0b08ed2193f0d548c9cebe2199429486a51375f6b4e5fb94fc3d73acf4b052737a213e432b7c2fc",
    "9": "cd7c84b6a0b64af165ae82d77e8e9734c6180e4e77f1d517043d1bf54d0df612a483e82e187a20a39be0694f390e5ec7"
  },
  "postgres_version": "16.14 (Homebrew)",
  "source_commit": "08afce6c9a0b453d9c8748c3c7730fe099b4c445",
  "state": "PASS"
}
```

After that terminal probe, a fresh handle/process check passed and MEDIA was
unconditionally transferred to the canonical selected-source owner
`01a19f83-12cc-7f21-8ad3-55749f012d9b`. This writer makes no Cargo/target/PG use
until their explicit release. The persistent goal remains ACTIVE: the complete
current union and the eventual merged candidate require qualification before
publication. Live authority/root activation and whole BE07/IN01/IN05 remain open.

For reproduction, the following standalone driver was used after pinning the
exact Cargo executable in sibling `binary.json`. It imports the retained owned
cluster helpers without changing them; it must run only with a named native slot.

```python
import sys, json, hashlib, signal
from pathlib import Path
root=Path('/Users/lhs/Desktop/mcr-worktrees/be-participant-producer')
sys.path.insert(0,str(root/'backend/scripts/mcr'))
import migration_smoke as migration
import bootstrap_smoke as bootstrap
work=Path(__file__).resolve().parent
pin=json.loads((work/'binary.json').read_text())
binary=migration.executable(Path(pin['snapshot']))
bootstrap.require(hashlib.sha256(binary.read_bytes()).hexdigest()==pin['sha256'],'snapshot checksum mismatch')
cluster=migration.Cluster({name:migration.executable(Path('/opt/homebrew/opt/postgresql@16/bin')/name) for name in migration.PG_TOOLS})
report={'state':'FAIL','commands':{},'source_commit':pin['source_commit'],'binary_sha256':pin['sha256']}
def interrupted(number,frame): raise migration.SmokeError('signal',str(number))
previous={number:signal.signal(number,interrupted) for number in [signal.SIGINT,signal.SIGTERM]}
try:
 cluster.init();cluster.start()
 report['postgres_version']=cluster.sql('postgres','show server_version;')
 bootstrap.require(report['postgres_version'].startswith('16.14'),'PG16.14 required')
 cluster.sql('postgres','create database mcr_concert_bootstrap template template0;')
 report['migration_sha384']=migration.migration_manifest(root/'backend','concert-service')
 for path in sorted((root/'backend/services/concert-service/migrations').glob('*.sql')):
  if not path.name.endswith('.down.sql'): cluster.sql(bootstrap.DATABASE,path.read_text());report['commands'][path.name]=0
 release=cluster.root/'release';release.mkdir(mode=0o700)
 doc=json.loads((root/'contracts/fixtures/core/bootstrap-open.json').read_text())
 files={'bootstrap':doc,'avatar':doc['avatar_manifest'],'map':doc['map_manifest']}
 files.update({t['theme_id'].lower():t for t in doc['theme_manifests']})
 for name,value in files.items():(release/(name+'.json')).write_text(json.dumps(value))
 bootstrap.run_test(cluster,binary,'native_guarded_participant_persistence_and_reread',report)
 report['state']='PASS'
except (migration.SmokeError,OSError,ValueError) as error:
 report['error']=error.detail if isinstance(error,migration.SmokeError) else str(error)
finally:
 try:cluster.cleanup();bootstrap.require(not cluster.root.exists(),'owned root remains');report['cleanup']='PASS'
 except (migration.SmokeError,OSError) as error:report['state']='FAIL';report['cleanup']='FAIL';report['cleanup_error']=str(error)
 for number,handler in previous.items():signal.signal(number,handler)
print(json.dumps(report,sort_keys=True))
sys.exit(0 if report['state']=='PASS' else 1)
```
