# BE27 native-80 command failure diagnosis

Disposition: **concrete shared cause identified for both command failures**.
The first draft creation assigns a later application-clock timestamp, then
validates it against an earlier database timestamp. The owner's subsequent
native tracing confirms the precise failed predicate in both tests. The owner
has since changed the absent-row fallback to the same database timestamp;
native qualification of that correction remains the owner's responsibility.
This is a diagnosis handoff, not BE27 acceptance or an approval gate.

QA checkout `/Users/lhs/Desktop/mcr-worktrees/qa-intro-cursor`, branch
`codex/mcr-qa-intro-cursor`, starting commit
`671479a0c95103ef7e7d3dcd038546cf2bb34511`. Only this new report is writable.
The prior cursor report remains unchanged. The BE checkout was read-only;
QA performed no Cargo, dependency installation, binary execution, database or
server/process work, and edited no application, test, proto or manifest.

## Exact failure path

Both tests in `backend/services/concert-service/tests/intro.rs` begin with an
absent-row `IntroCommand::set`, expected revision 0:

| Native-80 failing test | Initial command | Source |
| --- | --- | --- |
| `receipts_revision_conflicts_and_invalidation_failure_roll_back` | Empty draft text, `null:null` bounds, no expiry | Lines 381–388; helper at 334–339 |
| `empty_publish_close_expiry_and_event_isolation` | Three spaces, same bounds and no expiry | Lines 450–462 |

These are valid **draft** inputs. `IntroDraft::validate_at` permits empty draft
text; expected revision 0 means absent. Fixture actors/events/keys are nonnil,
the event phase is LIVE, and its cutoff is one day ahead. The canonical request
uses the same event UUID for scope and `eventId`, with a registered set operation,
bounded integer revision and ordinary empty/space strings. Changing draft text
or inventing a different event binding does not address this failure.

The failing repository sequence is:

1. `Producer::apply` samples `db_now` with `statement_timestamp()`.
2. It checks the event cutoff and reads the current owner introduction: absent.
3. The new-row `created_at` falls back to **a later** `Utc::now()` call after those
   SQL awaits. State defaults to DRAFT and revision to 1.
4. `next.validate_at(db_now)` rejects `self.created_at > now` in
   `backend/crates/concert-application/src/intro/mod.rs:381`.
5. The adapter maps this to `CommandError::InvalidInput`.
   `PgCommandRepository::execute` propagates `producer.apply(...).await?`
   (`backend/services/concert-service/src/adapters/out/repository/receipt.rs:106`),
   and the introduction adapter maps it to `IntroError::Invalid`, displayed as
   `Invalid introduction input`. This aborts before the introduction insert and
   before the remaining duplicate/conflict/rollback or publish/close assertions.

Candidate 80 **already contains** `statement_timestamp()` and
`next.validate_at(db_now)`. Those changes alone did not solve it: creation and
validation still used different samples. No native-80 success is claimed.
Pagination passes because its rows are seeded by the separate raw-SQL publish
helper; it does not execute this new-draft producer path.

## Runtime confirmation and artifact limits

Verified the supplied native-80 receipt and immutable binary against their exact
hashes below. Receipt 80 runs four tests, two PASS/two FAIL, exit 101, with cleanup
PASS. It names both failures but has no failed-predicate tracing.

During diagnosis the owner's newer `/private/tmp/mcr-be27-native-82.json` became
available and was inspected read-only. It records the same two failures, LIVE
phase, `setConcertIntro`, `current=false`, `bounds=null:null`, no expiry, validation
Invalid, then receipt InvalidInput. Its redacted temporal metadata proves the
specific predicate:

| Test | Validation DB time, UTC | Assigned creation time, UTC | Difference |
| --- | --- | --- | --- |
| Empty/publish/close | `2026-11-03T21:12:57.018507Z` | `2026-11-03T21:12:57.018838Z` | +331 microseconds |
| Receipts/conflicts/rollback | `2026-11-03T21:12:57.165889Z` | `2026-11-03T21:12:57.166145Z` | +256 microseconds |

These exact UTC strings are recorded evidence, not locally displayed schedule
times. No identity, private draft payload or secret is logged in this report.
Receipt 82 was no longer present at a later read; its observed hash and necessary
redacted transition are preserved here. That later file read failed, not a native
test execution. Native 80 and its binary remained present at that check. Receipt
81, observed before 82, also located both failures at receipt InvalidInput but
did not contain the decisive timestamps.

## Deterministic minimal proof

Executed the following Python 3 standard-library predicate isolation in memory,
exit 0, without dependencies or file creation. It uses fixed integers rather
than scheduling or clock-skew assumptions, and verifies the observed metadata
arithmetic separately. This models the Rust guard; it is not a Rust/native rerun.

```python
from datetime import datetime

pairs = [
    ("2026-11-03T21:12:57.018507+00:00", "2026-11-03T21:12:57.018838+00:00"),
    ("2026-11-03T21:12:57.165889+00:00", "2026-11-03T21:12:57.166145+00:00"),
]
assert [round((datetime.fromisoformat(b) - datetime.fromisoformat(a))
              .total_seconds() * 1000000) for a, b in pairs] == [331, 256]

for text in ("", "   "):
    db_now = 1793740000000000
    def failures(created):
        return {
            "nil_ids": False, "oversize": len(text) > 1000,
            "nul": "\0" in text, "bad_bounds": False, "bad_revision": False,
            "created_after_now": created > db_now, "expired": False,
            "empty_non_draft": False, "contact_non_draft": False,
        }
    assert [k for k, v in failures(db_now + 331).items() if v] == ["created_after_now"]
    assert not any(failures(db_now).values())
```

All other flags reflect the exact valid fixture shape: nonnil identifiers,
revision 1, unbounded age policy, absent expiry and DRAFT state. The only failing
predicate is the creation timestamp comparison, for both text lengths 0 and 3.

## Minimal correction and verification owner

For a missing row, initialize `created_at` from the **same `db_now`** used by
`next.validate_at(db_now)`, e.g. replace `.unwrap_or_else(Utc::now)` with
`.unwrap_or(db_now)`. Preserve a stored row's original creation time on updates.
Keep the future-creation rejection, expiry/cutoff checks, authorization, revision,
receipt and transactional rollback behavior. Do not change valid empty drafts
or weaken either failing test to get a green result.

The sole BE author owns the fix and its native validation. During the final
read-only pass the author had already made that fallback correction and removed
the temporary tracing. The corrected repository snapshot has
`statement_timestamp()` at line 231, `.unwrap_or(db_now)` at line 274 and
`next.validate_at(db_now)` at line 303. This observed source change is not a
passing binary receipt. The focused regression should control the validation
time and later app time, then retain both native tests' remaining duplicate,
conflict, rollback, publish/close, expiry and event-isolation assertions.

## Source pins and detected changes

Owner checkout: `/Users/lhs/Desktop/mcr-worktrees/be-intro-discovery`, observed
HEAD `00ecd45999dacf29addf8ede9532060112eb9a03`. BE27 source is still working
content; HEAD alone does not identify the tested snapshot.

| Input | SHA256 / evidence status |
| --- | --- |
| Native-80 receipt, independently hashed | `98ec53c779e38e3dc5b5957d348d6da9a7bfcb936ecf870a085d37d69aaa738b` |
| Immutable candidate-80 binary, independently hashed and matched to receipt | `d7fb615cf5b76cfca91cc5f889915c50b9d12d64ca277d28acdbb2c7f11e8f13` |
| Candidate-80 repository source pin supplied by Main | `61efc1885a850b69ba808fd9bde2f40cc3cd87c5f8df1a4da55b34552c2a741f` |
| Initially inspected traced repository working file, independently hashed | `51d23b083b765d3a3b881926c0a8f7843cd47db136fecfe08596d18fbec843d8` |
| Later corrected repository working file, independently hashed | `02c039f2bc3a18f52580e887f59e1d67da05da01f2363f4d2345741d6bec454b` |
| Native-82 receipt, observed before it disappeared | `163be54c4401602c8c5d04284dd707ecbd5edbe4ae07f8b9eff9133bc446b30e` |
| Native-82 binary identity recorded by that receipt, not independently hashed | `0ec945b8c973708e4885b9a37ed4c11bf95e7a1abb509a49da00bd9e810fc03b` |
| Initially read `backend/services/concert-service/tests/intro.rs`, independently hashed, matches supplied candidate-80 pin | `0cf6d3feca83928f71fb4dd4c80bbc52c963995957d563c5556b895163a0fdf0` |
| Later owner-edited `backend/services/concert-service/tests/intro.rs`, independently hashed | `62386a18ef8d42d2f2f2a81227f700df386f4246c183785efd87ceea5299c45a` |
| `backend/crates/concert-application/src/intro/cursor.rs`, independently hashed, matches supplied candidate-80 pin | `c392f7843796d9fe6593a8313301e1d19354e4d333c34ced600fb2eb97e40471` |
| `backend/crates/concert-application/src/intro/mod.rs`, independently hashed | `d3f518330e277242a16ff8173b6c7b62ff7a3a805e4a0aaf61548ec3c44ba3d4` |
| `backend/crates/concert-application/src/commands/mod.rs`, independently hashed | `f9f46f950d148a51f9ad8f364196f4d006e47b4bb892c087912572284d12dc9c` |
| `backend/services/concert-service/src/adapters/out/repository/receipt.rs`, independently hashed | `01012f034b30aa5f53f43402feae9ca72048cbb526e727794dbed3dd5ca4355f` |

The initially read tracing file is not relabeled as candidate 80. Removing its
tracing in memory did not reproduce the supplied candidate-80 source hash, so
that attempted reconstruction was not treated as source identity proof. The
runtime attribution instead rests on the explicit later native-82 transition,
matching failing tests, direct validator source and deterministic predicate proof.
The report records the detected working-file transition to the corrected hash,
rather than claiming the writer stayed frozen throughout diagnosis.

A final hash stability assertion also detected a concurrent owner test edit to
the later test hash above; it was not treated as candidate-80 test evidence.
The corrected repository, cursor, validator, canonical-request and receipt files
retained their listed hashes in the subsequent snapshot. The owner reports a
later candidate progressed past these production failures to a separate
test-order issue and is adjusting its assertions. That is owner-reported
continuation, not a QA native receipt or an expansion of this diagnosis into
full BE27 acceptance. No test was edited by QA.

Direct findings and the minimal recommendation were sent to the sole BE27 author
and Main. Final delivery contains only this report, with staged path/whitespace
checks and clean checkout confirmed in the handoff. QA native fix validation and
full BE27 acceptance remain **NOT_RUN**. No build/database resources or source
ownership are retained by this QA assignment.

## Separate follow-up: fixture identity, authority and revision accounting

Main assigned this follow-up after the timestamp diagnosis goal was actually
COMPLETE. A new unbudgeted goal was created and returned ACTIVE. This section
addresses the remaining fixture; it does not reopen the confirmed timestamp
finding or add a native-acceptance gate. Source remains read-only for QA.

Observed native receipt 83 has SHA256
`2beb307ab4dcefd7f92ed387204f40184294837687f8d457dbddcaf4dd09abd4`:
three tests PASS, one FAIL, exit 101. The remaining empty/publish/close test
reports **Introduction authorization denied**, rather than the earlier Invalid
input. That receipt alone does not identify a close statement.

The initial follow-up test snapshot was the previously recorded `62386a...`.
It used `f.foreign_actor` with **`f.event`** for the cutoff draft, despite that
identity's command authority belonging to **`f.foreign_event`**. Its main-event
public profile does not grant command authority. The accepted-draft call is
therefore the first unhandled denied operation in that source path, before the
post-cutoff close assertions. Changing the denial expectation or bypassing
authorization would invalidate the intended cutoff test.

During this read the author introduced a dedicated `cutoff_actor`. A single
atomic read pinned the revised test to SHA256
`27c553ab116553759b7152f1fab46cbc4a0c07e20e5f8a3fd30918a96b3edbdb`.
Its relevant bindings and outcomes are:

| Identity / operation | Exact binding and expected state |
| --- | --- |
| `f.actor` | Main event owner: empty set creates DRAFT 1; failed empty publish leaves 1; valid set becomes DRAFT 2; publish becomes PUBLISHED 3; the earlier successful close becomes CLOSED 4. |
| `f.other` | Separate main-event owner of the raw-SQL expired fixture; not either command row being closed. |
| `f.foreign_actor` | Command grants in `f.foreign_event`; a main-event profile also exists, but there is no main-event command grant. |
| `f.cutoff_actor` | Newly distinct UUID with profile and set/publish/close/list/own-read grants in `f.event`; its row is separate from `f.actor` and the foreign-event row. |
| Accepted cutoff set | Lines 562–574: cutoff actor, main event, expected 0, expiry `cutoff - 1s`; creates DRAFT revision **1**. |
| Beyond-cutoff set | Lines 579–594: same row, expected 1, expiry `cutoff + 1s`; Invalid before insert, leaving DRAFT **1**. |
| Event cutoff moved to current DB clock | Lines 595–600; changes event settings, not any introduction revision. |
| Set after cutoff | Lines 601–612: same row, expected 1; Denied before mutation, leaving DRAFT **1**. |
| Close after cutoff | Lines 613–620 incorrectly pass expected **2** in this pinned snapshot. Correct expected revision is **1**, preferably `accepted.owner.revision`; successful close yields CLOSED **2**. |

This follows the actual implementation, not a revised expectation chosen to fit
a test exit. `IntroCommand::close` derives its target from the supplied
**event and actor**, not the command key (`intro/mod.rs:218–224`, `295–305`).
The repository selects the row by event/owner, increments only successful writes,
rejects mismatched revisions before mutation, and propagates producer errors
through transaction rollback. Its close branch requires current revision and a
non-CLOSED row (`repository/intro.rs:330–342`). Close deliberately skips the event
cutoff restriction while retaining command authorization; a cutoff does not
authorize an unrelated owner or revive an already CLOSED row.

The original late `Close(f.actor, expected 3)` cannot succeed because that row is
already CLOSED 4. A new idempotency key does not reset its revision. Even changing
that argument to 4 would still hit the CLOSED-state rejection. Reordering the
old closes cannot fix this. To test a successful safety close after cutoff, use
the separately authorized cutoff actor's still-open row and its actual revision
1. Retain the original owner's earlier close/direct-not-found assertions.

QA executed an in-memory standard-library model, exit 0: deterministic synthetic
UUIDs plus the source's SHA256-based intro-ID derivation produce distinct keys
for `(main, owner)`, `(main, cutoff)`, `(foreign, foreign actor)` and
`(main, foreign actor)`. A transition ledger verifies owner CLOSED 4, missing
main-event foreign authority, unchanged cutoff DRAFT 1 after Invalid/Denied,
Conflict for cutoff close expected 2, and success/CLOSED 2 for expected 1.
No native process, production/test edit or dependency was used.

Repository hash remained `02c039f2...` at this review; the test changed from
`62386a...` to the atomic `27c553...` snapshot above. Both exact findings were
sent directly to the BE owner and Main. The author owns the fixture correction
and native proof; this source/state diagnosis makes no full BE27 acceptance
claim and does not delay integration of the earlier committed diagnosis.

A final read detected the author's correction to `Revision::initial()` for the
cutoff actor's close. That later test hash is
`c1974e3c85d1aa47b8acada9acdb6b468775b9b7256fe2a019d54d2b362f26e2`;
the repository hash remained unchanged. The correction matches the revision-1
ledger above. It is source evidence only; QA did not run the revised native test.
