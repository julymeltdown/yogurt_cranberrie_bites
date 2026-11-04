# BE42-AUTH-CONCERT-APPLY-01

Status: **IN_PROGRESS**. Source authored; Cargo, native qualification and
publication **NOT_RUN**. This transport does not accept the owning business
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
| `cargo +1.88.0 test --manifest-path backend/Cargo.toml --locked --offline -j 2 --config profile.dev.package.contracts-build.opt-level=2 -p auth-service --test concert_deletion --no-run --message-format=json` | NOT_RUN | Original dispatcher regressions, including the two new clock cases. |
| Same build, `-p auth-service --test concert_deletion_rpc` | NOT_RUN | Plaintext refusal and negative-only authenticated malformed-receipt fixture. |
| Same build, `-p concert-service --test account_deletion_rpc` | NOT_RUN | Method/shape checks and actual paired source/Apply native case. |
| Frozen executables, default unignored tests | NOT_RUN | All pure cases; no ignored native case inferred from this run. |
| Frozen Auth `--ignored --exact authenticated_malformed_receipts_and_original_clock_are_rejected --test-threads=1 --nocapture` | NOT_RUN | Actual mTLS peer, deliberately malformed responses only; no business owner acceptance. |
| Frozen Concert `--ignored --exact actual_auth_to_ticket_apply_restart_and_cancellation --test-threads=1 --nocapture`, `MCR_BE42_APPLY_PHASE=SOURCE` | NOT_RUN | Actual Auth use cases, original dispatcher clock, paired source/Apply mTLS, real Ticket commit/replay, missing owners, caller/peer/tuple failures, uncertain reply and cancelled precommit rollback. |
| Identical Concert executable/argv, `MCR_BE42_APPLY_PHASE=RESTARTED` after physical owned PostgreSQL restart and fresh test PID | NOT_RUN | Same durable Ticket fence/source identity and eight pending Auth steps. |

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

All Cargo, copied tests and PostgreSQL require a fresh allocator21d OFFER and
ACCEPT. Freeze source/runtime/SQL/tool readsets, Cargo JSON and the three
executables outside the target, release the target with zero handles before
every copied execution and PG process, and retain failure and cleanup receipts.
Record executed commands, exit codes, limitations and delivered ancestry after
the actual bounded round. No archive restore, new worktree or main execution.
