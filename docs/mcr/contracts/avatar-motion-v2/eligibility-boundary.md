# Candidate validation and an eligible successor

This document answers the canonical C06 owner's integration question. It does not
change the candidate or allocate retained public contracts to a second writer.

## Exact current boundary

The Rust implementation is in source commit
`83b23afd` at `backend/crates/concert-domain/src/avatar_motion_v2/mod.rs`;
subsequent producer commits add raster checks and handoff evidence. Cargo
execution is still NOT_RUN pending a named existing-target allocation.

| API or evidence | Establishes | Does not establish |
|---|---|---|
| `decode_config` | Closed 2.0.0 representation/header/slot IDs, required nullable keys, raw duplicate/unknown-field refusal, bounded input | Identity, authorization, persistence, artwork review, public acceptance |
| `Catalog::decode_candidate` | Typed manifest plus JSON-value equality to the exact immutable candidate; all pins, directions, frames, palette, cuffs, source attribution and denial flag remain fixed | A reviewed release, canonical public release authority or response hash binding |
| `Catalog::verify_resource` | Allowlisted local path, exact byte length/SHA256 and PNG header dimensions | Visual identity, gait quality, rights review or actual public asset delivery |
| `Catalog::manifest_sha256` | SHA256 of embedded canonical `MANIFEST_JSON` bytes | A signature, approval, authorization, hash of arbitrary input formatting or new `content_hash` field |
| Independent producer Python checks | Actual PNG decode, mappings, inventory, distinct animated RGBA cells, rear-eye transparency, source/original hash parity | Human visual judgment or positive public service journey |
| Motion83 `df0522fa` | Independent canonical fixture consumer and rendering parity, all shared configs, same frozen hash, default denial | Real multiuser HTTP/WS or admitted release; its fixture has zero live sockets |
| `Catalog::admit_production` | Always returns `ArtNotReviewed` | No positive production admission is possible in this release |

`Catalog::decode_candidate` compares JSON values after typed parsing. Reordered
object keys or whitespace need not fail. A caller that requires artifact-byte
identity must independently verify the external canonical file SHA256:
`9a555f726a9e421f14ed876117be87bb1bb8fa128ac401ab38f8927760776fd6`.
This candidate has **no `content_hash` field**. Adding one fails the immutable
manifest schema and Rust catalog comparison. Neither the canonical file hash nor
matching resource pins can be used as an eligibility receipt.

No positive eligible derivative is representable by the current release-1 decoder:
changing `production_eligible`, qualification, a hash field, source record or frame
mapping is rejected. A reviewed successor must be explicit and separately pinned,
with a new manifest identity/revision and corresponding schema/typed catalog
policy. Root owns any retained C03/C05/public type, parser/persistence, bootstrap,
HTTP/profile/save/realtime, registry or release-admission adaptation. The current
1.0.0/1.1.0 behavior remains frozen. The real map-4/two-client path is independent.

## Existing source authority; no new method-approval gate

The selected source authority is already registered: pack
`mcr_user_characters_20261103`, revision 3, all 20 original hashes and bytes verified.
The ten-artifact handoff and authored inputs are pinned in the candidate. The
originals, source descriptors and source provenance have not been modified.

`docs/mcr/art/evidence/motion-candidates-v1/authored-reconstruction.json` explicitly
records: "No additional method approval remains pending." It records scoped
reconstruction authorization, not visual product admission. A successor does not
need newly invented art, a redraw, another method authorization, or a main/SL
acknowledgment simply to review these already authored candidate pixels.

Supplied front pixels and authored side/rear/component pixels must keep their
existing distinct attribution. The six preset source identities are photo_01;
body palettes are photo_02; hair references photo_01/photo_04/photo_05; outfits
photo_01/photo_06; eyes photo_09; accessories photo_19. Contact-sheet rows are source
location guides and do not become independent sprite sheets or 314 canonical IDs.

## Concrete remaining inputs for an eligible successor

| Input | Evidence available now | Required qualification or integration output |
|---|---|---|
| Source-to-derived identity | Exact selected originals, 6 preset source frames/attribution, authored reconstruction and component source pins | Art/QA owner records visual comparison of each of the 6 identities in all 4 views to selected photo_01, plus the 28 layered components against their declared sources. Preserve authored status; record accepted differences or concrete corrections. |
| Motion and geometry | 64px cells, foot (32,60), 20 outputs, actual 5 distinct RGBA cells per animated direction, source descriptor mappings | Art/QA owner verifies readable idle and 4 walk phases per direction, facing, stable identity, no clipping/foot drift/jitter, clear silhouettes and acceptable head/body proportions at the intended 56–72 CSS px size. Distinct pixels alone do not prove acceptable gait. |
| Layer and skin semantics | Exact 8 palettes, body semantic mask policy, hand masks, dressed y<42 clip, explicit rear-eye invisibility, deterministic composition | Art/QA owner checks all body/outfit pairs over every direction/phase, optional outfit/accessory cases, and hair/eyes/accessory pairwise coverage for seams, exposed cuffs, incorrect recoloring, clipping or hidden facial details. Existing 1176-case consumer RGBA equality proves parity, not original-source visual acceptance. |
| Theme/mobile readability | Independent fixture screenshots and native behavior at 360/390/430 in Revenge/Parade/Danger | Record actual visual review of those same pinned outputs for contrast, recognizability and pixel sampling. Bind each reviewed screenshot/contact sheet to candidate source/atlas hashes and consumer source commit; record reviewer, commands, outcomes, limitations and corrections. |
| Reproducible review receipt | Current candidate hashes and source histories; no positive review receipt in this producer | A new evidence receipt names the actual reviewed artifact hashes, coverage and decisions. A boolean or renamed candidate is insufficient. Corrections, if needed, get separate derivative files and pins; supplied originals remain immutable. No main/SL review gate is implied. |
| Eligible release and public binding | Candidate-only manifest/config shape and canonical file hash; no `content_hash` or admitted release here | Root explicitly defines successor identity/version and content-hash preimage/algorithm, binds source resources and actual review evidence, and qualifies the generated C03/C05 union plus bootstrap/theme/world manifest-hash relationship. Do not hash a document including its own hash without a defined exclusion/preimage rule, reuse a static 1.1 hash or waive an exact generated-union guard. |
| Actual product acceptance | TEST_ONLY fixture save/world/profile, production default-deny tests | Root and consumer owners validate authorized HTTP save/load/profile/realtime round trips with revision conflict handling and the same admitted release, then actual two-account/live service/device/release gates as required. These are not producer-owned work or preconditions for the independent map-4 journey. |

Art/QA review is a concrete technical assignment with an evidence output, not a
request for management approval. This producer supplies source mapping and pinned
fixtures; it does not claim to have performed the missing visual qualification.
