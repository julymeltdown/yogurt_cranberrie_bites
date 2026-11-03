# IN01 qualified Concert provisioning checkpoint

2026-11-04: **IN01-CONCERT-PROVISION bounded delivery PASS; IN01 NOT_RUN**.
Two independent canonical Auth accounts completed real signup, captured email,
verification and login. The optional qualified Concert CLI applied and verified
all12 SQLx migrations, repeated migration without changing the ledger, and
verified persistence after an owned PostgreSQL restart. Bootstrap returned503
because Concert serving is absent. Cleanup passed; no frontend gate config or
live account lifetime remains. This evidence does not accept the World flow.

## Assignment, ancestry and ownership

- Worktree: `/Users/lhs/Desktop/mcr-worktrees/be-concert-provision`.
- Branch: `codex/mcr-be-IN01-concert-provision`.
- Exact base: `3e3b54a3aa6052536cc48b5afadc0992e7482084`.
- Implementation and native execution source:
  `2b4976319111bce84357a009ba94bdcee0730857`, direct child of that base.
- This documentation checkpoint follows that implementation commit. Its final
  commit is recorded in the delivery handoff; it changes documentation only.

The exact four owned paths are:

1. `backend/scripts/mcr/in01_provision.py`
2. `backend/scripts/mcr/test_in01_provision.py`
3. `docs/mcr/backend/in01-provisioning.md`
4. `docs/mcr/backend/in01-concert-provision.md`

No shared helper, service, API/proto contract, manifest, lock, infrastructure,
frontend, asset, source archive, build bundle or target was modified. No Cargo or
build command was run. Only Delivery publishes master, serially integrating this
committed handoff. The former Concert build checkout and artifacts remain frozen.

New orchestration uses Python's standard library and the existing read-only
`dev_stack`, `browser_auth_smoke`, `migration_smoke` and `runtime_smoke` helpers.
No new package is required. Existing migration SQL and fixture/event patterns
retain their provenance in [the provisioning guide](in01-provisioning.md).
Their current bytes are pinned by the receipts below. Product scope still comes
from `MCR_IMPLEMENTATION_PLAN_V2_EN.md`; this checkpoint does not reduce IN01.

## Behavior and regression boundaries

The new owned database name, `mcr_concert_in`, satisfies both the unchanged
Cluster namespace guard and the current Concert CLI's `mcr_concert_` URL guard.
An existing database is refused rather than adopted.

Default mode remains `BOUNDED_DIRECT_SCHEMA_NO_SQLX_LEDGER`: exactly five direct
schema files and an OPEN revision1 fixture event. It makes no SQLx ledger or
Concert restart qualification claim. Supplying both `--concert-binary` and
`--concert-build-receipt` (alias `--concert-receipt`) selects
`CURRENT_BINARY_FULL_SQLX`. Invalid optional receipts fail before services start
or accounts are created. This mode requires a newly owned, marker-validated
Cluster capability; attach cannot migrate, restart or initialize its owner's DB.

Full mode runs actual `--migrate`, `--verify-migrations`, repeat `--migrate`, and
`--verify-migrations` after restarting only the fresh owned PostgreSQL cluster.
It compares all12 ledger entries to current source, checks that the postmaster
identity changed, verifies the generated OPEN revision1 event persisted, and
requires all six services to recover readiness. CLI/ledger failures retain a
fixed diagnostic and phase, execute owned cleanup and never fall back to direct
schema initialization. No Auth identity, admission, role, participant, membership
or ticket is synthesized. The existing bodyless GET, HTTP error refusal,
historical six-service qualification, attach ownership, no-art and private-output
guards remain enforced.

The Concert build receipt must identify a successful build, valid source SHA,
exact executable SHA256 and the exact complete currently tracked backend source
path set. Unsafe/missing paths, malformed hashes and any runtime/helper/SQL/
dependency/lock drift fail closed. The only permitted differences are the two
literal provisioning driver/test paths, with both build and execution hashes
recorded. There is no general seventh-service historical bypass. The existing
six-service validator remains strict by default and is unchanged.

## Frozen artifact bindings

Six-service bundle:
`/Users/lhs/Desktop/mcr-build/current-service-683484b/bundle`.
Build revision: `683484baaf78792292337ad0c197a5d574536db3`.
Receipt: `/Users/lhs/Desktop/mcr-build/current-service-683484b/build_receipt.json`.
Receipt SHA256:
`beed9cfaad633a9c9e00cd85cbf4af21b9572a1eae03c94680cd067969ac27dc`.
All1,286 pinned backend paths match; the recorded drift list is empty and scope
is `SOURCE_HASHES_MATCH_BUILD_RECEIPT`.

Concert binary:
`/Users/lhs/Desktop/mcr-build/concert-current-11d7cb0/bundle/concert-service`.
Build revision: `11d7cb00165f78b2ec9c564dd93669edad104e88`.
Binary SHA256:
`db5d0e0db054e06a9be25361da8ddb33c791a85c96d4189f9d0a194ebce2c100`.
Receipt: `/Users/lhs/Desktop/mcr-build/concert-current-11d7cb0/build_receipt.json`.
Receipt SHA256:
`bc29f79ae045ce0424c28bed47e80b6e384c8f2ec4da93575ee68a706d3bf4bc`.

Concert's manifest contains1,288 paths. Runtime source pins match, with exactly
these two driver differences recorded; this is not a zero-drift1,288-file claim:

| Path | Build SHA256 | Execution SHA256 |
| --- | --- | --- |
| `backend/scripts/mcr/in01_provision.py` | `040de8a51c27495c38df1a63174473b77d83312e4a8de5a178f8ac6bb3633559` | `61a9d0855ca540788bc267e5d560ecdd727a6839cb9b5f1e6d83e75f71fdf56b` |
| `backend/scripts/mcr/test_in01_provision.py` | `6f2e7defbf446427c432b5bbfeec2f802345d65f5f76e6e9dba84b897d588324` | `f0b4966390cdcf636122141e07363c9b2a878381772514b7158f49978dec64c1` |

Scope: `RUNTIME_SOURCES_MATCH_BUILD_RECEIPT_DRIVER_DIFFERENCES_RECORDED`.
`execution_driver_source_revision` is separately recorded as `2b497631...`;
Concert's `source_revision` stays `11d7cb001...`. Neither is silently relabeled.

Keep the read-only compiled-in migration directory available:
`/Users/lhs/Desktop/mcr-worktrees/be-concert-build/backend/services/concert-service/migrations`.
The copied binary resolves those files when executing migration commands.
Do not delete that checkout as routine worker cleanup.

Prior qualification remains separate and frozen:
`/Users/lhs/Desktop/mcr-build/concert-current-11d7cb0/qualification_receipt.json`,
SHA256 `2fc1b061f9631cd0f9ef84c6bcef0ce2e5db469cd65c965bfc6e2cfcf62fe7aa`;
and `migration_receipt_rerun.json` in that directory, SHA256
`b27ad7336ef50e37348742ee5be3d690ab20bb8d21cc1034f81289931a944db9`.
The new fixture's12 SHA384 migration digests equal that prior qualification's
full checksum map. These retained artifacts were read, never regenerated.

## Actual native execution

From the clean committed implementation checkout:

```sh
PYTHONDONTWRITEBYTECODE=1 python3 backend/scripts/mcr/in01_provision.py owned \
  --bin-dir /Users/lhs/Desktop/mcr-build/current-service-683484b/bundle \
  --binary-receipt /Users/lhs/Desktop/mcr-build/current-service-683484b/build_receipt.json \
  --concert-binary /Users/lhs/Desktop/mcr-build/concert-current-11d7cb0/bundle/concert-service \
  --concert-receipt /Users/lhs/Desktop/mcr-build/concert-current-11d7cb0/build_receipt.json \
  --release-revision 2b4976319111bce84357a009ba94bdcee0730857 \
  --output-dir /private/tmp/mcr-in01-concert-provision-2b49763-20261104 \
  --hold-seconds 0
```

Exit0. Actual interval: `2026-11-03T18:16:37.208639+00:00` to
`2026-11-03T18:16:47.728451+00:00` (2026-11-04 KST).
PostgreSQL `16.14 (Homebrew)`, new owned root, ports and output namespace.

Sanitized evidence:
`/private/tmp/mcr-in01-concert-provision-2b49763-20261104/receipt.json`.
SHA256 `b57b2a05bf12b1efae60229d640088f9c9233145ea046c3c67b0863b17646025`.

| Observed check | Result |
| --- | --- |
| Account0 signup / verify / login | HTTP200 /200 /200 |
| Account1 signup / verify / login | HTTP200 /200 /200 |
| Real verification email capture |2 messages; both private stages AUTHENTICATED |
| Canonical user IDs |2 distinct IDs; signup/verify/login identity checks passed |
| Full migration / verify / repeat / verify after restart | CLI exit0 for all4 commands |
| Exact ledger before / after repeat / after restart |12 /12 /12 entries, versions1–11 and16 |
| New postmaster / event persistence / six-service recovery | PASS / PASS / PASS |
| Authenticated bootstrap | HTTP503; `REAL_BOOTSTRAP_OR_ROOMS_UNAVAILABLE` |
| Default room and World | NOT_RUN |
| Synthetic roles, admissions, participants or tickets | None seeded |
| Gate config | NOT_WRITTEN_REAL_WORLD_AND_ART_PREREQUISITES_REQUIRED |
| Provisioner / canonical accounts / cleanup | PASS / PASS / PASS |
| IN01 | NOT_RUN |

Post-run checks verified output0700, all five files0600, two distinct canonical
IDs, and no generated email/password/access-token/cookie values in the sanitized
receipt. Private account state was read only for boolean/counter verification;
no credential values are included here. All recorded process stops exited0 with
`forced:false`, and the owned stack root is absent. No `config.json` exists.
Receipt lifetime is `ENDED_ACCOUNTS_AND_EVENT_REMOVED_WITH_OWNED_CLUSTER`:
private checkpoint files are sensitive local evidence, not usable live inputs.

The frozen six-service receipt, Concert receipt, Concert binary and prior final
qualification receipt were rehashed after the native run and match the pins
above. No native retry or artifact mutation was required.

## Validation and limits

```sh
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover \
  -s backend/scripts/mcr -p test_in01_provision.py -v
python3 scripts/mcr/check-plan.py
git diff --check
```

All commands exited0. Focused tests:36/36, retaining27 previous checks and adding
9 Concert checks for exact driver differences, runtime/lock/binary/manifest/path
refusals, successful full repeat/restart, bounded-mode distinctions, pre-start
receipt refusal, command failure and ledger mismatch cleanup/no fallback.
Planning integrity passed, including all13 original archive checksums and
required task/IA coverage. Planning checks do not certify application behavior.

Remaining prerequisites include composed Concert serving, real Gateway-to-Concert
HTTP/WS routing, reviewed current owner/eligibility adapters and release/trust
inputs, selected-source-pack avatar and map/skin resources, reviewed default
room, genuine art approval, and the real two-user browser/World gate. Virtual
movement, admission, capacity, tickets, visibility and reconnect were not run.
No legacy art, invented room, role, ticket, release bundle or live credential was
used to hide those prerequisites. This bounded delivery leaves full IN01 and
release acceptance with their actual integration owners.
