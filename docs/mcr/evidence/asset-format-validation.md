# C06 format-validation correction

Parent candidate starts at `f80b6d1fd6b9d86c3012cc12cea6de2b278065c5`.
The Python 3.14.3 environment registered the UUID checker, but neither URI nor
date-time. Installed `jsonschema==4.26.0` returns successfully when a format
checker is absent. The previous requirements-only environment could therefore
accept a rehashed malformed provenance timestamp or source URI.

The correction installs the exact required optional plugins in an isolated venv
and makes `validate-assets.py` inspect all five local schemas before creating its
validators. Every declared format must have an actual registered checker; a
missing checker refuses validation with an installation instruction. Independent
QA then reproduced a pinned-plugin edge: `$` with `match()` accepts a final LF.
Instance-only URI/date-time wrappers now reject raw whitespace, controls and
non-ASCII input before invoking the real checker; URI percent encoding remains
valid. No global checker registry is changed. No contract,
golden fixture, product identity policy or resource allowlist is changed.

Using the venv's Python, `validate-assets.py` validates all six documents;
`unittest discover -s scripts/mcr/tests -p test_assets.py -v` passes 27 tests;
the retained core/realtime/domain/bundle suites pass 117 tests; planning and
`git diff --check` pass. New tests rehash complete bundles before checking invalid
calendar/time/URI values, and accept canonical UUIDs without a new version/variant
policy plus lower-case RFC3339 `t`/`z`. Full-string cases reject LF, CRLF, tab,
NUL, DEL and non-ASCII suffixes while allowing encoded URI spaces.
The explicit missing-checker test refuses
an incomplete registry. The unmodified host environment also exits 1 with the
expected missing-plugin message instead of returning `CONTRACT_READY`.

[Machine evidence](asset-format-validation/result.json) records tool versions,
source and unchanged-fixture hashes, expected refusal and all log hashes. Logs
are retained beside it. Python packages were installed only in a parent-owned
temporary venv; the global Python environment was not changed. Reproduce with
`python3 -m venv /tmp/mcr-format-check`, then its `python -m pip install -r
scripts/mcr/requirements-contracts.txt` and its Python for the commands above.

The pinned date-time plugin does not support leap seconds; this patch does not
claim complete general RFC3339 conformance or invent a leap-second table.
This closes the observed C06 tooling gaps. Other validators using `FormatChecker` benefit
from installed plugins but do not gain this explicit registry-refusal guard in
this patch. C06 remains `CONTRACT_READY`, not frozen or accepted. Runtime loading,
original artwork, venue truth and whole-project acceptance remain separate work.


Parent integrated both corrections serially as `6a1e394`/`04a8268` and the
independent [QA report](../qa/asset-format-review.md) `edd0226` as `bab2f1f`.
Final combined candidate `bab2f1f` passes all **144** asset/core/realtime/domain/
bundle regressions using the isolated pinned environment. All final source and
fixture pins match. [Integration receipt](asset-format-validation/integration.json)
and combined log record this result separately from the isolated-candidate logs.
QA independently closed both findings; no schema, fixture or acceptance count
was changed.
