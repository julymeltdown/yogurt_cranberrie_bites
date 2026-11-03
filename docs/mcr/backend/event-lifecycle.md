# BE41 event lifecycle — implementation record

Status: IN_PROGRESS. Base `cd7192febf9c2764169b55d63a70d7e00d13fd08`, branch
`codex/mcr-be-BE41`, isolated checkout `/Users/lhs/Desktop/mcr-worktrees/be-event-lifecycle`.
Persistent goal is ACTIVE, unbudgeted. Source milestones are not BE41 acceptance.

## Provenance and contract

Newly authored Rust modules; no Bigyeting rendering or source code copied.
Consulted BE15 command/receipt/outbox implementation at base above, C04
`contracts/domain/v1/{admin,mural,common}.schema.json`, `operations.json` and
`policy.json`. Archive is `admin.Archive`, not a separate archive schema.
The seven public steps are READ_ONLY, FREEZE_MURAL, SNAPSHOT, EXPIRE_GRANTS,
EXPIRE_BEACONS, EXPORT, CLEANUP. BE38 owner source consulted at its worker HEAD;
shared migrations/exports/composition remain Delivery-owned. Source archives unchanged.

## Proposed explicit retry amendment (not frozen)

Existing `closeConcertEvent` POST route only. START retains existing
`{expected_revision:event,confirmation:CLOSE_EVENT,reason}`. RESUME is
`{job_id,expected_revision:job,confirmation:RESUME_CLOSURE,reason}` with a new
idempotency key, current initiating actor/OPERATOR/Auth/event-policy checks and
same bound job in READ_ONLY. Exact same-key/body transport replay returns the
previous command acceptance; it never repeats job effects. GET status never
resumes work. Failed step CAS and stable owner operation keys protect recovery.
A receipt SUCCEEDED means durable local admission actually committed, not that
all downstream steps succeeded. BE15 prohibits SUCCEEDED/202; use existing
success status 200 with the job reference and project current authorized job.
202 remains pending/uncertain only. Delivery must register/generate the amendment.

## Required remaining implementation and evidence

Durable PostgreSQL job/step persistence; current fenced authority; actual owner
freeze, archive projection, grant/beacon + reservation/party/world expiry;
configured ownership-aware retention; strict anonymous archive reads with current
Post/media removal fences, immutable mural snapshot and revision-bound cursors.
No fake applied owner receipt. ARCHIVED only after required cleanup succeeds.

Rust 1.88.0 locked/offline, jobs=2, debug=0. Shared target is Delivery-owned;
no Cargo command run. Domain/application roots not exported yet. Unit compilation,
real owned PostgreSQL persistence/restart/partial-failure, HTTP integration and
acceptance are NOT_RUN at this source milestone.
