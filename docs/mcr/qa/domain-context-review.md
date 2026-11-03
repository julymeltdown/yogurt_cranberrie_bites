# C04 party/gift consumer context review

Status: **Scoped context/receipt qualification PASS; C04_REVIEW_CANDIDATE.** No whole-contract freeze, feature acceptance or server/runtime qualification.
Date: 2026-11-03. Base `5e829d0d681c047d9414042dc5c570f2bfad0a9f`; isolated branch `codex/mcr-fe-C04-context`.
Reviewed candidate `8c9e1a53fe7393e503d93fd2e1602c71da8b4fab`, applying parent `ca9f955cd22f7e4c1da029d034c1923f1e98bb9d`.
Worker changes only `scripts/mcr/tests/test_domain_consumers.py` and this report. Validator, policy, schemas, generated artifacts and existing test updates are parent-owned inputs.

## Independent coverage and result

Thirty new focused unittest methods pass against the actual candidate. Valid controls accompany adversarial context/identity cases; existing fixture profiles, party, giveaway, reservation and receipt are reused rather than a second catalog.

| Concern | Evidence in the assigned test |
|---|---|
| Six list/detail/own-activity reads | Valid payloads preserved; missing/noncanonical actor/event/path rejected; actual foreign/encoded/wrong paths and malformed/unknown/duplicate queries rejected even with empty pages |
| Detail and nested identity | Requested party/giveaway ID, every available row event, own owner/applicant/member/recipient identity checked against captured context |
| Independent collections | Duplicate entity IDs rejected within each collection; two memberships for the same actor in different parties accepted; per-collection requested page bound enforced |
| Public versus private projections | Public gift DRAFT and injected peer lists/private instructions rejected; own giving DRAFT and public paused/terminal gifts remain readable |
| Pagination | All independent activity cursors and empty pages with continuation remain intact; no mutation of caller payload |
| Member wire change | Required canonical `event_id` tested both in source schema and the actual browser artifact; omission rejected in each |
| Receipt lookup | Captured actor/event/path, exact query operation/key, expected nonnull final target and closed query validated; foreign/absent context and mismatched bindings rejected |
| Receipt lifecycle | All four documented states accepted with matching context; direct 202 requires captured canonical key and PENDING; SUCCEEDED must use registered final status; FAILED reason must belong to the command; RETIRED cannot carry result/failure data |
| Unknown outcome | 404 validates only as an error projection; it is not a successful or invented UNKNOWN receipt. Replay/retention behavior is a reviewed policy obligation, not execution proven by this test |

## Rule review and consumer impact

No actionable finding remains in the proposed bounded predicates. `validate_http` now accepts explicit `request_path`, `request_query`, `idempotency_key` and `expected_resource_id`; consumers must pass captured values, not infer them from a returned DTO. Error-envelope validation remains distinct from qualifying a successful resource projection.
The required `party.Member.event_id` fixes otherwise uncheckable membership event binding. The source schema and generated browser component agree. Source search found no populated existing Member fixture or application decoder using the old shape; current frontend references are generated transport metadata, not a Member wire consumer. Existing receipt-oracle callers needed the new context arguments and were updated by the parent outside this worker's ownership.
The new `policy.json.idempotency.status_recovery` deliberately conflates never-recorded/inaccessible/unavailable receipts as 404. That outcome never authorizes replay, a new key or presumed success. RETIRED/expiry similarly never enables reexecution; even SUCCEEDED requires a current authorized activity/resource reread before presenting current domain state. No generic retry policy is enabled.
The `consumer_projection` rules retain server authorization of linked resources, policy/session fencing and all seven C03 decisions. NORMAL apply needs meet/mutation availability; SAFETY cancellation remains possible in READ_ONLY despite `mutations=false/EVENT_READ_ONLY`. Other denied/unavailable/unknown eligibility still fails closed. A missing row on one page never grants action eligibility.
This closes the prior R4/R5 contract-context and recovery-rule ambiguities within these selected operations. It does not establish actual producer authorization, cursor authenticity, database relationships, retention-clock enforcement or replay prevention. UUIDs alone do not establish result kind; the command's response type plus the subsequent authorized reread must preserve it.

## Exact inputs and commands

| Candidate input | SHA-256 |
|---|---|
| `scripts/mcr/validate-domain.py` | `212e205966d594599091a37263eac864de5c1aebdc1163c72cbb84220a7c0049` |
| `contracts/domain/v1/operations.json` | `42ed48ee0cad846b94f9f7a5d39a38beeeb5f5b41e0129f157e51ff31c635223` |
| `contracts/domain/v1/policy.json` | `508d22af964b718cdcbd5a25b32b9a1e3f7b651a5b8c25c87d56f1f6b68d0ebe` |
| `contracts/domain/v1/party.schema.json` | `7cbea6af623668a36d641e7498bf52f4869b9bc44594b46e7f4a3806cc907d03` |
| `contracts/generated/mcr-domain.openapi.json` | `c47e767505e660d8f5eb69bb9759eae1f0aaee6472e4fb963edc82e060385f51` |

`python3 -B -m unittest discover -s /Users/lhs/Desktop/mcr-worktrees/fe-domain-context/scripts/mcr/tests -p test_domain_consumers.py -v`: **exit 0, 30 methods**, recorded in `/tmp/mcr-c04-context/candidate-final.log`. The first candidate run passed all initial 26 methods; four additional checks qualified page bounds, artifact agreement, terminal readability and failure reasons before the final run.
The initial base run was honestly red at the API boundary: 26 methods, 105 errors because the old signature lacked `request_path`. A separate private legacy-signature diagnostic discarded only unsupported arguments to characterize old semantics: 64 assertion failures, seven successful methods and three schema errors from the intentionally new membership field. This diagnostic was not represented as a qualified API run. No baseline characterization continued after the parent candidate arrived.
Private `/tmp/mcr-c04-context/manifest.json` SHA-256: `cca8d2386ecefa6a01148b4977c7aa71a6ece801d7784a312e68a32df583d7e0`. It binds candidate/input/fixture/test hashes and preserves both baseline diagnostics and passing candidate logs. Final committed test bytes match its tested source pin; the handoff separately records the commit/report hash.
`git diff --check` and exact two-path staged review: exit 0. Handoff descends directly from the candidate above; no push. Goal remains ACTIVE through parent integration/review fixes.
No existing 63-test domain/bundle suite, closed R1–R3 probes, frontend build/browser, Cargo/PostgreSQL or live service/device/load gate was repeated. Those are not replaced by these focused contract checks. The parent may make a bounded subset-freeze decision; this report advances no whole-C04/task/IA acceptance count.
