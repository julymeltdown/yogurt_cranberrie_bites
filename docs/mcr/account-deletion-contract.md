# BE42 / FE36 canonical deletion targets — 2026-11-04

The canonical Auth deletion saga includes Concert cleanup in the same durable
workflow. The ordered target inventory is now:

1. `PROFILE`
2. `FRIEND`
3. `POST`
4. `NOTIFICATION`
5. `CONCERT_TICKETS`
6. `CONCERT_SESSIONS`
7. `CONCERT_SHARING`
8. `CONCERT_PRIVATE_DATA`

The sole schema source is `backend/scripts/docs/openapi/schemas.py`.
The retained OpenAPI generator, MCR core generator and domain assembler propagate
this enum. The protobuf target remains a string; no field number, endpoint,
response shape, saga status, step status or idempotency rule changes.
This dated source revision extends C03's target vocabulary; it does not establish
runtime readiness. The corresponding producer and consumer must be integrated
together before claiming account-deletion compatibility or acceptance.

Completion requires every required target exactly once, each `SUCCEEDED` with
its completion timestamp, plus consistent saga completion metadata and the same
canonical user/saga identity. A legacy four-step result, missing/duplicate/unknown
target, failed step or missing timestamp cannot establish completed cleanup.
Wire schema validation alone does not enforce this aggregate invariant: BE42's
repository and FE36's decoder must enforce it independently and test it.

`deleted: true` means the account was disabled; it does not certify full cleanup.
An uncertain DELETE is never automatically repeated. Only an explicit GET may
recover the same user's deletion status under the existing suspended privacy
scope. Pending, retrying and dead-lettered cleanup stays visible; raw last-error
details and credentials must not enter UI state.

Auth's new forward migration 0015 follows the already published 0014. Existing
successful legacy steps are preserved, older completed sagas reopen for missing
Concert work, and dead-lettered work retains the operator recovery boundary.
Missing real Concert cleanup owners must fail closed, not report success. Native
typed-fixture qualification does not prove revoked reconnects, erased personal
data or production owner composition.

Main owns this contract revision and generated artifacts. BE42 owns the separately
assigned backend implementation; FE36 owns its separate consumer patch. No
frontend implementation imports backend code and neither worker edits the shared
contract as a side effect.

Validation on the contract-only patch: 24 core contract tests and 17 domain
bundle tests pass, including generator determinism and unknown-target rejection.
Retained OpenAPI generation check passes for 92 operations. Structural comparison
of all four generated JSON artifacts confirms the target enum is their only
change. These checks do not run the backend saga or frontend deletion flow.
