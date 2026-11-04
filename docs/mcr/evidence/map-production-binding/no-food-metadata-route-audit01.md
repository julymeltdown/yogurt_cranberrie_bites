# Legacy FOOD metadata: actual product-consumer reachability

Disposition: **the specific legacy FOOD metadata row has no product label-rendering or navigation path in the audited source**. No change to `metadata.ts` or its focused historical test is justified by this finding. Those transferred paths remain unchanged; original geometry, places, source originals, authority and all labels are preserved. This is a bounded static route/consumer audit, not a new runtime/native acceptance claim or an ART05 gate.

## Source path and control flow

`frontend/src/world/reference-scene/metadata.ts:20` retains `{label:'FOOD',x:658,y:937,interactionId:null}`. The legacy identity is map ID `mcr_reference_scene_r3`, revision2, PARADE,768x1296 and geometry `20168a1d...`; `getReferenceOverviewMetadata` can return that metadata object for its matching geometry/dimensions/theme. Thus it is inaccurate to call the whole export unreachable or deleted. The currently retained Map4 revision4/geometry `899aca9b...` does not select that legacy branch.

The actual production consumer search (`rg -n 'getReferenceOverviewMetadata|\.mapLabels' frontend/src --glob '*.{ts,tsx}' --glob '!**/__tests__/**'`) finds only `IllustratedMap.tsx`: import line7, call line58, and sole label-array read line76. That read is `reference?.mapLabels.find(p => p.interactionId !== null && p.interactionId === interaction?.interaction_id)`. The FOOD row has null interactionId and cannot pass this predicate, even if an old matching release makes the legacy metadata object available. The selected row supplies only marker X/Y; its `label` string is never rendered. The returned metadata object is not serialized into product UI.

The remaining DOM markers/links iterate the current authorized place catalog and canonical interactions, and obtain visible/accessible text from `place.label`; they do not map/render `reference.mapLabels`. Full-composition registered labels have their separate explicit semantics. `world/reference-scene/overview.ts` paints ground, plaza and props, then the separate qualified paint/v4/v5 label drawers. It does not read legacy `mapLabels` or draw their FOOD text. A null-interaction legacy metadata row cannot synthesize a feature route or place authority.

`__tests__/metadata.test.ts:7` asserts equality with the historical source proposal and line11 counts four null-interaction rows. These assertions preserve the original metadata projection, not a product FOOD label/feature requirement. Because the row has no rendering/navigation consumer, the conditional requested patch is not applied and no historical test assertion is rewritten.

## Current-source verification boundary

Audited owner source checkpoint `7d5ad0c8d768681f3603b0eb22f3ddc389ad74e9`; Root checkout observed HEAD `c34c5cf7f640c66b219cff186d0dc3e48e5dac6f`. All seven exact files below are byte-equal in the two checkouts, including the actual metadata/UI consumer/rendering/model boundaries. Root's full production-tree search independently has the same sole consumer. Its resolver has unrelated current changes, so this report does not claim whole resolver/tree equality; current Root still calls `preflightReferenceScene`. No obsolete owner checkout is substituted for current production consumers.

| File | SHA256 |
| --- | --- |
| `frontend/src/world/reference-scene/metadata.ts` | `e5eb4619b49d437960e281a8f51af7a089fb826fdc7a9f4f30427246af892d26` |
| `frontend/src/world/reference-scene/__tests__/metadata.test.ts` | `c08a28dd007de9a0f1d35f903200b06452e46476e5dbdd3e23f3805f832a0a03` |
| `frontend/src/features/places/ui/IllustratedMap.tsx` | `a655570fcaef239ee8ec72b876a881c2b76b03815b0a7fa08125588b7f076112` |
| `frontend/src/world/reference-scene/overview.ts` | `1c64590a29a57071026f33d0cdd602cb601e4662d25e7476596ab6967beb3474` |
| `frontend/src/app/world-entry/reference-scene-overview.ts` | `5586a2e9b149b89460441db1e421a1e5ca6c09d30716ec5482916cff57c1b937` |
| `frontend/src/world/reference-scene/model.ts` | `139f92df0ae1ae1abf7fbb9822beee82faa4fdb4c51ee873a3cdbb7d267a715a` |
| `frontend/src/world/reference-scene/spatial-v3/model.ts` | `d4c1c74ca1f92777812af90cdc5a4dcc5bc8411ac6626ebee1fcb95ada90783f` |

Read-only searches and byte/hash readback completed; no source edits, application tests, browser, captures or source/runtime allocation were performed for this audit. No new assertion of default/live/operator availability is made. In particular, this narrow result does not certify arbitrary server catalog labels, baked text in historical images, or other FOOD strings elsewhere. Those are separate product/content paths and cannot be inferred from this metadata row's presence or absence.

The sole writable transfer for `metadata.ts` and `__tests__/metadata.test.ts` was unused and is returned unchanged to Root with this committed documentation handoff. No broader source ownership is acquired. ART05 atomic source intake/its separate resource queue and mural placement122ec874 remain unchanged. No King/SL review or acknowledgment is required.
