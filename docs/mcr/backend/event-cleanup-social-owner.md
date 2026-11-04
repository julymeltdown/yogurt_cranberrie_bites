# BE41 bounded SocialGrants local owner

Status: IN_PROGRESS for production integration; bounded component native
qualification PASS. Full EXPIRE_GRANTS and BE41 acceptance remain open: local
SQL revocation does not certify private withdrawal, World policy fanout,
production authority, or composition. The original full BE41 goal remains BLOCKED.

Base: `5f72fcf00fa7ba12599d6417d2f4744c5c49fbcb`. Sole new paths are
`src/event_cleanup_social_owner/mod.rs`, `tests/event_cleanup_social_owner.rs`
under Concert service, and this document. Root retains exports, registry,
main/config, all SQL and publication.

## Native behavior

`SocialGrantsOwner` implements the retained `LocalOwner` protocol for exactly
`Owner::SocialGrants` / `Step::ExpireGrants`. Supply it to `Owners` and the actual
`CleanupRuntime`. No pool, independent commit, network mutation, lifecycle
admission, policy default or enabled production registration is provided.

The adapter locks the actual event, job and cleanup head on the supplied native
transaction. It requires READ_ONLY, the initiating actor, current pending/running
job, exact next step, revision, pinned retention, page ordinal, limit, cursor and
both already established lifecycle and relationship deadlines. Deadline GUCs
are commit guards, never privileged authority; the real caller must establish
current authority and retain its source fences. This leaf never creates or
extends deadlines. Missing composition fails closed.

Canonical storage is migration 0006: `concert_social_grants` and
`concert_social_requests`; handles and per-platform consent bindings remain
untouched. ACTIVE grants and PENDING/APPROVED requests become REVOKED through
the native revision/state triggers. Existing terminal rows remain unchanged.
No deletion, cascade, ciphertext read, decryption, approval, public projection,
private retention action, archive or expiry timestamp is manufactured.

The opaque key is a type byte followed by canonical UUID bytes, with one platform
byte for grants. Grants sort before requests. A page examines at most limit+1
keys and verifies at most `limit` resources, mutating at most that many rows.
Processed counts mean verified resources, including terminal tombstones, rather
than only changed rows. Table scans and the terminal EXISTS fence can inspect
more rows; the write/resource page bound does not claim an indexed scan bound.
Canonical writers serialize on the event lock; unsupported direct SQL writers
are outside the adapter authority boundary.

The receipt UUID is the first 128 SHA-256 bits with UUID version/variant bits,
over a versioned domain, exact request binding, continuation flag, resource count,
ordered keys, terminal states and row revisions. Its operation is the exact
`BatchRequest::operation()`. Receipt revision 1 identifies this proof profile,
not a fabricated aggregate Social revision. Verification rereads actual rows,
requires all earlier resources terminal, and at convergence requires no live
request/grant anywhere in the event. A valid UUID or affected count is insufficient.

The first verification deliberately precedes the immutable 0046 batch append.
Effects, checked receipt and cursor commit together through CleanupRuntime;
verification failure rolls back speculative writes. Restart resumes the retained
cursor/operation. Completed replay verifies the current terminal postimage and
matching immutable batch, without applying another mutation. A detached or wrong
scope, changed retention, forged receipt or missing deadline cannot advance.

## Remaining integration dependency

Migration 0051 accepts policy signal provenance from retained participant command
receipts/outbox or an actual request deadline. Closure batches are neither source.
Reusing those branches would forge a command or expiry. Root owns a separate
typed, sealed closure-batch provenance branch and SQL extension, including exact
parent receipt/revision/job/step/actor/event checks in the same transaction and
the actual dispatcher. Until that exists, local database revocation convergence
must not be presented as complete private withdrawal or full EXPIRE_GRANTS.
Root must also export/register the leaf and provide actual privileged authority,
reviewed retention and the already established relationship authority fence.

## Developer qualification

The test file uses a source path shim until Root supplies the library export.
Its source pin binds the exact adapter and test. Ignored native probes require
an explicitly owned isolated PostgreSQL socket, owner token, fixture marker and
frozen migration directory. The test authority and retention are synthetic;
preceding mural/snapshot owners are explicit prerequisite fixtures. READ_ONLY
uses the retained native helper, not a receipt returned by this leaf.

Completed probes cover SQLx migration receipts/replay, bounded multiple pages,
cross-event isolation, handle/binding preservation, empty and terminal resources,
exact terminal replay, apply-before-verify-before-append rollback, stable retry
operation, absent authority/deadline/transaction, forged receipts, wrong scope
and retention, concurrent workers, actual database restart and resume.

The compiled source is `66f14795ca728f57da2c7363874ccdef833f8c53`; only this
document changes after qualification. The command was `cargo +1.88.0 test
--locked --offline -p concert-service --test event_cleanup_social_owner --no-run
--message-format=json --config profile.dev.package.contracts-build.opt-level=2`,
with jobs 2, dev/test debug 0, incremental 0, both Rust flag variables and compiler
wrappers unset. Compilation exited 0 in 51.46 seconds. No dependency or source
repair, cache deletion, install, SQL edit or ignored-file cleanup was needed.

Under fresh lease `BE41-SOCIAL-GRANTS-MEDIA-202611041343B-Z`, the copied executable
ran one source-pin check plus all 11 required database probes: **12 PASS, 0 FAIL**.
Every ignored database probe was invoked explicitly with its exact name. Native
work exited 0 in 5.35 seconds. All 40 frozen SQLx migrations ran against a fresh
socket-only installed PostgreSQL 17.10; replay preserved their actual receipts.
The actual postmaster restarted from PID 82441 to 82618 and resumed the committed
page without duplicating its operation. Prepared transactions remained zero.
Authority, retention and preceding mural/snapshot fixture limitations above apply.

Evidence is retained locally at
`/private/tmp/mcr-social-owner-qualification-89urj7wm`:

| Artifact | SHA-256 |
| --- | --- |
| `qualification-audit.json` | `a226d7d466d088a7e30ac0d3264d0918dcaea30cc527bbfc553cb12bf5535441` |
| `full-release.json` | `032f08e034ebc6b0a5bacf8be86f79c272c22625464aec5abdd50532f15ab832` |
| `frozen.json` | `9c6dec72ebe4730fc346bf7e8ef9698a03b57d1450a7ac063c940e766758f7e9` |
| Frozen executable | `63c854feceafdb6312b5a598bd93dad31259992a7e36add4e6f14d7522802725` |
| Full selected source archive | `d62815f97d7bbf9ba1dffe3dac77a977c0aebc47f44b107fbacb3b83b97ce0b9` |

The archive matches all 2,375 selected source files, including the unchanged
schema, manifests, lockfile and retained engine. The adapter/test source pin is
`f7217fb5377d843959a7c2312628565edd693190d5cd8c028d87f3b8e8c0ed47`.
Frozen and runtime receipts pin the installed tools, migration files, executable,
source archive, test commands, guardian accounting, actual restart and cleanup.

The shared target was unconditionally released with zero handles before even
the copied source-pin test. There was no subsequent target access. The owned
cluster and TMP were removed; no owned compiler, copied test, database process,
listener or artifact handle remained at full release, 22:49:48 KST on 2026-11-04.
Whole peak was 3,318,857,728 bytes including both preserved worktrees and all old
artifacts; target positive growth was 83,238,912 bytes. Minimum free space was
30,728,523,776 bytes. The 5 GiB whole cap, 1 GiB target growth cap, 384 MiB new
output cap and 8 GiB continuous free floor were preserved. No old lease was reused.
