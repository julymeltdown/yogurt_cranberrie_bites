# Intro account deletion: BE27 / BE42-PD-INTRO

Status: IN_PROGRESS. Owner compilation and seven exact native invocations PASS
at immutable source `5d363519864f9e756914f348fbb2909d02881860`.
DRAFT page-boundary/isolation, PUBLISHED, CLOSED and three physical restart
replays passed. Production composition/public acceptance remains separate.
No production composition or whole BE42/IN03 acceptance is claimed.

## Ownership, source and dependency

Root stopped Intro-specific edits and transferred the new owner leaf, its module
export, narrow repository helper, reserved migration `0065_intro_account_deletion.sql`,
new `tests/intro_deletion*.rs`, `scripts/mcr/intro_deletion/**`, and this document.
A supplemental grant covers only `ScopedIntroExecutor::actor`'s user-first fence
before existing event/runtime guards. No main/config/proto/contract/aggregate path
is authored here. Root is the normal publishing developer; no manager review or
acknowledgment gate applies.

Retained isolated checkout: `qa-contract-coverage`; task branch
`codex/mcr-be-intro-account-deletion`, original canonical base
`f08d44c9a3add9695e9c39e1375f19e2037f10a4`. The explicitly authorized unmodified
provider ABI dependency was fast-forwarded to
`acee2fac57847ec42656c991f4db7170a552d8fd`, including its canonical `b97608d4`
ancestry. Its six aggregate paths and reserved migration 0072 are dependency
inputs, excluded from authored scope. Source-only ABI intake is not native proof.
Root separately authorized exact corrective dependency
`cfa8793e1697d886d875a5d59a1912a5daa12c0b`, taken unmodified as
`3ea6a420db3045832699441164f9ced86e660178`. Its sole aggregate hunk borrows
`source_binding` when binding SQL, retaining the value for reauthentication.
This dependency is excluded from the eight authored paths and grants no aggregate
editing ownership. At intake the Intro compile/native checks were NOT_RUN; the allocated own-artifact
results below supersede that historical state. Dependency-owner results are not
substituted for this artifact's validation.
Prior Intro/Age/QA09 branches and receipts remain preserved. Actual persistent
goal metadata remains the incompatible unfinished BLOCKED QA09 goal; it is not
reset, falsely completed or used as a gate on this manually authorized source work.

## Actual owner transition

`IntroAccountDeletion` implements only `PrivateDataCategoryOwner` for INTRO_TEXT,
version 1. Its factory adds that single owner to an explicit registry; Root must
compose it without replacing the core or another owner. Construction requires the
actual canonical Auth deletion verifier and an explicit Social evidence port.
There is no permissive default authority, background worker or detached effect.

Each round checks the exact PrivateData command/source and original monotonic
deadline, verifies the current source again, then establishes the existing common
user deletion fence before any receipt, event or Intro row lock. Its own receipt
is an immutable effect/idempotency record bound to actor/user/saga, full command,
source, exact retention seconds/nanoseconds, category/version and common fence.
Only the aggregate owner writes the central category COMPLETE receipt.

The owner erases at most 128 event-scoped Intro rows per transaction, in stable
event order. It takes the existing event root before each explicit DELETE.
DRAFT/PUBLISHED/CLOSED use this separate authorized erase transition: no fake
Intro command receipt, update to a terminal row, disabled trigger or fabricated
native witness. Original 0049 and retained synthetic-schema bytes are unchanged.
No public text, age bounds or authors are copied into the effect receipt.

Forward migration 0065 authorizes DELETE only from the exact live owner-effect
transaction under the common fence. INSERT/UPDATE after that fence fail closed;
normal canonical authoring also takes the user lock and checks the fence BEFORE
its existing event/runtime/policy locks. Old normal update and deferred runtime
guards retain their behavior. Direct uncovered DELETE and TRUNCATE are denied.
This is a privacy deletion transition, not a broad event/admin cleanup grant.

The database deadline is conservatively derived from the original monotonic
remainder and checked by a deferred effect-receipt guard at commit, including
incomplete rounds. A final actual-source check precedes commit. Failure rolls
back the transaction; timeout is not a claim that a dropped SQL future alone
proves rollback drainage. Native assertions must establish durable absence of
partial effects. Exact terminal replay returns the unchanged owner revision;
changed identity/source/deadline does not become a replay.

## Social owner read contract

Social owner `16e` supplied the frozen read shape from producer `4592a705` and
reported qualified Sharing successor `46defd23`. `PgIntroConsentDeletionEvidence`
reads the actual stored Sharing receipt in the SAME transaction after the common
user fence. It derives the Sharing-target command via the existing Rust binding
API and requires exact saga/user/actor, retention seconds+nanos, identity digest,
Sharing command digest, target SHARING, immutable nonzero 32-byte Sharing source,
matching common fence revision 1 and terminal completion. Different target source
bindings are not compared to each other.

It additionally requires no unpublished deletion audience and no pending/approved
Social request or active grant for the user. The proof is rechecked even on
terminal replay before INTRO_TEXT positive completion. Missing, nonterminal or
rebound evidence returns retryable progress revision 0 / complete false. No Social
receipt/request/grant is mutated. Its terminal receipt proves its own withdrawal
and publication; this read neither grants consent nor certifies browser/WS behavior.
The ordinary BE15 Intro consent hook requires a real Intro mutation/receipt and
is deliberately not used to impersonate one during deletion.

## Developer validation plan and limits

Formatting of the five owned Rust files completed with Rust 1.88 rustfmt, using
`skip_children=true`; this is parsing/formatting only, not compilation or runtime
qualification. The allocated build and native attempts below supersede the
earlier source-only status. No public Gateway/browser journey has run.

The first allocated Rust 1.88 build exited 101: the new borrowed-connection fence
helper called the existing transaction-only `is_fenced_in` interface. The helper
now runs that same existence query on its borrowed connection after the common
user lock. This stays inside its assigned hunk; no World helper is changed.
The correction was committed at `9a9b54805b57a62f98462e7dcc07262259060220`
and passed its in-lease retry without changing the original absolute deadlines
or byte charges.

Planned exact compile: `cargo +1.88.0 test -p concert-service --test intro_deletion
--no-run --locked --offline -j2 --message-format=json`, with the accepted target,
debug/incremental policy and byte/time/process accounting. Freeze the one test
artifact plus its Cargo JSON and full source hashes outside the target, release
the target, then invoke the new qualifier only within accepted native capacity.

New artifact inventory: one pure rebound/target/deadline case, three ignored
native DRAFT/PUBLISHED/CLOSED cases, and one exact post-restart replay case.
Each populated native case uses the existing actual canonical Intro authoring
fixture before erasure. Assertions cover uncovered DELETE denial, original-deadline
lock contention, missing/nonterminal Sharing progress, source withdrawal after
DELETE with rollback, actual erasure, stable replay, changed source/saga/actor/
retention/target, recreation denial and immutable receipt. Each store is restarted
physically before the exact replay case. The DRAFT case authors 129 event-scoped rows through actual canonical commands
and checks the 128-row incomplete boundary before the terminal second page. All
three cases retain a different canonical principal's actual Intro unchanged.
The pure binding case passed in earlier rounds. DRAFT first failed before erase
qualification; the separately allocated 1853 retry identified Product scope
capacity unavailable (capacity 16). After the owned helper fix, the newly built
5d3635198 artifact passed all three states, the 129-row page boundary, retained
other-principal Intro, and physical restart replay after each state. Earlier
failures remain retained; the old binary is not proof of this test change.

Deletion Auth-source and sealed Sharing rows in this artifact are explicitly
controlled test fixtures. They exercise the real owner transition and sealed
read protocol, not production Auth deletion transport or actual Sharing
publication. Production composition and cross-owner saga execution remain Root's
integration duties. The qualifier preserves the original marker/inode/postmaster
Cluster guard and actual four-owner migration ledgers; it creates no alternate
shared cluster attachment. Private diagnostic reports remain local and mode 0600.

## Provenance

Adapted headless Rust test request/canonical-scope helpers from the first 180 lines
of `intro_product.rs`; existing fixture modules are referenced unchanged. The
qualifier reuses the original owned cluster/PKI/source-inventory interfaces without
editing their implementations. Dependencies remain the retained Rust workspace,
SQLx/PostgreSQL16, existing mTLS helpers and Python standard library. No package,
manifest, lockfile or `docs/source/` change is authored here.

## Actual allocated evidence and remaining owner work

Private evidence root: `/private/tmp/mcr-intro-deletion-20261105`. Build source
is exactly `9a9b54805b57a62f98462e7dcc07262259060220`. The one frozen test EXE
is 49,470,576 bytes, SHA-256
`bb64bb85d565299868f297258a0b3722eede05d56d74b1d2d0b6bf8e7e2f1795`.
Its Cargo JSON SHA-256 is
`966304dc004cfa2f436003a503e067f9863856439a7c53ccedd17e18b406a76b`.
Original source, SQL, tool, supervisor and Cargo inventories are retained.

| Attempt | Actual result | Retained receipt |
| --- | --- | --- |
| First no-run build, source `40a35a60` | exit 101, connection/transaction mismatch | `cargo-0-result.json` |
| Same-lease no-run retry, source `9a9b5480` | exit 0, exact test artifact produced | `cargo-1-result.json` |
| First controller | FAIL: its arbitrary 30-second fresh-capacity handoff wait expired; no PG/native case started | `full-release.json` |
| Copied continuation 1 | FAIL: the observer applied normal postmaster-path checks during initdb, before the original owner validated the running postmaster; no feature case passed | `copied-continuation-1/full-release.json` |
| Copied continuation 2 | pure binding case exit 0; DRAFT native case returned SmokeError; cleanup PASS, capacity guardian failure null | `copied-continuation-2/native-report.json` and `full-release.json` |

The second continuation observed actual PostgreSQL 16.14, one postmaster,
98,632,800 bytes of PG data at the sampled peak, 20 processes and 500,334,592
bytes RSS. Its prior-failure-inclusive whole-accounting peak was 8,243,472,896
bytes. Disposable owned PG roots, native groups and guardians were gone at
actual FULL epoch `1793817673.0812428` (2026-11-05 03:41:13 KST). The compiler
target had already been released at epoch `1793817336.067255`, with compiler
groups and target handles zero, and was never reopened. Its complete initial
B0 of 18,645,423,456 bytes was centrally charged; all 837,285,632 bytes of this
job's positive target growth remain charged in retained history.

The original copied continuation's native cutoff was `1793817756.067255` and
FULL cutoff `1793817936.067255`. After the failed DRAFT case, the remaining
window was declined as insufficient for the complete bounded cohort. No clock
was extended, failed receipt removed, or native PASS inferred from compilation.

The qualifier now retains the original redacted structured `SmokeError.detail`
in its mode-0600 diagnostic report. A diagnostic/report-only successor can reuse
the same EXE only with explicit `binary_source_commit` plus its complete frozen
source hash map: ancestry and changed paths must be limited to this reporter and
this document, and every other frozen input must match. Its current source pin
and the Rust artifact's build pin are recorded separately. This does not permit
Rust, SQL, fixture, contract, manifest or release-asset changes without a new build.

The fresh named copied retry `INTRO-COPIED-RETRY-20261104T1853Z` accepted at
`1793818570.827701` and reached actual FULL `1793818738.040242`. Its pure case
passed; DRAFT exited 101 with `Product scope capacity unavailable`. Cleanup
passed, guardian failure was null, all owned PG roots and workload groups were
removed, and the guardian was reaped with exit 0. No target access occurred.
Observed peak PG was 98,706,528 bytes, 19 processes, RSS 500,563,968 bytes and
prior-inclusive whole footprint 8,688,686,720 bytes. Its original native/work/FULL
cutoffs were respectively `1793818990.827701`, `1793819050.827701`, and
`1793819230.827701`; the failed round ended early, with no extension.
Evidence: `copied-retry-1853/attempt-0/native-report.json`,
`attempt-0-result.json`, `decision-0.json`, and `full-release.json`.

The new test's inherited headless terminal helper bound delivery, marked
serialization and completed the actual canonical owner scope, but omitted the
separate delivery-completed receipt. The unchanged production ledger correctly
kept those scopes charged against its capacity of 16. The 129-command DRAFT
fixture therefore stopped before deletion qualification. Only the new owned
`intro_deletion.rs` helper is corrected: after serialization it verifies the
exact bound delivery capability, settles actual owners/children with
`finish_response`, records delivery completion only after that succeeds, and
asserts the durable completion receipt. This mirrors `DeliveryService` ordering
through its public ledger/scope APIs. It performs no direct receipt SQL, changes
no production limit or guard, and does not certify a real Gateway HTTP callback.
The original shared fixture and `intro_product.rs` stay byte-identical.

The newly allocated `INTRO-DELETION-FIX-MEDIA-20261104T1902Z` accepted at
`1793824390.396103` (2026-11-05 05:33:10 KST) on literal Wall cleanup target
release `1793824304.859531`. Source remained clean and immutable
`5d363519864f9e756914f348fbb2909d02881860` through compilation and qualification.
Exact no-run command above exited 0, from `1793824393.891726` to
`1793824439.068912` (45.18 seconds), Rust 1.88, locked/offline, j2, debug0,
incremental0, contracts-build opt2, flags/wrappers unset. The new single EXE is
49,497,664 bytes, SHA-256
`922b866a7c83e6a1d0599744f6ff675b1853cb45fb7490565badf3458ea4513c`.
Cargo JSON SHA-256:
`fa021e5f292aed078f45986c28d506707a0a1d595772e2cf0a0e8549f05c1a39`.

Private new evidence root: `/private/tmp/mcr-intro-deletion-rebuild-20261105`.
`checkpoint.json` freezes the full source/input map, Cargo JSON and exact EXE.
The independent guardian first sampled capacity before compiler startup.
Original absolute compiler/freeze/work/FULL cutoffs were respectively
`1793824510.396103`, `1793824570.396103`, `1793824990.396103`, and
`1793825170.396103`; native stopped no later than `1793824861.258141`.
There was no extension or target reentry. Target release `1793824441.258141`
was routed to publishing-operator owner 207 before native/PG work. Compiler,
groups and target handles were zero; the guardian had switched to copied-only
sampling with zero target readers. Its transition is recorded in
`guard-target-released.json` and `target-reader-zero.json`.

Qualifier command: `python3 -B <evidence>/native_bootstrap.py`, which executes
`scripts/mcr/intro_deletion/qualify.py --checkpoint <evidence>/checkpoint.json
--pg-bin-dir /opt/homebrew/opt/postgresql@16/bin --report
<evidence>/attempt-0/native-report.json` under the original owned Cluster.
Native process exited 0 from `1793824476.9772959` to `1793824508.112695`
(31.14 seconds). Exact PostgreSQL version 16.14 was asserted at every start.
Each test used `--exact <case> --test-threads=1 --nocapture`, with `--ignored`
for the populated and restart cases; each reported exactly one passed test.

| Exact case | Invocations | Exit/result |
| --- | ---: | --- |
| `intro_deletion_wrong_target_and_rebound_source_fail_before_database` | 1 | 0 / PASS |
| `intro_deletion_draft` | 1 | 0 / PASS: 129 actual canonical commands, 128-row incomplete page then final row, other principal retained |
| `intro_deletion_published` | 1 | 0 / PASS |
| `intro_deletion_closed` | 1 | 0 / PASS |
| `intro_deletion_restart_replay` | 3, once after each state | all 0 / PASS |

The four distinct postmasters were 77016, 80447, 81004 and 81487, with distinct
actual server start times retained in `postmaster-history.json`. The original
four-owner migration-ledger comparisons and absence of prepared owner work
passed after each populated case. All assertions for uncovered DELETE denial,
original lock deadline, missing/nonterminal Sharing, source-withdrawal rollback,
exact/changed replay identity and recreation/receipt immutability passed.

Report `attempt-0/native-report.json` SHA-256:
`214818bec0f206f29ce0808354b91c0883dbd32b6c907a60af9c307d0a74a39b`.
It records state PASS, cleanup PASS, wholeBE42Accepted false and wholeIN03Accepted
false. Actual FULL was `1793824509.001697` (2026-11-05 05:35:09 KST), with
controller reaped exit 0, guardian reaped exit 0/failure null, no owned roots or
groups and target permanently released. FULL receipt SHA-256:
`e4d87d91a1f13c4423153c61504184630d37b773ef2a29c8e450c42fe6b4bb4c`.
Peak whole footprint 9,625,991,264 bytes, PG 120,849,792 bytes, 20 processes,
RSS 2,920,120,320 bytes, and minimum free 17,087,430,656 bytes stayed within
the accepted caps. Positive new target growth 763,778,752 bytes remains charged
alongside prior 837,285,632 bytes and all failed-round history; release is not
reclamation credit. Resource FULL was sent before documentation/handoff.

The native qualification is this provider's bounded owner evidence, not a
production Auth deletion transport, Sharing publication, whole category
registry, whole BE42 or public IN03 certificate. Owner 00c composes the Concert
PrivateData registry/config/recovery, owner 232 owns Auth transport, and Root
owns actual main/ServingWorld binding. They receive this exact provider slice
and the required narrow normal-write fence in `intro_live/authority.rs`; no
whole-branch merge or synthetic category acknowledgment is implied. The
production evidence choice is `Arc::new(PgIntroConsentDeletionEvidence)` on
the same pool and actual canonical deletion-authority Arc. Normal committed
handoff/publication proceeds directly through Root without manager review.
Any later Rust/SQL/test input change needs its own newly allocated build;
the released target cannot be reopened under this round.

Original source revision for the following hashes: `f08d44c9a3add9695e9c39e1375f19e2037f10a4`.

| Source | SHA-256 |
| --- | --- |
| `backend/services/concert-service/tests/intro_product.rs` | `be3e566adbc429f13679388e9f4589fbd8865f94d10a557080a7674c73a0d0a5` |
| `backend/services/concert-service/tests/intro_product_support/fixture.rs` | `83238ba29fb6f9fc567e9748f20526eb395869ac4699785ff0a11d6adff0b3b1` |
| `backend/services/concert-service/tests/intro_product_support/qualify.py` | `f05ba0ed2ae28bbb1b7d8f3d204c94572d58c2d2b9bf0ab3998960be3a9fd6be` |
| `backend/services/concert-service/src/social_live/account_deletion.rs` | `fed95c3407274d6e51f5e5446e40c2d189ce680f3cf2baa982261aab312783cd` |

## Remaining original BE27 public boundary

Direct counterpart 209 traced the actual existing public runner: it discovers a
preexisting Intro and reads `/people/{authorId}`; that is not a direct Intro read.
Its runner does not yet perform author/set/publish/close, demonstrable cursor
progression, or list absence plus GET `/intros/{introId}` denial. It retains its
frontend harness ownership. Root must provide fresh same-event canonical author
and viewer principals, actual resource/cursor dataset, live Gateway/expiry and
server-qualified eligibility cases before those public scenarios can execute.
Unknown/expired/untrusted/provider-unavailable restricted discovery requires both
list absence and exact Intro-detail denial. Real provider-positive coverage needs
actual inputs; internal privacy cleanup does not wait for age credentials.

The register's historical CF03 three-helper failure is stale: its owning document
records correction and final 10 guarded plus 1 mTLS passes. No delivered facade,
admission or startup suite is rerun merely to replace that historical record.
All live public IN03 outcomes remain NOT_RUN for this patch until actually proved.
