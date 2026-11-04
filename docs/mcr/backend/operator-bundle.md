# Source-bound operator candidate

The new `backend/releases/operator-bundle/quality-v6/` contains three real
ThemeManifest descriptors and a freshly authored, inert Bootstrap template.
The deterministic builder is `backend/scripts/mcr/build_operator_bundle.py`.
This is a component handoff, not event admission, operator activation or visual
acceptance. Every capability is false. Template event and participant UUIDs are
stable validation identities, not seeded admissions or real principals.

Inputs are Spatial publication `0a8c422645458c9397af9289e15089ee3f9cb0ad`,
its source qualification SHA-256
`70c3d1ac268c6e5da39d30a20e01406fc7eb27668d6f0e6a66e1d87de15d7d07`,
canonical map revision 4/content `sha256:736afd273a067f6c40db76b8cc54412416fba88984a50d9a6b6738d869831fb8`,
and the byte-preserved selected whole-character manifest
`contracts/fixtures/assets/selected-static/avatar-manifest.json`. The original
artist files, geometry, selected originals, map and avatar manifest are read-only.
The four virtual-place records retain UNREVIEWED/VIRTUAL_CONCEPT/CONCEPT_ONLY.
The exact engineering-reviewed scenery provenance is carried into each era; it
does not imply physical venue verification or user approval of art fidelity.

The builder checks C03 Bootstrap schema and semantic consistency, the complete
C06 asset bundle, canonical content hashes, map/avatar/place/interaction bindings,
source qualification and binding hashes, every pinned scene source/resource,
PNG full decode and dimensions, source geometry and original place equality,
Tiled grid and texture dimensions, and the selected original/derived avatar
identity. Theme texture uses the actual shared 768×1296 ground required by the
canonical Tiled resource. Era artwork uses its own pinned 1024×1024 atlas through
`scenery`; older 256×128 theme textures are not substituted. Only palettes are
read from the retained palette source. `bundle.json` pins source and output bytes.

## Validation

The local system Python has Pillow but lacks the pinned URI/date-time plugins.
An initial run correctly failed closed. The retained contracts environment
provides the exact `scripts/mcr/requirements-contracts.txt` packages; its site
packages were selected with the system interpreter to retain Pillow:

```sh
PYTHONPATH=/private/tmp/mcr-fe14-format-venv/lib/python3.14/site-packages python3 backend/scripts/mcr/build_operator_bundle.py
PYTHONPATH=/private/tmp/mcr-fe14-format-venv/lib/python3.14/site-packages python3 backend/scripts/mcr/build_operator_bundle.py --check
```

Both commands exited 0: five deterministic outputs, CONTRACT_READY, NOT_ADMITTED.
Other environments should install the pinned contracts requirements and Pillow
in their own validation environment; the runtime does not depend on this local
path. The local environment paths are evidence only.

The actual Rust constructor regression is authored in
`backend/services/concert-service/tests/product_runtime.rs` as
`actual_source_bound_operator_candidate_parses_without_enabling_capabilities`.
It uses the production `ProductReleaseBundle::from_source_bound_files` entry
with Bootstrap, selected avatar, map, three themes, default-disabled features,
actual public resource root and source binding. A second call substitutes a
metadata fixture directory and must fail. Compilation/execution of this new
regression is **NOT_RUN**, queued for the next allocated target; the earlier
root ledger binaries do not qualify it.

Frontend may consume the three exact descriptors for its own component tests
without waiting for full backend production startup. Production release
construction, actual PublicProfileReader factory join, HTTP/WS and operator
admission remain open. Nothing in this handoff flips a runtime feature.
