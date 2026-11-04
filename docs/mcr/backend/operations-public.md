# Operations public composition

Task: OPERATIONS-PUBLIC-01. Producer owner: backend205. Publisher: Root1fc.
Status: IN_PROGRESS. Runtime validation: NOT_RUN. No king or SL review gate.

## Registered additive access contract

The versioned `operations_access_v1` policy registers
`getOwnConcertOperationsAccess`, a bodyless SELF GET at
`/api/concert/events/{eventId}/me/operations-access`. Its closed response is
`admin.OwnOperationsAccessV1` with `server_time` and nullable `access`.
The access lease contains exactly actor/event, MODERATOR or OPERATOR, current
positive safe-integer role revision, actual request UUID as decision identity,
and UTC checked/expiry times. This informational read is never a bearer grant.
No active assignment returns null; missing configured authority returns 503.

`server_time` and non-null `checked_at` must be the SAME actual PostgreSQL clock
sample. Expiry is the earlier of assignment expiry and that sample plus 60
seconds. The producer must retain its current role exclusion fence through the
actual canonical Root HTTP terminal. A business-transaction SHARE lock alone is
insufficient. Production role assignment/withdrawal requires configured trusted
issuer material, durable exact revisions, expiry and revocation, not fixture SQL
or an environment role boolean.

The browser binds its full captured canonical PrivacyScope and qualifies the
full request RTT against a monotonic request-start clock. Its local deadline is
request start plus the bounded server TTL, never response receipt plus TTL. It
rejects unknown, backward, late, expired, aborted or scope-mismatched results and
synchronously retires the old decision before replacement or null. Every read
rechecks exact scope and expiry. Every privileged server operation independently
checks current authority; clients cannot infer an unobserved server withdrawal.

The existing registered `getConcertSettings` / `admin.EventSettings` response is
the OPERATOR-only authoritative producer of event revision and phase for FE39.
The access response deliberately contains no invented event facts. FE37 owns
Moderator consumers; FE38 and Root153 own Operator/clock/common adapters. They
consume an immutable registered producer handoff after actual qualification.

## Current evidence and limitations

The published client-foundation commit `bc75182e` contains delivery/readset
evidence; it does not register a role/clock producer. The previous USER/no-media
Report path remains separate and retains its real canonical owners.

Source checks (strict `/private/tmp/mcr-contracts-admission/bin/python -B`):

- `-W error -m unittest discover -s backend/scripts/mcr -p operations_public_contract_test.py`: 4 passed, exit 0.
- `scripts/mcr/validate-domain.py`: 175 schemas, 115 operations, 55 fixtures, exit 0; candidate state, not runtime acceptance.
- `scripts/mcr/generate-domain-transport.py --check`: exit 0.
- `backend/scripts/docs/generate-mcr-domain.py --check`: exit 0.
- `git diff --check`: exit 0.

These checks establish closed wire registration only. Production role issuance,
durable delivery fencing, restart recovery, actual authorized/denied/revoked
HTTP, and combined Operations composition are NOT_RUN. No privileged route is
claimed mounted by this contract-only source change.

The report evidence byte operation is not frozen: actual Media report-purpose
association/READY ownership and its EOF fence must first be traced and composed.
Reviewed-place release, notice source/legal review and FE41 operational
maintenance/application-version decision inputs remain real-owner integration
work; no successful stub or hardcoded decision may substitute for them.

## Source ownership

Root explicitly delegated only the additive domain admin schema/operations/
policy, exact regeneration postimages in the two domain OpenAPI artifacts and
`frontend/src/shared/api/domain-operations.generated.ts`. CORE contract ownership
remains backend207. The generator scripts are reused unchanged. No frontend
handwritten implementation, core schema, proto or source-pack content changed.
All `docs/source/` bytes remain unchanged.
