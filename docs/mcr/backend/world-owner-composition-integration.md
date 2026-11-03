# World owner composition integration — 2026-11-04

Status: **INTEGRATED source and isolated native composition**. Production World
serving remains unavailable until reliable known-invalidation delivery and local
acknowledgment work. This does not accept IN01 or the full realtime task.

Author commits `2e33bba3b6da453f0443a47e6acdd9fef4be98cc` and
`78e3ce3c2b0743285f115c3be1dc317ecbc28efb`, based on `aa33fa7`, are delivered
as `1d008582` and `4a05e467`. The eleven assigned paths and every existing-file
preimage match. All 1,577 author source pins were verified, and all delivered
files match the final author bytes. No additional general or SL review was added.

The bridge uses actual typed Auth, Profile and relationship clients and the
current Concert membership. It preserves the sealed parent and effective
deadline through admission, projection and recipient filtering. A local SQL
publication lease shares the runtime native-send fence; pause disables sends
before releasing locks and reports rollback failure. Configuration validates
the required owning-service endpoints and credentials, but cannot bypass the
missing invalidation qualification.

## Combined validation

The combined candidate includes the newly published Auth 0015 deletion upgrade
and Concert 0021 moderation migration, which were absent from the author's base.
That concrete integration difference justified a fresh four-owner native run.

On `4a05e467`, Rust 1.88.0 locked/offline compilation with two jobs, debug info
disabled and incremental compilation disabled passed in 21.24 seconds. The three
exact Cargo-JSON-selected test executables were copied outside the BACKEND target,
and all target handles were confirmed closed before private PostgreSQL started.
All 1,626 current source/input hashes remained unchanged after qualification.

Fresh copied-executable checks pass **30 library + 28 runtime + 16 WebSocket**
tests. The real guarded Auth/Friend/Profile/Concert native composition passes
against the current migrations, including first admission, current projections,
Friend filtering, local publication locking, switches/renewals, expiry/revocation
and cleanup. Every command exits zero; the owned PostgreSQL root is removed.
This is direct-SQL migration application in isolated databases, not evidence of
the production migration ledger or deployed transport. The author's four SQL
bridge and one sealed-Auth bridge checks remain bound to their recorded source;
they were not counted as freshly rerun.

See the [machine-readable integration evidence](../evidence/world-owner-composition-integration-2026-11-04.json)
and [author's behavior, provenance and native evidence](world-owner-composition.md).
The Main BACKEND reservation is released; no native cluster remains owned by
this integration check.

## Remaining C05 work

Authorization remains bounded by the existing effective Auth deadline (at most
two seconds), maintenance and credential expiry. Known logout/block/suspension/
closure must additionally reach an affected World owner and disable or invalidate
its relevant publication state before acknowledgment and further delivery.
Independent remote revisions are observations, not a global transaction.

The completed Auth-deletion chat has accepted C05-AUTH-INVALIDATION in its own
checkout: actual Auth mutation capture, a transactional durable scoped journal,
safe committed replay cursors and native rollback/restart proof. Auth migration
0016 is reserved for that work. Network exposure and World serving are not
enabled by this producer assignment. Global distributed atomicity and withdrawal
of bytes already handed to native transport are not additional product gates.
Approved character art, live browser flow, devices, load and deployment remain
unqualified.
