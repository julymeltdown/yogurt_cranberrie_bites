# BE02 local development and backend image preparation

State: **IN_PROGRESS for BE02, pending integrator review.** The native development
path is executed and usable for subsequent BE03 tests. The image recipe and its
build context are reviewed candidates; Docker build/run is **NOT_RUN**. No release,
production deployment or C03 auth acceptance follows from this handoff.

## Candidate and ownership

- Assigned base: `0fb22af06fbc139a8f9b6740d8cf780eaa55557a`.
- Implementation: `66456392f35765e02d2201b1e3b1b53fb96a9adc`, a direct child of
  that base on `codex/mcr-be-runtime`.
- This document is delivered in a separate documentation-only descendant.
- Exact changed paths: `backend/scripts/mcr/dev_stack.py`,
  `backend/scripts/mcr/test_dev_stack.py`, this document, and the four files in
  `infra/mcr/backend/`: `Dockerfile`, `Dockerfile.dockerignore`,
  `build-context-review.json`, `verify_local_auth.py`.
- Every command ran in `/Users/lhs/Desktop/mcr-worktrees/be-runtime`. The
  integration checkout, frontend, manifests/locks, shared contracts, root scripts,
  `docs/source/` and existing runtime/migration helpers were not modified.
- Main AGENTS, progress, frozen C03, and character source pack revision 3 were read.
  The selected twenty JPGs and archived `photo_03` are outside this implementation;
  no legacy character fallback or asset change was added.
- All new code/recipe/prose is authored MCR tooling. Existing helpers are imported
  through their Python APIs, not copied or patched. The prebuilt binaries retain
  the fingerprints recorded in [native runtime evidence](runtime-smoke.md).
  The integrator owns their build/source binding and any future rebuild.

## Run a useful local stack

Run from the repository root with Python 3.11+ and the qualified native PostgreSQL,
Redis, OpenSSL and macOS `lsof`. Choose NEW absolute evidence filenames each time;
existing files are refused. Replace the example absolute directories with your
prebuilt binary/evidence locations.

Terminal A owns the foreground lifetime:

```sh
PYTHONDONTWRITEBYTECODE=1 python3 backend/scripts/mcr/dev_stack.py start \
  --bin-dir /absolute/mcr-build/backend/debug \
  --ready-file /absolute/local-evidence/dev-001-ready.json \
  --report /absolute/local-evidence/dev-001.json \
  --duration-seconds 3600
```

Wait for its `READY` line. The ready file provides the dynamic Gateway/metrics/gRPC
addresses, private inbox path and control socket ownership metadata. It is mode
0600; keep it private. It contains no database URL, service credential, JWT key,
password, bearer token or refresh cookie. Its ownership marker is a local control
capability. The directory and socket are independently checked before control
requests; stale descriptors cannot signal arbitrary PIDs.

Terminal B can inspect, exercise the inherited auth seam, and stop the same stack:

```sh
PYTHONDONTWRITEBYTECODE=1 python3 backend/scripts/mcr/dev_stack.py status \
  --ready-file /absolute/local-evidence/dev-001-ready.json
PYTHONDONTWRITEBYTECODE=1 python3 infra/mcr/backend/verify_local_auth.py \
  --ready-file /absolute/local-evidence/dev-001-ready.json \
  --report /absolute/local-evidence/auth-seam-001.json
PYTHONDONTWRITEBYTECODE=1 python3 backend/scripts/mcr/dev_stack.py stop \
  --ready-file /absolute/local-evidence/dev-001-ready.json
```

`status` performs fresh owned health checks; a rejected/unavailable control result
exits 1. `stop` requests the original owner's cleanup over its private Unix socket.
It never discovers/signals a PID. Wait for Terminal A's final cleanup result.
Ctrl-C/SIGTERM after READY and elapsed duration are also normal clean-stop paths;
an interruption before READY is a failed startup. The duration starts at READY,
defaults to one hour and cannot exceed eight hours. The owner rechecks health every
two seconds. There is no detached daemon, attach/resume option, persistent source
volume, existing database reuse or automatic service rebuild.

Each invocation creates fresh owned stores and keeps them alive for the full
foreground session. Multiple test processes can use the same ready descriptor
and services while that session lives. All six stores, mail captures, keys,
configuration and private logs are removed after owned stop. The external ready
file and redact-safe final report remain as evidence; the ready file becomes stale.
Never use this fixture for data that must survive the end of a development run.

### Auth integration details for BE03

Use Gateway `/auth/*`, not the direct Auth application listener. The direct Auth
application routes stay disabled; its operational JWKS listener remains available.
The future browser `/api/auth/*` ingress mapping is not implemented by this tool.
No frontend pages, browser reverse proxy or reset-password UI are supplied.

The launcher reuses `runtime_smoke.environments()` and validates that unchanged
baseline before applying narrowly authored local overrides:

- Parity mode, external migration verification, fresh generated credentials and
  keys, seed disabled, isolated Unix-socket DBs, owned nondefault loopback ports,
  Kafka disabled, external OAuth destinations held non-listening locally, no
  inherited HOME/environment/provider/telemetry settings.
- `GATEWAY_REFRESH_COOKIE_SECURE=false` **only for this explicit local HTTP
  fixture**. HttpOnly/Path/SameSite handling still comes from the actual service.
  This does not change production cookie policy or service configuration source.
- Default CORS origin is the current Gateway loopback origin. Optional
  `--browser-origin http://localhost:5173` accepts only an exact explicit loopback
  HTTP origin. It adds no frontend server or network connection. CORS configuration
  alone does not implement C03's Origin/intent mutation guard.
- Verification/reset mail destinations are a newly reserved local HTTP capture
  listener with a per-run generated bearer credential, delivered only to Auth's
  clean child environment. The listener directly takes ownership of its held
  socket; it does not bind/attach to an existing mail service.

Auth's actual two-second outbox worker delivers verification and reset messages.
Captures retain `Idempotency-Key`, kind and actual payload in private 0600 JSON
files under a 0700 inbox; repeated identical delivery is idempotent. Conflicting,
unauthorized, malformed, oversized and excessive deliveries are refused. Complete
files publish atomically, so test processes cannot observe half-written JSON.
No request body, email address, verification code, reset link or password is
printed. Captures and service logs are deleted with the owned root.

The checked-in `verify_local_auth.py` generates a synthetic email/password in
memory, performs real signup, reads actual private delivery, verifies email,
logs in, refreshes, logs out, requests reset, reads actual reset delivery and
confirms reset. Reports contain only status codes/counts and a script hash.
The source reset link uses `#token=...`, not a query parameter; the test reads
that fragment in memory and sends the token to the actual confirm operation.
This is a practical test client, not a replacement Auth implementation or a
fixed-code/seed bypass. Run it only against a launcher-owned disposable stack.

Inherited parity OAuth routes/adapters are still present. This tool does not
qualify/enable providers, claim disabled-provider policy enforcement, or exercise
OAuth. BE03 must implement and test frozen C03's actual producer/browser guards,
capabilities, route adaptation, response caching and invalid/reused credential
cases independently. A successful local email seam is not C03 acceptance.

## Build context, protobuf and SQLx audit

The pinned upstream `boilerplate-grpc-rust/` Dockerfile and Compose file expect
that workspace as context (`context: .`). They were intentionally excluded from
the imported MCR tree, along with source deployment/env definitions. There is no
imported backend Dockerfile to run from the new repository root. Reusing the old
Compose would also carry source service names/defaults and is not this handoff.

The new authored recipe uses repository-root context:

```sh
docker build -f infra/mcr/backend/Dockerfile \
  --build-arg SERVICE=auth-service \
  --build-arg SOURCE_REVISION=REPLACE_WITH_REVIEWED_GIT_SHA \
  -t mcr-local/auth-service:candidate .
```

Supported `SERVICE` is explicitly limited to the six inherited services. The
builder uses Rust 1.88.0 and `cargo +1.88.0 build --release --locked`; it copies the
selected backend build tree to `/mcr/backend`. All 29 workspace member manifests,
root `src`, local crates, tools, service sources, proto files and public API build
inputs are present. `.env` files, keys, logs, target output, Git data, frontend,
user assets and native-run evidence are excluded by a recipe-specific context
file. Docker documents the colocated `Dockerfile.dockerignore` naming and its
precedence in [build-context guidance](https://docs.docker.com/build/concepts/context/#filename-and-location).
This is a source review, not proof of the actual Docker transfer.

All six `build.rs` files call `protoc_bin_vendored::protoc_bin_path()` and
`contracts-build` resolves the CURRENT service's `CARGO_MANIFEST_DIR/../../proto`.
Keeping `/mcr/backend/services/<service>` and `/mcr/backend/proto` preserves that
relation; generated Rust remains in Cargo OUT_DIR. No global protoc install,
proto relocation or shared-contract edit is needed.

No SQLx checked query macros occur in the inspected application sources. Runtime
query APIs and embedded `schema-probe.sql` are retained; compilation does not
need a live database. Migration-runtime loads SQL from the compiled absolute
`CARGO_MANIFEST_DIR/migrations` path on migration AND normal startup/external
verification. The final image therefore packages each selected service's SQL at
`/mcr/backend/services/<service>/migrations`, with its `runtime-grants.sql` beside
it. Receipt counts remain 17/12/7/14/30/4 (84 total). Copying a binary alone is
insufficient. The runtime is nonroot and contains no fixture credentials or host
port publication.

The [static context record](../../../infra/mcr/backend/build-context-review.json)
binds pinned upstream Dockerfile/ignore/Compose hashes, current recipe hashes,
all service SQL paths and the 29-member workspace audit. It is **not** a container
build receipt. `docker info --format '{{.ServerVersion}}'` exited **1** because the
existing daemon was unavailable. Docker was not started, restarted or modified;
no image was built or launched. Base tags and apt repositories remain mutable;
resolve reviewed platform digests/package inputs and execute Linux build,
migration, dynamic-link, nonroot and health checks before image qualification.
The recipe permits explicit builder/runtime image overrides for that later gate.

## Executed checks and handoff evidence

Implementation commands from the assigned worktree:

- `python3 -m unittest discover -s backend/scripts/mcr -p test_dev_stack.py -v`:
  **14 PASS, exit 0**. Actual private mail/control sockets are exercised; no live
  service/database is used by these unit tests.
- `python3 -m unittest discover -s backend/scripts/mcr -p 'test_*smoke.py'`:
  unchanged helpers' **55 PASS, exit 0**.
- `python3 scripts/mcr/check-plan.py`: **PASS, exit 0**, planning scope only.
- Static member/proto/migration/recipe review: **PASS**, container execution
  **NOT_RUN**. `git diff --cached --check`: **PASS, exit 0**.
- Native start used the real CLI above with supplied binaries at
  `/Users/lhs/Desktop/mcr-build/backend/debug`, duration 120 and NEW
  `/Users/lhs/Desktop/mcr-build/be02-dev-final-02{,-ready}.json` files.
  Start, separate seam verifier and separate control-stop all exited **0**.

Final native session: **2026-11-03 13:40:17–13:41:03 KST**
(`04:40:17–04:41:03 UTC`). Six services reached liveness/readiness 200 with every
probe up. Actual signup/verify/login/refresh/reset-request/reset-confirm returned
200 and logout returned 204. Two actual Auth deliveries reached the private
inbox. Controlled stop returned PASS/cleanup PASS; owned root was absent and
PID-filtered checks found no remaining recorded service/Redis/postmaster PID.
The checked-in verifier deliberately records C03 acceptance as NOT_RUN.

Fresh deadline and interruption runs additionally verify automatic clean stop,
startup interrupt failure with cleanup, and repeated SIGINT/SIGTERM after READY.
Their expected launcher exits are 0, 1 and 0; the driver assertions exit 0.
Private native JSON files remain under `/Users/lhs/Desktop/mcr-build/`; the
integrator may archive selected redact-safe reports into its own evidence paths.
No old report was overwritten. The earlier ad hoc auth driver initially assumed
a reset query token; it failed after actual delivery because the source uses a
fragment. Its launcher still cleaned up. The checked-in client and final run
correct that assumption without changing Auth behavior.

| Artifact | SHA-256 |
|---|---|
| `backend/scripts/mcr/dev_stack.py` | `472a32302fff4b74185983c233a0595a9003f7fa5d83ff81345c0857681cfed7` |
| `backend/scripts/mcr/test_dev_stack.py` | `82b62e1119c85a899801d3bf95ed8b6613428b8cf0ccc11b7d042ba2e7da2567` |
| `infra/mcr/backend/verify_local_auth.py` | `665c1c1378ae89f6e7a0244518aba3f0d8f64393e1516953e0ff86af653f85fd` |
| `be02-dev-final-02.json` | `5b3d16224db2bb8793f1882455361493ab65cdcd0a5cb3509cf526acce9a5063` |
| `be02-dev-auth-seam-final-02.json` | `0927a2ca7aeb8fdc9dccc65c6f5ab137634bd7eda56893c0fa7917a12795f4cb` |
| `be02-dev-deadline-02.json` | `54705747221ac52a4c219d3d7809d8730bc3296b7c2c6864b28d80af596bc93f` |
| `be02-dev-startup-interrupt-02.json` | `86e49166abce6d4cc24b385245eb45f18967c60ae364634092c9bb20b6f59d4c` |
| `be02-dev-ready-interrupt-02.json` | `a265ab0d0b907202ccd08b4d0071db1c1cd45ca1866605c4e634cd0d1c247048` |

Final source/hash checks also confirm unchanged helper hashes:
`runtime_smoke.py = da2ac13ca1502e80a2528c982bf28b61c06707b4018c2a436e7f8bf41512e7a8`,
`migration_smoke.py = a46f2dc3a6dcab9310230cd75997dfcd86571c88fd066a973a8a0c3b1a8c6eb8`.
No helper change is requested. Reports executed before commit name the assigned
base; script hashes bind their actual code to implementation commit `6645639`.

Remaining limits: native macOS/PostgreSQL 16.14 and supplied binaries only; no
new Cargo build, Linux/container execution, PostgreSQL 17, least-privilege runtime
role qualification, backup restore, external providers/mail/Kafka, Concert,
frontend/device/multiuser/release acceptance or production deployment. The fixture
uses a disposable owner role. The main integrator owns BE02 acceptance, BE03
producer work, central progress/evidence updates, serial integration and push.
