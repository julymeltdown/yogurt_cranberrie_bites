# Community runtime clock

## Durable assignment

Objective: finish the original Community clock remainder with a measured supported-runtime input decoder, trusted production profile factory, production ports wiring, original C03 lifetime binding, developer-owned validation/fixes, documentation and scoped publication. The earlier Admin goal remains blocked on its genuine serving inputs; this assignment does not reset or complete it.

Completion requires meaningful invalid-input, unsupported-runtime, expiry, pending-response withdrawal and cleanup checks; actual browser/server measurements and production binding verification; committed exact-path handoff and canonical publication receipt. QA08 performance and physical-provider availability do not gate clock authoring. Synthetic tests do not satisfy measured-runtime completion.

Current status: `IN_PROGRESS`; **source qualification is complete**. All 66 tests and full types passed at the recorded cut; properly admitted final lint and syntax passed as recorded below. Genuine browser/server measurement and production assembly are `NOT_RUN`; scoped publication is pending. No production measurement is checked in and no numeric defaults are enabled. Source delivery retains these runtime limits explicitly.

## Source and ownership

Base: `2492e35bb9fb61380bf97fa936983ec16c0722f4`. Exclusive branch: `codex/mcr-fe-community-clock-runtime`.

Owned new paths: `frontend/src/app/community-clock-runtime/**`, this document and `docs/mcr/evidence/community-clock-runtime/**`. Exact shared hunks: the new clock import, creation, auth return field and cleanup in `frontend/src/app/ports.ts`; optional runtime lifetime/qualification integration only in `frontend/src/app/community-adoption/bootstrap.ts`. Preserve other ports consumers and the separately owned FE41 policy hunks. No main, Auth, authority/timeline, WorldShell, provider, contract, manifest or lock changes.

## Trusted input seam

`createProductionCommunityClock(eventId)` reads the compiled `VITE_MCR_COMMUNITY_CLOCK_INPUT` string. `createCommunityClockRuntime(raw, environment)` and the closed types in `input.ts` are the explicit implementation/test seam. Missing, malformed or unsupported inputs produce no profile. The production auth return exposes `communityClock` only when the factory succeeds, and production ports disposal withdraws the factory first.

Root owns a **separate later assembly** with a new buildId and exact input digest catalog. The frozen identity phase at `680ca0e71` is not amended. Input is never accepted from URL parameters, localStorage, a Bootstrap field or user roles. The evidence digest identifies captured bytes; it is not a signature or an authorization grant. Trusted release input custody supplies provenance.

The exact JSON object has only `current` and `measurement`. `current` and `measurement.binding` must independently match all of:

| Field | Required representation |
| --- | --- |
| candidate | Lowercase 40-character source commit |
| environmentId | Nonempty environment identifier, at most 128 characters |
| eventId | Canonical UUID |
| origin | Exact HTTPS origin, or loopback HTTP origin |
| bootstrapContractSha256 | Lowercase 64-character digest of current C03 contract bytes |
| concertBinarySha256 | Lowercase 64-character serving binary digest |
| serverClockSource | Lowercase 40-character producer source revision |

`measurement` contains exactly `kind: MCR_C03_CLOCK_MEASUREMENT_V1`, `classification: MEASURED_RUNTIME`, `evidenceSha256`, `binding`, `browser`, `samples`, `observationsMs`, and `uninterrupted: true`. `browser` contains exactly actual `userAgent`, `platform`, and boolean `crossOriginIsolated`. Each of 3–64 ordered disjoint samples contains exactly finite nonnegative `requestStartedMs`, `responseReceivedMs`, and actual decoded `serverTime`. The 3–4096 performance observations cover the whole capture. Raw input is bounded to 512 KiB. Everything retained is frozen. Synthetic test objects are clearly marked in the test source and are never a release artifact.

## Bounds and their limits

Current producer source sets `completed_at = Utc::now()` inside the original request (`backend/services/concert-service/src/bootstrap/mod.rs`) and serializes `record.completed_at` with `SecondsFormat::Secs` (`bootstrap/release.rs`). This establishes an in-request anchor and a 1000 ms wire quantum. It establishes no general hardware drift bound.

For samples i,j with server delta D > 2000 ms, the decoder uses the full request intervals: minimum browser delta is `start[j]-receipt[i]`, maximum is `receipt[j]-start[i]`. The symmetric rate envelope is the maximum of 1, `(D+2000)/minimum`, and `maximum/(D-2000)`. It rejects a rate above 2. The two endpoint wire quanta explain 2000; no RTT midpoint is used. Exercised duration supplies finite maximum age, maximum observed gap supplies the discontinuity ceiling, and maximum observed RTT supplies a conservative additional uncertainty envelope. Existing authority separately adds the actual wire quantum.

These are **conditional measurements for the recorded environment and finite window**, not a proof of future platform-wide oscillator accuracy. [W3C High Resolution Time](https://www.w3.org/TR/hr-time-3/) defines the performance clock and privacy coarsening; it does not supply the application's finite rate bound. A measurement artifact must disclose environment, actual observations, provenance, assumptions and calculated limits. No old synthetic 30 s / 1000 ms / 1.01 / 5 s profile is promoted.

Server validity ends at the last measured server timestamp plus exercised duration. At the SAME original decoded C03 receipt, runtime qualification rejects a server timestamp older than the last sample, changed precision, wrong event or expired evidence. It clamps authority age to `(serverValidUntil-serverTime-1000-extraUncertainty)/rate`. This ceiling applies from the original request start. A profile also expires after one exercised duration of its own uninterrupted runtime. New requests cannot extend either lifetime. No device calendar is used to assert freshness.

## Lifetime and retained authority

The existing generic timeline supplies performance observation identities and discontinuity handling. The factory withdraws on origin/browser mismatch, backward/nonfinite clock observations, changed performance origin, excessive gap, hidden/frozen/resumed/page-hidden/BFCache restoration or disposal. It does not start another product Bootstrap request.

`beginCommunityBootstrap` optionally subscribes to this runtime before dispatch. It checks the runtime before timing creation, at decoded completion and on retained reads. It applies `runtime.qualify` to that exact source before the existing authority constructor. Callback failure and synchronous subscription withdrawal fail closed. A C03 that arrives after runtime withdrawal cannot create authority. Existing privacy and AbortSignal cleanup and development-only TEST_ONLY profiles remain supported.

## Validation and handoff

New tests exercise the decoder and original binding using explicitly synthetic unit data. Fresh finite 21d offers are required for test/type/lint/browser workloads. The source runner must preserve retained Admin/Gift/FE39/historical evidence and nonshrinking WHOLE/NEW accounting. Actual measurement and production browser checks use an existing authorized serving cohort; no new stack, install or build is implied.

`__tests__/measurement.native.ts` is the prepared actual-cohort capture driver, not executed by source tests. Its private config has exactly `binding`, absolute `participantFixture`, explicit increasing `sampleOffsetsMs` (3–64 values, at most 60000 ms) and `observationIntervalMs`. The supplied ordinary participant fixture provides `userId`, `eventId` and an existing access token; the driver does not mint credentials. It visits the actual origin and issues only authenticated GETs to the canonical C03 endpoint. Canonical contract/hash/scope validation finishes before each receipt observation in the same browser clock. The full interval includes driver round-trip overhead, making the bounds conservative. Visibility and lifecycle observations remain active throughout capture. Only nonsecret timing/binding/fingerprint evidence, its digest, the measured record and derived limits are written; traces, videos, screenshots and saved browser state are disabled. Root separately supplies current assembly bindings and catalogs the exact artifacts. A genuine capture alone does not prove production ports joined those bytes.

The frozen handoff will include exact source commit, old-file preimages and hunks, closed exports/schema, strict readset hashes, measured cohort and derived limits if available, command exits, resource release, exact paths and publication routing. A source PASS alone must not be reported as a measured production clock PASS.

First source round: `b2095630e9f1b6b4422511776b1c1cc4d44a6590`, offer `COMMUNITY-CLOCK-SOURCE-20261104T2140Z`. Vitest: 48 passed / 18 failed / 66 total, with one resulting unhandled rejection. The 18 failures stopped at canonical Auth admission before C03 because the historical fetch fixture returned 503 for the newly required admission read. Types/lint/syntax were not run after that failure. Both resource controls passed; actual FULL was `2026-11-04T21:11:57.785130Z`, with no owned processes, dependency link, temporary output or listener retained. Immutable evidence: `docs/mcr/evidence/community-clock-runtime/source-01/`.

Root's additional exact test-fixture grant permits only the closed synthetic ADMITTED GET response in `frontend/src/app/community-adoption/__tests__/fixtures.ts`, before original C03 request accounting. Production Auth, session logic, the loader and held/invalid response modes remain unchanged. The affected tests require a fresh source successor offer.

Second source round: `7548cc940dae30e97dd656eae80bd4e880a6485c`, offer `COMMUNITY-CLOCK-RETRY-SOURCE-20261104T2120Z`. All **66 tests passed**, and full nonincremental TypeScript checking passed. Scoped deny-warning lint found only `no-control-regex` in the intentional ASCII control rejection; syntax remained not run after that failure. The replacement uses equivalent character-code checks, preserving rejection of codes 0–31 and 127. The passed suites are retained at their immutable source/readset and will not be replayed for this expression-only lint repair. Actual FULL was `2026-11-04T21:20:17.431900Z`, with both controls passing and all owned resources released. Evidence: `docs/mcr/evidence/community-clock-runtime/source-02/`. Lint and syntax still require a fresh short offer.

Cleanup correction: the short `2125Z` offer accepted at `21:28:35Z` stopped in preflight (executor exit 1 in 0.723 seconds) before any check, guardian or dependency link. A 1,362,620-byte Node compilation cache from source-02 was found under `frontend/docs/.../source-02/work/`, because a relative TMPDIR was interpreted from the child frontend directory. The old guard charged these bytes as source growth/OTHER, but its temporary-absence field covered only the intended root prefix and was incomplete. Cache identity/digest and removal are recorded in `source-03-preflight/release.json`; immutable old receipts remain unchanged. Only that derived owned cache was deleted. The runner now resolves its output prefix absolutely, pins/disables Node compilation caching inside the absolute work prefix, and checks for misdirected cache on release. Historical WHOLE/NEW high-water accounting is preserved despite cleanup. Lint and syntax still need a distinct fresh offer; the passed product tests and full types remain at the recorded cut.

Final computational checks: source `7b8f77152a703b97a592ce8320af6e1c663d1e0c`, evidence `source-04/`. Scoped deny-warning lint and Python syntax both exited 0. Absolute temporary-path cleanup, both live controls, process reaping, link removal and port rebind passed. Actual FULL: `2026-11-04T23:14:50.435935Z`; peak 5 processes / 158023680 bytes RSS / 2482999296 bytes WHOLE / 120725504 bytes NEW. **Resource qualification is rejected:** the owner accepted at `23:14:46Z` after the offer's `22:30Z` expiry. The raw receipts are preserved with an explicit classification record. These computational passes must not be described as a valid resource-admitted run. The runner now requires the literal timezone-aware offer expiry and matching source pin, and rejects expired admission before preflight or any validation child. That final guard amendment requires fresh syntax qualification. The allocator's account-limit failure was observed and reported once; no failed turn or expired offer is treated as a lease.

The source-04 computational result did not satisfy final source qualification. The following fresh round closes that requirement without replaying the passed suites.

Final admitted round: `e25cdc21f6121ee8464fd3da9de1c1c2fcfec4db`, offer `COMMUNITY-CLOCK-EXPIRY-QUALIFICATION-20261104T2325Z`, literal expiry `2026-11-04T23:55:00Z`. Actual ACCEPT was `23:26:04Z`; actual FULL was `2026-11-04T23:26:06.518951Z`. Scoped deny-warning lint and Python AST syntax both exited 0. Both independently running controls passed. Peak: 5 processes / 155615232 bytes RSS / 2487570432 bytes WHOLE / 125960192 bytes NEW. Minimum free space: 17425666048 bytes. All owned descendants, dependency links, intended/misdirected temporary output and listener were released, with port rebind confirmed. FULL was sent directly to allocator 21d and successor art231 before documentation. The first release message's ACCEPT typo was immediately corrected to the literal control timestamp above. Immutable evidence: `docs/mcr/evidence/community-clock-runtime/source-05/`. The prior expired-offer classification and cleanup correction remain preserved.

Remaining delivery: scoped publication, actual existing-cohort measurement and the separate Root build-input join. The 66 tests and full types remain valid at their recorded source/readset; the subsequent equivalent ASCII-expression lint repair and runner-only changes are documented above. Actual production clock qualification remains `NOT_RUN` and no release input/profile is supplied. The original Admin goal remains blocked and unchanged.
