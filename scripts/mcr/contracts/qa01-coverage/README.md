# QA01 contract coverage

Run with Python and the exact packages in `scripts/mcr/requirements-contracts.txt`.
The retained validation environment can be reused without installing dependencies.

```sh
<contract-python> -B scripts/mcr/contracts/qa01-coverage/run.py
<contract-python> -B scripts/mcr/contracts/qa01-coverage/test_coverage.py
```

The positive command validates the frozen canonical input hashes, every source
schema/reference, fixture catalog and discovered fixture, every registered HTTP
operation, generator reproducibility and every generated operation/status/media
response schema. JSON/HTML/header witnesses are deterministic schema instances;
they are not observed service responses. Binary schemas and bodyless responses
are classified explicitly, without inventing JSON bodies.

`--output docs/mcr/evidence/contracts/qa01-coverage/report.json` saves a detailed
inventory, all response witnesses, source hashes, dependency versions and limits.
`QA01_WRITE_TEST_RECEIPT=1` makes the regression command save its negative receipts
in that same evidence directory. No output is allowed outside the evidence lease.
Imports and generator checks suppress Python bytecode writes; generators run only
with `--check`. No application, canonical input or generated artifact is rewritten.

`--probe <name>` corrupts an in-memory input before the ordinary validation path.
Every supported negative probe must exit 1; it does not turn rejection into a CLI
success. The regression suite first requires a real positive run, then checks the
expected rejection class/message for every probe. Missing dependencies, missing
format plugins, duplicate keys, nonfinite JSON, unsafe/duplicate catalogs and
empty source coverage also have regressions.

The committed `pins.json` records the initial actual input revisions/hashes and
complete route identity inventory. `--freeze` creates that file **only when it is
absent**, and cannot be combined with a negative probe. It refuses to overwrite
an existing pin. A later canonical revision requires an explicit documented pin
update and new complete validation; do not repin merely to silence drift. The
normal command fails on any missing, added or changed pinned input.

## Audited forward baseline

Root explicitly assigned forward reconciliation after the C07 source-only round
released all resources. The historical baseline remains preserved byte for byte
at `archive/pins-d042638789bc84587be7bbbe09b32c324570c75f.json`.
The audit record `archive/forward-baseline-c525bd8c3129c191f9ed97f355ba5ead0ec006f6.json`
records all 84 input deltas (16 changed, 68 added), old/new SHA-256 values,
published producer ancestry, producer task/evidence dispositions, and the exact
operation delta: only additive `getConcertIntro`, with no removed or changed
route identities. Motion candidate evidence remains candidate-only; none of
this pin accounting grants art, runtime, privacy, service or release acceptance.

The active pins are not yet changed. Fresh validation of the complete temporary
candidate must pass before replacing them. Legacy QA01 also needs explicit
coverage for the newly published motion and Social policy fixtures; silently
ignoring these files or repinning around a failure is prohibited. After an
audited forward update, the ordinary public positive command, all original
negative probes, and the complete C07 harness must run again on the candidate.

There is no mutation of CI composition in this assignment. These commands can be
invoked by a separately owned CI entrypoint. Coverage establishes offline QA01
criteria only, without promoting C07, production routes, art, live services,
devices or IN gates. See the evidence README for the task disposition and pins.
