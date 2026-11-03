# BE15 maintenance invalidation origin

Task status: IN_PROGRESS pending public composition/acceptance. The bounded
maintenance-origin extension compiled and passed2/2 native tests, actual restart
deduplication and owned cleanup at9c4e900. Public startup/consumption is NOT_RUN.
Exact source/binary/report pins are in `community-evidence/handoff.json`. Explicit scope delegated by delivery chat on 2026-11-04.

Migration0011 is forward-only. It permits exactly two typed origins on the
retained outbox: RESOURCE_INVALIDATED with a real receipt and
MAINTENANCE_INVALIDATED without a receipt or actor. Original command FK and
leases/attempts/backoff/acknowledgement/pruning rules remain. No second queue or
fake command is created. `Delivery.receipt` becomes `Option<Uuid>` and `kind`
is validated against it before claiming commits.

`commands::maintenance::MaintenanceInvalidation { id, event, reference }` and
`repository::outbox_maintenance::enqueue_maintenance_in(&mut PgConnection,value)`
are the exact new API. Caller must hold its aggregate transaction/locks, use a
stable maintenance UUID and commit mutation plus enqueue together. Rolled-back
enqueue leaves neither row. Same ID and metadata returns the same ID; changed
metadata conflicts. A permanent immutable metadata-only tombstone survives
queue pruning, so a retry cannot redeliver a pruned event. No automatic tombstone
reclamation. Queue active cap10000 and maintenance lifetime cap1000000 use the
same BE15 budget lock; reviewed reclamation/throughput qualification is separate.

Consumers durably deduplicate the outbox UUID and treat community notifications
as coarse event invalidation with fresh authorization. Revision cannot act as a
per-user cursor across parties. No sensitive projection or authorization follows
from a maintenance event. Real consumer composition and load remain NOT_RUN.

New code is authored here. Owned cluster helper is adapted from
`tests/community_party.rs` at4bf2bd4, itself adapted from BE15 owned validator at
f0e44bd. No source archive changes or new dependencies. Dedicated native
`tests/maintenance_outbox.rs` covers transactional rollback, metadata conflicts,
deduplication, lease expiry/reclaim/stale ack, retries, queue pruning/tombstone
retention and process restart. It creates no actor/receipt for maintenance.

Validation command after compilation:
`python3 -B backend/scripts/mcr/community_smoke.py --maintenance --repository-test <absolute-maintenance_outbox-test-binary> --report <new-absolute-report.json>`.
The harness reuses marker/inode/postmaster validated `migration_smoke.Cluster`,
private Unix socket/no TCP, and removes only its owned directory. Test authority
and real service policies are not inferred from these native queue checks.
