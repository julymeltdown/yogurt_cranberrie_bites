# IN01 canonical account and owned-event provisioning

Status: **IN_PROGRESS**. This is a provisioning tool, not IN01 acceptance.
Current offline ownership/failure/config checks pass19/19. Actual two-account
provisioning is NOT_RUN pending a current qualified runtime artifact binding;
owned PostgreSQL event fixture execution passed at9ad192c. No art receipt or world result is
invented to turn those prerequisites green.

## Assignment and provenance

Worktree `/Users/lhs/Desktop/mcr-worktrees/be-in01-provision`, branch
`codex/mcr-be-IN01-provision`, base
`cd7192febf9c2764169b55d63a70d7e00d13fd08`. Exact owned files:

- `backend/scripts/mcr/in01_provision.py`
- `backend/scripts/mcr/test_in01_provision.py`
- `docs/mcr/backend/in01-provisioning.md`

The previous Community checkout/writer remains stopped. No frontend, central
contract, manifest, lock, root service, CI or source-archive changes are included.
Only Delivery publishes master; bounded delivery does not wait for main/SL review.
The persistent goal stays active through meaningful native qualification,
committed handoff and concrete fixes.

New tooling is authored here using Python's standard library. It imports the
unchanged `browser_auth_smoke.Browser`/cookie checker and unchanged `dev_stack`
ownership, private mail, native lifecycle and cleanup helpers at the assignment
base. Those helpers transitively import `runtime_smoke`/`migration_smoke`.
Their exact source hashes are captured in the sanitized receipt. No framework,
new dependency or Rust target is added. Python3.11+ and the launcher's already
qualified native PG/Redis/OpenSSL/lsof dependencies are required for owned mode.

Main explicitly clarified on2026-11-04 that deterministic initialization in a
brand-new owned development cluster is permitted; a product create-event API is
not required for a fixture. The initialization adapts the event INSERT pattern
from `tests/support/bootstrap/mod.rs` and the schema/default-room pattern from
`world_admission_smoke.py` and `tests/world_admission.rs` at the base. All IDs are
generated from the output owner's nonce; no synthetic Auth user or admission is
seeded. PostgreSQL constraints stay active.

## Commands and private outputs

Paths must be absolute. Use a canonical private output location outside every
Git worktree, for example a NEW directory beneath `/private/tmp/`; symlinked
output ancestors, preexisting output directories and public output files are
refused. Output directory is0700, files0600. Existing file checkpoints are
fd-relative, atomic and fsynced; ownership and directory identity are checked.

Fresh foreground stack, with existing qualified six-service binaries:

```sh
PYTHONDONTWRITEBYTECODE=1 python3 backend/scripts/mcr/in01_provision.py owned \
  --bin-dir /absolute/qualified/copied-binaries \
  --binary-receipt /absolute/successful-six-service-build.json \
  --release-revision REPLACE_WITH_40_HEX_DECLARED_RELEASE \
  --output-dir /private/tmp/NEW-in01-private-run \
  --hold-seconds 3600
```

The binary receipt must contain a successful `exit_code:0`,40-hex
`source_revision` and exact `binary_sha256` entries for all six retained services.
It binds actual bytes; it does not assert that a different current checkout was
compiled. The unchanged launcher verifies each migration ledger against current
source. A stale binary/migration mismatch fails closed and runs owned cleanup.
No automatic build or target copy is attempted.

Alternatively, provision accounts on an explicitly owned running dev stack:

```sh
PYTHONDONTWRITEBYTECODE=1 python3 backend/scripts/mcr/in01_provision.py attach \
  --ready-file /absolute/private/owned-ready.json \
  --release-revision REPLACE_WITH_40_HEX_DECLARED_RELEASE \
  --event-id REPLACE_WITH_EXISTING_OWNED_EVENT_UUID \
  --output-dir /private/tmp/NEW-in01-attached-run
```

Attach validates the private descriptor, root marker/inode, control socket,
READY response, matching Gateway origin and private inbox. It never starts,
stops, migrates or initializes the attached owner's databases/events. Its newly
created account namespace is unique. No existing user credentials are accepted.
The owner retains the stack and accounts; attach receipt cleanup explicitly says
NOT_REQUESTED rather than implying account deletion. Own mode cleans its entire
fresh cluster after its bounded foreground lifetime, including on failure/signal.
A completed own-mode receipt labels the accounts/event removed; its old private
credentials cannot be presented as still-live provisioned inputs.

`private.json` checkpoints generated emails/passwords, actual canonical UUIDs,
verification stage, and returned access/refresh-cookie material. It is never
printed or committed. Partial signup keeps its generated credentials for manual
owner recovery; the tool refuses blind reattachment/retry in the same output
namespace. No raw verification code is retained outside the actual private mail
capture. A new invocation creates a new owned namespace, never overwrites one.
`receipt.json` records fixed diagnostics, status codes, hashes and readiness.
Inherited lifecycle diagnostics are additionally scrubbed against the generated
private values. Report messages never include arbitrary response/exception text.

## Real owner boundaries and event initialization

Both accounts use actual Gateway `/api/auth/email/signup`, actual Auth outbox
HTTP delivery captured by the owned private mail sink, `/api/auth/email/verify`
and `/api/auth/login`. Signup/verify/login UUIDs must match, and the two final
UUIDs must differ. Tokens/cookies never substitute for browser login in the
frontend gate. Origin/intent, no-store, correlation and cookie checks come from
the unchanged browser-auth helper.

Fresh own mode creates only new `mcr_in_concert`, never reattaches an existing
store, and applies the exact bounded fixture migrations0001/0002/0003/0004/0005.
It inserts one OPEN event at revision1. This is explicitly direct native fixture
schema initialization, **not** a SQLx migration-ledger/runtime qualification.
The current schema/digests are recorded. A future full Concert runtime must use
its qualified full migration path; these fixture tables are not silently adopted
by a runtime expecting a SQLx ledger. No participant, avatar, restriction,
operator role, consent, event admission, ticket, membership or owner lease is
inserted. The real bootstrap/world owners remain responsible for those results.

Optional `--map-manifest`, `--avatar-manifest` and `--art-review` supply existing
resource declarations. The tool rejects fixture/unreviewed provenance and invalid
IDs/revisions, and the avatar manifest must match the user's selected source
pack's declared runtime manifest. Only then can a default-room row be initialized
with those exact pins/capacity50. This does not validate rendered art or replace
`ProductReleaseBundle` validation. Without approved source-pack runtime inputs,
the event is still initialized while default room remains NOT_RUN with an exact
missing-resource code. No legacy or invented fallback manifest is used.

Readiness attempts real authenticated bootstrap and rooms GET for the declared
event. Identity/event/default-room mismatch is a failure; unavailable real routes
or disabled world capability are explicit NOT_RUN prerequisites. Successful
reads prove no ticket consumption, room admission, mutual visibility or movement.
The fresh six-service launcher currently does not start/compose Concert, so its
actual bootstrap/rooms outcomes must be recorded rather than synthesized.

## Frontend gate configuration

Only after both actual bootstrap/default-room reads succeed, and supplied
`--browser-origin`/`--art-review` exist, can `config.json` be emitted. It has exactly
the keys accepted by `frontend/e2e/real/world-gate/config.ts`: version, isolated,
origin, eventId, releaseRevision, two independent accounts, serviceReceipt and
artReview. Token/cookie keys are excluded. `services.json` freezes source/binary
and observed service pins; the supplied art receipt is hashed, never authored or
approved by this tool. The frontend origin still needs actual app/API/WS routing;
its Playwright gate uses two fresh browser contexts and real UI login. Manual
art review, actual game/socket count, movement, theme/customization and reconnect
remain separate IN01 obligations. A bounded provisioning PASS always retains
`IN01:NOT_RUN` and can coexist with no emitted gate config.

## Verification checkpoint

- `PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s backend/scripts/mcr -p test_in01_provision.py -v`: exit0,19 offline checks.
- `python3 -B -m py_compile backend/scripts/mcr/in01_provision.py`: exit0.
- `git diff --check`: exit0.

Initial offline run rejected macOS's symlinked `/var` temp ancestor; test fixtures
now use canonical paths while retaining explicit arbitrary-symlink refusal.
No permission check was relaxed. Historical build receipt600582d6 does not match
current shared-target Gateway/Auth/Friend bytes; those stale pins were rejected
before any launch. Current qualified copied binary/receipt paths are requested
from Delivery/World SL. Native Auth/event/config execution is not inferred from
these offline checks. Exact final commands, evidence hashes and handoff ancestry
will be appended after qualification.


## Owned fixture execution and runtime preparation follow-up

At clean9ad192c, fresh PostgreSQL16.14 initialization passed with all five exact
fixture migrations, OPEN revision1 event, no synthetic participants/admissions/
rooms/tickets/memberships/leases, an actually rejected revision-invalid update,
existing-database reattachment refusal and validated owned cleanup. Sanitized
receipt `/private/var/folders/j4/41h_5mjj7j9d8f2j2bzcsngw0000gn/T/mcr-in01-event-proof-3dz8axz9/receipt.json`,
SHA256`f053c1a38bed8785e59ceb6d42736ac639ae4d649358ae99e4c2d45c144aca5f`.
This executes event initialization only; Auth remained NOT_RUN in that fixture.

Owned foreground mode now also exposes the unchanged launcher's private Control
protocol via `ready.json`, allowing actual owner status/stop while holding the
provisioned accounts. `private.json` includes the freshly owned Concert database
URL for explicitly coordinated runtime ownership, never in the sanitized receipt.
After stack close the old ready descriptor is stale and must be rejected.
A control shutdown failure still invokes database/process cleanup and cannot
produce a green result.

For a usable full migration ledger, optionally supply BOTH `--concert-binary`
and `--concert-build-receipt`. The receipt has the same successful source/binary
binding shape with a `concert-service` entry. Before launch the binary bytes are
checked. The new Concert database is migrated/verified through the actual CLI
and its full SQLx ledger compared with current source checksums; only then is
the event fixture inserted. This path remains NOT_RUN until qualified Concert
artifacts are supplied. It is distinct from the bounded direct-schema event
proof above and starts no unconfigured Concert service by itself.

Needed owner seams remain precise: a qualified copied six-service binary/source
receipt; optional current Concert migration artifact; actual Gateway→Concert
bootstrap/rooms/ticket and WS routing; qualified service eligibility/current
owner adapters; reviewed selected-source-pack avatar runtime plus map/skin
release and genuine art receipt. The provisioner does not acquire any of those
owners' writable paths.18 offline tests now additionally verify inherited-secret
scrubbing, invalid-event preflight with zero allocation, and control-error cleanup.


Offline compatibility was additionally executed through the unchanged frontend
`loadConfig` implementation with Node24.22.2 at
`/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node`:
`--experimental-strip-types --input-type=module -e <import loadConfig; await loadConfig(privatePath)>`.
Exit0; exact closed shape, private ownership/mode, two distinct offline identities
and receipt byte pins accepted. This used explicitly offline shape-only values
and existing documentation bytes as hash inputs; no product art approval,
provisioned real user or browser/world execution was asserted. No frontend file
or dependency was written. Planning integrity `python3 scripts/mcr/check-plan.py`
also exited0 with all13 source archive hashes intact.


## Explicit historical Auth qualification mode

The world producer located an immutable six-service snapshot at
`/Users/lhs/Desktop/mcr-build/current-email-bundle/600582d-1793708008138257000/bin`,
with sibling `receipt.json`, SHA256
`766d7c6c6fd392c907bd1216768e2131b04d06ed4eb1fadafd1b1a2259066f0d`,
built from600582d6b5deda6037c190c303f80752ba4a6bb3. Its receipt contains1108
source hashes and all six exact binary hashes; build runtime_validation was
NOT_RUN, with separate historical email-flow execution evidence. It predates
the current Concert/world composition and cannot qualify it.

Default own-mode provisioning now requires all supplied build source hashes to
match current source as well as exact binary hashes. Any source drift is denied
before launch unless the operator explicitly passes `--historical-auth-only`.
That mode records every mismatching source path and labels its qualification
HISTORICAL_AUTH_ONLY_NOT_CURRENT_WORLD; it can never emit a world-gate config.
This permits meaningful real canonical signup/mail/verify/login against a pinned
isolated Auth server without pretending current runtime code or world integration
was executed. The unchanged launcher's current migration ledger verification
still applies; a schema mismatch is a real failure with owned cleanup. Current
world-ready artifacts remain unavailable according to the producer.
