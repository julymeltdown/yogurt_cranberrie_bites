# Wall B backend implementation

Status: IN_PROGRESS. Task `WALL-B` implements the human-selected complete option B
in `docs/mcr/product-decisions/wall-reference-tools.md` at source
`7d5d91bd1121b1a95b2302ca02e9224019a5deaa`. Performance/load qualification is
DEFERRED_BY_USER; functional, security and integrity validation remain required.
No new runtime acceptance is asserted by a contract freeze.

## Ownership and delivery

Reused checkout: `$MCR_WORKTREE_ROOT/qa-database-races`; branch
`codex/mcr-be-wall-tools-b`. The original QA03 branch/proof is preserved.
Root `1fc` is the named publisher. The former Wall owner returned these paths
and works on C07 only. Root transferred exact mural engine/subscription, migration
0058, Wall-only composition/Gateway/contract/generator hunks and public generated
transport data; no other frontend consumer, C07, manifest, lockfile, proto or
source-archive ownership is inferred. No king/SL review or acknowledgment gate.

Work: WALL-B-01 frozen closed contracts; WALL-B-02 bounded cell planning;
WALL-B-03 incumbent PostgreSQL engine/pencil budget and history; WALL-B-04 actual
canonical scope/native witness/production composition; WALL-B-05 meaningful
functional database/HTTP/restart/IN04 compatibility validation; WALL-B-06 evidence,
committed publication handoff and concrete fixes. Required work stays active.

## Wire boundary

The old PlacePixel, pixel acknowledgment, snapshot and delta schemas remain
unchanged. New member operations are `eraseConcertMuralPixel` POST `/mural/erase`,
`fillConcertMuralRegion` POST `/mural/fill`, `undoConcertMuralAction` POST
`/mural/undo`, `getConcertMuralTools` GET `/mural/tools`, and
`getOwnConcertMuralAction` GET `/mural/actions/{idempotencyKey}`, all beneath the
canonical event prefix. Commands are closed, at most 1,024 bytes, and require the
original key and global expected revision. Undo uses `target_action_id`.

`mural.ToolCapabilities` is a version 1 private actor/event/revision-bound read.
It reports explicit availability, confirmed background index 0 or null, fill cap
256, durable rolling allowance 512/600 seconds, remaining cells, database time,
next cell/cooldown availability, and latest/unresolved own action correlation.
`mural.OwnAction` binds original operation/body/key, actor/event, receipt/action,
revision/count/commit/cooldown/undo expiry and reset requirement. It discloses no
before-colors or other authors. All new tool successes require a snapshot reset;
old pencil successes retain their existing public operation and acknowledgment.
GET result recovery never POSTs, invents a key, or substitutes cell equality for
receipt and original-command correlation. Missing capabilities fail closed.

Closed new reasons are MURAL_NO_CHANGE, MURAL_FILL_TOO_LARGE,
MURAL_BUDGET_EXCEEDED, MURAL_UNDO_EXPIRED, MURAL_UNDO_NOT_LATEST,
MURAL_UNDO_UNAVAILABLE, MURAL_TOOL_UNAVAILABLE and MURAL_ACTION_UNRESOLVED.
Legacy pencil maps no-change and cell-budget denial to its existing INVALID_INPUT
and RATE_LIMITED-compatible path. Failed/no-change/conflicting actions charge no
successful cell budget or cooldown; attempt limits remain independent.

## Generated baseline repair

Root explicitly delegated a separate baseline repair commit `0e75917206d716b4f7f80825f9d5526a3c11f35a` before
Wall contract outputs: only current C05 Realtime_world_PolicyInvalidated,
Realtime_world_ServerMessage and Realtime_world_PolicyBaseline definitions in
both OpenAPI copies, plus the existing getConcertIntro projection in the backend
copy, the canonical realtime policy projection in both copies, and the backend
domain policy version 0.1.2→0.1.3. No C05 or Intro source was modified. Existing producer ancestry is preserved
under the source revision above. Exact input/output provenance follows:

```json
{
  "source_revision": "7d5d91bd1121b1a95b2302ca02e9224019a5deaa",
  "inputs": {
    "contracts/realtime/v1/world.schema.json": "10d328b555e24f444454bd1d01770bdc4acc3aabeeca8d6a3c5fb10281f2a794",
    "contracts/domain/v1/operations.json": "f801d703dfc5509bbe06a2f0fa07a170b6442a48b2204c7ead460fc9a3600112",
    "contracts/realtime/v1/policy.json": "ef61bfba5337a06216e98a9d3eba4ed952dfb405815088b926832276d2464cd3",
    "backend/scripts/docs/generate-mcr-domain.py": "587d18f45f40f1ee54e051e5cf70bc39786426c94cf140a334f23c30e2cebb03",
    "backend/scripts/docs/generate-mcr-core.py": "91de68ad25bfd695690ab5a61f5de81a27ff0efd49ed45db7fad0fa560bec93e"
  },
  "outputs": {
    "contracts/generated/mcr-domain.openapi.json": {
      "old_sha256": "905d6f5b6f66f95cb68499a5fbfb32802539fce515f421226d79cb375ecfb887",
      "baseline_repair_sha256": "4c185d9099b7e02cc61fce1e58b475be169e78bd5ba20b295e99827fc07e034f",
      "wall_generated_sha256": "e7de84d358c18ae3b0431953b582e4f305a1f89c5146b771d7da9cab04e159ef",
      "top_level_delta": [
        "x-mcr-realtime-policy",
        "components"
      ]
    },
    "backend/docs/api/mcr-domain.openapi.json": {
      "old_sha256": "85082d7267ad83f3a8d7f3f4a71107722fbf4d3025764a0fe3be2d819e151a23",
      "baseline_repair_sha256": "4c185d9099b7e02cc61fce1e58b475be169e78bd5ba20b295e99827fc07e034f",
      "wall_generated_sha256": "e7de84d358c18ae3b0431953b582e4f305a1f89c5146b771d7da9cab04e159ef",
      "top_level_delta": [
        "paths",
        "x-mcr-realtime-policy",
        "x-mcr-domain-policy",
        "components"
      ]
    }
  }
}
```

## Validation state

All 101 existing domain regression tests and five focused Wall contract tests
pass (exit 0). The domain validator passes
114 operations and 55 fixtures, including eight new Wall fixtures; runtime status
remains NOT_IMPLEMENTED in this planning validator. The first full domain run preserved three count failures
(47→55 fixtures, 135→140 bundled operations, 164/19→173/21 domain/realtime
definitions). These are the exact added Wall inventory and approved pre-existing
C05 baseline repair; assertions still verify every registered schema/operation.
A follow-up retained one additional duplicate-ID expected-count failure
(135→140), corrected to the exact new operation inventory. Generated transport
metadata and OpenAPI reproduction checks pass (exit 0). Native/Cargo/PG/browser jobs are NOT_RUN: no resource lease is held.
The allocator must offer a usable bounded validation round before that work;
source implementation proceeds independently. Frozen-source qualification will
record commands, exit codes, source/binary pins, failures and actual limitations.

Baseline qualification: first frozen-readset generator check exited 1 (stale
policy projection), retained in private evidence. After the exact output-only
repair, unchanged generator generation and `--check` both exited 0 against a
byte-exact public7d5 readset. Both mirrors equal the full canonical postimage.
The source-qualified baseline was handed to Root independently of Wall work.


## Runtime candidate (not yet qualified)

The implementation extends the existing mural row and BE15 receipt transaction.
Migration 0058 adds explicit immutable index-0 provisioning, private action/body
correlation and before/after history, a durable charge ledger independent of
receipts/history, and a same-transaction native witness tied to confirmed real
Auth/Friend owner registrations. Existing grid and palette bytes are unchanged
by the migration. Operator provisioning is an explicit insert of the qualified
event ID into `concert_mural_tool_config(event_id, background_index)` with index
0; no default palette, implicit migration activation or client provisioning exists.
The production mode requires `CONCERT_WALL_TOOLS_B_MODE=live`, the incumbent
shared canonical composition and `CONCERT_WALL_TOOLS_B_RECEIPT_KEYS_JSON` with
one active and optional retained signing keys. Configuration values are secrets
and must not enter product responses or evidence.

The producer uses the actual original parent and its OwnerGuardScope, current
canonical Auth/Friend policy, active event admission, root-row serialization,
DB time, cooldown, all affected locks, exact revision and shared allowance.
Successful new tools advance reset_revision and the snapshot identity rather
than inserting a false single-pixel public operation. Pencil keeps its original
ack and delta while sharing actual cell charges and private history. Read-only
registered Wall command status reauthorizes on its own native transaction;
lookup never executes a command or inserts a new correlation.

New native test/support/controller paths record source provenance for the real
canonical mTLS fixture adapted from Intro at the base revision. The controller
requires committed source and an outside-target frozen binary with exact SHA,
private socket-only PG16 identity, actual migration receipts, exact test inventory,
no prepared transactions after each case, restart-state preservation and cleanup.
The allocated Cargo and PG rounds have not run. Fixture source covers fill
256/257/no-op, shared undo/pencil allowance, idempotent replay and receipt/action
recovery after producer recreation, reset delta, and injected storage rollback.
Additional source cases cover safe activation against recent unaccounted legacy
pencil activity, simultaneous same-actor tabs, stale revisions,
shared cooldown, pause and admission suspension; these remain unexecuted.
Remaining native coverage includes cross-actor/lock/control races, expiry and
canonical authorization changes; actual Gateway/startup/Phaser/two-client IN04
remains open. Current C05 has no mural server-message variant; no protocol event
or second world connection has been added. The shared provider owner is resolving
its existing protected polling/qualified world binding with the backend owner.


The shared frontend provider's capability freshness ceiling is 5,000ms from
monotonic request start, never response arrival or server_time. An already
expired response cannot open the editor. Shorter parent/release/content deadlines
win. Expiry or read failure, abort, visibility loss, actor/event/privacy/generation,
room/epoch/authority change or release close withdraws the editable projection;
visible qualified providers refresh before expiry (for example every two seconds).
This is a client UI freshness ceiling, not a grant or a substitute for the
atomic current server checks on every command. No new wire expiry is implied.
Root granted one additional owner_media.rs fail-closed match arm solely to
reject the new Wall Operation variant from the existing media producer.


Explicit provisioning also locks the mural root and rejects activation while
any legacy pencil operation in the preceding 600 seconds lacks its durable
one-cell charge. Historical before-colors cannot be reconstructed truthfully.
Use a new mural or wait for the untracked legacy window to expire before the
operator insert; no historical budget is silently forgiven or fabricated.
Concurrent activation and pencil writes serialize on the same root. Once
provisioned, every successful pencil uses the shared history/charge transaction.


## Additional public baseline publication

C07's next preflight exposed stale existing transport metadata. A separate
single-file baseline commit `efc1f56b19d3cd33d64a0aff6a960f0b772277bf`
was generated against byte-exact public `3cbaeae5e5c33d4ef3327b6f0f3e0f27c9544254`
inputs with the unchanged public generator. It changes only one input-hash
comment in `frontend/src/shared/api/domain-operations.generated.ts`, preserves
mode 100644, and contains no private Wall contract or runtime work. The complete
readset reproduced metadata drift (exit 1), regenerated (exit 0), and passed
`--check` (exit 0, 109 operations). An initial incomplete readset omitted the
transitive core schema; its FileNotFoundError was preserved separately before
correcting the frozen readset. The qualified output SHA-256 is
`5abefd1e27a1472291b706eed0ddb7c1c06cc9290ea32d105844caaa0ed66308`.
Root reported the exact commit published atop the frozen public base with its
canonical publication receipt. No additional owner proof loop was requested.
This does not qualify C07 or the Wall runtime.

## Pending allocated round

Wall has its own queued allocation, behind the recorded OwnReports and Party
cohorts; Root's combined executable cohort grants this owner no resource lease.
No Cargo target is inspected or inherited. The requested build round uses Rust
1.88.0, locked/offline, one job, development/test debug information off and
incremental off. The exact accepted target, deadlines, byte allowance, free-space
floor and guardian become part of evidence before any Cargo start. Relevant
commands target domain Wall unit cases, application command regressions,
Concert library/startup compilation, `--test mural_tools_b` (one pure case and
seven ignored canonical native cases), and Gateway Concert product route tests.
The executable, source manifest (including the contract oracle), checksums,
commands/exits and failures must be frozen outside the target before unconditional
build-resource release and a separate accepted native PG16 round. No native
job runs during the Cargo lease. Candidate config tests also reject missing
shared composition/keys, unknown modes, malformed/duplicate/nonzero-key violations
and retain key rotation. All Rust/native cases remain NOT_RUN.


A source compatibility pass found delta admission rejecting percent-encoded
forms accepted by frozen C04: `after_revision=%31` and `%61fter_revision=1`.
The backend parser/runtime and Gateway now decode the one closed field for
validation while retaining the exact original signed target bytes. Zero,
leading zeros, plus/whitespace, duplicate decoded aliases, malformed encodings
and values beyond MAX_REVISION remain rejected. The existing pure/native/Gateway
candidate cases now include those encodings; inventory is unchanged. The actual
C04 target oracle confirmed the three equivalent valid forms and rejected the
leading-zero and duplicate-alias examples. Rust/native execution is still queued.


The source SQL integration pass also found the new application command registry
was not yet reflected in BE15's `concert_command_operations` table. Migration
0058 now inserts exactly the three frozen mutation IDs before tools are used;
otherwise the receipt foreign key would reject every new action. Existing
canonical Fill/Erase/Undo native cases exercise that actual FK path when run.
No historical migration or unrelated operation registry entry is changed.


The expiry/retention candidate uses a private five-second receipt TTL fixture at
INSERT, then removes that fixture and preserves actual immutable correlation.
The action still goes through the actual canonical producer. It checks clipped
undo expiry, waits past that original expiry, rejects undo without side effects,
runs the actual BE15 expiry retirement, deletes only the expired private action,
and verifies the charge cannot be deleted during its live window. Capabilities
must retain the consumed 256 cells; a second 256-cell action reaches 512 and the
next one-cell edit is denied. This covers receipt/history retirement without a
synthetic clock or fabricated authority. It remains NOT_RUN until allocated PG.

## Native allocation enforcement

The native controller now requires the exact already accepted allocation ID,
deadline, free-space floor and owned-byte ceiling on its command line. These
arguments record an allocation; they do not grant one. It measures only its
marker-owned cluster, without following links or inspecting Cargo targets.
Every native test runs in a controller-owned process group with checks at
250 ms intervals. Deadline, floor, ceiling and per-test timeout failures stop
that live child group, retain redacted diagnostics in the report, and reach the
existing owned-cluster cleanup. Cluster startup, PKI and restart also have
checks around their bounded helper calls; a whole-round external guardian is
still required if the accepted offer requires a hard deadline across those
helpers. The controller does not implicitly extend an accepted allocation.

Source-only guardian tests pass (two cases, exit 0), including an injected
free-space breach that stops its own live child, retains its failure/output,
and leaves a separate process running. This exercises no Cargo, PostgreSQL or
browser. Native behavior and cleanup under actual PostgreSQL remain NOT_RUN.

## Contract-only publication packet

The frozen 21-path contract commit is
`c18d5ba9fd27a73356d9be68614863172303c81f`. A selective public readset at
`db661820d6af1770839cfda8d0dc6b9ada1ece1b` (237 files, 3,801,760 bytes)
passed the unchanged OpenAPI and transport checks before the overlay. Applying
exact C18 postimages then passed 101 domain regressions, five Wall regressions,
the domain validator (114 operations/55 fixtures), and both generated checks,
all exit 0. Both unchanged generators reproduced the exact three C18 outputs.
There is no current-public projection adjustment at that recorded revision.
The TypeScript blob is `69546e8f5ddf0dc4fc7e0acd6c0980646bfe2ad5`, SHA-256
`29496fd06ebb6f8567499a980c793b778ca3580e6eae1230bf7d3ddb49937117`;
both OpenAPI mirrors retain the Wall output hash recorded above. Root received
the exact path list, public/source/output manifests and command/exit records.
This handoff excludes later engine, native, startup and migration commits and
confers no runtime acceptance. If publication inputs change, retain unrelated
operations and regenerate from the receiving public base with the unchanged
generators rather than overwriting newer contract work.

The first allocated Rust round passed all five domain Wall cases and exposed
one application regression inventory assertion still expecting 59 mutations.
Root explicitly transferred only that test hunk: the inventory is now 62 and
checks all three new Wall POST paths, preserving unknown/read-operation refusal
and every other case. The original failure is retained in the allocation logs;
this paragraph does not claim its rerun or subsequent runtime checks passed.

The application rerun passed eight cases. Service compilation initially reused
cross-checkout domain/application artifacts missing the public Wall modules;
refreshing only the four already-owned mural module timestamps preserved their
bytes/mode/ownership and caused a successful configuration compile/rerun. The
metadata and original failure remain in private allocation evidence. Native
qualification can emit a new private `--live-owner` receipt containing its exact
cluster inode and marker hash, allowing an independent deadline guardian to
rescue only that owned cluster. Contract runtime checks use the retained strict
Python environment with URI and date-time checkers, without dependency changes.

## First native qualification disposition

The first build round stopped at its 1 GiB resource ceiling during the Gateway
build; its measured new peak was 1,078,116,352 bytes. Target and full release
occurred at 17:05:00 UTC; no binary copy or PostgreSQL job ran in that round.
The allocated narrow continuation reused the passed domain/application/config
and startup evidence, froze one native executable before Gateway, and passed all
nine Gateway cases. Its build resource peak was 1,009,004,544 bytes, under the
new 2 GiB allowance. Target release was 17:16:39 UTC with zero open handles.

The copied binary at source `bb40c0eac1fffd038dd2bec59fa8abdfc24e8339` then
passed the pure case and three canonical native cases: fill/undo/budget/recovery,
257-cell/no-op/region-lock atomic denial, and pencil/shared-budget/storage-failure
rollback. Sixteen actual responses passed the strict C04 oracle and every
completed case had zero prepared transactions. The fourth case exposed a test
fixture error: its per-call global zero-prepared assertion observed the other
deliberately concurrent request still running. Each request still performs its
actual original-scope settlement; global zero-prepared validation now occurs
after both joined requests finish. Serial calls retain the per-call check.
This is a candidate test fix, not a passing native rerun. All failures remain
recorded. Native cleanup passed, the private PostgreSQL root was removed and
the guardian reaped at 17:17:27 UTC. The later native cases, physical restart,
production default-app/two-client flow and IN04 remain unaccepted.

## Completed native qualification and publication handoff

The corrected executable at source
`cd7f658407c1c0a9cda9687226e23cde7758b3c8` passed its pure case and all
seven canonical native cases, exit 0, using PostgreSQL 16.14. The strict C04
oracle accepted all 35 actual producer responses. Every case finished with zero
prepared transactions. Actor/event budget isolation, concurrent settlement,
receipt expiry and history retirement passed alongside the original three cases.
All seven cases applied the actual migration chains: Auth 18, Concert 44,
Friend 18 and Profile 9. These are native canonical Auth/Friend transaction and
Wall Product facade results, not synthetic successful authority objects.

The controller stopped and restarted its actual PostgreSQL postmaster
(PID 90734 to 94179, distinct server start times), recovered unchanged durable
state digest `766287a13b0bf41d0ed6e4dbc60d2376`, and passed its post-restart
check. It removed its exact marker-owned root and reaped the independent guardian.
The frozen executable SHA-256 is
`d711b161e5ecbb75dec518cbeb1646ccadf5ee7a90fece592c67f46d44f55d69`.
Its source checkpoint records 3,637 file hashes, including build/dependency,
migration, contract and native-oracle inputs. No target access occurred after
release, and the copied executable and checkpoint were checked for drift.

The final accepted resource round retained previous failed outputs. Build NEW
peaked at 739,631,104 bytes; the native round peaked at 837,779,456 bytes,
21 processes and 511,508,480 bytes RSS, within the original 2 GiB NEW/12 GiB
whole allowance. Actual target release was 2026-11-04 17:23:56 UTC with zero
compiler/target handles, sent directly to the next owner. Native FULL release
was 17:24:42 UTC. No resource remains held. The earlier resource breach and
fixture failure above remain historical failures; this passing rerun supersedes
their pending qualification disposition.

The retained focused Rust results are domain five, application commands eight,
service configuration two and Gateway nine, all passing, plus the production
Concert executable `cargo check` exit 0. Rust 1.88, locked/offline single-job
builds used debug info 0, incremental off and the retained contract-package
optimization overrides. The only changes after those focused checks were the
native checkpoint closure and concurrent-test fixture correction; the final
native executable compiled that corrected source successfully.

Committed evidence is
`backend/scripts/mcr/wall_tools_b/native-qualification-2026-11-05.json`: exact
source/executable pins, commands/exits, per-case results, migration counts,
resource/restart/cleanup receipts and hashes of the private detailed evidence.
Private round evidence remains in `/private/tmp/mcr-wall-race-fix-2x7_ll20`;
earlier failed rounds remain in `mcr-wall-compile-gu7d9h2_` and
`mcr-wall-retry-ifm40w2y` under the same temporary directory. Private locations
are diagnostic evidence, not portable deployment inputs.

This backend scope is ready for the named publisher Root to integrate directly
with its current shared-file owners and relevant combined checks. Frozen C18
contracts are already published as `e7bd68cfa35fb620e2eed060a71a72e50718b9dc`;
do not overwrite newer public contracts with this branch's older complete files.
Actual startup/Gateway HTTP, default App with persistent Phaser, two real clients
and complete IN04 remain NOT_RUN. Performance/load qualification remains
DEFERRED_BY_USER. The separately assigned initial mural seed producer remains
an open follow-up; this handoff does not qualify an unprovided art pack.
