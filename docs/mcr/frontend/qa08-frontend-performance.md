# QA08 bounded frontend performance measurement

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
