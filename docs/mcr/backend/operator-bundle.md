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
metadata fixture directory and must fail. The new regression is now qualified by the fresh run recorded below; the earlier
root ledger binaries do not qualify it.

Frontend may consume the three exact descriptors for its own component tests
without waiting for full backend production startup. Production release
construction, actual PublicProfileReader factory join, HTTP/WS and operator
admission remain open. Nothing in this handoff flips a runtime feature.


## Actual production constructor qualification

At clean compiled source `c7fdd2581a9543e1748929739865ae79d436f870`,
Rust1.88 locked/offline jobs2 debug0 incremental0
`cargo test -p concert-service --test product_runtime --no-run --message-format=json`
exited 0. Two earlier attempts exposed stale shared-target artifacts: an absolute
World checkout proto include path, then platform artifacts missing modules that
the actual source exported. Narrow cleaning of contracts-build/Auth/Friend/Profile
and then platform repaired these failures; no source workaround was applied.

The exact emitted test binary and Cargo JSON/full 1,853 source hashes were copied
to `/private/tmp/mcr-operator-callback-native-20261104`. Its SHA-256 is
`681e188a5d3a8ad24947c76b144a3bc0f730f8a11b2c711e94166428cdcde7a2`.
The MEDIA target was explicitly released with zero handles and 64,910,725,120
free bytes before the private copied binary ran. All five tests passed, zero
failed/ignored (0.67s). This includes actual production source-bound construction,
all capabilities disabled, whole-character/map4/three-theme identity, and refusal
of an unrelated metadata-only resource root. No database was needed or accessed.

The same compile covers the root callback, Participant/SELF generation and narrowed
subject-denial source dependencies. It does not execute the callback's real-owner
join or the new internal mapper test. Portable exact input hashes and limits are
in `constructor-qualification.json`. The subsequent builder-only metadata rename
from `not_run` to `not_certified_by_builder` clarifies its own boundary; all four
executed descriptor/Bootstrap input bytes and the Rust test/callback sources remain
unchanged. Production PublicProfileReader composition and HTTP/WS are still NOT_RUN.
