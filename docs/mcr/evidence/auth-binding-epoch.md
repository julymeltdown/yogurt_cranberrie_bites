# Auth provider binding generation prerequisite

Date: 2026-11-03. Parent prerequisite `e9433c0` on isolated
`codex/mcr-auth-identity-composition`, migration `0013_identity_binding_epoch.sql`.
This supplies the real generation required by the [admission direction](../identity-admission-decision.md);
it does not expose an RPC or make Concert production authority available.

Owning link writes can preserve identity-row UUID and assign linked_at with
transaction-stable now(). Neither is a reliable generation. The new owner sequence
assigns `binding_epoch` in 1..9007199254740991, never cycles, and overrides input
on every INSERT/UPDATE, including harmless updates. Existing rows are backfilled.
Conservative invalidation requires a fresh binding read. Rollback consumes sequence
gaps; delete/reinsert cannot restore the old generation. Exhaustion refuses writes.

Parent verification used PostgreSQL 16.14 in a fresh marker/inode-owned private
Unix-socket cluster. All thirteen retained-plus-new Auth SQL files were applied
transactionally by psql; an identity seeded before 0013 proved existing-row backfill.
The [native receipt](auth-binding-epoch/native.json) records all source SHA384 hashes
and six successful checks: retained migrations/backfill, equal-timestamp relink in
one transaction, caller epoch overwrite, reused row UUID after deletion, rollback
gap and atomic write refusal at sequence exhaustion. Owned cleanup and independently
checked directory absence pass. No global service or imported database was touched.

This is a migration/SQL probe, **not** SQLx-ledger or application qualification.
The Auth-owner worker must run its actual repository with retained migrations and
0013, lifecycle/restart checks and the exact committed candidate. Existing Auth
flow regressions also remain required. Production sequence privileges, restore
policy, signed RPC/admission and revocation bounds remain separate obligations;
never reset/wrap the sequence or claim backup-restore replay safety from this probe.
