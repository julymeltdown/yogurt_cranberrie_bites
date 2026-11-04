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
UTF-8 password byte limits and finite JSON values. Integer validation follows
JSON Schema for representable JavaScript numbers, including values above the
safe-integer range; exact preservation of arbitrary 64-bit integers requires an
explicit future wire representation. Unsupported canonical keywords/types/formats fail
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

The follow-up producer audit records 84 published input deltas (16 changed and
68 added), with exact published postimage hashes and six responsible producer
groups. The route inventory advances from 134 to 135 solely by `getConcertIntro`;
no existing route is removed or changed. Historical pins are archived exactly.
Active pins remain unchanged until the temporary candidate passes the complete
coverage path. A new C07 helper and two regressions cover the candidate/reviewed
motion metadata fixtures; those follow-up additions remain NOT_RUN pending a
fresh allocation. The old checker also needs an explicit ownership extension to
invoke that helper and enumerate four v2 schemas. Social fixtures already use
existing trace/wire-rejection branches and need no new dispatch logic.

## Forward-baseline preflight

Root granted the three exact legacy checker extensions and additive regressions.
Commit `fcee6a69` invokes offline motion coverage before the exhaustive fixture
comparison, appends the four explicit v2 schemas, and counts root-only schemas
without requiring `$defs`. Existing Social dispatch and original negatives are
unchanged. Commit `a5d372ba` also corrects integer compatibility: the canonical
Auth `expiresIn` has `int64` format without a safe-integer maximum, so the client
uses JSON Schema integer semantics and adds independent numeric boundary cases.
Those client additions require fresh generation and validation; the earlier C07
PASS is not extended to them without execution.

The amended source-only allocation
`C07-FORWARD-BASELINE-SOURCE-B-202611041534Z` began at
2026-11-04 15:43:16 UTC on clean
`a5d372bac1e2ae0f737a08d7b607f735d9b30a84`. Its first and only workload was:

```sh
<retained-contract-python> -B scripts/mcr/contracts/c07/qualify_baseline.py --pins <round>/candidate-pins.json --output <round>/preflight.json
```

Exit 1: `ValueError: generated OpenAPI drift`. The candidate passed source schema
checking (229 definitions, including the four newly enumerated schemas), then
failed comparison with the canonical domain generator before fixture coverage.
No active pin update, client regeneration, full QA01 negatives or new C07 check
run followed that failure. The exact preflight/log and control/source hashes are
retained under local `mcr-tools/c07-baseline-20261104T1521Z`.

The historical archive still matches the untouched active pins byte for byte.
Root separately assigned the already-public C05/Intro generated baseline mirror
correction to the Wall producer; this is a named source dependency, not a full
Wall implementation barrier or permission to mask drift. After its qualified
publication, this owner must consume the corrected baseline, refresh the exact
audit, and rerun the complete candidate/public QA01/C07 path.

This allocation reached FULL at 15:44:21 UTC with no worker/guardian process,
temporary workspace or link remaining. Nonshrinking carry was 3,252,854,784 bytes,
new positive growth 819,200 bytes, free space 18,352,828,416 bytes; the guardian
recorded a peak of two scoped processes and no breach. The latest human direction
allows the designated publisher's single canonical delivery receipt; repeated
independent readback proofs and proof-only commits are not required. Functional
validation, fixes and the meaningful committed handoff remain required.

## Published prerequisite consumed

Root published the separately qualified two-mirror correction as
`42a542e8f4b045a1afe426c1f6bdd56ba2c2b842`, from producer
`0e75917206d716b4f7f80825f9d5526a3c11f35a`. Its frozen public canonical inputs
and unchanged generator passed generation and `--check`; both mirrors have
SHA-256 `4c185d9099b7e02cc61fce1e58b475be169e78bd5ba20b295e99827fc07e034f`.
The publisher supplied the single authoritative delivery receipt. No redundant
consumer remote proof was performed.

Consumption commit `2b390c234665252d03793779c3cc38ee00026cad` changes only those
two already-published mirrors. The new audited forward candidate is recorded in
`archive/forward-baseline-2b390c234665252d03793779c3cc38ee00026cad.json` under the
QA01 checker directory. Exactly two hashes differ from the failed temporary
candidate; all other audited input deltas and the single additive route remain
the same. Historical pins, their byte-identical archive and the failure receipt
remain unchanged. This source consumption includes no Wall feature work.

The next source-only allocation was not accepted: fresh free space was
15,013,621,760 bytes, below its required 17,179,869,184-byte start floor. No
guardian or workload started and no clock or slot was retained. Source and
controls are ready under local `mcr-tools/c07-published-baseline-20261104T1554Z`;
the temporary-candidate preflight and complete public QA01/C07 checks remain
NOT_RUN on this corrected source until a usable allocation is accepted.
