# C07 generated client and deterministic contract harness

Status: IN_PROGRESS. C07 source qualification passed; full QA01 is blocked by
historical pin drift. Combined CI and publication are pending.

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

The source-only round `C07-ORIGINAL-SOURCE-B-202611041514Z` began at
2026-11-04 15:14:21 UTC on source
`5b9e6f4a3903a886bec03beb3465fcfcf477aee9`, with a 600-second work deadline
and 120-second cleanup allowance. Input/runtime/guardian SHA-256 pins and the
original cutoffs are retained in the local round's `accept.json`. The local
artifact directory is `mcr-tools/c07-20261104T1514Z` beside the delivery checkout;
machine-specific absolute paths are deliberately omitted from committed prose.

Initial generator execution exited 0 and produced 14 inventoried artifacts:
135 operations, 243 named schemas, 1,528 operation/status/media witnesses, and
28,290 independent Python compatibility decisions. No fixture claims live
business-rule or multiuser acceptance. The seven C07 harness checks passed:
generator drift exit 0; schema-mismatch, missing-operation and output-drift probes
each exit 1 with the expected drift diagnostic; inventory regression exit 0;
strict TypeScript compilation exit 0; five Node runtime tests exit 0.
`checks/receipt.json` records exact argv and exit codes, with separate logs.

Full QA01 `run.py` exited 1 at its unchanged exact pin comparison. Its public
unittest suite exited 1 because the required positive setup failed, so none of
its twelve legacy negative probes is claimed as executed. The two newly granted
namespace regressions pass independently. Logs retain both the initial failure
and the subsequent suite result after isolating those two new tests in their
own class; no old negative test or setup requirement was weakened.

The frozen pin file names canonical candidate
`d042638789bc84587be7bbbe09b32c324570c75f`. Sixteen already-pinned files have
different bytes at delivery base `bcbcf3ed`; additional legacy canonical inputs
are also absent from that old pin inventory. `legacy-integrity.json` records the
base/current/pinned hashes. All 231 pre-generation canonical/source snapshots,
including `docs/source/` and the old pin file, remain byte-identical to the
pre-generation snapshot. This failure predates C07 output generation.

The initial exec launcher encountered `setsid` EPERM because the execution tool
already supplied an isolated process group. The corrected launcher retains that
group only when its PGID equals its own PID; both versions and the failure are
retained. No application/runtime failure is hidden by this launcher correction.

Root subsequently authorized an exact, separate audited forward-baseline scope:
the active QA01 pin file, provenance-only README changes, and two new immutable
archive records. That work starts after this round releases its resources. It
does not authorize canonical input changes, silent repinning, unqualified
producer deltas, global exclusions or a passing CI claim before full rerun.
