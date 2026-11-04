# C04 0.1.3 introduction detail

Status: `CONTRACT_READY`. This additive contract closes the original BE27
requirement in `MCR_IMPLEMENTATION_PLAN_V2_EN.md:482`: direct-ID and list paths
must enforce identical eligibility, including unknown age failing closed for
restricted rows. The historical endpoint table omitted a standalone detail
operation. Root explicitly delegated this narrow contract change on 2026-11-04;
this is an addition to required scope, not a scope reduction.

The contract patch is based on consumer revision
`6dbc84a0ebf5541a07e9358931d8829ef4440a5e`. The existing operation objects and
C03, C05, C06 and C04 payload schemas are unchanged. Only the C04 policy version
advances from `0.1.2` to `0.1.3`.

| Field | Value |
| --- | --- |
| Operation | `getConcertIntro` |
| Method and path | `GET /api/concert/events/{eventId}/intros/{introId}` |
| Path identifiers | Canonical UUID `eventId` and `introId` |
| Request body | Absent (`request: null`) |
| Query | Closed `common.EmptyRequest` |
| Success | HTTP 200, existing `social.IntroPublic` |
| Authority | `MEMBER`, `public_read`, `READ` |
| Retry and caching | `SAFE_READ_AFTER_REAUTH_ONCE`, `no-store` |
| Idempotency | No key, pending receipt or mutation replay |
| Errors | Exact existing `listConcertIntros` read error registration |

The same approved C03 public author union is reused. Private age bounds, claim
provenance, expiry and owner state remain forbidden public fields. Actual list
and detail eligibility is enforced by the same scoped Intro executor. A denied,
expired, unpublished or missing target returns 404 `RESOURCE_UNAVAILABLE`;
unknown authority/dependency remains a failure, not an empty success. An age
restriction never accepts unknown verification.

`contracts/generated/mcr-domain.openapi.json` was regenerated using the existing
`backend/scripts/docs/generate-mcr-domain.py`; its policy extension records
version `0.1.3`. The bundle's existing informational version remains generator
owned. No backend API copy, frontend generated file, root router, Gateway parser
or capability configuration is changed by this patch.

Validation (all exit 0):

- `python3 backend/scripts/docs/generate-mcr-domain.py`
- `python3 backend/scripts/docs/generate-mcr-domain.py --check`
- `python3 scripts/mcr/validate-domain.py`: 109 C04 operations, 164 schemas,
  47 fixtures; candidate, runtime not implemented, freeze not accepted.
- `python3 -m unittest discover -s scripts/mcr/tests -p 'test_domain*.py'`:
  101 tests passed. New regressions cover exact additive registration, canonical
  event-bound IDs, rejected nonempty queries, the shared closed public projection
  and rejected private fields. The combined HTTP bundle now has 135 operations.
- Existing operation objects were compared before and after insertion; all were
  identical. C03/C05/C06 schema paths have no diff. `git diff --check` passed.

This offline contract evidence does not certify service authority, canonical
owner RPC, PostgreSQL, public HTTP, client caches or IN03. Intro's producer change
is a separate commit. Root retains central dispatch and final delivery; Party213
retains its currently assigned shared routing hunks. FE owner153 must regenerate
its transport metadata with `scripts/mcr/generate-domain-transport.py` in its own
lane after consuming this operation pin. No cross-lane generated write is granted
or performed here.
