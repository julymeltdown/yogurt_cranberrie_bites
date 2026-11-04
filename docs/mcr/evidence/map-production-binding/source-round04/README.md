# Actual corrected ART registration: source preparation

Current status: SOURCE_QUALIFIED_NATIVE_NOT_RUN at `575726966fd8b52d04e9ba1ec4d8be3129286071`. The following preparation section records the original NOT_RUN state; the actual outcome below supersedes its pending-registration and no-offer statements.

Preparation status: NOT_RUN. The 115 source03 cases remain qualified at their original source revision; five new actual-resource cases bring this round's plan to 120 tests in 18 files. No new native, live admission or visual acceptance is claimed.

Corrected ART source is `8fcd1287496aedbde12ef0d2e426855f8f8ec4d6`, carried as ten read-only shared public files at `d10bc07d` under Root153's explicit custody/intake routing. No ART document was copied or overwritten; no generator, raster export or capture ran. Fresh stat of the final ten committed postimages totals 6,782,808 bytes (478 bytes above the artist's earlier at-release 6,782,330-byte count, following its post-release README prose). The actual final input size is charged in this round's carry, together with all previous owned source/evidence and new metadata. No edits to those public files are authorized to this consumer.

The owned `map4-service-v2/layout-source.json` is a byte-exact textual source snapshot of the 45,370-byte artist layout; `resources.json` records its declared SHA-256 `418af3bf14ee2e8b79eeb2b038f64b48713de17967993aa4e7fa3ce22c8d26b9` and the actual three scene descriptors. It does not invent a presentation digest. The candidate accessor remains null until allocated deterministic registration runs.

The new source helper `map-presentation-v2/register.mjs` reads the actual ten public postimages and compares them with the committed artist source, validates the nine immutable original inputs through Git object reads, checks raw layout and scene bytes/hash/dimensions, compares the owned layout snapshot, and verifies the corrected provenance pin. It imports the actual closed runtime semantic constants and retained style, then computes the canonical layout/assets/semantics/style/font digest. Only `--write` authors the two exact NEW-v2 candidate source postimages: `registration.ts` and `current.ts`. No public ART or old v1 source is modified. `--check` independently verifies those deterministic source postimages without writing them.

Within a fresh explicitly accepted lease, the exact planned sequence from `frontend/` is:

```sh
# Supply only the newly offered lease ID and its actual accepted deadline.
python3 ../docs/mcr/evidence/map-production-binding/source-round04/supervisor.py registration
# Commit only the generated registration.ts/current.ts in the existing checkout.
python3 ../docs/mcr/evidence/map-production-binding/source-round04/supervisor.py tests types lint registration-check
```

Both invocations require the original same `MCR_MAP_SOURCE_LEASE` and `MCR_MAP_SOURCE_DEADLINE`; they do not extend or renew a clock. The supervisor uses the absolute retained Node24.22.2 path and read-only dependency lock, runner config loader, private TMP/cache/build-info and guarded absent-only dependency link. Each invocation cleans its own handles/link/TMP, while final explicit FULLRELEASE occurs only after all jobs, owner source fingerprints and accounting finish. Scoped diagnostic fixes/retries retain failed logs within the original accepted limits. No offer or acceptance exists yet for this round.

Requested envelope: at most 360 seconds total/300 active, at least 60 cleanup; each child at most 180 seconds and one Node child at a time. Additional output/TMP/source materialization stays below 32MiB, whole carry/output below 64MiB; fresh free space at least 16GiB and continuous floor 8GiB. The ten shared ART files, 45KB metadata snapshot and all earlier source/evidence count toward whole carry. No install, build, browser, server, native capture, new checkout/worktree, ART generation or private C03 processing is requested.

Initial readiness stat reports 8,621,518 whole carry bytes and 16,827,580,416 free bytes. That free-space sample is below the required 17,179,869,184-byte (16GiB) fresh-start floor, so it is not an ACCEPT or permission to start. Resource21d receives the actual numbers and must provide a usable fresh offer after the floor is met. No peer cleanup or implicit lower floor is authorized.

The five new cases exercise actual resource/digest/layout consistency, each real selected-era loader path with fake decoded handles only, immutable World/map authority, real four canonical display mappings and v1 compatibility. Browser PNG decode, actual label/font metrics, scroll/focus/layout, World pixels/camera/lifetime and visual convergence remain the separate reserved Root canonical v2 gate after source qualification. The artist's saved corrected images were viewed in place; the scene remains more open/schematic than the target despite denser planting and clearer era cues. Those limitations remain active rather than being hidden by a passing source check.

## Actual allocated outcome

Fresh lease `FE19-MAP-V2-SOURCE04-202611041344Z` was accepted at 2026-11-04T13:44:42Z, with active deadline 13:49:42Z and hard deadline 13:50:42Z. Actual preflight was clean `e4967ef0ed83b313788b6ed7e47f3f47ddfdc460`, 30,783,033,344 free bytes, 8,622,102 accounted carry bytes, exact 6,782,808-byte ten-file ART carry, and absent owned link/TMP. The small carry increase from the earlier readiness count is retained documentation. No deadline was extended.

Registration `--write` exited 0 and committed only `registration.ts` and `current.ts` at `b11512f4`. The runtime accessor now returns the immutable real-art model. The independent outer Python guardian stopped the first test attempt before any reported cases at 13:45:40Z: default Vitest fork plus transform helper exceeded the four-process ceiling (observed five including guardian), with 629,211,136 bytes RSS. `tests-01.log` and `guardian.jsonl` retain that failed/aborted attempt; it is not a successful test run. The guardian terminated its captured descendants; because the supervisor was also stopped, its owned link/TMP were then explicitly cleaned. The first aborted stage did not retain a complete individual descendant PID receipt; only its supervisor PID and overall peak were recorded. Final scoped process checks are empty. This limitation is explicit in `full-release.json`.

Developer fix `575726966fd8b52d04e9ba1ec4d8be3129286071` selects a single threaded Vitest pool and adds supervisor SIGTERM cleanup. The original same lease/deadlines governed the retry. The outer guardian independently sampled descendant process count, RSS, additional bytes, disk floor and active deadline every 300ms; it kept a hard stop separate from the per-job supervisor. Retry peaks were four processes and 631,078,912 bytes RSS. Registration and Git custody stages and the successful source stages ran sequentially. No concurrent Node root job was started.

Actual commands from `frontend/`, using the accepted lease/deadline environment:

```sh
python3 ../docs/mcr/evidence/map-production-binding/source-round04/supervisor.py registration
# Explicit two-file root-cwd git commit b11512f4, then the process-cap fix commit 57572696.
python3 ../docs/mcr/evidence/map-production-binding/source-round04/supervisor.py tests types lint registration-check
```

The successful second invocation produced **120 passing tests in 18 files**, scoped TypeScript exit 0, scoped oxlint exit 0 and deterministic registration `--check` exit 0. `attempts.jsonl` records exact absolute tool argv, source logs, PIDs and times. `source-pins.json` records 35 actual consumer/shared/ART postimages at qualified source `57572696`, SHA-256 `b9cc2df98183e176a12b7cfa5a9b3ee7fde9efee858441ac3dc43eef5ccb522d`. Original v1 and source03 paths have no diff from the accepted base. Public ART remained byte-exact `8fcd1287`; all nine original inputs were checked against Git objects. No generator, export, build, browser, server or native job ran.

The registered presentation is `map4-reference-composition-v1`, hash `sha256:9fcddca91030aaff0f3c3efaa1018fdeddb4708c56c9eea80024deb0bee9f522`. This digest binds actual raw-layout descriptor, all three scene descriptors and runtime semantics plus the retained immutable style/font/icon values. It does not change original Map4, theme, geometry or avatar identity.

FULL_RELEASED at **2026-11-04T13:47:36.175301Z**. All 13 retained known PIDs/PGIDs were absent, scoped process scan empty, owned dependency link and private TMP/cache absent. Retained whole carry before the small release receipt was 8,641,514 bytes; all-attempt observed additional peak was 10,064,425 bytes, within 32MiB, and accounted whole peak below 19MiB, within 64MiB. Final free space was 30,903,795,712 bytes. Zero browser/server/native/ART generation jobs and no lease remain held. Post-release source documentation and committed custody may continue; no new validation execution is authorized by this released slot.

Source qualification does not certify real PNG decode, font/label metrics, World RGBA/camera/lifetime invariance, 360/390/430/short route/focus controls, physical device behavior, production admission or reference fidelity. Root owns the next canonical native gate; the full visual gap remains active. Artist correction recipe `50adff5f` remains AUTHORED_NOT_BUILT and is not substituted into this qualified candidate.
