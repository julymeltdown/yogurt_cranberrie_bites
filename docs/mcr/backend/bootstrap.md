# BE06 bootstrap producer

State: IN_PROGRESS for whole BE06. The bounded producer passes native qualification.
This is the participant/default producer under the
unfinished BE04 identity goal. Whole BE04, public Gateway entry, production art,
and IN01 are not accepted by this work.

The worker starts at `34d4bb614325b915569489b59e6deefba88a002f` on
`codex/mcr-be-bootstrap`. Its writes are the application/service `bootstrap/`
modules, their dedicated tests, migration 0004, the dedicated Python native
harness and this document. The only root changes are `pub mod bootstrap` in the
two owning libraries. The integrator separately owns dependency/lockfile changes,
shared RPC/contracts, routing, main/config and publishing.

## Composition

`BootstrapService::new(PgPool, Arc<ValidatedProductRelease>)` implements generated
`concert.v1.ConcertService`. Register it behind existing `rpc::POLICY` and
`platform::grpc_method_policy::guard`. The service itself also requires sealed
verified Gateway, actor and original deadline extensions. An email-only canonical
account needs no X/provider binding. Auth roles do not become event roles.

`ProductReleaseBundle::from_product_files(bootstrap, avatar, map, [danger, parade,
revenge], BootstrapFeatures)` loads six explicit UTF-8 JSON files. The C03 template
must name the exact configured event. All documents are immutable after loading.
The frozen generated `backend/docs/api/mcr-core.openapi.json` embeds C03 and all
five C06 schemas; native Rust validation enables formats and has no remote
reference/network resolver. C06 hashes, avatar layers/presets/regions, map grid
sizes, reachability, theme references and texture tile bounds receive additional
cross-document validation. Canonical hashes follow Python's sorted, ASCII-escaped,
integer-only representation. Duplicate keys, floats, oversized files, unknown
fields, unknown items, private fields and invalid defaults fail closed.

Defaults are explicit: the C03 preference theme is the creation theme, and its
avatar configuration must equal the supplied avatar manifest's fallback preset.
Existing persistence always wins on repeated bootstrap. The source digest includes
the source bundle and the effective capability configuration.

Product provenance must be reviewed at every nested provenance object and must
never be `ORIGINAL_FIXTURE`. Avatar provenance must name the user-selected pack
`mcr_user_characters_20261103`; this is a metadata identity check, not proof of
image bytes, licensing truth, compositor correctness or visual acceptance.
There is no approved product atlas in this delivery. Synthetic fixture provenance
is accepted only by `from_synthetic_files(..., allow_synthetic=true)` in the
isolated test harness. That constructor cannot silently become product startup
configuration. Product composition must use `from_product_files`.

All optional capability switches default false. Missing avatar/map/theme resources
keep world unavailable with `DEPENDENCY_UNAVAILABLE` even if its switch is true. Only the integrator may enable a
switch after qualifying its corresponding producer. `mutations` remains false
because this producer does not implement user mutations. Age status is `UNKNOWN`;
there is no age-policy producer here. No Profile email/provider data is persisted.

## Transaction semantics

The transaction locks the current event, then event-local admission, participant
and avatar. OPEN/LIVE permits first bootstrap to insert ACTIVE admission revision
one and participant/avatar revision one. Unique `(event_id,user_id)` keys plus
`ON CONFLICT DO NOTHING` preserve prior rows across concurrent and repeated calls.
The migration keeps admission identity immutable and requires restriction revisions
to advance exactly one. It introduces ACTIVE/SUSPENDED state, not a moderation API
or global Auth revocation protocol.

Existing SUSPENDED admission denies bootstrap and is never reactivated by an
insert conflict. READ_ONLY/ARCHIVED performs existing-member reads without default
or admission inserts; a missing admission/participant cannot become a new member.
DRAFT denies bootstrap. Current event title, slug, timezone, phase and revision
come from the locked database row, not stale template values.

The production path rechecks verified authority after database awaits and before
commit. The entire transaction is bounded by the original monotonic deadline.
Response binding and complete schema/bundle validation run before commit; invalid
release/event/item bindings roll back all inserts. Authority is checked again
after commit and immediately before response. This establishes a transaction
linearization point, not instantaneous revocation after an owner's observation
or after locks are released. A deadline during commit can have an uncertain
client outcome; retrying the same bootstrap remains idempotent.

The response binds the exact actor/event, current public display name and bio,
persisted preferences/avatar and UTC clock. Top-level avatar and profile avatar
are identical. Every successful RPC supplies `cache-control: no-store` metadata.
Errors use bounded safe diagnostics; no SQL text or private dependency data is
returned.

## Verification

Final source-bound qualification on 2026-11-04 KST: PASS. This is generated
in-process gRPC plus a real PostgreSQL cluster, not a TCP/public browser journey.

Completed checks (all exit 0):

- `python3 scripts/mcr/check-plan.py`: exit 0, planning provenance only.
- `python3 -m unittest discover -s backend/scripts/mcr -p test_bootstrap_smoke.py -v`:
  exit 0, 3 harness artifact-selection/no-skipped-test checks.
- Retained Rust 1.88.0, locked/offline, two jobs, shared target, dev/test debug zero:
  `cargo test -p concert-application --test bootstrap`: 6 tests.
- `cargo test -p concert-service --test bootstrap`: 4 deterministic tests;
  2 native tests intentionally ignored in this default run and explicitly executed
  by the owned harness.
- `cargo test -p concert-service --lib bootstrap`: 2 tests (RPC policy and C06
  canonical Unicode/DEL encoding).
- `cargo test -p concert-service --test bootstrap_authority`: 3 tests covering the
  valid guarded call and sixteen invalid workload/signature/assertion variants.

Build settings were `RUSTUP_TOOLCHAIN=1.88.0`, `CARGO_PROFILE_DEV_DEBUG=0`,
`CARGO_PROFILE_TEST_DEBUG=0`, and the exclusively leased shared Cargo target.
The final Cargo artifact manifest was produced inside `backend/` with:

```sh
cargo test --locked --offline -j2 -p concert-application -p concert-service \
  --test bootstrap --no-run --message-format=json > /tmp/mcr-bootstrap-build.jsonl
```

Native command (exit 0 after the final build):

```sh
python3 backend/scripts/mcr/bootstrap_smoke.py \
  --build-metadata /tmp/mcr-bootstrap-build.jsonl \
  --pg-bin /opt/homebrew/opt/postgresql@16/bin
```

The harness creates a private PostgreSQL 16.14 Unix-socket cluster using the
existing marker/inode-owned `migration_smoke.Cluster`, disables TCP, applies the
actual service `--migrate` command, verifies migration ledger checksums, reapplies,
and invokes exact non-skipped native tests selected from Cargo compiler metadata.
The Rust fixture verifies root marker/inode, directory ownership, role, database,
socket, server data directory and version before any writes. It tests 16 concurrent
calls, actor/event separation, persisted preferences/avatar, suspension winning a
real database lock conflict, response/event rollback, expired authority during an
await, generated protobuf service through its actual guard, no-store metadata and
read-only phases. PostgreSQL is stopped and restarted, then a separate native test
checks exact surviving membership/default/restriction state.

A fresh private Python venv installs pinned contract requirements and invokes the
existing `validate-core.py:validate` and C06 bundle validator on the actual decoded
RPC JSON bytes. Cleanup stops only the owned postmaster and removes only the
marker/inode-validated root. The receipt includes exits, migration hashes, binary
hashes, response hash, actual native test count, restart PIDs and cleanup result.
No existing PostgreSQL cluster, Docker volume or live account is used.

## Source-bound native receipt and delivery

This branch contains migrations 1, 2 and 4. Migration 3 belongs to BE08 and was
absent at base 34d4bb6. Delivery owns the combined ledger 1/2/3/4 (and later 5)
qualification; this isolated run cannot certify that integrated ledger.
The dependency prerequisite is original commit `236f29c` (worker cherry-pick
`0977dff`); do not replay its duplicate patch into master. The following producer
commit changes only the assigned paths and two one-line exports. Its base ancestry
is `34d4bb6 -> 0977dff -> producer handoff`; obtain its exact commit from the
handoff message and Git log because a commit cannot embed its own final hash.

No Bigyeting rendering or implementation was copied. New code reads the frozen
C03/C06 contracts, consumes the previously reviewed RPC authority boundary, and
imports the existing owned migration harness's `Cluster` and ledger helpers.
No new product asset or private record is supplied. Selected originals and
`docs/source/` remain unchanged.

Final receipt:

```json
{
  "binary_sha256": {
    "service": "ecd2f9fa150e7a54235a81e645998fa4acc21bdc6976dff3a930ea56df8444ef",
    "tests": "4c746d4d33ebec454fa2184e170535d49868cb3e40ee6409a27b65186ef827c2"
  },
  "cleanup": "PASS",
  "commands": {
    "actual-rpc-c03-validator": 0,
    "migrate": 0,
    "native_bootstrap_restart_persistence": 0,
    "native_bootstrap_transactions_and_guarded_response": 0,
    "reapply": 0,
    "verify": 0
  },
  "migration_sha384": {
    "1": "bf3d01697c1c6167fd999a92aa4323297f0e9c8b063176e2938f78dec8d1278f06cd84469d583a76314ac8aa520d8a5f",
    "2": "6eae0492a1aedf4b2eaf30b44761a853376623d9cc65cc268941e416882998b40c46ff61f0acb8bd285be33a936f2e6b",
    "4": "cb60d4fbd67462ec3f6a899148dbcb6cf6d855171b6c1787f06c9e5274923b58b2fc7298c87353d8f513e85bf571d8dd"
  },
  "native_tests": 2,
  "postgres_version": "16.14 (Homebrew)",
  "response_sha256": "b90646e19d50bcc015758305c9eb9578116f05638c7f10c6d4139e6f585e7b8a",
  "restart": {
    "after_pid": 74203,
    "before_pid": 73667
  },
  "state": "PASS"
}
```

Validated source/contract SHA-256 values (documentation excluded):

```json
{
  "backend/crates/concert-application/src/bootstrap/mod.rs": "dfcc600b1320c922c69509188ff2f855d6798a140f17493425b84678dbf00178",
  "backend/crates/concert-application/src/lib.rs": "a6a8b7ec41c2a43e91d0129e450eff731b6ab53292eb318a015c54741a136292",
  "backend/crates/concert-application/tests/bootstrap.rs": "b8437423c9707493805d4da3ceb21a522fe77f73d4040ac79e14b41cfe698b6e",
  "backend/docs/api/mcr-core.openapi.json": "c8de71d5f3dfc125fd3a5206bc404a76df39367a45fa6c3f67ed6323de91c15d",
  "backend/proto/concert/v1/concert.proto": "1af584ae11306bf7ef29030a8670fe3068b4f758b1417a0bbcdade691a0024b6",
  "backend/scripts/mcr/bootstrap_smoke.py": "c3b10e54fb77d024a6ae5d713046e7b1f14527ef6a39a7bb0f7e2edb755fec17",
  "backend/scripts/mcr/test_bootstrap_smoke.py": "61b84771a195280c4eb5cc20faf87208484f00f9ab855e77b4966f3a88ba2f57",
  "backend/services/concert-service/migrations/0004_bootstrap_admission.sql": "cb1c0093eeaef79dbe783052447c34c78aad5c6daebdc2f6f3dc7f4b17112ac9",
  "backend/services/concert-service/src/bootstrap/json.rs": "ffb2d937d5ebcf94fe0f8305e137dfac6ee5217f7b633fca4ca46995531503b9",
  "backend/services/concert-service/src/bootstrap/mod.rs": "4d34333835bf3a0a5add0810d4a5b137b1bb824cf6a3955d32af67c88770ad3f",
  "backend/services/concert-service/src/bootstrap/release.rs": "aa68f033b47b7005ebe2d7bd55b97eff1182662ef440273d57f051d72c1f75c7",
  "backend/services/concert-service/src/bootstrap/schema.rs": "3b2d45c33b07d40a81e18ed304da7b2aafad74275ad38aa65a3790022f0aee38",
  "backend/services/concert-service/src/lib.rs": "1a1068a4911d88df3f38cee76bbd550bfc038c27f99ac3b20b734e46280dee76",
  "backend/services/concert-service/src/rpc.rs": "d7e0aae477284aae294b8e2bcfda5ee8e6c5df38d203697e9e425cc162caaf22",
  "backend/services/concert-service/tests/bootstrap.rs": "7a2270fad773e32618c5d7244e409fbd28e8cabd0f4c268b40b151967a27ff39",
  "backend/services/concert-service/tests/support/bootstrap/mod.rs": "34d24864a49b1e895261ec3e0335a8a2357b861466bc90b450fd78cdcf694308",
  "scripts/mcr/validate-assets.py": "94cb9f3eae7b184794fefa626b4d00e4ff690c80975f0c140072f2d094ff8cf0",
  "scripts/mcr/validate-core.py": "69e29751133b16e1e226593d79c008cbd7586e10ca0b93442126f7ffac536954"
}
```
