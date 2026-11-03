# Explicit event closure resume contract

This integrator decision freezes the bounded START/RESUME amendment for BE41 and
FE39 implementation. It does not accept all C04 contracts, mount the route or
certify actual cleanup owners.

The existing authenticated operator POST remains
`/api/concert/events/{eventId}/admin/closure`. Its closed request union uses the
existing `confirmation` discriminator:

- `CLOSE_EVENT`: `expected_revision` is the event revision; `reason` is required.
  Only OPEN/LIVE events may start one durable job.
- `RESUME_CLOSURE`: additionally requires `job_id`; `expected_revision` is that
  job's current revision. The exact event/job/original initiating actor must
  match, the event must be READ_ONLY, and the job must be PARTIAL_FAILURE.
  Current Auth, operator role and event-policy authorization remain mandatory.

Resume uses a fresh idempotency key and compare-and-swaps the failed boundary.
Completed steps remain completed; stable owner-effect identities prevent replay
of downstream effects. An identical same-key request is transport replay of
previous command admission, never an implicit resume. GET status and worker
polls do not retry a failed boundary. No force flag or additional endpoint exists.

A committed admission returns the authorized job with status 200. Status 202
remains pending/uncertain admission, not successful cleanup. ARCHIVED is allowed
only after every required step and retained-content/privacy obligation succeeds.

The schema, operation mode metadata and conditional policy were updated together;
OpenAPI and frontend operation metadata were regenerated. Six explicit fixtures
cover valid START/RESUME and invalid missing-job, mixed-start, extra-field and
malformed-job payloads. The source oracle and relocated generated schema both
enforce the closed union. Validation used the retained contracts environment:

- `scripts/mcr/validate-domain.py`: exit 0; 47 fixtures.
- `python -m unittest discover -s scripts/mcr/tests -p 'test_domain*.py'`:
  exit 0; 98 tests.
- Both generators and their `--check` modes: exit 0.

Existing FE39/BE41 owners may continue their exact scoped implementations against
this published contract. Native Rust/browser behavior, database concurrency,
authenticated route composition, real owning cleanup adapters and release
acceptance require their own evidence; schema validation does not prove them.
