# Participant producer — Phase A

Status: **CONTRACT_READY** for the bounded Phase A component. All eight guarded
producer tests and both native PostgreSQL probes pass. This slice supplies an isolated guarded producer
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

## Provenance and delivered boundaries

All new producer, proto and tests are newly authored from C03/C06. Dependencies
are retained platform workload/actor/deadline guards, BE07 AvatarPersistence and
the canonical strict avatar decoder, preference decoder and bundled C03 schema.
There is no old rendering, art replacement, frontend import or private owner data
in the wire response. Original source/art resources remain unchanged.

This writer's exact eight changed paths relative to the reconciled remote are:

- `backend/proto/concert/v1/participant.proto`.
- `backend/services/concert-service/build.rs`: additive participant proto step.
- `backend/services/concert-service/src/lib.rs`: participant module export only.
- `backend/services/concert-service/src/participant/context.rs`.
- `backend/services/concert-service/src/participant/mod.rs`.
- `backend/services/concert-service/src/participant/ports.rs`.
- `backend/services/concert-service/tests/participant_producer.rs`.
- `docs/mcr/backend/participant-producer.md`.

Existing build steps/module exports retain upstream bytes. Shared contracts,
root listeners, Gateway, existing bootstrap RPC, configuration, manifests,
lockfiles, migrations and avatar/domain code receive no edits from this writer.
Peer-authored dependency commits are preserved through normal merges. The old
avatar checkout remains stopped. The current user-authorized delivery policy
allows the implementation owner to validate and publish this exclusive scope
directly; king/SL code or query review is not a separate gate.

Source ancestry starts at `03fa2cd`, then `a2a60245`, `f9e7323b`, `a4c4169e`.
`dc11fe42` preserved the recovered dirty standalone-protoc evidence as an owned
checkpoint. Subsequent source/test checkpoints include `6dbea2d6`, `08afce6c`
and `3e11090a`. The canonical selected-avatar implementation is peer commit
`86123961`, with its fixes and World compatibility/corrected fixture incorporated
through peer `b37ae80a`. Their delivery `b13d79e6` is an ancestor of the freshly
fetched remote. No unqualified dependency is published by this slice.

C03/C06 selected-contract source is `3398d833`; AvatarConfig retains layered
1.0.0 and whole-character 1.1.0 as an exclusive union. The canonical parser,
catalog, row decoder, bootstrap fixture and migration 0025 stay owned by the
selected-source author. This producer consumes their API without flat layered
field assumptions or a duplicate parser. The exact selected manifest pin is in
the native receipt below. Fixture artwork remains metadata/component evidence,
not real multiuser or product-art qualification.

## Executed qualification

The tested source is `65bc598ff325a529c161afc7a6f0327c4008fce5`. Reconciliation
merge `cf063ee9` preserved the current remote through `d2a5fbdd`; `git diff
65bc598f cf063ee9 -- backend` was empty. Its subsequent change is this evidence
document. Runtime evidence is therefore tied to the unchanged backend source.

Planning check `python3 scripts/mcr/check-plan.py`, scoped Rust 1.88.0
`rustfmt --check --edition 2021`, `git diff --check`, and the exact path/shared
content/frontend-import audit exited 0. The retained vendored protobuf compiler
compiled and decoded the new package, all eleven request fields and all three
methods successfully, independently of Cargo. This preserves the recovered
standalone-protoc evidence; it is not substituted for the actual Rust/RPC tests.

The named MEDIA lease used only the existing target below. Fresh preflight found
76.67 GiB available and zero target handles. BACKEND peer compilation was
separately owned. The command was:

```sh
env RUSTUP_TOOLCHAIN=1.88.0 \
  CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/media-combined-fa4b4b3/target \
  CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 \
  CARGO_INCREMENTAL=0 \
  cargo test --manifest-path backend/Cargo.toml --locked --offline -j2 \
    -p concert-service --test participant_producer --no-run --message-format=json
```

The first current-source compile encountered a cached Auth build-script checkout
path mismatch. Selective `cargo clean --manifest-path backend/Cargo.toml --locked
--offline -p contracts-build` removed 20 files / 816.4 KiB from that leased target;
the identical compile then exited 0. No application source or lockfile changed.
Cargo's exact compiler-artifact record was selected using target name/test kind
and this checkout's service manifest path, with successful build-finished receipt.
The emitted executable was copied and its SHA-256 verified before releasing MEDIA:

- Tested source: `65bc598ff325a529c161afc7a6f0327c4008fce5`.
- Copied executable: `/tmp/mcr-participant-variant-3e11090a/participant-producer-tests-65bc598f`.
- SHA-256: `ed398539f44f31340f310cb50fbbd624cf5b08520297385939f337a8a11107e7`.
- Compiler/pin receipts: `/tmp/mcr-participant-variant-3e11090a/`:
  `compiler-artifacts-retry.jsonl`, `binary.json`, `focused.stdout`,
  `native-report.json` and the exact `native_probe.py` reproduced below.

| Command | Exit / result | What it proves |
|---|---|---|
| Copied executable `--test-threads=1` | 0: 8 passed, 0 failed, 2 ignored | Actual generated/guarded RPC, workload/signature/body/actor rejection, current owner negative gates, separate mutation permit, original deadlines including slow capability/noncooperative Profile, strict inputs, C03 response validation, conflict/reread and all six whole-character DTO roundtrips. |
| Native driver: `native_guarded_participant_persistence_and_reread` | 0: exactly 1 native test passed | Generated guarded RPC + real BE07/PG persistence; actual SQL UPDATE before permit withdrawal then rollback, save/conflict/authorized reread, preference preservation, read-only refusal/read success and suspended-admission denial. |
| Native driver: `native_guarded_selected_participant_roundtrip` | 0: exactly 1 native test passed | All six frozen presets save/reload through guarded RPC and real catalog/persistence, exact revision progression, stale conflict, unknown-preset refusal and preferences retaining the last selected avatar. |
| Native cleanup | PASS | Private marker/inode-owned socket-only PG16.14 root was stopped/removed and its absence asserted. |

The two ignored native tests were explicitly executed by exact name; the driver
requires the `1 passed; 0 failed; 0 ignored` result for each, so omission is not
counted as success. Both probes use separate event/user rows in a fresh isolated
database. All current concert migration SQL applied; this is not a qualification
of the migration-runner ledger. The sequence used to prove UPDATE execution is
nontransactional, while the avatar row's rolled-back revision remains 1.

The first executed source `08afce6c` had 7 guarded passes and the new selected
matrix failed at preset 1 (INVALID_ARGUMENT); its layered native test passed.
That exposed the then-missing canonical parser dependency. The current complete
8+2 result resolves it without weakening a contract or hiding the failure.

MEDIA was explicitly released to replay-client owner
`01a1a1fe-16e1-7ee3-a9dc-8b5f037cc24c` after the compiler terminated, exact copy
was pinned and a fresh zero-handle check passed. Both native probes then used
only that copy. This writer holds no target or native PostgreSQL resource.

Native receipt:

```json
{
  "binary_sha256": "ed398539f44f31340f310cb50fbbd624cf5b08520297385939f337a8a11107e7",
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
    "0023_world_read_only_barrier.sql": 0,
    "0025_whole_character_avatar.sql": 0,
    "native_guarded_participant_persistence_and_reread": 0,
    "native_guarded_selected_participant_roundtrip": 0
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
    "23": "ce5d6c8ef9f16b0d32ebb636573e58f7669742cf0176167ae2a2a86d8f5bd1b0898bd77d5c72e3e1cc610bfe89fb0953",
    "25": "74923ba426bff066811b0236f4efb9111f6df4a4594b310724697795ce60737ee9b93ce7d2a4ae37163ab34bac46ebbc",
    "3": "cbf152183fd2bddecfdc5c15ffbcb94b70ae86bb94fe28cc5e6acbefd836dd74b4379b6ab1ccc0992217542b4020205a",
    "4": "cb60d4fbd67462ec3f6a899148dbcb6cf6d855171b6c1787f06c9e5274923b58b2fc7298c87353d8f513e85bf571d8dd",
    "5": "185a8f46a6defbc3a46f1c9495c857c32a3eec9faeaf6dae1c8e0c0592e88648d8b43d57020a84fc9fb99674d05494f6",
    "6": "be4ca65f89626dc0d6007a64dd151784800b3a47945e5b8bb3cd90e100904dd7c76bc95dbb32b0b63e7f391a64f040df",
    "7": "bfa9a46d76f127d3b7fe6ff381608bec2dedefc7405f7de735f01d9f7e104a267eef1f6e7102f3dc1abc41b6aec88c4b",
    "8": "f09cfb2513ae016ed0b08ed2193f0d548c9cebe2199429486a51375f6b4e5fb94fc3d73acf4b052737a213e432b7c2fc",
    "9": "cd7c84b6a0b64af165ae82d77e8e9734c6180e4e77f1d517043d1bf54d0df612a483e82e187a20a39be0694f390e5ec7"
  },
  "postgres_version": "16.14 (Homebrew)",
  "selected_manifest_pin": {
    "content_hash": "sha256:0627c7d178b22535a561e1270db6d8bf9e2383ba84d03a63962e48e565c59b2b",
    "manifest_id": "mcr_selected_static_20261103",
    "manifest_version": 1
  },
  "source_commit": "65bc598ff325a529c161afc7a6f0327c4008fce5",
  "state": "PASS"
}
```

Reproduction driver (run only with an assigned native slot and a sibling
`binary.json` produced by exact Cargo artifact selection):

```python
import sys, json, hashlib, signal
from pathlib import Path
root=Path('/Users/lhs/Desktop/mcr-worktrees/be-participant-producer')
sys.path.insert(0,str(root/'backend/scripts/mcr'))
import migration_smoke as migration
import bootstrap_smoke as bootstrap
import avatar_smoke as avatar
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
 report['selected_manifest_pin']=avatar.write_selected_release(cluster.root,doc)
 bootstrap.run_test(cluster,binary,'native_guarded_participant_persistence_and_reread',report)
 bootstrap.run_test(cluster,binary,'native_guarded_selected_participant_roundtrip',report)
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

## Remaining product obligations

This is qualified isolated producer-component delivery. Default ports remain
unavailable and startup/Gateway do not register this service. Actual canonical
Auth/Profile/mutation authority clients, original-bound outgoing owner calls,
qualified root/HTTP composition and the full Profile PATCH protocol require
separate explicit ownership. The response age remains UNKNOWN. World avatar
publication remains with its World owner. Synthetic test grants cannot be used
as product authority. Public browser/network, real multiuser, device/load/release,
whole BE07 and IN01/IN05 acceptance are not established by these component tests.
