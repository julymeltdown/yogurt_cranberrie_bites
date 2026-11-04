# QA04 backend role and resource matrix

Status: NOT_RUN for actual HTTP. This entry point is separate from synthetic SQL
and frontend fixtures. It invokes only operations in the frozen domain registry;
it does not import frontend mocks or create raw-private-claim/beacon-list routes.

The runtime/seed owner supplies `MCR_QA04_REAL_PLAN` as an absolute private local
JSON file under `/private/tmp` (canonical path, current owner, mode 0600), following
the frontend QA04 plan format. Supply `MCR_QA04_BACKEND_REPORT` as a new absolute
report path, then explicitly run the ignored
`actual_qa04_http_authorization_matrix` test in `social_remaining`.

Required roles are owner, member, stranger, blocked, suspended, expired and admin.
Each has a distinct real canonical user and a `tokenEnv` reference. The plan names
the isolated origin/environment, service and seed revisions, principal handoff,
and all 21 role × list/detail/mutation cells. Each cell covers handle, claim,
evidence, beacon and recipient disclosure through applicable existing public
operations. Claim checks use actual public status/aggregate disclosure boundaries;
no browser raw-claim endpoint is invented. The seed owner binds concrete resource
IDs, typed query/body, intended mutation, expected status, forbidden canaries and
positive values. Mutations require explicit seed-mutation intent and unique keys.

Role names confer no authority. An active stranger/admin can have an empty 200
SELF collection, while an unrelated detail request can correctly return 404.
Expected errors must match the operation's reason catalog; input/media failures
cannot masquerade as authorization proof. Administrative status gives no private
handle or evidence bypass. Positive responses require positive assertions so an
empty unrelated payload cannot satisfy a protected-resource read accidentally.

All input is qualified before requests. The runner disables redirects, bounds
response size and time, checks no-store, and withholds tokens, payloads, canaries
and resource IDs from evidence. Missing setup returns exit 2/NOT_RUN with zero HTTP
dispatch; that refusal path was executed locally. No token or seed plan is tracked.

Captured request targets and response DTOs/statuses are validated by the retained
offline C04 oracle via the assigned Social test helper. It rejects extra fields,
duplicate JSON keys, non-finite numbers and undocumented statuses. It uses the
existing `scripts/mcr/requirements-contracts.txt` dependencies, performs no schema
fetch, and records oracle/schema hashes. Missing dependencies refuse execution.

Even successful HTTP remains PARTIAL (exit 2): actual WebSocket revocation,
cached authenticated product UI and public client behavior remain required.
Dependency/rate failures keep HTTP
PARTIAL. Runtime/source/seed provenance comes from the named runtime owner's
handoff; this harness does not independently attest process ancestry. Original
QA04 and BE28–30/37 acceptance remain open until the required actual flows pass.
