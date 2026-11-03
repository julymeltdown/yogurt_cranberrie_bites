# Domain candidate integration — 2026-11-03

Candidate `b615e35` was integrated by fast-forward from the integrator's isolated
`codex/mcr-contracts-C04` worktree, based on `c654507`. Only domain schema,
operation/policy/mapping, fixture, oracle/test and handoff paths were included.
The follow-up registers its checks in the existing contract command and updates
coordination/progress. No frontend/backend implementation, dependency, asset,
source archive or existing C03/C05 schema changed.

`bash scripts/mcr/check.sh contracts` exited **0** from the integration checkout:

- Inherited final OpenAPI: 92 operations and two artifacts reproduce.
- Core target OpenAPI reproduces; 33 core fixtures pass.
- Six asset metadata documents and 24 realtime fixtures pass.
- Domain candidate: 144 definitions, 106 operations, 35 indexed positive/negative
  fixtures and all 15 inherited adapter mappings plus two status queries resolve.
- Asset/core/realtime/domain regressions: **21 + 23 + 31 + 37 = 112 tests pass**.

The domain status remains **REVIEW_CANDIDATE / NOT_ACCEPTED**, printed by the
validator itself. The test suite exercises static/semantic contracts only. There
are no database race, actual HTTP policy, provider, multiuser or runtime receipts
for C04. The [handoff](../domain-contract.md) lists the missing query/response
coverage, deletion/legal/retention work, final assembly and independent reviews.
Accepted project tasks remain 10/112 required; this increment does not advance
that count or any IA/integration gate.

The FE07 worker delivered `d7c720c` and `8ba0081`. Parent source review identified
a potential stale-scope dispatch between render and effect registration in
`useScopedLoad`, and unbounded per-route scroll memory. Both were returned to the
frontend owner for regression evidence and fixes. Parent inspected the worker's
360px world and 390px deep-link screenshots and its browser receipt; they show an
instrumented shell, not a live world. That candidate is not integrated here.
