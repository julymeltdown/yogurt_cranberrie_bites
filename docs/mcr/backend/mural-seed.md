# Initial mural seed: local producer and art input

Status: INTEGRATED for the source-qualified local producer; its compilation and
four exact native qualification cases pass. Authenticated server snapshot and
default-App/two-client IN04 remain NOT_RUN, so overall Wall acceptance stays open.
This follows the completed Wall qualification handoff. It introduces no public
endpoint, client write authority or startup integration. Root explicitly assigned
`src/mural_seed/**`, its new tests, this document, only its `lib.rs` module export,
new `backend/scripts/mcr/mural_seed/**` and additive migration
`0063_mural_seed_provenance.sql`. Historical 0019/0058 remain unchanged.

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
compatibility must pass native validation before this producer is called
qualified. Only the explicitly assigned additive 0063 migration is included;
no production composition ownership is implied.

The source search found only fixture `INSERT`s (`mural_smoke.py` and tests) and
existing runtime readers/mutations; no qualified production initial-state
provisioner was found. This input freeze allows art to proceed independently of
Wall tools/native qualification. Runtime use waits for the responsible producer's
validation and the actual local operator binding, not a manager acknowledgment.

## Implemented local producer candidate

The actual art owner delivered source-qualified commit
`ca5d4bcad50d6159597309a961a29a8e4d1f3bb9`, exact six-file pack at
`frontend/public/wall/initial-motif-v1/`. Selected manifest SHA-256 is
`3f1aa318f0e4b7d8fa7f079b1101f7386619291a8dbb3bb04ef70820952f34ce`.
Its background index 0 is the artist's actual `#171617`. Artist qualification
and native/390px art proof do not qualify this server producer or a live Wall.
The backend consumes a selective frozen six-file readset from that revision;
it never edits frontend art or falls back to another pack.

`mural_seed::load_pack` freezes bounded local file bytes, rejects unsafe relative
paths and symlink components, and checks closed manifest/provenance keys, the
complete operator-selected manifest SHA and all four exact file SHA values.
`validate_pack` checks all 16,384 indices, ordered uppercase palette values,
bounded native PNG decoding, opacity and every reconstructed RGB pixel. Editable
SVG bytes are pinned and UTF-8 checked; server code neither renders the SVG nor
independently repeats the art owner's source/visual qualification. That source
qualification remains an explicit prerequisite of the operator's selection.

`mural_seed::provision` accepts only a validated pack and explicit typed local
`OperatorSelection`: real canonical event UUID, qualified source revision, real
operator/run binding, cooldown and initial state. The owning workflow and its
database credential must be local and operator-only. This type does not mint
an Auth role or prove authority from browser input. There is no public route.
The transaction locks event then root, with bounded lock/statement timeouts.
It rejects all existing operations, checkpoints, limits, locks, actions,
correlations and charges. A new seed inserts revision 1, background config and
immutable provenance atomically. It issues no update, checkpoint or delete.

0063 stores full 32-byte manifest and input digests, exact source revision and
operator binding, independent initial snapshot UUID, original state timestamps
and database provisioning time. The input digest binds event, source, complete
manifest digest, evidence document digest, background role, cooldown, state and
operator binding. An exact retry additionally compares every pristine root
field and explicit config; a foreign row or changed content is refused. Digest
bytes are never truncated into a UUID. UPDATE/DELETE/TRUNCATE of provenance
are refused. Reading the actual public snapshot creates an existing immutable
checkpoint; subsequent seed retries then correctly fail the pristine guard.

The new controller composes existing `world_in01_prepare.py::seed_concert` and
the actual Concert `--prepare-in01-catalog` command in a fresh marker-owned PG16.
The canonical event comes from that real owning producer and source release.
For this isolated run, operator binding derives from the actual marker bytes,
device/inode and uid, hashed without disclosing the marker token. Test-qualified
cooldown 1,200 ms and `EDITABLE` are explicit isolated inputs, not production
approval. It does not create admissions, fake principals, attach an old runtime
or invent a privileged Ops role. A separate SQL fixture exercises atomic
commit-failure rollback, concurrent retries, foreign/mutated rows, history and
provenance immutability; those fixtures are not claimed as production bootstrap.

The four exact `tests/mural_seed.rs` cases are ignored by default and require
explicit actual art/owned PostgreSQL inputs. The controller runs each exact case,
checks migration receipts/prepared-transaction quiescence, restarts its actual
postmaster, compares state/config/provenance persistence and cleans only its
marker-owned cluster. It requires a frozen source/two-executable/art checkpoint
and a fresh accepted resource allocation, with an external whole-round guardian
when required by that allocation. Syntax parsing alone passed; Rust/native
results from the completed accepted round are recorded below. Authenticated
server C03/snapshot, default App, two-client IN04 and production provisioning
remain open.

## Completed source-bound qualification and runtime handoff

Executed source `3b690326b21e1344dd4577f26797ec30151e4253` passed all four
exact seed cases, exit 0, with zero prepared transactions after every case.
Both the seed test executable and actual Concert binary compiled under Rust
1.88, locked/offline single-job debug-0/incremental-off builds. The first cached
build-script attempt referenced a peer proto absent from this checkout; that
exit-101 failure is retained. Refreshing only eleven byte-identical own-checkout
source mtimes fixed the cache selection. No dependency/source changes, target
cleanup, cold target or clock extension was used.

The native run verified the actual art pack and ten rejection checks; atomic
rollback on an actual deferred commit failure; exact operator/source retries;
rejection of a different manifest with identical pixels; concurrent retries;
foreign/mutated roots and checkpoint refusal; and immutable full provenance.
The separate SQL fixtures are explicitly test fixtures. The final operator case
instead uses existing `world_in01_prepare.py::seed_concert`, real Concert
`--migrate`, `--verify-migrations` and `--prepare-in01-catalog` commands, all exit 0.
All database cases verified 45 actual Concert migrations, including 0058/0063.

That owning bootstrap returned canonical event
`f1fd7b85-d393-5410-8bc3-5ea6b71aeb77`. The actual isolated operator run inserted
revision 1, snapshot `082fbdd3-a349-413f-bc71-fc044d75b354`, explicit cooldown
1,200 ms and `EDITABLE` state at `2026-11-04T17:51:00.204285Z`. Full input digest:
`2a531aa0efe8d0b3d2be3402e66f4cd65e2848b1827d2e224a3047b9db2ffca2`.
The exact database pixels retained artist SHA-256
`f29eaf4ea632e2a34b6dd4fdfdc6f8a3d07ab780af96e19c2a64c28faf42c721`.
The recorded local operator/run binding is derived from this run's actual owned
marker/device/inode/uid. It is evidence of that isolated operator run, not a
reusable production identity or Ops-role authorization. The native retry
preserved its original snapshot and provisioning timestamp.

Actual PostgreSQL restart changed postmaster PID 12546 to 13968 and server start
time while retaining state/config/provenance digest
`2a68f7bd9b32d1b98be54d2b4f7aa722`. Exact owned cleanup and independent guardian
reaping passed. Actual target release was 2026-11-04 17:50:29 UTC with zero
compiler/target handles; native FULL release was 17:51:01 UTC. Build NEW peaked
at 960,196,608 bytes; whole native-phase NEW peaked at 1,043,431,424 bytes,
13 processes and 256,376,832 bytes RSS. These fit the original 2 GiB NEW/14 GiB
whole allowance, including retained failures/history. No resource remains held.

Frozen executable SHA-256 values:

- Seed test: `343853a2d7add4df480786b734283ca481c71b8393d5a9c3e6d8a843e879375d`.
- Actual Concert main: `a15ab6773495ef22b76f8b8c281126eafb2dd1dc08ec7b8ac0b4fbad384c5839`.

The checkpoint records 3,748 source/dependency/oracle hashes plus all six exact
art-file hashes. Committed evidence is
`backend/scripts/mcr/mural_seed/native-qualification-2026-11-05.json`; complete
private round evidence remains in `/private/tmp/mcr-wall-seed-pea8v92n`.

For the next owning runtime, use a freshly owned, current-schema canonical
event from that same owning bootstrap. Freeze the selected six-file qualified
pack and its manifest SHA. Invoke `load_pack` then `provision` with explicit
`OperatorSelection`; the tested isolated adapter is exact ignored test
`operator_provision_bootstrap_event`, with `MCR_MURAL_SEED_PACK` and the verified
`MCR_CANONICAL_ROOT`, root identity/token/system and Concert URL supplied privately
by the owning controller. It derives the real local run binding rather than
inventing a product operator. No new main CLI or HTTP endpoint is introduced.
The complete four-case controller accepts `--checkpoint`, `--pg-bin-dir`, new
`--report`/`--live-owner` and the existing accepted lease ID/deadline/byte limits;
it creates and removes its own cluster, so its completed database cannot be
reused as the next live runtime.

The responsible runtime developer must then serve the actual Wall owner with
the same canonical core, real Auth/Friend/admission, explicit live Wall keys and
fresh point-in-time C04 capabilities. Read the authenticated server snapshot and
compare every pixel/palette value with the published pack before the default-App
two-client IN04 journey. A snapshot creates a recovery checkpoint, after which
the seed provisioner must refuse further retries. Do not overwrite an existing
mural or add a client seed overlay. The older R7 stack lacked 0058 and is not
valid evidence. This native producer run makes no C03/HTTP/browser/device/load
or production Ops-role claim. Publication and the next ready work do not wait
for manager review or acknowledgment.
