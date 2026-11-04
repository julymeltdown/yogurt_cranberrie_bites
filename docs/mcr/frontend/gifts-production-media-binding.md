# Gifts detail protected-media binding

Task: FE-GIFTS-MEDIA-DETAIL-01. Implementation owner: this isolated frontend lane.
Base: `5da644264ee354f63e609db64d65a7c5e63c9c11` (published master).

The detail view accepts the existing `RenderGiftImage` port and sends the server's
`image_media_id`, giveaway ID, title and full current privacy scope to it. The
detail media group is keyed by scope, giveaway and attachment identity. A null
attachment displays “No photo”. A non-null attachment with no usable port displays
“Image access unavailable.”. Root remains responsible for visibility, protected
metadata/content reads, loading, denial/retry, Blob URLs, shared cache and privacy
withdrawal. Detail selection/loading and capability gates precede the photo slot.
Stock, pickup window, public place, unconfirmed reservation eligibility and the
existing reservation journey remain authoritative.

Exclusive writable paths are `Gifts.tsx`, the new `gift-detail-media` test folder,
this document and `evidence/gifts-production-media-binding/`. No CSS, shared media,
contracts, backend, routing, manifests, artwork or dependencies are owned here.

Root owns one required composition change in `CommunityFeatures.tsx`:

```tsx
<GiftDetail controller={controller} giveawayId={giveawayId} renderGiftImage={renderGiftImage} />
```

The optional prop is additive; old callers remain valid. Root's existing local
`renderGiftImage` already supplies `RootGiftImage` to the list. This leaf does not
create a second media owner or fetch a media URL independently.

Root supplied standalone source-only caller commit
`c34c5cf7f640c66b219cff186d0dc3e48e5dac6f` (SHA-256
`db2b1d0330b5af0347e55fda017da4d0bd1b586e9c2e8872d8fbae1390e577d8`).
The current published Root media dependency comes from `7eac13ce` (SHA-256
`f22e034423804ec41658fa26760d07926f4f2ebce339fed685072fbf09542d9a`).
Immutable copies under the new test folder are hash-checked and loaded at their
original module IDs for coupled Vitest/native qualification. A TypeScript compiler
host checks the same exact coupled sources in the full frontend program. The
ordinary full checkout typecheck also runs. Root shared files remain unmodified;
no unrelated Root HEAD changes enter this candidate. Published media CSS matches
the base bytes.

## Qualification

Source qualification **PASS** at `5af222c3cc6f2c2ab0876af5117368adbadaac48`:
8 test files / 96 tests, ordinary full frontend TypeScript, coupled full frontend
TypeScript against the exact two Root inputs, scoped oxlint and native-driver
syntax all exited zero. Evidence is in
`evidence/gifts-production-media-binding/source-02-qualified/`. The original
source offer began 2026-11-04 16:19:50.691116 UTC and finished 16:21:08.533357 UTC,
including the initial preserved baseline TS5097 failure and bounded repair.
The permitted `--allowImportingTsExtensions` invocation flag was applied to both
typecheck paths; no config or exclusion changed. The failed first report/logs
remain under `source-01-baseline-flag/`. Job processes, temporary cache and the
read-only dependency link were absent at release (16:21:39.354557 UTC).

The first native offer was **NOT_RUN** (historical startup refusal). The first native allocation was
refused by the startup guard before any workload was created: observed free
space was 16,866,349,056 bytes, below the 17,179,869,184-byte (16 GiB) start floor.
The allocation was immediately released; the exact failure and release record
are retained under `native-01-startup-floor/`. The implementation owner initially
misread the byte comparison before acceptance. A read-only preflight now emits
ACCEPT-ready data only when the numeric start-floor assertion passes. That refusal and its released allocation remain historical. A later fresh offer
qualified all 15 native contexts; see the final handoff below. No resource is held. Dedicated tests exercise the real controller, Auth privacy,
Root media provider/reader and route transitions, alongside existing FE24 and Root
provider regressions. The development-only native fixture adapts the published
Community adoption harness, preserving the real default router, canonical
bootstrap loader, selected whole-character runtime and Root media components.
Only HTTP/C05, map admission and conservative clock profile are synthetic.

The synthetic attachment uses existing shared-reader 1x1 PNG test bytes and a
fixture UUID, never a production record or an artwork fallback. Native states are
direct detail, list/detail/back, null photo, denied/retry and held content/privacy
withdrawal at 360/390/430 pixels. No real inventory, recipient or media rights
are fabricated.

## Production boundary

The frozen Giveaway DTO has a required, nullable UUID `image_media_id`. Null means
no image. The backend protected media adapter remains the authority for uploader,
actor, event, purpose and sanitized bytes. A giveaway UUID, public art filename or
external URL cannot substitute for this ID.

The five reference gift motifs have no approved provisioned media records or
rights-to-media mapping. Their production binding remains **NOT_READY**. This
implementation adds the protected detail path; it does not register artwork,
invent production media IDs, change FE24 acceptance or complete live IN02.


## Native harness corrections after first admitted round

The replacement offer permits a 12 GiB browser start floor, preserves continuous
8 GiB free space, and adds a 256 MiB total new-byte cap across source, Git,
failures, copies, profiles and outputs. The first admitted round retained three
failed attempts under `native-02-import-failure`, `native-03-readiness-failure`
and `native-04-lifecycle-failure`; no complete context passed. All resources were
automatically removed before the work deadline, and the final ephemeral listener
was successfully rebound during release verification.

The fixture stylesheet paths and startup-ready wait were corrected. An OS port-0
probe now selects an ephemeral port before Vite binds it strictly; Vite's own
zero value had selected its default 5173. The final failure exposed a harness
expectation error: `WorldShell` covers the world behind mobile full panels, and
its covered cold start can precede presence admission (zero sockets/one canvas).
The next admitted run showed why that was an incomplete interpretation:
`world/composition/index.ts` explicitly retains started presence across panel
coverage and only document backgrounding withdraws it. Checks now require the
cold covered start, then World (one socket/canvas) and return to the panel (the
same one socket/canvas). Input/render coverage and presence lifetime are distinct. Held-content privacy cases remain
covered throughout and require late-response abortion without creating a photo
URL. The TEST_ONLY URL observer separates the exact shared PNG fixture bytes
from public character texture URLs; protected-photo revocation is asserted
without treating public textures as private attachments.

Those failures remain historical; the final fresh native round below passed. Production source,
contracts, artwork and the exact two Root inputs are unchanged.


## Corrected native round and cleanup repair

The corrected 16:51 UTC offer admitted 15 functional cases which all printed
PASS, and retained 21 captures. The final structured result was not persisted
before the original work deadline. Process-group signalling then raised EPERM
in cleanup. The remaining owned temporary files/link were manually removed at
16:55:03.228246 UTC, about 16 seconds after the 180-second total deadline;
process inspection found no remaining job processes. The latest listener's
exact port/reuse evidence was missing with the final report. This round is
**incomplete**, not a qualified PASS. Exact logs, release record and captures
remain under `native-06-deadline-cleanup-failure/`.

All 23 retained original failure/incomplete PNGs were inspected, including the
two earlier direct-detail images. Their file hashes and candid state/visual
assessment are in `native-failure-image-assessment.json`. Ready's white rectangle
is the decoded shared test pixel. It proves neither a production gift photo nor
approved artwork binding. Null, loading and denied states remain distinct.

The repair writes the allocated/listening port immediately, persists each
completed case, and saves the final functional report before browser/server
teardown. A separate cleanup report records both closures. The independent
native guardian tracks process identities and signals only matching owned
PID/UID/start-time entries, avoiding stale/reused/system process groups.
Signalling errors are retained and cannot prevent temporary/link cleanup.
The final fresh round below proves all cases, final result, clean teardown,
bounds, exact port reuse and own screenshot assessment. Publication remains with Root.


## Qualified handoff

**FE-GIFTS-MEDIA-DETAIL-01 is developer-qualified and awaiting Root publication.**
Source: `5af222c3cc6f2c2ab0876af5117368adbadaac48` (96 tests, ordinary and exact
coupled full frontend TypeScript, scoped lint, driver syntax PASS). Production
`Gifts.tsx` remains SHA-256
`7fcfef793f1883117f1b3b1eac2ffe36d1e15834cafbd921619dc10c9468bb5a`.

Native: `09ade32c687bb6b38e5dd80d221a005aa9beaedb`, Browser2 named
`GIFTS-REPORT-CLEANUP-BROWSER2-202611041704Z`, independently admitted
2026-11-04 17:05:12.146320 UTC. Both commands exited zero. All **15 contexts**
(360/390/430 × direct detail, list/detail/back, no photo, denied/retry, held
content/privacy withdrawal) passed, with no page errors. Structured per-case and
final results, immediate port/read-set admission, browser/server cleanup and
independent guardian records are retained under `native-07-qualified/`.

The real default router, canonical Auth/bootstrap, Root media provider/readers,
selected whole-character Phaser and actual privacy fencing ran against explicitly
synthetic HTTP/C05, conservative TEST_ONLY clock profile and existing shared PNG
bytes. Cold covered entry has one canvas before presence starts. The World
roundtrip starts one socket; composition retains it when the panel covers the
canvas again. Disposal removes canvas/socket, and privacy invalidation removes
protected photo URLs. This is a synthetic functional gate, not a live-service or
production artwork claim.

Runtime and automatic cleanup finished 17:06:03.099702 UTC. Explicit release was
verified at 17:07:07.924181 UTC, inside the original 120-second work and 180-second
total allocation: no matching job process identities, temporary files/cache,
dependency link or listener remained. All port records agree on **54695**;
listener query found none and bind/listen reuse passed. Retained Root dependencies
were preserved. Peak accounting was 13 processes, 2,613,542,912 bytes RSS,
2,135,114,013 bytes whole charged footprint, 32,681,984 bytes temporary,
2,691,072 bytes durable and 32,792,576 bytes total new growth; the minimum observed
free space was 31,546,585,088 bytes. Every offered bound passed.

All **21 qualified captures** SHA-256-match original PNGs individually inspected
by this owner during the prior functional run. The current-file hash-to-inspected
original mapping and candid visual assessment are retained in
`native-07-qualified/image-assessment.json`. No new CSS density candidate enters
this frozen read set. The white ready plane is the known 1×1 decoded test image;
null/loading/denied text is legible, and initial viewport screenshots do not
claim to show the entire scrollable detail journey.

Root should consume only the exact leased leaf/tests/doc/evidence paths and its
own one-line `GiftDetail` caller binding. Frozen read-only caller and media inputs
retain the hashes recorded above. No central file, CSS, backend, contract,
manifest, lockfile, character art or production media registration changed here.
Root's canonical same-candidate publication receipt establishes remote delivery;
no proof-only commit or coordinator acknowledgment is required. Whole FE24 and
live IN02 acceptance remain unchanged, and production motif mapping is NOT_READY.
