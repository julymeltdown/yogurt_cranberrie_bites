# Remaining relationship and social privacy delivery

Status: IN_PROGRESS. Original BE28–BE30 and BE37 remain open; this record does
not certify IN03, public HTTP, WebSocket, browser-cache or release acceptance.

## Source and ownership

Starting commit: `3da28fcb`, branch `codex/mcr-be-social-remaining`, isolated
checkout `be-social-remaining`. The prior Auth/Friend owner-store checkout remains
immutable at `dc70d62049d8e8430c88b9ebfc6f071740020538` for other developers'
joined-process qualification. No source archive, manifests, lockfiles, contracts,
Auth startup or canonical-authority implementation changes are assigned here.

Initial write scope is this record and new Friend-service
`tests/concert_social_remaining*.rs` / `tests/concert_social_remaining_support/`
files. Existing implementation edits require an explicit single-writer transfer.
That transfer is ownership coordination, not a code-review or approval gate.

## Original criteria reconciled against retained behavior

| Task | Retained implementation and evidence | Actual remaining boundary |
| --- | --- | --- |
| BE28 | `relationship/social.rs` and `crypto.rs`: managed-key AES-GCM, normalized version-bound encrypted handles, removal tombstones, secret-free metadata and redacted debug. Existing real-PG qualification is recorded in `relationship-policy.md`. | Managed-secret composition and actual public transport are not bound. Do not duplicate encryption or invent a default key. |
| BE29 | Requester consent, recipient decision, reciprocal platform/version-bound grants, expiry/revoke and BE15 receipts/outbox already exist. | `SocialEligibility` has only unavailable and test implementations. A real fenced introduction/claim adapter and registered producer/root lifetime are required. |
| BE30 | Reveal rechecks membership, restrictions, pair/owner revision, current eligibility, expiry and exact reciprocal handle versions. Dedicated response allowlist sets no-store on success/errors. | No production caller constructs `SocialService`; no registered social routes. Real owners, eligibility and response-lifetime composition remain required. |
| BE37 | Bidirectional owner facts, tombstones, durable invalidation and guarded owner scope exist. Retained block removes follows and pending requests, and unblock does not restore them. | Follow/accept read block state before acquiring the deletion-fence subject locks. A concurrent uncommitted block can become visible while that later write waits, leaving a stale authorization decision. Add native contention proof and serialize before checking, without changing published migrations. |

Source trace covers `MCR_IMPLEMENTATION_PLAN_V2_EN.md` rows BE28–30/37 and privacy
section 8.3, the project assessment/execution plan, retained relationship evidence,
Friend persistence and migrations 0011/0015/0016, and guarded authority source.
Historical injected-authority producer tests are persistence evidence, not real
Auth/Friend/intro/claim or browser acceptance.

## Current validation

Cargo/PostgreSQL: NOT_RUN for this increment; no build target or private database
lease acquired. No implementation completion or remote publication is claimed.

## Implemented successor scope

The explicit single-writer transfer covers exactly nine paths: Friend
`src/adapters/out/persistence/follow_repository.rs`, its new
`tests/concert_social_remaining.rs` and `tests/concert_social_remaining_support/owned.rs`;
Concert `src/relationship/eligibility.rs`, `src/relationship/mod.rs` (one export),
`tests/social_remaining.rs`, `tests/social_remaining_support/{owned.rs,native.py}`;
and this document. Base remains `3da28fcb`; application/test checkpoint is
`7b2205c4` (including `5e6eeb4a`, `bd7de61f`, `7cf9d383`). No migration changes.

Friend mutations reuse the existing sorted advisory subject locks from migration
0011 before their block snapshot. Accept discovers the request pair without a
row lock, acquires those subject locks, then rereads the request FOR UPDATE.
This matches block's subject-before-request order and avoids the acceptance /
block cancellation deadlock. Both plain and receipt-backed follow, pending
request creation, block and unblock use the same order. Existing receipt replay
semantics are retained; a historical receipt is not fresh relationship authority.

`PgSocialEligibility` consumes the caller transaction, existing published exact
introduction revision/owner, sorted canonical age-owner roots, and the latest
claim per canonical user before checking status or provenance. It requires
verified age for both sharing participants, applies the introduction audience
bounds to its requester, and clamps the result to request, owner, provider,
introduction, claim and birthday deadlines. It checks database time after SQL and
preserves shorter lock/statement timeouts. SocialService already pins this result
into its deferred transaction-end guard. No secret-bearing DTO is added.

The canonical `from_guarded` constructor derives actor/event/session/request and
original expiry from an acquired `OwnerGuardScope`. `GuardedAuthority` still
requires the actual producer transaction to have been registered by its owner.
This module does not auto-register an unbound transaction or invent a settlement
receipt. The current retained SocialService opens internal transactions; a real
producer/root bridge must register those transactions and settle their actual
outcomes before any public response can be qualified.

### Retained source provenance and prerequisites

The private canonical claim mapping is adapted from BE26
`src/adapters/out/age_provider/claim_reader.rs` at base `3da28fcb`, SHA256
`f4bb5672dce733616373113bf59a28e0b12b9c90c3b2017235a2c420c9f87b1f`.
It uses the existing concert-domain `assess` / `AgeBounds` policy; no alternative
age arithmetic, identity store or provider callback is introduced.

| Unchanged dependency at the base | SHA256 |
| --- | --- |
| BE26 `age_provider/schema-v1.sql` | `1c2e786250e6a4e61b26c4232211ef48d2b4cb7a2e51b258c57b697d1e6a46d5` |
| BE27 `repository/intro_schema_v1.sql` | `fa42cf9028f7eb6caaf20e4b8c34c0ad192396e867d813dfdf917cb1d3f6448c` |
| Friend `0011_user_deletion_fences.sql` | `73be9a37e9b2049ea1dae45c9f4d42505af14ce9c70b1b7358ab8cb307466765` |

BE26/BE27 schemas remain isolated prerequisite artifacts, not registered runtime
migrations. The native runner applies them explicitly only inside its owned test
cluster. Authority owner207 confirmed that the real verifier implementation,
provider credentials/issuer/audience/signature/nonce verification, qualified
availability source, runtime schema and route composition are not yet delivered.
`SharingVerifierStatus` is a trusted server port with an unavailable default;
fixtures implement SQL-held synthetic status solely for local qualification.
An absent source, unsupported/newer claim, unknown owner, expired deadline or
unavailable provider cannot grant sharing.
