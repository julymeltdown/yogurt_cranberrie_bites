# AR03 selected-character motion and composition

Original AR03 status: **IN_PROGRESS**, not ACCEPTED. This delivery implements and
qualifies the assigned candidate artwork: six complete presets, all four genuine
directions with idle/four distinct walk phases, and an independently composable
wardrobe with eight skin/body choices, six hairstyles, four eyes, six outfits and
four accessories. Product/contract adoption is outside this art assignment.
No original requirement is removed or inferred accepted from a fixture.

Assignment base: `a3526254a716e14e3134de551c3b277379a1096f`.
Branch: `codex/mcr-art-AR03-motion`. The four exclusive paths are this document,
`docs/mcr/art/evidence/motion-candidates-v1/`,
`frontend/src/entities/avatar/derived-assets/motion-candidates-v1/`, and
`frontend/public/world/avatars/derived-v1/motion-candidates-v1/`.
Original sources, prior static releases, catalogs, renderer, contracts and
backend are unchanged by this delivery. Publication/ancestry is recorded in
`evidence/motion-candidates-v1/delivery.json`.

## Source authority and authorship

The authority is selected pack `mcr_user_characters_20261103`, revision 3. All
20 selected JPEGs and the preserved unselected `photo_03` duplicate retain their
catalog SHA-256 and byte lengths. `docs/source/` remains byte-identical.
The source sheets were visually inspected, including the six complete designs,
bodies, front/rear hair, outfits, bottoms, eyes, eyewear, headwear and props.

There are two explicitly different provenance classes:

- **Supplied-pixel derivatives:** masked native cutouts, nearest-source-pixel
  registration and articulation of the six supplied front figures. Every retained
  native RGB and every front-walk pixel can be traced to its actual original.
- **Authored faithful reconstruction:** missing orthographic views, clean bodies,
  hair, garments and accessories reconstructed from selected references with the
  built-in imagegen tool. These pixels are never described as supplied views.
  Original files are not overwritten. Five selected generated source PNGs are
  saved in this project's candidate directory, with exact prompts, references,
  output hashes and methods in the evidence directory.

The later human-scope authorization explicitly permits faithful reconstruction
and supersedes the initial restrictive no-redraw assignment wording. No further
method approval or finished-frame upload is needed. The source accounting
120 required / six supplied / 114 not supplied never constituted a request for
114 user-provided images. The implementation authors the missing views/poses.

The six identities retain the black side fringe/red tie, pale choppy hair/parade
jacket, red hair/goggles/lightning jacket, black bob/striped top, brown tousled
hair/dark closures, and black curls/teal-yellow jacket. Reconstructed rear jackets
use conservative existing-color continuations, the striped top continues its
stripes, and the goggles have an explicit rear strap. Profiles actually face
left/right with one visible eye; rears have no frontal face or pasted front
closures. These are new faithful surfaces, not claims about unphotographed facts.

## Runtime-ready candidate resources

The primary entry point is **`combined-release-candidate.json`**. It references
one **1280×1856, 470,010-byte PNG**, with 576 physical 64×64 cells. Both complete
presets and wardrobe composition use this same texture; switching between them
requires no additional avatar request. The runtime descriptor is 42,925 bytes;
the two resources total 512,935 bytes before transport compression. It remains `productionEligible: false`.
All identifiers in these files are local artwork labels, never inventory IDs or
accepted saved `AvatarConfig` values.

| Resource | Actual content |
|---|---|
| Combined `presets` | Six identities × front/left/right/rear × idle/four walks = 120 full frames |
| Combined `wardrobe` | Eight bodies/skin palettes, six hair, four eyes, six outfits, four accessories |
| Wardrobe cells | 456 cells including articulated body/garment/cuff masks and shared stationary head UVs |
| `candidate-index.json` | 172 source cutouts, 172 registered reference parts, one source-part atlas |
| `front-garments.json` | Six original-pixel garment masks with supplied hands cut out; retained as extraction evidence |
| Earlier per-family candidate files | Reproducible intermediate atlases/provenance; not extra default runtime fetches |

Native references cover eight bodies (`photo_02`), 16 front hair (`photo_04`),
16 actual rear hair (`photo_05`), 16 outfits (`photo_06`), 16 bottoms (`photo_07`),
18 eyewear (`photo_08`), 18 eye pairs (`photo_09`), 16 eye/brow groups (`photo_17`),
16 headwear (`photo_19`), 16 wearables (`photo_20`) and 16 props (`photo_21`).
Their reference-only registration does not silently make all 172 items compatible
wardrobe choices. Native outfits/bottoms retain baked parts unless explicitly
masked; generic cutouts are labeled unfitted. No prop grants entitlement behavior.

## Pixels, motion and composition

Source cutouts flood only perimeter-connected neutral paper: minimum RGB 118,
channel spread at most 18. Remaining components need a dark-outline pixel with
maximum RGB at most 112, retaining disconnected eye/accessory groups. No blanket
white deletion occurs. Pale hair, collars, eyes, lenses and ornaments survive.
Crop-border foreground fails generation. Contact-sheet hints are not blindly
used as rectangles: actual paper/alpha gutters are measured, including staggered
prop rows. Authored sheets use explicit alpha threshold 128, then nearest
sampling; retained RGB is preserved until the declared semantic skin operation.

The extraction approach adapts existing `derived-assets/mask-presets.mjs`, source
commit `26a8eb61afc1bfc5310c1628543691b4e725e840`, blob
`2f8d61ed082c4460db9b7242af8ca5ac2780fa5a`. Read-only hair windows come from
`derived-assets/hair-layout.json`, source commit
`e7fe93b4c3675374e2471bf87447e66fdf32d0dc`. Exact input hashes/blobs and dependencies
are in `source-pins.json`. All new rendering/tooling is authored in this patch;
no legacy renderer or upstream visual library is imported.

Every full frame uses the 64×64 cell and fixed foot `(32,60)`, with clear padding.
Complete preset silhouettes are 56px high. Articulated shoulder/hip partitions
move arms against alternating planted/lifted feet under a small stationary joint
overlap. Profiles have actual projected bodies and lateral leg swings. Heads
stay stationary. No whole-sprite bob, stretched/mirrored front, or repeated full
idle masquerades as a walk. Small front gaits are deliberately subtle at 64px.
Original-front and authored-direction binary pixel maps remain separately
attributed and independently verified.

Wardrobe bodies are clean authored bodies with no baked eyes/hair. Eight skin
palettes use actual selected `photo_02` skin RGB quantiles; only a semantic skin
mask changes. Black outlines/base clothes remain identical across skin choices.
The four eyes use supplied `photo_09` cutouts: dark, brown, red and turquoise;
profiles show a single selected eye. Rear eyes are explicitly non-visible.

Clothes contain no baked hand color. Reviewed cuff-sized white components are
removed only at cuffs; collars, braid, white cloth and stars remain. Cuff masks
render the selected body's skin behind the garment and respect cloth occlusion.
When dressed, the body's base torso is clipped below y=42; the garment supplies
the torso/legs and its own animated cuffs. This prevents double hands, exposed
base underwear, accidental whole-image tint and incorrect source-baked skin.

The explicit draw order is body/head or complete base, cuff skin, garment, eyes,
hair, accessory. Hair/eyes/accessories share stationary head UVs across phases;
this is appropriate attachment reuse, not missing whole-body motion. Generated
hair has actual transparent face openings; intentional blond panels remain.
The four accessory choices are red bow, black bow, cat-ear headband and red/black
star headphones from `photo_19`, with genuinely reconstructed side/rear geometry.
Local view indices do not assert an automatic front/rear pairing in the original
source sheets.

## Validation and reproduction

Use Node 24.22.2, the retained installed Playwright package and Chrome
156.0.8078.80. Set `MCR_ART_PLAYWRIGHT` to the installed
`@playwright/test/index.mjs` absolute path. No installation, lockfile change or
application dependency mutation is required. Each generator/verifier owns and
closes its loopback-only allowlisted server/browser. Generators read immutable
selected/generated inputs; they do not repeat image generation on reproduction.
JPEG decode/PNG encoding reproducibility is qualified to this environment.

Run these generator stems, in order, with `node`, under
`frontend/src/entities/avatar/derived-assets/motion-candidates-v1/`:
`produce`, `produce-rig`, `produce-attachments`, `produce-eyes`, `produce-garments`,
`produce-directions`, `produce-components`, `produce-combined` (all `.mjs`).
The six verifier stems are `verify`, `verify-gait`, `verify-eyes`,
`verify-garments`, `verify-directions`, and `verify-components`.
The optional `qualify.py` wrapper runs all eight generators, compares every
public byte, runs all six verifiers and records the checks (set `MCR_ART_NODE`
when Node 24 is not on PATH).
`preview.mjs` serves the fixtures for manual inspection; it does not start or
change the product. Exact command exits and repeated-generation byte identity
are recorded in `checks.json`.

[Preset evidence](evidence/motion-candidates-v1/direction-validation.json)
verifies all 120 source-attributed frames, binary alpha, padding/foot anchors,
five distinct phases for every preset/direction, and stationary heads. Original
front RGB maps to `photo_01`; reconstructed views map to the authored source,
not to original JPEG pixels.

[Wardrobe evidence](evidence/motion-candidates-v1/wardrobe-validation.json)
verifies physical cells, five distinct body/outfit phases, palette-only skin
changes, identical black outlines/base clothes, proper cuff alpha, stationary
attachment UV reuse and **1,176 pairwise compositions** covering skin/outfit/gait,
hair/accessory and eye/hair interactions. This is deliberate pairwise coverage,
not an assertion that every possible wardrobe combination was visually reviewed.
Source cutout, original garment mask and eye-occlusion reports remain separate.

Both primary fixtures exercise real requestAnimationFrame loops and identical
RGBA across World/Profile/Feed/Gift/Party at 56/64/72 CSS pixels, widths
360/390/430, and REVENGE/PARADE/DANGER. They have no page errors or horizontal
overflow. Each loads one avatar descriptor plus one shared texture, then makes
zero avatar requests during selections, directions, theme changes, animation or
preset/wardrobe switching. Debug pixel-proof requests are separately counted.

Developer visual inspection covered original references, all six preset
turnarounds, all 24 coordinated wardrobe look/views, dark skin with blond hair,
cat ears, and native/mobile front/profile/rear captures. A selector overflow,
incorrect light-probe label, grid-edge hair clipping, tiny bright closure/cuff
ambiguity and joint cuff occlusion were found and fixed without weakening source
RGB, alpha, anchor or distinct-motion invariants. Source JPEG fringes remain in
exact-source derivatives rather than being falsely called repainted originals.

## Disposition and handoff boundary

All assigned candidate art categories and direction/pose families are present
and locally qualified. No external design input or method permission remains.
The assigned goal is completed only after reproduction, committed publication,
verified remote ancestry and the developer handoff are finished.

Original AR03 remains IN_PROGRESS until the responsible consumer integrates and
qualifies the assets through the versioned public composition contract. Current
C06 synthetic item IDs are not these artwork labels. Adoption must express the
atlas/UV maps, body skin palettes, cuff masks/occlusion, stationary attachments
and rear-eye invisibility, map to authorized inventory IDs, and validate the
actual renderer/editor and vertical flows. These files do not grant a registry,
root, renderer or backend ownership transfer or silently amend C06.

The evidence is actual art/Canvas/browser qualification, not Phaser, a physical
phone, application build, backend authorization, multiuser or release acceptance.
Those boundaries are not source-production blockers or a new king/SL review
queue. Planning checks cannot substitute for them. This is a complete assigned
candidate-art handoff, not automatic product admission or an original-scope cut.


## Closed art receipt and proposed application adoption

The assigned art goal was read back COMPLETE after publication; the current
goal service returns null. No new goal was created. The checkout was clean
before this receipt, with no owned preview/verifier server process or temporary
dependency symlink remaining. Art delivery `2fb80680` and publication receipt
`145d0e21` are published ancestors of the observed remote. All 373 public files,
29 tools and qualified evidence inputs still match their delivery pins.

[Consumer handoff](evidence/motion-candidates-v1/consumer-handoff.json) records
full descriptor/atlas/provenance hashes and proposed exact consumer files and
function hunks. The existing production whole-character adapter uses front-idle
frame zero; the layered compositor accepts only its baked skin tone. The active
Customize branch also selects WholeCharacterEditor rather than AvatarEditor.
Application adoption must therefore route a versioned runtime through the actual
registry, editor and World lease and prove direction, walking, layers and save
behavior in the real Phaser journey. Updating a decorative gallery is insufficient.

World is the sole NEXT allocator. All consumer hunks are proposals, not ownership
grants. Root153 v8's six scope groups remain exclusive, including protected root
wiring; no MapScene, root/main, shared/Auth, composition contract, generated or
backend rights transfer here. The handoff identifies the additional contract,
inventory mapping and Root journey test paths requiring owner coordination.
This closing receipt changes only owned documentation/evidence. Original AR03
remains IN_PROGRESS until actual application adoption and its validation finish.
