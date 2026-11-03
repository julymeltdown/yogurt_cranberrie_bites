# FE-REF-MAP — portrait illustrated Places

Status: **MOCK_READY**. Actual production Places and root are qualified with deterministic HTTP/C05 fixtures. Live-service acceptance and full reference-art fidelity remain unproven.

Assignment base: `f837bef676cd26869c26c172a699a91b7842d832`. Reconciled publication base: `a4ae8555af8d43b1cddf8b07d71f48d14ad20940`. Own source `b54dc5e4045f710343bfaabaf779fc56f61fb4ec` is equivalent to original `47794c2a`; own resize fix is `95bb14589ef91e9ad9bc4e7b8581d229bbbf0875`. The containing evidence commit completes this scoped delivery. Only `frontend/src/features/places/**` and this document changed. `world/illustrated/overview.ts` is unchanged.

## Reference and provenance

The supplied ten-screen reference was viewed, SHA256 `d9a8fae2f96a82253542019ab673589b3938627956eace15c8594c702b5c4617`. Its Map informs the portrait illustrated surface, direct icon/label plaques, red target pin and right-edge zoom. Original source remains 1152×896 and orthogonal; the reference is a taller diagonal, stage-led composition. Initial proportional cover cropping exposes Studio/Park; pan or Places reaches edge landmarks. Fit reveals the entire source. No stretch, skew, geometry rewrite or replacement art was used. New reference-scene art proposals are not consumed or qualified here. Full diagonal-scene fidelity remains incomplete.

Original scenery: author `19d7085` / integration `bc60edb`. Incumbent loader/runtime: author `2afb527a03b13411f92fa8d0b0db636a868d45ea` / integration `3eefd4ef`. Prior static overview `80d91501694ede5ddc6ad28ac37010309c035878` is in the assignment baseline. Contracts remain C03 OpenAPI 0.1.0, C05/C06 1.0.0, scenery extension 1. Source pins hash unchanged art, geometry, selected character-source metadata, contract inputs/generated documents, required plans and preserved `docs/source/` bytes. No legacy rendering or new dependencies were imported.

Published root `deb5865d` (author `7201b875f9ed6a07a0a77b9ae4aeac2f3f2fbd57`) supplies one heading and a full-height body. Temporary local validation cherry-pick `82089f4b` was dropped during reconciliation. Its 14 root paths are attributed separately in source-pins and excluded from this publication.

## Behavior and boundaries

The local DOM viewport displays the incumbent owner's static projection with current authorized catalog IDs/labels and semantic landmark icons. Touch, mouse and arrow keys pan locally. Bounded zoom, Fit and Reset issue no HTTP requests or world-camera writes. Resize clamps effective zoom to its new fit bound. Annotation decluttering moves boxes only; leader lines preserve exact immutable interaction points. Arbitrarily dense catalogs remain best effort with a complete searchable Places dialog.

A red pin appears only for an explicitly focused/pointer-selected open, available target, at its exact interaction center. No physical arrival or user position is inferred. Weak controller-lifetime selection stores only public place ID/map hash; Back restores plaque focus. Closed/unavailable, removed, mismatched and source/review behavior retains existing controller authority. Selected character-pack metadata/art is unchanged.

Native Places/Legend dialogs handle Escape and restore opener focus. The observer disconnects on unmount. The consumer never revokes producer-owned URLs. Broken imagery retains schematic/list; pending theme/account replacement and disposal cannot restore stale plaques. The existing shell still owns one game and one world connection.

## Validation

[Validation manifest](../../../frontend/src/features/places/__tests__/reference-map/evidence/validation.json): all commands exit 0 on the reconciled production tree. Targeted 9 files /111 tests, strict TypeScript, lint, Vite build, UI-origin guard and planning check pass. The build uses `copyPublicDir:false`, avoids another public asset copy and retains the existing large-chunk advisory. The planning check certifies planning exports only.

[Native report](../../../frontend/src/features/places/__tests__/reference-map/evidence/native.json) uses production router/Auth/owner/Phaser/Places and published root without layout overrides. Chrome156 checks 360/390/430×844 plus 390×430. Portrait slots are 326/356/396×718. Cases cover keyboard pan, min/max zoom, Fit label non-overlap, target selection/source detail, Back focus, dialog search/Escape/focus, root close/reopen, valid/broken/pending theme replacement, closed/removed/mismatched catalogs, late account replacement and disposal. Per width: 24 URLs created/revoked, one mount/peak game, one disposal, zero sockets left and maximum one active socket. Gestures send zero requests, incumbent camera stays unchanged, and page errors are empty.

Five compressed reviewed captures: [360 portrait](../../../frontend/src/features/places/__tests__/reference-map/evidence/portrait-360.jpg), [390 portrait](../../../frontend/src/features/places/__tests__/reference-map/evidence/portrait-390.jpg), [430 portrait](../../../frontend/src/features/places/__tests__/reference-map/evidence/portrait-430.jpg), [390 Fit/selection](../../../frontend/src/features/places/__tests__/reference-map/evidence/fit-selected-390.jpg), [390 short](../../../frontend/src/features/places/__tests__/reference-map/evidence/short-390.jpg). Review found and corrected short-resize zoom below its new minimum, overlapping labels and zoom controls obscuring labels. Final native assertions cover these regressions.

Inherited deterministic shell admission/HTTP/C05 fixtures use simulated review metadata and an inherited geometric avatar: **TEST ONLY**, not selected-pack character or production approval. Live C03 admission, real two-account services, approved production characters, Safari/Android devices, FPS/load and complete ten-screen fidelity remain **NOT_RUN** or incomplete. No scope or acceptance task was reduced.

Disk headroom exceeded 8GiB before final build/captures. Sparse checkout omits tracked working copies only. No install/lock update occurred; the temporary retained-dependency symlink is removed before clean handoff.

## Delivery

The owner may publish these scoped commits once onto current `julymeltdown/mcr/master`, preserving peers and omitting separately published root work. Final handoff supplies fetched remote SHA and ancestry verification. Source, validation, preview inputs and delivered file hashes are pinned in the owned evidence directory. The containing commit identifies this document's delivery without a self-referential hash.
