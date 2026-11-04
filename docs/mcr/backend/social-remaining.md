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
