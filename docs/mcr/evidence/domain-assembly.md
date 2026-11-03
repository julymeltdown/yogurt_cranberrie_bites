# C04 combined HTTP assembly candidate

Date: 2026-11-03. Base: `03660135770241189f7bdbee4b53837bf6e60510`.
State: **REVIEW_CANDIDATE / NOT_ACCEPTED**.

`backend/scripts/docs/generate-mcr-domain.py` invokes the existing C03 builder,
which invokes the retained final port-rust assembler. It appends C04 and C05 HTTP
registrations without rewriting inherited Auth or the frozen core artifact.
The browser-readable output is `contracts/generated/mcr-domain.openapi.json`;
it has 132 unique operations (24 core, 106 domain, two world HTTP), 162 relocated
domain definitions and 19 realtime definitions. Existing core paths and schema
values remain equal to the frozen document. No backend/frontend implementation,
dependency, lock, source pack or original planning archive changed.

The candidate includes the closed query-schema reference and explicit scalar
parameters, exact path types, structured JSON mutation bodies, separate binary
image PUT/read content, public/authenticated access, constrained error reasons by
status, pending command receipts, Origin/intent and UUID idempotency headers,
no-store headers, media nosniff, and source domain/realtime policy. World tickets
do not acquire domain idempotency or receipt/replay behavior. C05 frames are
schema components, not a fabricated HTTP upgrade route.

The assembler rejects duplicate method/path registrations and operation IDs,
equivalent templates with renamed parameters, component/source-ID collisions,
undeclared path parameters and unresolved/external schema references. All 181
new definitions retain their source assertions after relocation. JSON Schema
schema checks and local registration/reference checks are explicit; no complete
OpenAPI specification validator or generated SDK is claimed.

Verification on this base:

- `python3 -B -m unittest discover -s scripts/mcr/tests -p test_domain_bundle.py`:
  **14 tests pass**, exit 0. Covers all registrations, original core equality,
  source-schema relocation, exact query projection, path/body distinctions,
  every domain error status/reason matrix, receipt/security/origin/idempotency
  metadata, binary types, non-replayable world tickets, positive indexed domain
  payload fixtures, collision/reference rejection and deterministic drift check.
- The first run exposed an inherited parameter `$ref` in the local integrity
  checker; it now resolves registered parameter components before comparison.
  The first test run also used the wrong existing origin fixture filename;
  corrected before the passing run. Neither issue changed frozen inputs.

`bash scripts/mcr/check.sh contracts` exited 0: inherited 92-operation/two-artifact
drift, frozen C03 and wider candidate drift, six asset documents, 33 core, 24
realtime and 35 domain fixtures; **21 + 23 + 31 + 46 + 14 = 135 tests pass**.
The original frozen core output remains unchanged. Plan and whitespace checks
are recorded with the integration checkpoint.
Per-operation payload/status/context fixtures, global deletion/legal/retention
contracts, remaining media/aggregate semantics and independent BE/FE reviews still
prevent C04 freeze. Runtime authorization, database races, providers, actual
multiplayer, device/load and release gates remain unqualified. No task/IA count
advances from this assembly work.
