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
