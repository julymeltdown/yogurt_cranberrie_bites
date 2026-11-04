# C07 generated client and deterministic contract harness

Status: IN_PROGRESS. Original C07 criteria implementation; qualification and publication pending.

The generator consumes the canonical `contracts/generated/mcr-domain.openapi.json`
and the existing contract/fixture corpus without changing those inputs. It emits
all registered HTTP operation request/response types, public schemas, Zod adapters,
a validated fetch client and deterministic test transport into
`contracts/generated/api-client/` and the identical public TypeScript mirror at
`frontend/src/shared/api/generated/`. Application consumers are outside this grant.
The manifest inventories source hashes and every generated file. The independent
fixture namespace is `contracts/fixtures/c07/`; both namespaces are exhaustive:
missing, unexpected, non-JSON fixture files and byte drift fail the check.

## Provenance and ownership

Base: `bcbcf3edf4eba23a3011358018142574077d1426`, branch
`codex/mcr-contracts-C07`, existing exclusive `be-avatar-motion-v2` checkout.
The original C07 writer returned ownership before source work. Root explicitly
transferred the unused frontend generated prefix after the frontend owner's STOP.
Root remains the publishing owner; no king or SL technical review is required.

All rendering remains newly authored. No rendering or legacy application logic
is copied. The reusable witness algorithm is imported read-only from
`scripts/mcr/contracts/qa01-coverage/witness.py`, and the independent Python
validator is imported read-only from `scripts/mcr/validate-core.py`.
Their exact revisions and canonical JSON hashes are recorded in the generated
manifest. TypeScript, Zod and Python dependencies retain existing project pins.
The Zod adapter uses the public `z.custom<T>` API documented at
[Zod custom schemas](https://zod.dev/api#custom).

Root's narrow follow-up grant permits only QA01 input namespace selection and
fixture discovery changes plus exact regressions. Commit `970cc4e6` separates
these two C07 prefixes from legacy QA01 discovery. The legacy pin file and
existing fixture/schema/generator/output bytes remain frozen. Outside these exact
prefixes, new canonical inputs still reach the unchanged pin comparison and
unclassified non-JSON fixtures still fail. CI invokes full QA01 and C07 together.

## Behavior and limits

Request path/query/header/body schemas and response status/media/header/body
schemas are checked at transport boundaries. Binary bodies use `Blob`, form
bodies use `URLSearchParams`, and redirects remain manual. The client accepts
an injected in-memory token getter and fetch; it stores no credentials and never
exposes browser-owned Set-Cookie headers. Server authorization remains authoritative.
There is no automatic refresh, retry, application router change or live backend.

Structural TypeScript types represent wire shapes. Zod adapters enforce the
canonical schema refinements using bounded local-reference resolution, boolean
and composed schemas, conditionals, enum/const, object/array constraints, formats,
UTF-8 password byte limits and finite JSON values. Integers outside JavaScript's
safe range fail closed; exact large-integer transport requires a future explicit
contract representation. Unsupported canonical keywords/types/formats fail
generation. Zod errors contain a schema name, not private response values.

Fixtures are schema witnesses and mock transport evidence, not server responses
or business-rule acceptance. Every operation/status/media and named schema is
covered. Python JSON Schema decisions provide independent positive and negative
compatibility cases. Mock scenarios have an explicit clock and seed; no wall
clock, random source, backend, browser or device is used.

## Reproduction

Use the retained Python requirements from `scripts/mcr/requirements-contracts.txt`,
the pinned Node 24.22.2 executable, and dependencies resolved by the retained
frontend lockfile. Local qualification installs nothing and writes temporary
compile output only under the caller's artifact directory.

```sh
python -B scripts/mcr/generate-api-client.py
python -B scripts/mcr/contracts/qa01-coverage/run.py
python -B scripts/mcr/contracts/qa01-coverage/test_coverage.py
python -B scripts/mcr/contracts/c07/run.py --node /absolute/path/to/node --dependencies /absolute/path/to/node_modules --artifacts /absolute/path/to/c07-evidence
```

The harness regenerates expected bytes, checks exact inventory, and requires the
deliberate schema, missing-operation and output mutations each to fail with exit 1
and a specific drift diagnostic. It also runs namespace mutation regressions,
strict TypeScript compilation and Node runtime tests. Each command's exit is saved
in `receipt.json` with bounded logs. `.github/workflows/mcr-contracts.yml` runs
the same checks after frozen dependency installation in an ephemeral CI runner.

## Qualification record

Not run yet. This section will record actual source/contract revisions, commands,
exit codes, evidence paths, limits and publishing ancestry before handoff.
