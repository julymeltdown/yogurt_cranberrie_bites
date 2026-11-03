# Authenticated Auth admission RPC review

Date: 2026-11-03. Status: **R1 CLOSED — source and final combined evidence
verified; parent disposition pending**. Independent source review found one P2
test-timing defect, now corrected. No additional actionable bounded finding or
introduced production authorization defect was identified in the inspected handler. This is
a bounded review of the Auth-owned read RPC, not whole BE04 or release acceptance.

## Source and ownership

The isolated QA branch is `codex/mcr-qa-auth-rpc`, checkout
`/Users/lhs/Desktop/mcr-worktrees/qa-auth-rpc`, clean starting base
`9dd8bc61d3e15118ed870ebb18991db20957d19e`. The former prerequisite-review
checkout is stopped. Only this document is writable for this assignment; no
implementation, shared contract, dependency, build target or source archive is
changed. No Rust executable, database, Docker or service was run by this reviewer.

The integrated handler commit `9dd8bc61d3e15118ed870ebb18991db20957d19e` has the
same nine delivered path contents as worker
`9bf3233b1029e2910f80b761c886b8662a3830f1`; explicit path comparison exited 0.
The parent's integrated platform/proto prerequisites correspond to reviewed
`a44f8fa711b1ac73ce97d9e18d13dfc2eeee2382` and test correction
`e79fd177c44ca4c18621ce23560eb7007e3e48fe`. This review reads their actual
combined implementation, not the earlier unavailable handler stub.

The nine handler delivery paths are:

- `backend/scripts/mcr/auth_admission_rpc_smoke.py`
- `backend/scripts/mcr/test_auth_admission_rpc_smoke.py`
- `backend/services/auth-service/src/adapters/in/grpc/concert_admission.rs`
- `backend/services/auth-service/src/adapters/in/grpc/method_policy.rs`
- `backend/services/auth-service/src/adapters/in/grpc/mod.rs`
- `backend/services/auth-service/src/adapters/in/grpc/server.rs`
- `backend/services/auth-service/src/config/mod.rs`
- `backend/services/auth-service/tests/concert_admission_rpc.rs`
- `docs/mcr/backend/auth-admission-rpc.md`

Review authorities include root/backend working agreements, specification BE04
and identity/security requirements, project assessment/execution context,
`docs/mcr/identity-admission-decision.md` and the Auth RPC composition contract.
Analysis prose and test counts do not replace specification acceptance.

## R1 [P2] CLOSED — live-proof expiry regression could expire before dispatch

Owning path at the reviewed base:
`backend/services/auth-service/tests/concert_admission_rpc.rs:789–803`, test
`deterministic_proof_expiry_bounds_a_longer_original_transport_deadline`.

The public signer uses a 30-second lifetime (`actor_delegation.rs:52,213–254`).
Issuing with `Utc::now().timestamp() - 29` therefore leaves only the remainder
of the current wall-clock second before actual expiry. For example, signing at
`S + 0.999s` gives `exp = S + 1s`, only about 1ms later. If transport/scheduling
reaches guard capture after that boundary, legacy verification can still pass
its skew check, but strict read context is absent. The response is then
Unauthenticated with zero owner calls, while this test requires DeadlineExceeded
and exactly one owner dispatch. A correct fail-closed server can fail this test.

The existing remainder check at lines 695–700 is in the already-expired
legacy-skew test; it does not protect this live-then-expire test. Consequence:
flaky controlled-suite qualification and failure to reliably exercise expiry
*during* the owner read. This finding follows directly from source clock
arithmetic; no Rust reproduction is claimed by QA.

Bounded requested correction: establish robust live authority at dispatch in the
intended test, preserve exactly one owner call, and still prove that actual proof
expiry wins over the longer five-second original deadline and two-second
resource ceiling. Do not weaken the assertions or retry until green. The
integrator accepted R1 and returned it to the handler worker.

The source correction is verified at worker commit
`5c59f1fe8b3ce96aea7cd825d1176ebbc7ecf9f6`, parent `9bf3233`. Only the test and
its documentation change. It completes server/key/channel setup, bounds phase
alignment before issuing one proof with 1.25–1.5 seconds of live authority,
and confirms one owner entry through a fresh `Notify` within 750ms while the
proof is still live. The owner remains pending until cancellation. Early denial
fails without retry; final assertions preserve DeadlineExceeded, response at or
after signed expiry but before two seconds, exactly one call and cancelled
budget. The misplaced wait in the already-expired test is removed. No remaining
actionable timing defect was identified in that correction; production code,
platform, proto and dependencies are unchanged.

Worker focused run `/Users/lhs/Desktop/mcr-build/auth-admission-rpc-R1-controlled.log`
has verified SHA-256
`c34854c87438c2548ca7a25292a0d3f969aa1268e99061c0391b11dbdc0868e3` and records
11 controlled cases PASS, zero failed/ignored and five native cases filtered.
The final parent qualification below also verifies this correction in the clean
combined candidate. R1 is closed for this bounded review.

## Handler, owner and composition review

Source locations below refer to the clean integrated base unless stated otherwise.

| Concern | Owning source and assessment |
|---|---|
| Real production injection | `auth-service/src/config/mod.rs:498,773` adds one `Arc<dyn CurrentAdmissionPort>` and injects `PgCurrentAdmission(pool.clone())`. `adapters/in/grpc/server.rs:23` delegates to the real feature handler. There is no production controlled-owner or positive fallback. |
| Caller and actor | `concert_admission.rs:14–38` independently requires verified Gateway caller, `VerifiedActor` and private-constructor `VerifiedReadAuthority`; both actors must equal the canonical request user. Auth method policy permits Gateway only and declares protobuf actor field 1. |
| Guard and signed body | `platform/src/grpc_method_policy.rs:33,297` authenticates registered caller before method dispatch. `actor_delegation.rs:647–719` captures start/deadline before bounded collection, verifies signature/issuer/audience/full method/exact frame digest, binds actor and consumes replay before installing context. Caller strings or `x-verified-actor` metadata alone cannot install it. |
| Canonical input | `concert_admission.rs:35–52,108` rejects nil, noncanonical user/family UUIDs; `auth-application/src/concert_identity.rs:27–84` closes provider names and validates canonical nonnil row IDs and positive safe-integer owner epochs. Shape is not treated as ownership. |
| Exact owner facts | `adapters/out/persistence/concert_identity.rs:41–78` queries exact user/family and, when requested, exact row/user/provider/epoch/nondeleted eligible binding. The statement snapshot also obtains database time and current account state. Missing/inactive/expired/revoked/foreign references remain negative facts. No binding request omits the provider relation altogether. |
| One original budget | `concert_admission.rs:33,53–69,96` derives and repeatedly checks the same authority deadline; `timeout_at` encloses the whole owner await. `read_authority.rs:45` clips original stop, pre-buffer resource stop and captured proof stop; `authority_deadline.rs:31,49` also checks current signed wall expiry. Buffering/acquisition/earlier waits cannot grant a fresh owner-read budget. |
| Late success and cancellation | `concert_admission.rs:55–66,100` signals cancellation on handler drop/completion and rejects late success even if an owner blocks a poll. The Pg adapter encloses acquisition/setup/query/commit in one budget, uses transaction-local statement timeout and observes cancellation. This is bounded read cancellation, not instantaneous database or HTTP/2 client cancellation. |
| Safe output/error mapping | `concert_admission.rs:70–96,117` rejects inconsistent requested/not-requested binding facts, returns only observation/account/session/binding fields with no-store metadata, and maps owner failures to fixed messages. It does not echo identity, provider subject, role, token or portable authority. Database diagnostics are discarded at the owner adapter. |
| Production layer equivalence | `auth-service/src/main.rs:84–100` installs `CallDeadlineLayer` and production `guard()` around the generated server. Socket test `Running::start` at test lines 297–319 uses the actual generated server, same guard and deadline layer. It uses fixture credentials/parity signing and plaintext owned loopback, not production mTLS qualification. |

The completed source review covers the entire 805-line generated-server test,
175-line handler and new smoke harness/tests, plus the owning application port,
Pg adapter, epoch migration, private authority/deadline helpers and production
registration. The existing owner snapshot is not a lease: a revocation committed
after that snapshot requires a fresh read or separately qualified invalidation.

## Test coverage and independent checks

The 11 controlled generated-server cases exercise successful dispatch;
missing/wrong/sibling callers; malformed/signature/audience/method/body/actor
proofs; replay; canonical IDs and binding shape; missing/invalid/elapsed original
deadline; legacy-skew actual expiry; fixed owner errors; inconsistent binding
facts; whole-await/late success; and live proof expiry. R1 concerns the last
case's setup, not removal of the fail-closed production fence.

Five native cases use real production Pg injection: email-only without provider
relation access, current account/session negatives, exact/foreign/deleted/relinked
binding, locked-query timeout plus one-connection pool recovery, and closed-owner
safe failure. The 22 retained owner cases cover additional expiry-after-wait,
binding generation, user/family ownership and cancellation/database boundaries.

The smoke harness creates fresh marker/inode-owned PG16 stores, applies/verifies
all 13 migrations, derives test counts from executable listings, rejects skipped
tests, restarts PostgreSQL, verifies persistence and requires cleanup. Its helper
rejects existing/symlink report targets and unowned input, redacts failures and
revokes success on cleanup failure. These safety branches use mocked fixtures in
the independent QA command below; QA did not start a cluster.

| Independently performed in QA checkout | Result |
|---|---|
| `python3 -B -m unittest discover -s backend/scripts/mcr -p test_auth_admission_rpc_smoke.py -v` | Exit 0; all eight focused mocked harness tests pass. |
| Compare all 178 worker final-receipt source pins against actual checkout | Exit 0; all match. |
| Verify nine handoff evidence hashes and companion log hashes | Exit 0; all inspected files match their receipts. |
| Compare all 178 parent native-receipt source pins against actual checkout | Exit 0; all match. |
| Check worker and parent recorded owned cluster roots | All four recorded directories absent at inspection. |
| Compare nine delivered path contents between worker `9bf3233` and integrated `9dd8bc6` | Exit 0; identical. |

## Retained runtime evidence and exceptions

These runs were performed by worker/parent and inspected by QA, not rerun here.
Worker receipt `/Users/lhs/Desktop/mcr-build/auth-admission-rpc-final.json` has
SHA-256 `be23b14b0e69aa1d1900ca83056112faa27e9354db206c30ce82ef3d86dd3b20`,
clean source `9bf3233b1029e2910f80b761c886b8662a3830f1`, five real RPC and 22
retained owner tests, PG16.14, 13 migrations, restart/persistence and cleanup PASS.

Parent receipt `/tmp/mcr-rpc-parent/native.json` has SHA-256
`3de4224c95d36f3acf0789890c8dc1b05e1fe7efcccc0b9a20b61f8df0230a14`, clean
combined source `9dd8bc61d3e15118ed870ebb18991db20957d19e`, the same five-plus-22
native qualification and PG16.14/migration/restart/cleanup success. Its expected
missing-owner refusal exits 101; that is an intentional negative check.

Parent `checks.json` and completed logs record 103 platform authority, two live
generic guard, 48 filtered Auth library, 23 application library, eight application
value, 13 Gateway transport and 11 controlled RPC tests passing. The initial
`live_grpc_guard` invocation exited 101 because no test target has that name;
`grpc_method_policy_network` subsequently passed. This is a corrected command
error, not an Auth source failure.

The worker's unfiltered Auth library attempt was **48 passed / one failed** due
to the unchanged Docker-only external-schema fixture acquisition. The explicit
filtered run is **48 passed / one filtered**, not whole-library PASS. Production
privilege/schema qualification therefore remains open in that evidence.

The parent's initial combined helper run is **96 passed / one failed**: BE15's
contract registry pins predate the event-scope correction. The preserved
`/tmp/mcr-rpc-parent/helpers.log` shows that exact pin mismatch, not a handler
failure. The integrator reports a separate central pin correction `d197379` and
follow-up checks. Subsequent inspection of the completed
`/tmp/mcr-rpc-parent/contract-followup.json` binds the correction run to
`d1973796ca6a2310c8d46041ef0de81cc6710dc0`: corrected helpers, contracts and plan
all exit 0. Its completed helper log records 98 tests PASS. This is retained
parent evidence, not an independent QA rerun or a claim that the original
97-case combined run passed. This review changes neither the central helper nor
its contracts and does not expand into a separate BE15 implementation review.

## Final combined qualification after R1

The parent integrated worker correction `5c59f1fe8b3ce96aea7cd825d1176ebbc7ecf9f6`
as `4bc75bbd5ead53a4218b5a36941c9ff9e2695767`. Final qualified candidate
`97a698a43e502a73ff00a489b3155b370cb9d681` follows that correction and records
the preceding qualification/ownership context. Explicit comparison of all nine
delivered handler paths between worker `5c59f1f` and this candidate exits 0:
the reviewed fix is the actual integrated source, not an unintegrated proposal.

QA inspected completed `/tmp/mcr-rpc-parent/final-checks.json`: `completed: true`,
that exact candidate, empty dirty paths, and build/test-build/controlled/native
commands each exit 0. The final controlled log records 11 passed, zero failed or
ignored, five native cases filtered. Final native receipt records five real RPC
cases and all 22 retained owner cases on separate owned PG16.14 clusters, all 13
migrations, verification/reapplication, restart persistence and cleanup PASS.
Expected refusal exits remain explicit rather than being counted as test errors.

All 178 final native-receipt source hashes were independently recomputed from
`git show 97a698a:<path>` and match, as do the RPC harness and all three helper
hashes. Both final owned roots, `/private/tmp/mcr-pg-us4k9wy4` and
`/private/tmp/mcr-pg-ua5utg2p`, are absent, including no dangling symlink at either
path. These are inspections of completed parent runs; QA performed no new Rust,
database or service execution.

| Final inspected artifact | SHA-256 |
|---|---|
| `/tmp/mcr-rpc-parent/final-checks.json` | `bf2d107f77289e4969fb0895325a17194cb60cbbed0c5c00e0405c38057a4565` |
| `/tmp/mcr-rpc-parent/final-deterministic.log` | `47d160938bdfbe290de73e559001d80c854a33c5d98d9aab33d4b9e25e69bbc2` |
| `/tmp/mcr-rpc-parent/final-native.json` | `2a3c7664c955f7a6558e214275875e2b41788bc59ec48fe9f6f021265c2b71e7` |

## Remaining limits and handoff

Pre-handler body collection is byte-bounded; source tracing shows elapsed time
is subtracted before owner dispatch. This review does not establish a separate
time limit for a stalled incoming body or a live delayed-body transport test.
Likewise, cancellation flag/timeout/pool recovery tests do not certify immediate
database termination or network-client cancellation delivery. These are platform
qualification limits, not newly introduced positive authority paths.

Production Concert still composes `UnavailablePolicy` in
`backend/services/concert-service/src/config/mod.rs:106–111`. Gateway JWT
verification-to-this-RPC orchestration, Profile/event eligibility, signed Concert
admission, world transport, production mTLS/role/sequence privileges, backup/restore
epoch safety, multi-replica replay and revocation/stream invalidation remain
separate obligations. No whole BE04, C04, integration-gate or release acceptance
is granted by this document.

The new unbudgeted persistent review goal was actually created and returned
ACTIVE. R1's correction, worker focused log and final parent combined qualification
are verified; this goal stays active through parent disposition. The review
document's initial commit is `d16759cb639ae806d7177ddf71a3eae7a5b8cf98`, followed
by `4d50bd29276ce05918eac1534c1e4558b11dea15`; the final evidence/closure commit
is supplied in the handoff. Every review commit changes only this document from
the stated QA base. The integrator alone publishes the committed review document.
No implementation fix or push is made by QA.
