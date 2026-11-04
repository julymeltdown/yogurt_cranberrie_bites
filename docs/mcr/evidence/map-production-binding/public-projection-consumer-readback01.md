# QA05 public projection: Map consumer readback

Disposition: READBACK_COMPLETE, PUBLIC_DESCRIPTOR_CORROBORATION_ONLY. No projection copy or custody transfer is requested. The Map consumer releases its retention dependency on the producer's temporary file; any remaining retention obligations belong to the producer and other consumers. This is a custody disposition, not technical approval or an acknowledgment gate.

The implementation owner read and hashed the existing file in place, without a runtime, browser, native capture, install, build or source-test job:

- Producer path: `/private/tmp/mcr-qa05-pair80-runtime51-hn2eqqbh/public-resource-capability-projection.json`.
- Actual size: 22,068 bytes; SHA-256 `c1606256a0a061aff988ff9dd14e3d11534bdbc5e8875b123dbf196360520e3f`.
- Actual file/directory modes: 0600 / 0700.
- `kind=mcr-public-resource-capability-projection`, `contract_version=0.1.0`.
- `fidelity=NOT_BYTE_EXACT_FULL_C03`, `resources_are_complete_manifests=false`.

Exactly eight top-level keys are present: `avatar_descriptor`, `capabilities`, `contract_version`, `fidelity`, `kind`, `map_descriptor`, `resources_are_complete_manifests`, `theme_descriptors`. A recursive key/string read found no matching participant, identity, age, preference, credential, header, cookie or UUID-valued fields. That structural observation does not substitute for a general privacy audit. No authenticated full C03 body or full-body digest was received, copied, produced or retained by this consumer; the recorded hash belongs only to the explicitly public projection bytes.

## Actual public resource comparisons

The owner compared the projection against the retained original Map4 manifest inputs in the existing read-only v1 source fixture. Map ID/revision/content/geometry/tilemap descriptors match; all three theme identity/content/map/geometry/avatar pins match; selected-avatar identity/version/content/representation match. This reads existing descriptors only and does not import, reconstruct or validate a complete C03.

| Public field | Observed value |
| --- | --- |
| World capability | `available=true`, `reason=null` |
| Other six feature capabilities | `available=false`, `reason=FEATURE_DISABLED` |
| Map | `mcr_reference_scene_r3`, revision 4 |
| Map content | `sha256:736afd273a067f6c40db76b8cc54412416fba88984a50d9a6b6738d869831fb8` |
| Geometry | `sha256:899aca9bd6ce499c6f46aff5eaf21e30ab34b301e988771b978a6fab7c2d469f` |
| Tilemap | `world/reference-scene-v2/runtime/spatial-v3/canonical-map-v4.json`, `sha256:60364d5f35bcc8ff82931b7e3a0724a4580880d4a481610e90475d73dcb6ceb9` |
| Avatar | `mcr_selected_static_20261103`, version 1, `whole_character` |
| Avatar content | `sha256:0627c7d178b22535a561e1270db6d8bf9e2383ba84d03a63962e48e565c59b2b` |
| REVENGE / PARADE / DANGER | All revision 1; exact retained Map4 theme hashes and linked Map/avatar pins match |

This corroborates that the producer observed positive World capability with the same public resource cohort. It does not amend the historical pre-admission operator fixture, which still has World disabled, or relabel the published eligible-attendee fixture as an actual authenticated frontend run.

## Producer receipt and provenance limits

The adjacent existing `lease-release.json` was also read and hashed: 1,870 bytes, SHA-256 `fda512681d226bbd47be1d1b8c90602168b2591d0bf9f1f01447f2edaab60960`. It reports allocation `QA05-PAIR80-RUNTIME51-202611041226Z`, state `FULL_RELEASED`, time `2026-11-04T12:37:23.681191Z`, the same projection hash/size and `raw_c03_retained_or_hashed=false`. These are receipt observations; this consumer did not rerun or independently inspect backend process/database cleanup.

The producer's handoff attributes the run to source `8da06969acfe46409e929dc9812bef00f8338ca0` and reports a completed real-pair probe with PARTIAL disposition, exit 0, retained earlier failures, restart/reconnect observations and released input/runtime holds. Those backend execution conclusions remain producer-owned evidence. Frontend runtime is explicitly NOT_RUN and no consumer revision was supplied for that pair.

No field named `contract_source_pins` or `source_provenance` exists anywhere in the supplied eight-key projection. The producer clarified that these reside in the already-existing sanitized sibling `report3.json`, not in the projection itself. The owner then directly read both JSON pointers in place:

- `/post_admission_projection/contract_source_pins`: 13 named contract-source SHA-256 pins.
- `/post_admission_projection/source_provenance`: producer revision `8da06969acfe46409e929dc9812bef00f8338ca0`, six backend peer source/binary pins, cohort-manifest and Concert pins, plus explicit frontend `NOT_RUN` / consumer revision `NOT_SUPPLIED`.

The existing report is 43,203 bytes, SHA-256 `88bd0fa2f02e6f29a9d4e38620a1743bad1aaefa1564083beb41d35f5ac1f0b2`. This is a sanitized producer-report hash, not a C03 body hash. Its same `/post_admission_projection` node binds the exact 22,068-byte public projection/hash, fidelity and contract version `0.1.0`; it records HTTP 200 at `2026-11-04T12:36:47.589055Z`, unmodified capabilities, RAM-only contract/asset validation PASS, and no full-body retention/hash or private identity/credential fields. These are directly read producer receipt statements, not independently rerun backend validation.

The report records Concert compiled source `7cd1e900d6bd1b4994482819486c828e43736822`, binary hash `b1ea0a07eab66aa2c1c313476b063b61cf54ee678ad0a0e44633944815f65433`; Gateway/Auth/Friend/Notification compiled source `34fa7a6d60f6e771f63db54f4a91e8fd1ebf2f8c`; Post/Profile compiled source `d0f063d18a0549935f80a536077c30962d89f9e9`. This therefore does not silently qualify a newer backend or reviewed-motion cohort.

The projection was rehashed after reading the sibling report and remains byte-identical to the original handoff. No report/projection blob was copied, regenerated or adapted into application input; no new job or retention dependency was created. The producer's provenance-location clarification resolves the wording distinction while preserving all original fidelity and acceptance limits. Producer custody continues under its own remaining obligations, and the producer reports Root153 has a separate exact copy with charged custody; this Map consumer holds neither copy nor deletion authority over those artifacts.

## Binding consequence

Do not adapt this incomplete projection into a complete authenticated C03 fixture, synthesize missing private identity/state, alter admission guards, replace the frozen v1 fixture or activate v2. It cannot establish default frontend admission, a real Map browser flow, live linked-feature operations, visual convergence, QA08, IN03 or complete QA05 acceptance. No new Map/native pair is requested.

The source-qualified v2 consumer remains inactive and awaits corrected final artwork/layout pins and its existing required validation/publication path. Old v1 writer leases remain returned. Original full reference-convergence scope and goal remain open; no allocation or temporary artifact custody is held by this Map consumer.
