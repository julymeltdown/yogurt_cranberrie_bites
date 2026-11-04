# Original QA01 and C04 disposition

Date: 2026-11-05 (Asia/Seoul). Source reconciliation revision:
`11473eaadce225a59770cb9bb599d5aab8947da7`.

**Original-row disposition: SATISFIED / CONTRACT_READY for the defined offline
contract scope of QA01 and C04, including the selected Wall B definition delta.**
No genuinely missing original contract-definition or fixture-coverage criterion
was found. This is a source-and-evidence reconciliation, not a new application
implementation or a new runtime qualification. No requirement is waived.
The central task ledger is unchanged; production feature and vertical-flow
acceptance remains with the responsible implementation owners.

## Criteria and evidence

| Original criterion | Concrete evidence and disposition |
|---|---|
| QA01: validate every fixture and generated response shape | The published QA01 harness classifies the canonical fixture families and checks catalog completeness, expected positive/negative semantics, generated schemas and every operation/status/media response schema. Current qualification: 198 fixture inputs, 150 operations, 1,727 response cases. The exact C07 fixture/client namespace has its own published exhaustive compatibility/generator/runtime qualification; its explicit separation is tested, not a blanket fixture exclusion. |
| QA01: drift and missing-operation probes | Published QA01 negative CLI probes and the C07 round04 drift, schema-mismatch, missing-operation and output-drift receipts retain the actual expected failures. |
| QA01: broken schema, unknown enum and empty coverage fail | The 17 published QA01 regressions include all 12 negative CLI probes, positive nonempty control, empty catalog/schema/operation rejection, missing dependency/format-plugin failure, duplicate/nonfinite JSON, namespace separation and nested fixture catalog guards. Broken-schema and unknown-enum probes exit 1. |
| QA01: selected Wall B and changed generated outputs | All eight `wall-tools-b/` fixtures, new operations, closed reasons, schemas, policy and generated outputs are in the current 243-input pin set. Every one of those 243 hashes still matches the reconciled canonical revision. Published qualification is reused; no unchanged 1,727-case rerun is presented as new evidence. |
| C04: party, reservation, social, age, feed, mural and admin commands | The canonical register has 117 domain HTTP operations, including 63 commands. The attached inventory records all 63 without sampling. |
| C04: actor, precondition, resulting state, error reason and retry for every command | Each inventory entry includes the exact operation register row, referenced policy preconditions/results and full status/reason matrix. No command lacks any of the required fields. Actor labels resolve in their operation-specific ownership/participant/audience rules plus common guards; no new generic Party audience semantics are inferred. |
| C04: permissions, idempotency and versions | Common canonical-user/event/current-role guards, phase-aware write policies, server-keyed actor/event/operation/key correlation, transaction-bound revision checks and authorized result recovery are retained. All commands require idempotency keys and pending receipts. The inventory resolves nested request revision/version paths, not just top-level expected_revision. |
| C04: public interface correspondence | The generated domain document embeds the exact current policy and preserves actor, rule, retry and write-policy metadata for all 117 operations. Public/backend mirrors and generator reproduction were included in the reused qualification. |

The original criteria are in
[MCR_IMPLEMENTATION_PLAN_V2_EN.md](../../../MCR_IMPLEMENTATION_PLAN_V2_EN.md)
(C04 and QA01). The functional amendment retains the same acceptance sentences
and adds the selected Wall B definition/fixture delta in
[functional-delivery-plan.md](../functional-delivery-plan.md).

## Reused qualification and source fidelity

Canonical qualification delivery:
`83f55f9639273a7994a30d6a591e642f7e4b674f`.
The current pin file is
`scripts/mcr/contracts/qa01-coverage/pins.json`, SHA-256
`147be6549c1f4c1d1307415ff2350b67bc2bdeace47f503050bfdf4da66198c5`.
Its source-candidate field remains historical provenance, not current HEAD.
All 243 pinned inputs match the reconciled revision byte for byte.

- [QA01 positive receipt](../evidence/c07-be38-public-wire/round04/qa01.log)
- [17 QA01 regressions, including 12 failing negative probes](../evidence/c07-be38-public-wire/round04/qa01-regressions.log)
- [C07 drift, negative, inventory, motion, TypeScript and runtime receipts](../evidence/c07-be38-public-wire/round04/checks/receipt.json)
- [Complete source inventory](../evidence/qa01-original-completion/source-inventory.json)

The inventory preserves exact existing source strings and 243 source hashes.
No upstream implementation or rendering is copied. This delivery authors only
this disposition and its bounded evidence directory; it changes no source
archive, application, fixture, schema, policy, generator, pin, dependency or CI file.
The checkout base is `37add6d996eacb7674c1c220150870d7090d5e09`;
read-only later-canonical reconciliation does not imply a checkout intake/rebase.

## Version and transition interpretation

Existing aggregate updates use their declared expected revisions. Party
applications and reservations bind the parent revision; rally binds both self
and meeting revisions. Beacon/meeting requests bind map/place revisions; social
consents bind both handle versions. Closure's CLOSE_EVENT and RESUME_CLOSURE
branches each require the appropriate expected revision. Service-policy updates
bind expected_policy_revision; minimum_client_version is a compatibility value,
not an optimistic-lock revision. The inventory distinguishes these exact names.

Create-only commands do not invent a caller revision for an absent aggregate.
Reaction/block membership, mark-all notifications, upload intent/byte staging
and provider verification use their explicit actor/resource/digest/one-use-state
and idempotency rules. Their lack of a generic expected_revision property does
not authorize stale aggregate updates or remove transaction-bound common guards.
The 10 aggregate transition maps, including terminal states, are preserved in the
inventory. Existing semantic tests cover invalid terminal transitions, stock and
capacity equations, age selection/boundaries, reciprocal sharing and revocation,
receipt correlation, mural reset/revision rules and privacy projections. These
are offline contracts and examples; they do not execute producer transactions.

The historical [domain candidate](../domain-contract.md) describes 106 operations
and an earlier fixture/schema inventory. Its historical counts and now-obsolete
independent-review gates are not current requirements. This disposition supplies
the current original-row mapping without rewriting that source history or
introducing a King/SL approval stage. Validator vocabulary hardening was considered;
absence of such a new test is not a missing original definition criterion.

## Limits and completion boundary

Schema witnesses do not claim exhaustive business-state/value/union combinations.
Actor and transition contracts do not prove actual database authorization,
concurrency, policy revocation, receipt persistence, live services, devices or
multiuser flows. No such run was performed for this documentation delivery.
Existing producer-specific failures and remaining feature work remain open.
The BE36 developer's newer-state report replay change is not certified by this
contract disposition; its native/HTTP validation belongs to that owner.

No central status/count change, scope cut, product redesign or new test matrix is
included. The preserved goal still reports historical `usageLimited` metadata;
explicit human resumption authorized this aligned continuation without resetting
or falsely claiming ACTIVE/complete. Publication uses the standing publisher's
single canonical receipt; no additional review/readback commit is required.
