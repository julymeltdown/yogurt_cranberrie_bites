# BE42-AUTH-CONCERT-APPLY-01

Status: **IN_PROGRESS**, developer qualification **PASS**; committed handoff
awaits normal publication. This transport does not accept the owning business
leaves, PrivateData erasure, all eight deletion targets or public BE42.

## Assignment, source and ownership

Owner207 uses its existing exclusive worktree, branch
`codex/mcr-be-BE42-auth-concert-apply`, based on public
`fa6f3f72addedd8a8d9b9486a4e2ed8be56e0ec6`. The prior source bridge is published:
fresh fetch, independent remote readback, all three patch IDs/path lists and
eleven postimages match; all fifteen `docs/source/` originals are unchanged.
The local dependency-only World commit was excluded from publication.

Root transferred twelve exact paths/hunks: new
`backend/proto/concert/v1/account_deletion.proto`; new-proto generation additions
only in Auth and Concert `build.rs`; Auth's new
`src/adapters/out/grpc/concert_deletion_client.rs` and its module export;
Concert's new `src/adapters/in/deletion_rpc.rs` and its module export;
Auth's new `tests/concert_deletion_rpc.rs`; Concert's new
`tests/account_deletion_rpc.rs`; this document; and the later explicit transfer
of only the original absolute-clock hunk in Auth's
`src/concert_deletion/mod.rs` plus its regressions in existing
`tests/concert_deletion.rs`. No main, configuration, SQL, manifests, lockfiles,
workers, other dispatch clients or business leaf edits are owned here.
Owner232 exclusively owns its Notification receipt hunk in
`user_deletion_dispatcher.rs`; that entire file is read-only here.

The existing unfinished `BE-CANONICAL-OWNER-AUTHORITY` goal is aligned. Its
historical `BLOCKED` metadata is retained; no reset, replacement, completion
or token budget is introduced. RootBE is the publishing developer. This owner
implements, validates, fixes and supplies the committed handoff. One canonical
publisher receipt establishes remote delivery; no repeated consumer readback,
proof-only commit or ACK is required under the latest human direction.
No king/SL/QA review or acknowledgment stage applies. QA08/load/performance
qualification is **DEFERRED_BY_USER**, not PASS; functional validation continues.

Production transport/protocol and tests are newly authored. Existing platform,
Tonic/Prost, Tokio, SQLx, UUID, Chrono and Serde dependencies are retained.
No dependency is added. Reused identity helper logic in the new Concert test
comes from `tests/deletion_source_rpc.rs` at the public base, SHA256
`40bf90944c4f5583e431ae482a5f1ec9ce1fbb5c6dad7005cdde21f0437bb31f`.
It invokes actual Auth signup, generated email verification, login and deletion;
no positive identity/session/claim/source state is SQL-seeded.
Existing `canonical_owner_composition_support/environment.rs` is read-only,
SHA256 `7d1b8a3a51be7c63a6aaeaa0a82a25dbb62f9e7ae46737a89ea0905166dd49f6`.
Only its ownership/PKI checks are adapted into the new Auth negative test.
The deadline wrapper derives from the qualified Auth source implementation at
the same base, SHA256
`f42ccce47f8891a522355d3afebece945b38faece287200ed64b030e5a93ab43`.

Read-only World and Sharing adapters remain their owners' source. The World
primitive SHA256 is
`9682fce1433f9de0e2a3af52f22d69ced6eeeea8bafb51e65cd68711e69902aa`;
Sharing SHA256 is
`fed95c3407274d6e51f5e5446e40c2d189ce680f3cf2baa982261aab312783cd`.
Neither source nor migration is copied into this delivery.

## Transport behavior

The additive `concert.account_deletion.v1.AccountDeletionService/Apply` is
background Auth workload traffic. It requires a CA-validated Auth certificate,
its registered credential and an Auth-only default-deny method policy. It
never fabricates a Gateway actor delegation. The command preserves exact saga,
user, actor, target and retention seconds/nanoseconds. Fresh nonce and absolute
expiry bind the one request/reply. There is no accepted-job or boolean ACK.

`ConcertDeletionClient::connect(https_endpoint, tls, auth_credential)` verifies
the Concert peer and exposes the four frozen application ports through
`owners()`. Each typed port rejects a different target and requires the original
absolute dispatch clock. `apply_with_deadline(command, original)` supports an
explicit caller clock and can only shorten it under an earlier parent. The
existing dispatcher now captures one original clock at dispatch entry:
`min(parent, now + configured_duration)`, maximum five seconds. It scopes that
clock across the owning call, uses `timeout_at` and checks expiry again before
accepting the receipt. An expired parent cannot start a second owner call.
The remaining four inherited target routes and port signatures are unchanged.

The client performs one RPC with no retry, positive default, cache or plaintext
fallback. Only an authenticated exact tuple/nonce/expiry, positive fence,
nonzero 32-byte source identity and `cache-control: no-store` can produce a
typed acknowledgment. Partial, rebound, failed or late replies remain failures.
`FailedPrecondition` maps to `Incomplete`; it never means committed completion.
Source identity is the trusted Concert server's assertion corroborated against
real Auth, not an independently portable hash accepted from a client.

Concert's `guarded_server(DeletionOwners, real_authority, registry)` requires
explicit owning adapters and a common `DeletionAuthority`. It independently
verifies canonical Auth provenance at entry and after the owner reports its
committed/drained result, checks the same original clock and stable source,
and checks exact command plus positive fence before replying. It does not
write an Auth job ACK or manufacture a leaf receipt. `DeletionOwner` direct
implementations delegate to the existing actual Ticket and Sharing adapters.
Sessions and PrivateData injection remain with Root and their actual owners.
Missing owners, incomplete drains, source changes or absent replies cannot ACK.
Both transport directions limit messages to 2048 bytes and redact errors.

## Required developer qualification

Three test executables are required, not a full-suite or separate QA gate:

| Command | Exit | State |
|---|---:|---|
| `rustfmt +1.88.0 --edition 2021 --config skip_children=true --check` on new Rust files and the extended Auth test | 0 | Source formatting only. |
| `python3 scripts/mcr/check-plan.py`, `git diff --check`, assigned-path/source-original checks | 0 | Planning provenance and twelve-path scope; all fifteen source originals unchanged. |
| `cargo +1.88.0 test --manifest-path backend/Cargo.toml --locked --offline -j 2 --config profile.dev.package.contracts-build.opt-level=2 -p auth-service --test concert_deletion --no-run --message-format=json` | 0 | Original dispatcher and the two new clock regressions compile; Cargo 19.92 s. |
| Same build, `-p auth-service --test concert_deletion_rpc` | 0 | Auth client/negative fixture compile; Cargo 1.11 s. |
| Same build, `-p concert-service --test account_deletion_rpc` | 0 | Actual paired RPC/native test compile; Cargo 42.93 s. |
| Frozen executables, `--test-threads=1 --nocapture` | 0 | 9 pure cases pass: 7 dispatcher, 1 Auth transport, 1 Concert transport. Four native cases accurately ignored; the two older Auth-native cases are not executed in this round. |
| Frozen Auth `--ignored --exact authenticated_malformed_receipts_and_original_clock_are_rejected --test-threads=1 --nocapture` | 0 | Actual mTLS malformed-receipt fixture passes, 0.05 s test body; negative-only, no business owner qualification. |
| Frozen Concert `--ignored --exact actual_auth_to_ticket_apply_restart_and_cancellation --test-threads=1 --nocapture`, `MCR_BE42_APPLY_PHASE=SOURCE` | 0 | Actual Auth/dispatcher/paired RPC/Ticket commit and replay, caller/peer/tuple refusals, missing owners, incomplete fixture, uncertain committed reply and real precommit cancellation/rollback all pass; 12.79 s test body. |
| Identical Concert executable/argv, `MCR_BE42_APPLY_PHASE=RESTARTED` after physical owned PostgreSQL restart and fresh test PID | 0 | Same actual durable Ticket fence/source identity and eight pending Auth steps; 0.10 s test body. |

The native case uses an existing marker/inode-owned PG16 Unix-socket cluster
with exactly five `mcr_canonical_*` databases. Only Auth and Concert migration
receipts are used; the other three owners' migration execution is **NOT_RUN**.
The cancellation gate first invokes actual Auth-source verification, then waits
on the real Ticket precommit check; it never issues synthetic positive authority.
Aborting the caller must release and roll back the actual uncommitted owner
transaction by the original deadline. A separate fault calls the actual owner
to commit, then loses the reply; the client must refuse ACK and a new dispatch
must replay the same real receipt. `IncompleteOnly` is a negative fixture;
it does not qualify Sharing or any missing business owner. Auth's canonical
eight-step statuses remain pending in this transport-only qualification.
The positive Ticket calls start with an empty ticket population and exercise
actual permanent fence/progress commits and replay. Populated ticket issuance,
session disconnection, actual Sharing drains and PrivateData erasure are not
qualified by these transport tests.

## Qualified source, resources and delivery

The source series is `ae4078ce43d66b1627f40816630f35788596e5d9` (twelve-path
implementation), `56cdf776f87e26dec1c9d7db271f36421fef3cd4` (stronger actual
transaction-drain witness), and
`073c7cdabda786f951e642afacf582aab8deef07` (documentation-only latest human
delivery policy). The last commit is the exact compiled/native checkpoint.
No compiler/runtime failure, retry, source fix, assertion relaxation or fake
positive authority was needed. Only this results document changes afterward.
All 3,773 frozen source/runtime/SQL readset pins remain exact, as do all fifteen
original source files. No manifests, business leaves, old transport clients or
main/config files change.

The accepted offer was `BE42-AUTH-CONCERT-APPLY-MEDIA-202611041543Z`, actual
`2026-11-04T15:44:11.772222Z`. Its original compile/freeze, work and full-release
cutoffs were respectively 15:54:11.772222Z, 15:58:11.772222Z and 15:59:11.772222Z.
After target release, the stricter native/full cutoffs were 15:49:37.651548Z and
15:50:37.651548Z. There was no implicit extension or workload outside the lease.

Fresh carry was 4,071,944,192 bytes, including the full checkout, full shared Git
directory, retained earlier evidence/custody, every historical positive target
charge and the conservative CF02 double charge. Live Git/source growth stayed
charged. The whole cap was 6 GiB, new target cap 1 GiB, outside cap 256 MiB,
three executable copies cap 96 MiB and process cap 80. Initial free space was
18,490,642,432 bytes, above 16 GiB; the 8 GiB runtime floor held. The existing
operational guard checked limits before children and throughout execution.
Rust 1.88/compiler and PG16/OpenSSL tool hashes were pinned; locked/offline,
two jobs, dev/test debug zero, incremental zero, contracts-build dev opt-level
two and both Rust flag variables unset were enforced.

Only the two explicitly owned Auth/Concert `build.rs` metadata timestamps were
refreshed for the known cross-worktree cached-script input hazard. Their content
bytes and SHA256 values were identical before/after; the private receipt records
both timestamps and hashes. No other build script, target cache or package was
cleaned. The guarded sequential compile/freeze wrapper took 71.568 s.

| Frozen executable | Bytes | SHA256 |
|---|---:|---|
| `concert_deletion-tests` | 13,961,856 | `4570f30f437e1432f7b01f519495b6b8fa444158cc8842a314bc4d007075edb1` |
| `concert_deletion_rpc-tests` | 17,391,776 | `7da0cfcdc506421e673cc4ff7dbced49ce51b8a41edf21bbd2869463549a2114` |
| `account_deletion_rpc-tests` | 28,224,032 | `89eda82189ff50c947ef4ff12dbbce24f23bb5e1eda058c078d26113bc01b245` |

The three copies total 59,577,664 bytes. Full source/runtime/migration/tool pins,
Cargo JSON and the three executables were frozen outside the media target.
Fresh absent-compiler/zero-handle proof unconditionally released that target at
`2026-11-04T15:46:37.651548Z`, before all copied tests or PostgreSQL work. There
was no later target access. Automatically produced binaries were charged as
target growth; no main was explicitly built, copied or run.

The native wrapper took 34.145 s, including one marker/inode-owned Unix-socket
PG16 cluster, the three pure invocations, negative client invocation, actual
SOURCE and physical restart/RESTARTED invocations. The source/Apply services
use real TCP/mTLS listeners in the test process; this is not a qualification
of separately launched production mains. Auth signup/verification/login/deletion
used actual repositories/use cases, with no SQL-positive user/session/source
seed. All 18 Auth and 42 Concert migration receipts matched source/checksums
in both phases. The three unused stores remain **NOT_RUN**. Actual PostgreSQL
and test PIDs changed; all five databases had zero prepared transactions before
teardown. Auth's observed saga retained all eight pending targets.

Cancellation qualification waits until every other Concert backend reports no
`xact_start`, including active or aborted transaction states, then verifies that
the actual uncommitted fence/progress rows are absent. An expired original call
cannot be renewed; a fresh dispatch can complete the same exact operation.
The separate lost-reply case confirms actual committed progress before refusing
transport ACK and replaying through the normal actual owner. Neither fixture
is used to infer a successful missing business owner.

Private evidence is retained under `mcr-be42-auth-apply-_ovs3cfr`: source/tool
pins, Cargo streams, `owned-build-script-metadata-refresh.json`, checkpoint,
phase streams, native report, highwater and target/full-release receipts. The
native report SHA256 is
`565cea6c7fb75977344be534b95a2daf004435b88d4480bcedf30bd055d82d5c`.
Whole allocation peaked at 4,308,652,032 bytes, target-positive growth at
71,086,080 bytes (retained as historical charge), and outside growth at
165,621,760 bytes. Full release was `2026-11-04T15:48:37.658723Z`, 120.007 s after
target release: owned PG/PKI/TMP absent, all saved processes/groups absent and
zero remaining evidence handles. Frozen evidence and executables remain.
No archive restore or new worktree was used.

Root receives this committed series and results-document successor for normal
non-force master publication. A canonical publisher receipt is sufficient;
there is no consumer proof-only commit or ACK gate. Root's actual startup,
worker/owner composition, populated business flows and public BE42 remain
separate required functional work. Performance/QA08/load remain deferred by the
user, with no pass inferred from these bounded correctness timings.
