# BE41 bounded SocialGrants local owner

Status: IN_PROGRESS. Native qualification NOT_RUN. Full EXPIRE_GRANTS and BE41
acceptance remain open: local SQL revocation does not certify private withdrawal,
World policy fanout, production authority, or composition.

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

Planned probes cover SQLx migration receipts/replay, bounded multiple pages,
cross-event isolation, handle/binding preservation, empty and terminal resources,
exact terminal replay, apply-before-verify-before-append rollback, stable retry
operation, absent authority/deadline/transaction, forged receipts, wrong scope
and retention, concurrent workers, actual database restart and resume. Native
checks await a fresh resource allocation; no old target lease is reused.
