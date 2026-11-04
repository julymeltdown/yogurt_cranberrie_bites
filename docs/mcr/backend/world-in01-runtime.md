# Isolated World startup preparation

Status: `IN_PROGRESS`. World remains `CLOSED`; IN01 two-client execution is
`NOT_RUN`. Optional Age, Social, Gift, Feed and media modes are not prerequisites
for this preparation.

`backend/scripts/mcr/world_in01_prepare.py` starts the existing disposable native
DevStack with private PostgreSQL, Redis, six service binaries and the local Auth
mail sink. It creates two distinct principals through actual Gateway signup,
email delivery, verification and login. No Auth rows, JWTs or sessions are
fabricated. Access tokens remain in memory. Separate mode-0600 principal files
contain credentials for subsequent real login, inside the owned mode-0700 root.

The optional `--concert-bin` path runs the actual Concert `--migrate` and
`--verify-migrations` commands against a newly created `mcr_concert` database and
checks the exact source migration ledger. It inserts operator configuration only:
the event and one two-person room with selected map4/avatar pins. It asserts that
admissions, participants and avatars remain empty. Actual authenticated bootstrap
must create those rows. An earlier preliminary harness directly populated them;
that path was removed and supplies no public-flow evidence.

Release paths and their exact bundle hashes are verified before startup. The
existing source-bound Rust constructor was independently qualified by the
operator bundle checkpoint; checking file hashes here does not rerun it.

## Actual preparation evidence

The portable receipt is `world-in01-preparation.json` beside this document.
The final operator-only run used the already copied six-service refresh binaries
and the already copied Concert binary from source `188db7c9`. No target, compiler,
new binary or new World capability was involved. Concert's migration CLI loads
SQL from its configured source migration directory; the receipt pins that exact
25-file set separately from the older binary's compiled source.

Actual results:

- Six services migrated, verified, survived PostgreSQL restart and passed current
  readiness; local Redis and real Auth mail delivery operated.
- Two different canonical Auth identities each completed signup, verification
  and login through HTTP, with the expected real refresh-cookie attributes.
- Concert migration and receipt verification commands exited 0. Event/room seed
  succeeded; participant/admission/avatar tables remained empty.
- Current schema probe returned true. Removing the required avatar representation
  constraint inside a rolled-back transaction made it return false; restoring the
  transaction made it return true again.
- Owned processes and PostgreSQL stopped, and the private root was removed.

The external-migration schema probe previously required
`concert_avatar_contract`, which migration 0025 replaces with
`concert_avatar_representation`. The probe now checks the actual production
constraint. This corrected SQL was exercised against PostgreSQL; a new Concert
binary containing the corrected probe still needs compilation.

## Next executable seam

The new `concert-service --prepare-in01-catalog` command is authored source only,
`NOT_COMPILED` / `NOT_RUN`. It loads the actual full product release with
`ProductReleaseBundle::from_source_bound_files`, validates map4 through
`bootstrap::spatial::load_candidate`, publishes with
`PgPlaceRepository::publish_release`, and compares repository readback. It
creates no principal, participant, admission, ticket or listener. Its configuration
uses the six existing release file variables plus `CONCERT_SOURCE_BINDING_FILE`,
`CONCERT_ASSET_ROOT`, `CONCERT_WORLD_EVENT_ID`, explicit local mode and the owned
Concert database URL. `--publish-catalog` in the Python harness opts into this
command only when a newly compiled binary is explicitly supplied.

Example from the repository root, using absolute private copied-binary paths:

```sh
python3 backend/scripts/mcr/world_in01_prepare.py \
  --bin-dir /absolute/private/six-service-binaries \
  --concert-bin /absolute/private/concert-service \
  --report /absolute/private/new-preparation-report.json \
  --hold-seconds 300
```

While held, stdout provides only a local private-input path. The descriptor and
all credentials expire when the owned process exits; the default hold is zero.
No stale path is a live runtime handoff. The command always reports preparation
as `PARTIAL`, never IN01 acceptance.

Still required: qualified World native fixes and production owner registration,
fresh catalog-command build/run, real Profile initialization through the current
user's public Profile write, real authenticated Concert bootstrap, room/ticket
HTTP requests, and two actual WebSocket clients with revocation and shutdown
evidence. No synthetic source fixture or successful preparation waives them.
