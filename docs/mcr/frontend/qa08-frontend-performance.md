# QA08 bounded frontend performance measurement

## Renewed current-candidate qualification — 2026-11-04

The renewed bounded client workload is **MEASURED**. Round02 completed all commands with exit 0 and released its allocation. Original QA08 remains **PARTIAL**: physical mid-range devices, live Auth/C05, TCP packet loss, 200 canonical principals across four rooms, reconnect storms, service ceilings and operator-cap decisions remain **NOT_RUN by this leaf**. Desktop CSS viewports and synthetic authority do not establish device, capacity, production-admission or product acceptance.

The public source baseline is `d62ce6979d6a3326f2da7271c3ae1eba54c6a22f`; measured source is `27a01c7224da1e9677fb13460621e0e2f71c7c5e` on `codex/mcr-qa-QA08-frontend-current`, normally reconciled in the existing checkout. The evidence/document commit changes no measured harness/application/public/contract input bytes. Ownership remains exclusively `frontend/e2e/qa08-rendering/**`, this document and `docs/mcr/evidence/frontend-performance/qa08/**`. No production, shared, backend, manifest, lockfile or CI source was edited.

Evidence: [raw Round02 measurements](../evidence/frontend-performance/qa08/current-round02-native/measurements.json), [2,630 exact input hashes](../evidence/frontend-performance/qa08/current-round02-native/input-pins.json), [actual command/resource/release receipt](../evidence/frontend-performance/qa08/current-round02/release.json), [native build/run log](../evidence/frontend-performance/qa08/current-round02/04.log), and [owner assessment/image hashes](../evidence/frontend-performance/qa08/current-round02/owner-assessment.json). All 2,630 inputs matched after execution. Historical results below are preserved as historical evidence.

### Source and method

The fixture uses the current canonical Auth controller, default `createProductionPorts` with its event argument, default production resource resolver, actual MapScene, routes and whole-character renderer. The exact published operator-bootstrap/local-input fixtures provide map revision 4 and `map4-service-v1` overview presentation. Only world availability, PARADE preference, login/ticket identity and a finite six-player stream are synthetic authority. The actual production `main.tsx` index is built independently with event/service admission unconfigured; its `/world` correctly says “World entry is not open yet” and has no canvas.

The operator map content hash is `sha256:736afd273a067f6c40db76b8cc54412416fba88984a50d9a6b6738d869831fb8`, selected-static avatar hash `sha256:0627c7d178b22535a561e1270db6d8bf9e2383ba84d03a63962e48e565c59b2b`, and PARADE theme hash `sha256:334011156771af9cfa199671f4977c517921bb877928245f501ecde3a55f163b`. Original selected-static descriptors/art are read-only inputs, including six actually rendered sprites; this is no art revision or design-reference acceptance.

Independent actual-production and optimized `qa08` fixture builds use private output trees and `publicDir:false`. The owned static server serves emitted build files and the complete retained public tree read-only. Only emitted files are Brotli-compressed; original public asset bytes and Content-Length remain unchanged. The pin set covers the entire public tree. Required non-favicon HTTP failures are rejected; Round02 recorded none. CDP encodedDataLength includes response headers, records cache hits individually, and excludes synthetic Auth/HTTP/C05 exchanges and blob payloads without network transfer. All-emitted-JS inventory sums and journey transfer sums have different scopes.

Environment: Chrome 156.0.8078.80 headless, Node v24.22.2, Darwin 27.2.0 arm64, Apple M3 Max, 16 logical CPUs, 51,539,607,552 bytes host RAM; actual Phaser renderer CANVAS. Main contexts use 360/390/430 × 844 CSS pixels, DPR 1 and reduced-motion preference. One repetition per width, five requested 5-second windows per width, no GC during timing windows. The separate cold context uses the default motion preference. Other host activity is not isolated; these finite observations have no benchmark confidence interval.

Startup ends at LIVE plus six actual sprites. RAF and actual Phaser POST_RENDER timestamps are retained. Rates are callback counts/actual window length, not GPU presentation rates; intervals use nearest-rank percentiles. The original mobile 30-FPS/33.333ms budget is a comparison only. No new FPS, memory-leak or capacity threshold is invented. Covered zero-render windows have unknown interval percentiles, not 0ms frame times.

### Rendering results

Rates below are Hz; durations/intervals are milliseconds. No sampled visible interval exceeded the original 33.333ms comparison budget. This desktop result does not qualify physical-device FPS.

| Width | Window | Actual ms | RAF Hz | Phaser Hz | Render p50 | p95 | p99 | max |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 360 | idle-visible | 5002.80 | 59.97 | 59.97 | 16.70 | 18.00 | 18.40 | 18.60 |
| 360 | keyboard-moving | 5000.50 | 59.99 | 59.99 | 16.70 | 17.80 | 18.40 | 18.60 |
| 360 | full-panel-covered | 5001.30 | 60.18 | 0.00 | N/A | N/A | N/A | N/A |
| 360 | delayed-ack-consumer | 5001.60 | 59.98 | 59.98 | 16.70 | 17.50 | 18.10 | 18.20 |
| 360 | recovered-visible | 5000.70 | 59.99 | 59.99 | 16.70 | 17.50 | 18.00 | 18.10 |
| 390 | idle-visible | 5001.80 | 59.98 | 59.98 | 16.70 | 17.50 | 18.20 | 18.50 |
| 390 | keyboard-moving | 5003.30 | 59.96 | 59.96 | 16.70 | 17.40 | 18.10 | 18.30 |
| 390 | full-panel-covered | 5001.90 | 60.38 | 0.00 | N/A | N/A | N/A | N/A |
| 390 | delayed-ack-consumer | 5000.70 | 59.99 | 59.99 | 16.70 | 17.40 | 18.10 | 18.60 |
| 390 | recovered-visible | 5000.80 | 59.99 | 59.99 | 16.70 | 17.50 | 18.20 | 18.70 |
| 430 | idle-visible | 5003.00 | 59.96 | 59.96 | 16.70 | 17.40 | 18.10 | 18.30 |
| 430 | keyboard-moving | 5002.90 | 59.97 | 59.97 | 16.70 | 16.90 | 17.30 | 17.90 |
| 430 | full-panel-covered | 5001.20 | 60.19 | 0.00 | N/A | N/A | N/A | N/A |
| 430 | delayed-ack-consumer | 5001.30 | 59.98 | 59.98 | 16.70 | 17.70 | 18.50 | 18.70 |
| 430 | recovered-visible | 5000.50 | 59.99 | 59.99 | 16.70 | 18.30 | 18.70 | 18.90 |

### Navigation, recovery and memory

Each width completed five cycles of panel navigation, serial reconnect, public-route teardown and World remount. Panels/reconnects preserved the observed actual game; each public return created exactly one new observed game, with one active socket and the same canonical Auth identity. Weak game references avoid retaining disposed games for identity tracking. All 15 public-route release samples had zero games/active sockets; final root disposal also recorded zero canvas, game, sprite, active socket, queued injection, fixture timer, heartbeat timer and socket listener counts. Each main width created/revoked 35/35 object URLs with zero active. The cold context released 6/6. Default production cache internals are **NOT_EXPOSED**, so cache-entry/byte release is not asserted for the renewed run.

CDP forced-GC heap/DOM samples bracket lifecycle measurements. The declared first cycle is excluded from growth regression, leaving four samples per width. Baseline/before/final values below are root-level samples; regression uses the separate cycle samples and should not be reconstructed from those three endpoints.

| Width | LIVE + sprites ms | Baseline heap B | Before root dispose B | After root dispose B | Remounted slope B/cycle | Released slope B/cycle | Remounted first→last B | Released first→last B |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 360 | 898.61 | 15,737,920 | 19,907,916 | 18,533,572 | 247003.20 | 276097.60 | 717588 | 865016 |
| 390 | 902.82 | 15,556,860 | 20,002,624 | 18,505,772 | 228389.20 | 279628.00 | 714864 | 873716 |
| 430 | 912.77 | 15,727,684 | 19,858,992 | 18,497,056 | 219782.00 | 271323.20 | 693736 | 844564 |

**Positive heap growth remains observed.** Retained fixture `Socket[]` records and their `.sent`/`.last` state, requests, delivery/phase histories, module graph and renderer state confound attribution. Raw `retainedHarness` wording mentions “cache diagnostic records”; it does not mean default resolver cache internals were observed. No leak-free conclusion follows from either growth or zero lifecycle counters. CDP JS heap/DOM exclude complete process/GPU/native memory; crossOriginIsolated=false and precise-agent memory is NOT_EXPOSED. This finite run is not a long-duration soak.

One synthetic inbound Delta was deliberately dropped per width; the next sequence exposed a gap, the real client requested resync, and the fixture answered with a fresh stream. All three recorded one drop, one resync, return to LIVE and the same game. This is application-message loss, not TCP packet loss. Each width also completed three main serial fault/reconnect actions and one per lifecycle cycle: 24 serial actions overall, not a concurrent reconnect storm. Main browser fault-to-LIVE timings were 54.40/39.40/39.80ms (360), 40.10/47.10/44.80ms (390), and 54.80/48.60/46.90ms (430); node automation elapsed 139.51–160.24ms includes click/poll overhead. Neither isolates intrinsic network transport latency.

The delayed synthetic ACK consumer services its FIFO every 100ms after two seconds of input; it is not a 100ms RTT model. All pending/injector queues drained to zero, with peaks 20/20 per width, below the incumbent C05/client 64-frame bound. Across 115 delayed deliveries, p50=1045.60ms, p95=1898.60ms, p99=1966.00ms and max=1997.80ms include queue backlog. C05's 262144 outgoing bytes, 20Hz simulation target, 10Hz Delta target, cap50 and capacity_qualification=NOT_RUN are contract settings, not measured service ceilings.

### Bundle and transfer results

| Independent build | All emitted JS bytes | Sum gzip JS bytes | Sum Brotli JS bytes |
| --- | ---: | ---: | ---: |
| Actual production | 4,008,256 | 876,835 | 709,277 |
| Optimized synthetic fixture | 4,082,792 | 894,095 | 723,666 |

All emitted output sizes were 7,355,631 and 7,436,591 bytes respectively. These include lazy modules and are not startup transfer claims. Vite's existing large-chunk advisory is retained without changing configuration.

Independent per-route CDP collectors observed production landing=267,106 bytes, `/login`=2,919,928 bytes, and unavailable `/world`=726 bytes. They run in one sequential browser context: later routes use warm caches. Login includes its actual cemetery/background/font assets. The corrected landing background returned 200, 4,033 encoded bytes (3,820-byte raw asset plus response overhead). Complete fixture World journeys were 3,152,751 / 3,152,583 / 3,152,583 bytes (360/390/430), covering timing windows, map panel and all five navigation cycles, not just startup. One unrelated `/favicon.ico` 404 (168 encoded bytes) is retained in the 360 journey. Required resource failures are empty.

The fresh 390 context constrained only actual static HTTP via CDP: latency100ms, download1,000,000B/s, upload512,000B/s. It reached LIVE plus six sprites in 5679.22ms with 3,112,194 encoded bytes. Its 5001.30ms visible window had 59.98Hz Phaser callbacks, p95=18.40ms, p99=18.90ms and max=22.50ms. Synthetic Auth/HTTP authority/C05 are unthrottled, so this explicit profile is not a live-service/mobile-network verdict.

### Saved-image assessment and preserved diagnostic

All seven Round02 PNGs were individually opened and inspected: [landing390](../evidence/frontend-performance/qa08/current-round02-native/default-landing-390.png), World [360](../evidence/frontend-performance/qa08/current-round02-native/world-360.png)/[390](../evidence/frontend-performance/qa08/current-round02-native/world-390.png)/[430](../evidence/frontend-performance/qa08/current-round02-native/world-430.png), and map [360](../evidence/frontend-performance/qa08/current-round02-native/map-360.png)/[390](../evidence/frontend-performance/qa08/current-round02-native/map-390.png)/[430](../evidence/frontend-performance/qa08/current-round02-native/map-430.png). Landing now shows its moon/cloud/hearts/garden decoration and selected character. World captures show six distinct selected characters on the current PARADE scene, stage/tree and controls. Map captures show current overview artwork, MAIN STAGE/PLAZA/WALL/FEED/MEET/GIFTS labels, zoom/footer controls and the virtual-position disclaimer. These observations establish the pictured workload only; they do not qualify accessibility, visual-reference convergence, physical devices or production arrival/admission.

[Round01 measurements](../evidence/frontend-performance/qa08/current-round01-native/measurements.json) and [release receipt](../evidence/frontend-performance/qa08/current-round01/release.json) remain immutable. All commands exited zero, but owner inspection found the owned static server omitted public assets outside `public/world`, returning 404 for the existing 3,820-byte landing background. Its collector MEASURED status is therefore **DIAGNOSTIC/SUPERSEDED for final current-workload qualification**. Its images visibly lack the background; its landing/navigation totals omit required workload. The server/full-public-pin/non-favicon-failure correction is in `506f7674b0ecb115af3246a1753f16820e3e5a44`, followed by retained diagnostic evidence at measured source `27a01c7`. Round02 used a fresh allocation and confirms the actual resource response and displayed change. This was an owned harness defect; no production fix was required. No failed request, raw receipt/log, image or older qualification was overwritten.

### Validation and resource release

Named sole-browser offer `QA08-FE-ROUND02-BROWSER-202611041349Z` was explicitly accepted at 2026-11-04T13:49:41.669671Z, with 30,798,520,320 free bytes. Original fixed work cutoff was **22:55:31.669671 KST** and hard full-release cutoff **22:56:41.669671 KST** (UTC times are in the raw receipt). The actual offer allowed 350s work/420s total, a 70s gap; the supervisor separately retains a 60s minimum cleanup-reserve constant. Acceptance/start/free-space checks did not renew either offered deadline.

Actual commands ran sequentially: Node syntax (0.586s), four metric selftests (0.570s), fixture TypeScript (4.021s), scoped deny-warnings lint (0.589s), and independent production/fixture builds plus native workflow (132.820s), all exit0. Supervisor elapsed was 138.791s including 0.204s cleanup. Receipt status PASS means the declared executable checks completed; it is not original QA08 acceptance.

Sampled whole/temp/durable peaks were 43,755,737 / 27,602,256 / 16,077,614 bytes; aggregate descendant RSS 2,596,503,552 bytes and 10 processes. These are sampled peaks, not final-report sizes. Limits were 256/192/48MiB, 8GiB RSS and 48 processes. Whole accounting includes all retained/new QA08 evidence, harness source, this document and private temporary builds/profile/cache/TMP. Actual acceptance floor16GiB and continuous floor8GiB were retained. Free after was 30,641,217,536 bytes. Receipt has remainingProcesses=[], dependencyLinkAbsent=true, temporaryAbsent=true, released=true and finishedBeforeOfferedHardDeadline=true. The dependency link and private temporary path were also checked absent afterward. FULLRELEASE was sent to21d; no executable work remains allocated.

### Current original-criteria matrix and delivery

| Requirement | Renewed evidence/status | Remaining boundary |
| --- | --- | --- |
| Current source/contract/art workload | MEASURED; default ports/renderer, map4, six sprites; 2,630 before/after hashes match | Synthetic service authority only |
| Production bundles and route/journey transfer | MEASURED; independent inventories and per-route request records | Cache/profile scope explicit above |
| Sustained and covered rendering | MEASURED; 15 × requested5s windows, raw RAF/POST_RENDER and seven inspected PNGs | Desktop finite samples; physical FPS NOT_RUN |
| Navigation, recovery, memory and disposal | MEASURED; 15 cycles, explicit warmup/regression, singleton/identity and release observations | Positive heap growth; no leak-free or cache-internal claim |
| Delayed consumer/static throttling/loss/reconnect | MEASURED; finite FIFO, fresh static profile, three application-message resyncs, 24 serial reconnects | Live-service/TCP loss/storm NOT_RUN |
| Original mid-range mobile30FPS | NOT_RUN | Named physical devices/OS/browser and same live candidate required |
| Original200 principals/four rooms/capacity/operator cap | NOT_RUN by this leaf; separate server owners20f/1fc | Real Auth/C05/server telemetry, room config and operator decision required |
| Reproducibility/failure preservation/release | PASS for declared commands/resource lifecycle; immutable diagnostic retained | Scoped evidence commit and direct Root153 normal serialized publication; owner verifies remote ancestry/postimages afterward and reports that proof separately |

Root153 is the named frontend publisher; no technical review/ACK gate is introduced. This report records source-freeze evidence; publication commit/remote postimage proof is supplied in the delivery handoff after publication, without modifying measured inputs. Original QA08 and all physical/service/product acceptance boundaries remain as stated above.

### Allocated reproduction

Use the existing checkout and retained locked dependencies, with a fresh named21d OFFER and explicit ACCEPT. No dependency installation or other checkout is needed. The supervisor requires clean frozen source, an absent dependency path, actual acceptance timestamp/free bytes and original absolute deadlines. The16GiB floor applies at actual acceptance/start and8GiB continuously; cap/floor/deadline failure does not authorize an extension or deletion of retained failure evidence. Choose a unique run name; outputs are never overwritten.

```sh
QA08_ALLOCATION='<fresh accepted offer ID>' \
QA08_RUN='<new unique run name>' \
QA08_ACCEPTED_AT_UTC='<actual acceptance UTC timestamp>' \
QA08_ACCEPT_FREE_BYTES='<fresh actual-acceptance free bytes>' \
QA08_WORK_DEADLINE_UTC='<original offered work deadline UTC>' \
QA08_HARD_DEADLINE_UTC='<original offered hard deadline UTC>' \
/usr/bin/python3 frontend/e2e/qa08-rendering/allocated-run.py
```

The workflow records exact argv/exits, pins, inventories, native samples and release receipt. Historical commands below describe the older runner and do not authorize bypassing this allocation guard.

## Historical bounded qualification

Original QA08 remains **PARTIAL**. This increment provides reproducible native frontend measurements with synthetic authority; it does not accept the original mobile/capacity task or select a production cap. Physical mid-range mobile qualification, real 200 sockets across four rooms, server reconnect storms and live admission remain **NOT_RUN**. All 112 required tasks and 61 IA entries remain in scope.

## Inputs and ownership

Only new `frontend/e2e/qa08-rendering/**`, this document and `docs/mcr/evidence/frontend-performance/qa08/**` are authored. No production source, configuration, manifests, lockfile, contracts, backend, original QA07 evidence or shared test helpers changed.

The initial source baseline is `7c8da1c4babd2551458320a87747bac576db0d94`. Root assembly is adapted from published `10784b26c2a5e8a544551d0b9ed8951b25b68afe`; FE15 input/production-port assembly from `50d950d79939a539ebe953744287f7a280ce488b`, finally published through `a84885f26b973a5cd49afbb29c86888563265924`. Those files are read-only inputs. SHA-256 manifests beside each measurement identify exact source, public art, C05/C06/core fixtures, harness, build configuration, package/lockfile and specification bytes. No external source code or legacy UI was imported.

The fixture uses canonical Auth session construction, `createProductionPorts`, `createWorldComposition`, default MapScene/Phaser, actual application routes, whole selected-character rendering, Root `sceneData('spatial', 'PARADE', true)` and its existing source resource allowlist. Default production admission remains closed. Fixture HTTP/login/tickets and C05 peer are synthetic, with six fixture entities; only static JS/art requests use the native HTTP server. This measurement does not requalify art, verify visual parity or authorize an art revision for production.

Existing reference-integrated native/metric helpers cover route geometry, resource/cache/decode/loader counts. Existing runtime/movement smokes cover RAF/game cleanup. This harness adds sustained native RAF and actual Phaser POST_RENDER timestamps, interval distributions, browser-exposed heap/DOM snapshots, independent production bundle inventory, delayed receiver and reconnect timings. QA05 retains separate fault/conformance ownership; no shared writer or conformance substitute is introduced.

## Measurement method

The runner builds the actual production `index.html` normally and an independent optimized fixture with explicit Vite mode `qa08`, into its own ignored cache. It inventories raw/gzip/Brotli byte counts and hashes per build output; sums are not initial-route transfer claims. Build outputs are served with actual Brotli when accepted, with compression prepared before browser measurement. Pinned public descriptors and PNG resources are served unchanged, including their uncompressed Content-Length. The native production landing waits for its actual heading and network idle; it is unauthenticated without backend services and is recorded separately from the fixture world.

Native Chrome runs headless on the recorded desktop environment with 360/390/430 by 844 CSS viewports, scale 1, reduced-motion preference for the three main contexts, one repetition per width and five-second requested windows. The separate cold context retains the default motion preference. Other host activity is not isolated; these are finite local observations rather than a benchmark confidence interval. These are not physical phone measurements. Sampling starts only after LIVE plus six real selected-character Phaser sprites. Each raw window records actual start/end, visibility, RAF callback timestamps and actual Phaser `POST_RENDER` timestamps. Rates are callback counts divided by actual elapsed time. They are not GPU presentation rates. Intervals use nearest-rank p50/p95/p99, include outliers, and preserve raw samples. The original 30 FPS budget (33.333ms) is a comparison only; there is no invented performance PASS threshold. Zero covered-world renders produce zero rate and unknown interval percentiles, not a fabricated 0ms frame time.

Scenarios are visible idle, focused keyboard movement through the actual public joystick, full-map-panel coverage, controlled delayed ACK delivery, three serial code-4500 faults followed by the actual Reconnect control, and recovered visible rendering. The delayed receiver services its FIFO every 100ms after two seconds of actual input; this is not a 100ms RTT model or real server saturation. Queue arrivals/deliveries and public replication pending/ACK/phase fields are recorded. The client uses frozen C05 `outgoing_queue_frames=64` as its pending bound (`src/world/network/replication.ts`); this run observes below-bound drain rather than boundary saturation. C05 also retains 262144 outgoing bytes, simulation target 20Hz, delta target 10Hz, room cap 50 and capacity qualification NOT_RUN. These are contract settings, not measured ceilings.

Reconnect reports distinguish node/automation elapsed time from browser fault-to-LIVE phase timestamps. Both include the automated click/retry path; neither isolates intrinsic network transport latency. Three serial recoveries do not model simultaneous reconnects, packet loss, resync/switch conformance, server load or a storm. Singleton checks track observed actual Phaser identity using weak references and active fixture sockets; they do not establish real multiuser behavior.

Cold-network measurement uses a fresh 390px context with CDP latency 100ms, download 1000000 bytes/s and upload 512000 bytes/s. These explicit parameters are not presented as a standard mobile profile. Only actual static fetches are constrained. Startup ends at observed LIVE plus six actual sprites and includes navigation and polling overhead and is not a live login/world-server latency measurement.

Heap samples use CDP Runtime.getHeapUsage, Performance metrics and DOM counters, plus exposed precise legacy performance.memory. Forced GC is explicit before baseline, before disposal and after disposal, never during frame windows. Cross-origin isolation/measureUserAgentSpecificMemory availability is recorded as observed. JS heap excludes full process, GPU and native memory. Retained fixture logs, snapshots, module state and renderer libraries prevent treating post-disposal heap differences as a leak finding or proof of absence. Disposal independently checks no canvas/game/active socket, empty injector queue and source-cache byte release. No process memory capacity is inferred.

## Results

Final measured code candidate: `f9d1af22bb66d8870a5e2442b64fded7fa013bf8`, rebased onto published source `ea9012fb6e4f6447fed5a6fb636b3b3a40991431`. The evidence/document commit changes no measured input bytes. [Raw measurements](../evidence/frontend-performance/qa08/final/measurements.json), [2138 exact input hashes](../evidence/frontend-performance/qa08/final/input-pins.json), [native build/run log](../evidence/frontend-performance/qa08/final-build-native.log) and [validation receipt](../evidence/frontend-performance/qa08/validation.json) are retained. Sampler status is **MEASURED**; task acceptance remains **PARTIAL**.

Environment: native Chrome 156.0.8078.80, Node v24.22.2, Darwin 27.2.0 arm64, Apple M3 Max, 16 logical CPUs and 51,539,607,552 bytes host RAM. Actual renderer: CANVAS. Screenshots [360](../evidence/frontend-performance/qa08/final/world-360.png), [390](../evidence/frontend-performance/qa08/final/world-390.png), [430](../evidence/frontend-performance/qa08/final/world-430.png) show the measured fixture; they do not establish design or physical-device acceptance.

All window values below come from actual callback timestamps. Rates are Hz; percentiles/max are milliseconds. Covered windows retain active RAF while Phaser rendering pauses. No sampled visible interval exceeded the original 33.333ms comparison budget; this finite desktop observation does not qualify mobile FPS.

| Width | Window | Actual ms | RAF Hz | Phaser Hz | Render p50 | p95 | p99 | max |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 360 | idle-visible | 5002.20 | 59.97 | 59.97 | 16.70 | 18.70 | 19.10 | 19.40 |
| 360 | keyboard-moving | 5001.50 | 59.98 | 59.98 | 16.70 | 18.40 | 18.70 | 18.70 |
| 360 | full-panel-covered | 5001.90 | 60.18 | 0.00 | N/A | N/A | N/A | N/A |
| 360 | delayed-ack-consumer | 5000.90 | 59.99 | 59.99 | 16.70 | 17.30 | 17.90 | 18.60 |
| 360 | recovered-visible | 5001.60 | 59.98 | 59.98 | 16.70 | 17.50 | 18.00 | 18.00 |
| 390 | idle-visible | 5002.90 | 59.97 | 59.97 | 16.60 | 17.60 | 18.00 | 18.30 |
| 390 | keyboard-moving | 5001.20 | 59.99 | 59.99 | 16.70 | 17.40 | 18.10 | 18.40 |
| 390 | full-panel-covered | 5002.40 | 60.17 | 0.00 | N/A | N/A | N/A | N/A |
| 390 | delayed-ack-consumer | 5001.00 | 59.99 | 59.99 | 16.70 | 17.40 | 17.70 | 17.80 |
| 390 | recovered-visible | 5000.80 | 59.99 | 59.99 | 16.70 | 17.20 | 17.50 | 17.60 |
| 430 | idle-visible | 5001.60 | 59.98 | 59.98 | 16.60 | 17.50 | 17.80 | 18.10 |
| 430 | keyboard-moving | 5002.60 | 59.97 | 59.97 | 16.70 | 17.30 | 17.60 | 17.70 |
| 430 | full-panel-covered | 5002.10 | 60.17 | 0.00 | N/A | N/A | N/A | N/A |
| 430 | delayed-ack-consumer | 5002.00 | 59.98 | 59.98 | 16.70 | 17.10 | 17.30 | 17.40 |
| 430 | recovered-visible | 5001.30 | 59.98 | 59.98 | 16.70 | 16.80 | 17.30 | 17.90 |

| Width | LIVE + six sprites ms | GC baseline heap bytes | Before disposal bytes | After disposal bytes | Pending / injector peaks | Browser fault-to-LIVE ms (3) |
| --- | ---: | ---: | ---: | ---: | --- | --- |
| 360 | 951.22 | 14,500,432 | 16,520,096 | 15,343,836 | 20 / 20 | 42.20, 47.00, 46.40 |
| 390 | 947.95 | 14,495,052 | 16,502,788 | 15,325,720 | 20 / 20 | 54.80, 45.80, 48.80 |
| 430 | 948.70 | 14,498,560 | 16,518,472 | 15,341,152 | 20 / 20 | 52.10, 48.80, 47.10 |

The controlled receiver produced 112 delayed deliveries: p50 1005.10ms, p95 1854.30ms, p99 1909.70ms, maximum 1954.30ms. This includes FIFO backlog at a 100ms service interval. All pending/injector queues drained to zero; observed peaks were 20, below the incumbent client bound of 64. Automated reconnect elapsed ranged 141.59–154.93ms, separately from the browser phase timings above. No real server capacity or storm ceiling is measured.

All three main viewport contexts retained one observed game, peak one active fixture socket and six sprite objects through recovery. Disposal recorded game/canvas/socket/sprite/injector counts zero and source caches with zero resident bytes/entries and disposed=true. Native JS heap after disposal stayed above its initial forced-GC baseline; the fixture still retains its logs/module graph. Neither that difference nor cleanup counters prove a memory leak or its absence. `measureUserAgentSpecificMemory` was NOT_EXPOSED and crossOriginIsolated=false; precise legacy heap snapshots, CDP heap and DOM counters remain available in raw evidence.

| Build inventory | All emitted JS bytes | Sum gzip JS bytes | Sum Brotli JS bytes |
| --- | ---: | ---: | ---: |
| Actual production | 3,579,934 | 801,365 | 662,715 |
| Optimized synthetic fixture | 3,635,936 | 817,871 | 677,077 |

Vite retains its advisory for chunks above 500kB; no warning threshold, dependency or code-splitting configuration changed. These inventory sums cover all JS outputs, including lazy modules, and are not route transfer sizes. CDP finished-response encodedDataLength sums for each complete world scenario were 2,451,652, 2,451,484, 2,451,484 bytes (360/390/430). They include protocol headers and the observed map-panel/resource journey; cache hits and each request remain individually recorded. They exclude synthetic HTTP/C05 authority. The unrelated favicon 404 is recorded rather than hidden.

The settled production landing observed “One world. Every era.”; its separate recorded transfer sum was 254,317 bytes, without live backend qualification. It is not an authenticated-world startup measurement. The fresh throttled 390px world reached LIVE plus six sprites in 4882.34ms, with 2,451,484 recorded transfer bytes. Its 5000.90ms visible window yielded 59.99Hz Phaser callbacks, p95 17.20ms and p99 17.70ms. This is the explicit static-fetch profile above, not a live network, device or capacity verdict.

Owner checks: fixture TypeScript, deny-warnings lint, three arithmetic/zero-render/actual-window Node selftests, plan integrity and UI-origin guard passed (exit 0), as did independent actual production and optimized fixture builds plus the final native run. The 2138 input files were rehashed without mismatches after the run. No central test discovery, CI composition or production build input was changed; the Node selftest filename intentionally stays outside Vitest’s default test suffix.

## Reproduction and boundaries

Use retained locked dependencies and the recorded native Chrome executable. From repository root with Node 24.22.2 on PATH:

```sh
node --test frontend/e2e/qa08-rendering/metrics.node-selftest.mjs
frontend/node_modules/.bin/tsc -p frontend/e2e/qa08-rendering/tsconfig.json --incremental --tsBuildInfoFile frontend/e2e/qa08-rendering/.cache/fixture.tsbuildinfo
frontend/node_modules/.bin/oxlint --deny-warnings frontend/e2e/qa08-rendering/fixture.tsx frontend/e2e/qa08-rendering/native.mjs frontend/e2e/qa08-rendering/metrics.mjs frontend/e2e/qa08-rendering/metrics.node-selftest.mjs
QA08_RUN=final node frontend/e2e/qa08-rendering/native.mjs
python3 scripts/mcr/check-plan.py
```

Retained logs normalize local checkout prefixes and trailing whitespace; raw numeric samples are unchanged. The runner requires 8GiB free space, uses ephemeral loopback ports and closes its own browser/servers in finally. Evidence output is restricted to a validated run name under the assigned prefix. `QA08_WIDTH`, `QA08_WINDOW_MS`, `QA08_SKIP_COLD` and `QA08_REUSE_BUILD` support diagnostics; the final full run uses none of those shortcuts. `initial/` preserves the historical 390px/1.5s probe; its automation `recoveryMs` field predates precise browser phase timing and is not intrinsic network recovery latency. `qualified/` and `published-base/` are historical pre-correction full diagnostics, despite their directory names and sampler status MEASURED. Their captures exposed missing avatars: the owned static server compressed the pinned character descriptor, changing Content-Length and correctly causing strict runtime rejection. The harness now compresses build outputs only and requires six actual sprites. Only `final/` is used for the reported selected-character workload. Historical production landing samples stopped at the lazy loading heading; final waits for the actual heading and network idle. No production source fix or admission bypass was needed.

No source defect was demonstrated that required extra production ownership. A measured maximum in these finite samples is not a device/service ceiling. Neither a sustainable user count nor a new operator cap can be selected from this fixture. Backend World owner separately confirms that real 200-socket/four-room/storm data is absent; its narrow component and two-client proofs cannot certify capacity. Required remaining qualification must use selected physical devices and the real service candidate with real Auth/C05/server telemetry. This increment does not change original QA08 acceptance, FE41/BE44 prerequisites, release gates or production claims.
