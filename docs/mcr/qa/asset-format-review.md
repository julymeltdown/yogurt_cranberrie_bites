# C06 format-checker prerequisite audit

Date: 2026-11-03. Independent report-only review, **review complete; pending
committed handoff and parent disposition**. Both reproduced findings are cleared
in exact parent candidate `0cfffcac959912738a957568b806ba897f3d41ca`.
This is C06 validation
tooling evidence, not artwork, venue, runtime or release acceptance.

## Scope and authority

QA branch `codex/mcr-qa-asset-formats` starts clean at published base
`f80b6d1fd6b9d86c3012cc12cea6de2b278065c5` in
`/Users/lhs/Desktop/mcr-worktrees/qa-asset-formats`. Only this report is writable
in the repository. Previous QA checkouts remain stopped. No application,
contract, requirements, validator or source-archive edits, Rust builds,
database/Docker activity or pushes were performed by QA.

Authorities are root working agreements; specification C06/C07, §7.1 schemas
and §7.3 data conventions; current `docs/mcr/assets.md`; and the five asset
schemas and actual validator. The specification's UTC product convention is
not silently amended by the broader RFC3339 format-only probes below.

## F1 [P2] — absent optional checkers silently accept invalid provenance

At the base, `scripts/mcr/requirements-contracts.txt:1` pins only
`jsonschema==4.26.0` and `referencing==0.37.0`.
`scripts/mcr/validate-assets.py:93` supplies a default `FormatChecker()` without
checking which formats were actually registered. Both the host Python 3.14.3
and a newly created requirements-only venv register UUID but lack URI/date-time.

Installed owning source `jsonschema/_format.py` explicitly returns from
`FormatChecker.check` when a format is absent. URI registration at lines 303–313
depends on `rfc3987`, or its fallback `rfc3986_validator`. Date-time registration
at lines 395–402 depends on `rfc3339_validator` and calls it with
`instance.upper()`. UUID uses standard-library `UUID(instance)` and required
hyphen positions; it imposes no UUID version/variant policy. Package metadata
lists optional `format`/`format-nongpl` extras, but neither extra is installed by
the original requirements. Registry presence is executable behavior, not a
consequence of passing a `FormatChecker` object.

Failure trigger: install only the committed requirements and validate a correctly
rehashed bundle with malformed provenance. Observed CLI results are exit **0**
and `CONTRACT_READY` for `source_uri: "https://exa mple.invalid/ref"` and for
reviewed `reviewed_at: "2026-02-30T12:34:56Z"`. A physical-place projection with
`PLACE_REFERENCE`, `UNVERIFIED`, and `source_uri: "not a uri"` also passes the
bundle API. This can falsely certify metadata syntax; it does not prove any
venue was actually reviewed or used by a production service.

### Actual C06 format inventory

| Format | Owning declarations | Registration needed |
|---|---|---|
| `uri` | `common.schema.json:57` provenance source URI; `map-manifest.schema.json:352` physical-place source URI | `rfc3986-validator` fallback, or optional `rfc3987` path |
| `date-time` | `common.schema.json:97,131` nullable reviewed timestamp and REVIEWED conditional | `rfc3339-validator`, which requires `six` |
| `uuid` | `common.schema.json:262` PlaceId, alongside its lowercase hexadecimal/hyphen pattern | Built-in standard-library UUID checker |

There are five format keyword occurrences across exactly three format names.
Tiled's data property named `format`, PNG/JSON resource-path patterns and `$id`/
`$ref` identifiers are not additional instance-format declarations. Resource
paths, asset IDs, content hashes, offline reference retrieval and cross-bundle
checks must remain intact.

## Isolated reproduction and semantic cases

QA created two fresh Python 3.14.3 venvs without system site packages. Each
installed the unchanged requirements with pip, exit 0. The second additionally
installed these exact pins, exit 0:

```text
rfc3339-validator==0.1.4
rfc3986-validator==0.1.1
six==1.17.0
```

The first venv has no registered `uri`/`date-time`; the second registers both
alongside `uuid`. The second resolves `rfc3339-validator`'s `six` dependency;
`rfc3986-validator` has no additional dependencies. No global Python packages
were changed. The venvs also resolved attrs 26.1.0, jsonschema-specifications
2025.9.1 and rpds-py 2026.9.1; the direct requirements are not a complete
transitive lock or a guarantee of byte-identical future package resolution.

The audit probe runs 21 scalar cases and 24 complete bundle cases per environment,
recomputing avatar/map/geometry/theme integrity references after mutation.
Requirements-only accepts 21 of the 24 bundles; the plugin environment accepts
10. Eleven malformed URI/date-time cases change from accepted to rejected;
three malformed/nonlowercase PlaceIds remain rejected in both. The 10 valid
bundles remain accepted. Host behavior matches the requirements-only outcomes.

| Cases | Requirements-only base | Base with required plugins |
|---|---|---|
| HTTPS URI and `urn:mcr:fixture:one` | Accept | Accept |
| URI raw space, relative path, empty string, `%GG`, unmatched IPv6 bracket | Accept incorrectly | Reject |
| UTC `Z`, RFC3339 offset, lowercase `t`/`z` with fractional seconds | Accept | Accept as current schema-format syntax |
| Impossible calendar date, arbitrary text, date-only, missing offset, hour 25 | Accept incorrectly | Reject |
| Physical-place source `not a uri` | Accept incorrectly | Reject |
| Standard fixture UUID, `aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa`, nil UUID | Accept | Accept under current PlaceId schema |
| Uppercase UUID | Format checker accepts; PlaceId pattern rejects | Same |
| Undashed UUID or nonhex UUID | Reject | Reject |

UUID constructor/version/variant restrictions from a frontend library cannot
replace these declared schema semantics. This report demonstrates the Python
side; it does not review or certify the separate frontend WIP correction.
Similarly, URI is not an HTTPS-only URL policy. Valid URNs and percent-encoded
characters must not be rejected just to imitate a missing checker.

All existing **21 C06 tests pass in both venvs**, and on the host: that suite
does not detect the missing format assertions. Separate actual CLI probes return
0 for each malformed bundle without plugins and 1 for each with plugins.
The audit driver itself exits 0 when those expected outcomes are observed;
that driver exit is not a claim that all payloads were valid.

## Narrow prerequisite recommendation and first candidate disposition

Use the three explicit optional dependency pins above with the existing two
requirements in a dedicated venv. Before constructing C06 validators, collect
the formats declared by all five local schemas and refuse any missing from the
actual checker instance. Share that checked instance across the validators.
Include the missing names and install route in the failure. Do not rely solely
on successful imports, an optional all-formats extra or `check_schema()`.

Tests must cover an incomplete registry even when the golden payload has null
format-bearing fields, rehashed malformed payloads, and valid values that expose
overrestrictive UUID/URL/timestamp substitutes. Direct optional pins are narrower
than installing every `format-nongpl` dependency. Using `datetime.fromisoformat`
or `urllib.parse` alone is not justified as exact RFC format validation; no
homegrown replacement is proposed by this review.

Parent candidate `27c6bbcbdbd30d1a755d7128dc30379e7190bf63`, base `f80b6d1`,
implements that prerequisite: exact three pins, recursive schema-format inventory,
missing-registry refusal and one shared checker, plus four meaningful test
methods. Independent read-only execution against that candidate produced:

- 25 C06 tests PASS, exit 0, using QA's plugin venv.
- Golden six-document CLI PASS, exit 0.
- All 24 audit bundle outcomes unchanged from the plugin-enabled base.
- Requirements-only candidate CLI refusal, exit 1: missing `date-time, uri` with
  an instruction to install the requirements in the validation environment.

QA verified all three candidate source pins, six unchanged-fixture pins and five
retained log hashes in its committed `result.json`. All five schema and six
fixture files are byte-identical to the base. The retained parent log records
117 core/realtime/domain/bundle tests PASS; QA inspected that evidence and did not
repeat those suites. F1 is source- and behavior-cleared by this candidate.

## F2 [P2] — registered plugins permit a final LF

The first candidate still accepts correctly rehashed full bundles containing
`source_uri: "https://example.invalid/ref\n"` or reviewed
`reviewed_at: "2026-11-03T12:34:56Z\n"`, reporting `CONTRACT_READY`.
Both pinned plugin regexes use a final `$` with `.match()`, which can match before
the final LF. This is an existing dependency-semantic limitation, not acceptance
widening introduced by the registration prerequisite. It is separate from F1.

The integrator corrected F2 in `0cfffcac959912738a957568b806ba897f3d41ca`,
directly on `27c6bbcbdbd30d1a755d7128dc30379e7190bf63`. After the missing-registry
guard, instance-only URI/date-time wrappers require each string character to be
printable ASCII `!` through `~`, then invoke the actual registered predicate.
Each wrapper captures its own predicate as a default argument and preserves the
registered exception tuple. Nonstrings retain the plugin behavior. No global
checker registry is mutated. Existing schema, hash, path, offline-reference and
bundle checks are retained.

Independent read-only qualification of that exact clean candidate:

- All 27 C06 tests PASS, exit 0; golden six-document CLI PASS, exit 0.
- Requirements-only CLI refuses missing `date-time, uri`, exit 1 as expected.
- All original 24 audit bundle outcomes match the plugin-enabled baseline:
  ten valid bundles accepted, fourteen invalid bundles rejected.
- Fourteen additional fully rehashed bundles reject LF, CRLF, tab, NUL, DEL,
  non-ASCII and raw-space suffixes for each of URI and date-time. This includes
  both independently reproduced final-LF failures.
- An encoded URI `%20` space remains accepted. The original valid URN, offset
  timestamp, lowercase `t`/`z`, nil UUID and nonversion UUID cases still pass.
- The global `FormatChecker.checkers` mapping is unchanged before and after
  construction and execution of the candidate validators.

QA verified the final candidate's three source hashes, six fixture hashes and
five retained log hashes against `result.json`. Git confirms all five asset
schemas and six fixtures remain byte-identical to `f80b6d1`. The parent's final
retained log records 117 core/realtime/domain/bundle tests PASS; these suites
were inspected, not independently rerun. **F1 and F2 are cleared for this exact
C06 tooling candidate; no further correction is requested in this scope.**

The selected RFC3339 plugin also explicitly documents that leap seconds are not
supported. This audit does not claim exhaustive normative RFC conformance or
silently redefine product timestamp policy to resolve that dependency limitation.

## Reproduction artifacts, commands and hashes

Owned temporary audit root:
`/var/folders/j4/41h_5mjj7j9d8f2j2bzcsngw0000gn/T/mcr-c06-format-audit-me13u42g`.
`probe.py` and JSON/log outputs are retained there; only this report enters Git.
The probe accepts a repository root, imports its actual validator, and writes
results to stdout. Reproduce using a fresh venv and its own Python/pip:

```sh
python3 -m venv /tmp/mcr-format-audit-reproduction
/tmp/mcr-format-audit-reproduction/bin/python -m pip install -r scripts/mcr/requirements-contracts.txt
/tmp/mcr-format-audit-reproduction/bin/python -B -m unittest discover -s scripts/mcr/tests -p test_assets.py -v
/tmp/mcr-format-audit-reproduction/bin/python -m pip install rfc3339-validator==0.1.4 rfc3986-validator==0.1.1 six==1.17.0
```

Use a new directory rather than overwriting an existing environment. The base
requirements-only reproduction refers to immutable `f80b6d1`; executing this
sequence against corrected requirements necessarily gives the corrected registry.
For the parent candidate, QA ran its scripts/tests read only with `-B`, using the
owned venv executable and placing all captured output under the temporary root.

| Source/evidence | SHA-256 |
|---|---|
| Base `scripts/mcr/requirements-contracts.txt` | `9cba6d4508d006391ab899893990d5ac5d03b4811a0f71f152c5883897ee8120` |
| Base `scripts/mcr/validate-assets.py` | `1819fc033e4ac306e661c4bae47b93a4a914ac19153ad8ce21c664b7a73ba173` |
| Base `scripts/mcr/tests/test_assets.py` | `af4ab3e64e82ec66bf5c510531536bf4d77c614a9afc0a2b64b67dfd9392055f` |
| Base `common.schema.json` | `b7be87ce387dd286bcd2ea895da04f1a8b886c1ea3dc35494f761ac05aad75b4` |
| Base `map-manifest.schema.json` | `ed01d26b2f561c9885ad3fc5fe66912ee8175334393e49a85652756e301d6800` |
| Installed `jsonschema/_format.py` in host and venv | `96bb3553b892076657f59d846a5b113987909b577d91e7f0d822e8d3c1bc3f8b` |
| Installed `rfc3339_validator.py` | `c220cb9495da5215a53ab173f695060294df2d1042a9cbaaf0cc622bcba09d6e` |
| Installed `rfc3986_validator.py` | `95fc6d48642f111952b25c040947765bccba669210c8c140b7ed9647fd7e470c` |
| Temporary `probe.py` | `4f0dfc8ee636f9f95df781be82d6374a13373283fd23b3c8214239f5e94019b6` |
| `requirements-only.json` | `9423ed95f73c5071da554c4143bad145221448f5d5ce1151c51f76392be53c95` |
| `required-formats.json` / `candidate-probe.json` | `0410579969d4da86dae4cea975c9fc0434b5c3f6ea2ea662e34ba4149a10df43` |
| `candidate-assets.log` | `e67c64e10594d3d14deec868e5b7586f68570222c032d726ee871b7c53c26222` |
| `candidate-missing-formats.log` | `9cc7b25faa0752c54abf231c43efb11da7026931c12a2d497a447a61c4a3e3b8` |
| `candidate-trailing-lf.json` | `5dbd6c0f9a904388200b01c391c7c91d76d99b83552c00cf1a120c1b97c73b4a` |
| Final `scripts/mcr/requirements-contracts.txt` | `fc6c67bbfd66de4a6fa8e5521823a733cedff5ea56657cbd49ca03a619504782` |
| Final `scripts/mcr/validate-assets.py` | `94cb9f3eae7b184794fefa626b4d00e4ff690c80975f0c140072f2d094ff8cf0` |
| Final `scripts/mcr/tests/test_assets.py` | `dede7e23b7f7c0ba88e26ec6b7dfdbf6b1ec2343374cc9f9112794839ca4c30a` |
| `final-candidate-probe.json` | `0410579969d4da86dae4cea975c9fc0434b5c3f6ea2ea662e34ba4149a10df43` |
| `final-candidate-full-string.json` | `5483a253566fa6b444cbe6edf46651eae6cf710d07a4860e4ffcc4e029f8cead` |
| `final-candidate-assets.log` | `3749d044f849a0b0b0943112970c3e76074e91007c97fc90ddc5b70d17490769` |
| `final-candidate-golden.log` | `19fa348abd240f607b8a66ea254de4dfd46a65965ce6ae721976eb664fd9afd2` |
| `final-candidate-missing-formats.log` | `9cc7b25faa0752c54abf231c43efb11da7026931c12a2d497a447a61c4a3e3b8` |

## Impact boundary and handoff

`validate-core.py:53,62`, `validate-realtime.py:77`, `validate-domain.py:89` and
`tests/test_domain_bundle.py:30` also instantiate `FormatChecker`. Installed
plugins affect them, but this bounded prerequisite adds no refusal/wrapper to
those validators. Their complete format/schema/runtime acceptance is not audited
here. No art, private-data policy, resource-loading, venue verification or product
release claim follows from this tooling correction.

A new unbudgeted persistent audit goal was created and actually returned ACTIVE.
F2 verification is complete. The goal remains active through committed handoff
and parent review fixes/disposition. Parent alone integrates and pushes. This report is the only
repository output of the QA assignment.
