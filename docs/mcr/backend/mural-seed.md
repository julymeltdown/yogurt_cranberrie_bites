# Initial mural seed: local producer and art input

Status: CONTRACT_READY for the authoring input below; producer implementation
and native validation are NOT_RUN. This follows the original Wall qualification
handoff. It introduces no public endpoint, migration, client write authority or
startup integration. Root explicitly assigned `src/mural_seed/**`, its new tests,
this document and only the corresponding `lib.rs` module export to WALL-B.

## Frozen authoring input v1

Input identifier: `mcr-mural-initial-seed-art/1`. The source contracts are
`0019_mural.sql` and C18 `c18d5ba9fd27a73356d9be68614863172303c81f`
(published as `e7bd68cfa35fb620e2eed060a71a72e50718b9dc`). The unchanged visual
reference is `docs/mcr/design/reference-target-2026-11-04.png`, SHA-256
`d9a8fae2f96a82253542019ab673589b3938627956eace15c8594c702b5c4617`.
The initial motif must be original, editable artwork derived from that reference,
with its actual provenance recorded. No approval or operator identity is invented.

The assigned art owner chooses its reserved pack directory. These relative
filenames and byte meanings are fixed:

| File | Required content |
| --- | --- |
| `source.svg` | Newly authored editable source, `viewBox="0 0 128 128"`; no external resource dependency. |
| `pixels.bin` | Exactly 16,384 bytes, one unsigned palette index 0–15 per pixel; row-major, offset `y * 128 + x`, origin top left. No alpha channel, header or compression. |
| `palette.json` | UTF-8 JSON array of exactly 16 strings, each uppercase `#RRGGBB`, in index order. |
| `preview.png` | Native 128×128 opaque rendering reconstructed from `pixels.bin` and `palette.json`; the preview is evidence, not the server state. |
| `manifest.json` | Closed manifest described below, hashed over its exact stored bytes for the operator-selected pack binding. |
| `README.md` | Actual source/reference provenance, authoring procedure, content/visual checks and limitations. |

Palette index 0 is the immutable background role and the erase destination. The
artist must choose its actual opaque color to match the reference; the backend
does not assign a default color or silently reorder the palette. Indices 1–15
have no additional hard-coded role or color requirement. Palette order and every
pixel index must agree across source, binary, preview and manifest. Never use
legacy character art; any character imagery remains subject to the separately
selected character-source pack.

`manifest.json` has exactly these keys (the values below describe types, not a
claim that a pack has already been produced or qualified):

```text
format: "mcr-mural-initial-seed-art/1"
asset_id: "wall-initial-motif-art-01"
asset_revision: 1
event_binding: "OPERATOR_BOUND_AT_PROVISION"
width: 128
height: 128
layout: "ROW_MAJOR_TOP_LEFT_U8"
background_index: 0
files: {
  source:  {path: "source.svg",  sha256: <64 lowercase hex>},
  pixels:  {path: "pixels.bin",  sha256: <64 lowercase hex>},
  palette: {path: "palette.json", sha256: <64 lowercase hex>},
  preview: {path: "preview.png", sha256: <64 lowercase hex>}
}
provenance: {
  origin: "ORIGINAL_MCR_ARTWORK",
  reference_path: "docs/mcr/design/reference-target-2026-11-04.png",
  reference_sha256: "d9a8fae2f96a82253542019ab673589b3938627956eace15c8594c702b5c4617",
  rights_statement: <nonempty truthful English source/rights statement>,
  qualification_evidence: <nonempty actual pack-relative evidence document path>
}
```

Hashes describe exact file bytes; the source hash is the editable source file's
hash, not a screenshot, palette hash or claim of ownership. The operator-selected
manifest SHA-256 binds the complete file digest set and provenance. A candidate
manifest is not proof of rights, review or provisioning authority. The art owner
must actually inspect the reconstructed native image and document its checks
before handing off a source-qualified pack. No king/SL review stage is added.

## Owning operator boundary

Art authoring is event-independent. No sample UUID is an assigned production
event. The local owning provisioner supplies the actual canonical event UUID and
the exact selected manifest digest, plus its source-qualified pack, background
index 0, explicit cooldown (0–3,600,000 ms), and explicit initial state
`EDITABLE`, `PAUSED` or `FROZEN`. Neither cooldown nor state has an inferred
deployment default. The local typed producer must validate all file digests and
pixel/palette dimensions against that selected pack before any database write.
Browser arrays, a browser-supplied hash, previews and client overlays confer no
authority. No public API or operator account/role is created by this leaf.

Provisioning will lock the actual event and atomically insert initial revision
1 with the complete selected pixel/palette bytes only when there is no prior
mural, activity/history or recovery checkpoint for that event. An exact retry of
the same bound seed may return the same pristine result. A different seed,
changed live content, existing history/checkpoint or a non-pristine aggregate
must be refused without a write. It must never overwrite or restore an existing
wall, advance existing revisions, alter command receipts/budgets, or create
client-side initial content. Exact durable retry binding and existing-table
compatibility will be proved in implementation before this producer is called
qualified. No additional migration or production composition ownership is implied.

The source search found only fixture `INSERT`s (`mural_smoke.py` and tests) and
existing runtime readers/mutations; no qualified production initial-state
provisioner was found. This input freeze allows art to proceed independently of
Wall tools/native qualification. Runtime use waits for the responsible producer's
validation and the actual local operator binding, not a manager acknowledgment.
