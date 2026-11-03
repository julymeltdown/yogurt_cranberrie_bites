# C04 target and pagination follow-up

Date: 2026-11-03. Candidate `d0649ec`, based on integrated `f795ed4`.
State: **REVIEW_CANDIDATE**, not frozen or implemented.

All 106 registered operations now name their exact path parameter types and one
of 25 closed query schemas. Scalar encoding, UTF-8 decoding, query size/field
bounds, state/stock/time filters and revision-bearing DELETE targets are explicit.
Party and giveaway activity use independently paged collections. The cursor
policy requires opaque authenticated encrypted or server-stored random tokens
bound to the actor, event, operation, collection, filters and snapshot boundary;
the parsing oracle validates syntax, not cryptographic authenticity or authority.

The oracle handles chained offline schema references, rejects undeclared fields
and duplicate encoded keys, and binds the target event to supplied current context.
Malformed numeric/boolean coercions, controls, percent encoding, foreign events,
ambiguous activity cursors and unknown receipt/audit command filters are covered.
The first expanded run failed on a chained timestamp reference; the resolver now
follows bounded references and rejects cycles. A test fixture incorrectly used
revision zero where the mural contract requires a positive baseline; it was fixed.

`python3 -B -m unittest discover -s scripts/mcr/tests -p test_domain.py`
exited 0 with 46 tests, including a target pass across all 106 operations.
`bash scripts/mcr/check.sh contracts` exited 0: both retained generators and the
browser artifact match, 6 asset documents, 33 core, 24 realtime and 35 domain
payload fixtures validate, and 21 asset + 23 core + 31 realtime + 46 domain tests
pass (121 total). `git diff --check` exited 0.

This is not complete per-operation HTTP payload/status/context coverage. Global
deletion recovery, legal/document publication, retention/closure, remaining
aggregate and media semantics, final domain OpenAPI assembly and independent
reviews still prevent C04 freeze. No runtime or task acceptance count changes.
