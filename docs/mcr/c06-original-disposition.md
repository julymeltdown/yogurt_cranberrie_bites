# Original C06 interface disposition

Status: **ACCEPTED for the whole original C06 task**. The required interface
definitions and small golden bundle cover every original criterion. This
disposition uses current source, retained qualification and one fresh execution
of the existing six-document golden validator. It does not require new artwork
or a broader runtime test matrix.

The authority is task C06 in `MCR_IMPLEMENTATION_PLAN_V2_EN.md`, section 10:
map/avatar interfaces define atlas frames/anchor, layer IDs, map version,
collisions, interactions and place-source metadata; small golden asset fixtures
must validate and align across all themes. The pinned, machine-readable
[criterion evidence](evidence/c06-original-disposition/criterion-disposition.json)
records the exact canonical source and retained receipts used here.

| Original criterion | Delivered interface and evidence | Outcome |
| --- | --- | --- |
| Frames and fixed anchor | Versioned avatar schemas define 64×64 cells, foot `(32,60)`, direction/frame layout and atlas regions. The existing negative tests reject changed anchors, cell sizes, direction order and frame layouts. The reviewed v2 successor and selected-character owner report retain the current production representation. | Satisfied |
| Layer IDs | Closed config/manifest layer IDs, composition order, item-to-layer matching, unique IDs and required layers are checked by the schemas and `validate_avatar`/`validate_config`. Reviewed v2 preserves its separately versioned composition and registry. | Satisfied |
| Map version and collisions | Map ID/revision/content and geometry hashes, dimensions, collision grid and collision identity are defined. `validate_map` checks shape, walkability and spawn-to-interaction reachability. | Satisfied |
| Interactions and place-source metadata | Canonical place/interaction IDs, source kind/URI/revision/review state and closure semantics are defined. References and reachability are checked. Virtual concepts remain explicitly unreviewed and cannot establish physical arrival or authorize a physical beacon. | Satisfied |
| Small golden assets across every theme | The unchanged six-document golden bundle validates with the current validator and strict format dependencies, exit 0. It contains one avatar/config, one map and all `DANGER`, `PARADE`, `REVENGE` themes. Every theme must share map identity/revision/hash, geometry/collision identity, exact place/interaction IDs and avatar identity/version/hash. | Satisfied |

The retained original format qualification records 27 asset tests, six valid
golden documents and an integrated 144-test contract run. Since that checkpoint,
the validator gained separately reviewed v2 and illustrated-scenery support;
the six original golden files remain byte-identical. That concrete source change
motivated the single fresh golden command below. No new tests were authored.

```sh
python -B scripts/mcr/validate-assets.py
# exit 0; documents_validated=6; themes=DANGER,PARADE,REVENGE
```

The actual command used the retained strict contracts Python environment with
URI/date-time validators installed. The validator reports `CONTRACT_READY`
because it emits a structural bundle result; its historical text about
integrator review is not a user approval requirement or a new acceptance gate.
This document records the responsible developer's original-task disposition.

Current producer/consumer evidence independently supports the interface:
the selected-character original completion report pins the approved reviewed
atlas, registry/compositor and mobile/theme identity receipts; the theme
integration index retains geometry/avatar mismatch rejection; the published
ART06 source pins and native report retain current three-theme geometry and
source identity. Controlled HTTP/C05 browser results remain labeled as such.
AR03's artwork disposition supports the avatar side but is not used by itself
to declare C06 complete.

There is no uncovered original C06 criterion or unassigned implementation leaf.
Current Map density/occlusion work, full visual reference matching, C07 hosted
qualification, actual multiuser services, provider inputs, physical venues,
devices, IN03/IN04 and release acceptance retain their own criteria and status.
Synthetic C06 golden images remain metadata fixtures, never product fallback art.
No source originals, source archives, contracts, schemas, generated types,
dependencies, lockfiles or application code change in this disposition.

Root is the normal publisher. Central task accounting consumes this committed
disposition through one canonical publication receipt; no manager review or
readback-only follow-up is required. The broader integration goal remains active.
