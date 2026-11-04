# FE-COMMUNITY-AUTHORITY-01 — scoped time and public places

Status: SOURCE_QUALIFIED for the original leaf and Root-transferred binding.
The full assignment remains IN_PROGRESS. The combined 53 tests, full frontend
TypeScript and scoped lint passed in a fresh accepted source allocation.
Root has prepared the targeted intake patch; intake and publication are pending. The native interaction matrix passed,
and its settled positive discovery control follow-up passed. The historical
frame styling is not new reference visual approval. Full FE21, FE22 and original
integration acceptance remain open.

Owner: `01a1a53d-36b8-7522-ae11-646f5e6f0b30`.

## Integration continuation — source qualified, native pending

Root153 explicitly transferred the following hunks after the pure-leaf handoff:
BootstrapBoundary original-request timing/result and the CommunityFeatures prop
in `WorldShell.tsx`; the existing discovery pair, CommunityEntryProvider,
release/access/context and Party creation composition in `CommunityFeatures.tsx`;
one optional typed `ShellPorts.communityClock` field in `ports.ts`; and existing
`communityEntryPanels` additions in `router.tsx`. New
`frontend/src/app/community-adoption/**` is assigned to this owner. Root stops
those hunks until the committed handoff; PanelFrame/header/footer ownership
remains separate. Every other existing-file restriction below still applies.

The exact shared-hunk baseline is Root
`32fed3eb41e2b57783cebb78e4aff11d3fa087ba`. Merge `fd73a55e` preserves the qualified
`71de884c` history while taking **all non-owned baseline bytes exactly from
32fed3**. The existing PanelFrame/PanelScreen region and the entire reviewed/static
owner-portrait renderer branch were compared byte for byte and are unchanged.

The continuation captures start before the original `privacy.run` /
`ports.bootstrap.load`, then decoded receipt after await. It owns the original
request signal and privacy cleanup until release; it neither adds a loader nor
uses the short-lived `privacy.run` signal as a post-receipt lifetime. StrictMode
discovery-owner replay does not dispose that parent timing. Canonical failure,
original abort, policy replacement, unmount and clock withdrawal are fenced.

`communityClock` has no production default. A configured profile must declare
`QUALIFIED_RUNTIME` or `TEST_ONLY` and an evidence reference; production builds
reject `TEST_ONLY`. Actual main remains unchanged and supplies no profile.
Absent time disables Community action releases and mutation projections; Party
browsing retains its read capability, while time-dependent Gifts stays
unavailable. Existing recovery-journal code is reused once per discovery owner;
unavailable storage disables pickup commands. Missing rally, private meeting,
media-upload and reviewed pickup-place producers stay absent. No map revision
is promoted to a meeting/place revision or arrival permission.

The continuation's focused source checks passed at `25ea81e0` (53 tests,
full TypeScript, lint and native driver syntax); the later native guardian
and driver publication/layout assertions are authored but **NOT_RUN**.
The controlled native run requires a fresh browser allocation. Native entrypoint:
`/src/app/community-adoption/__tests__/native/preview.html?entry=/meet&scenario=world-only`.
Its explicitly `TEST_ONLY` scenarios cover canonical World-only denial, absent
policy, virtual-only places, and a synthetic physical positive fixture. They
use the actual default router/providers/Phaser runtime and selected character
pack, with synthetic HTTP/C05 and an unmeasured test clock envelope. Neither
the prior 39-test leaf pass nor Root's Feed/FE41 freeze qualifies this new binding.

Checkout: `/Users/lhs/Desktop/mcr-worktrees/fe-profile-preferences`.
Branch: `codex/mcr-fe-community-authority`.
Base: `654974128a27c948a449fa9e58f466b8e612361d`, fetched from published master.
The clean old `codex/mcr-fe-FE34` branch is preserved at
`f98f9383dfdc364e95ac4aaf930c00f3273856a0`. The retired managed d68a checkout
and shared integration checkout are not used by this assignment. The prior
FE34 goal was complete; the new explicit unbudgeted goal is active through
implementation, developer validation/fixes, documentation, committed handoff,
Root wiring/native qualification, publication and fresh remote readback.

## Exact ownership

This owner writes only:

- `frontend/src/app/community-authority/**`
- `frontend/src/app/community-adoption/**`
- The four existing-file hunk transfers described above, against Root `32fed3`
- `docs/mcr/frontend/community-authority.md`
- `docs/mcr/evidence/community-authority/**`

Outside the four explicit hunk transfers, Root153 retains existing Auth/session,
root/provider, Community/Meeting entrypoint/bridge/router/status, character/media,
per-screen stylesheet and backend contract ownership. The leaf creates no request, Auth,
runtime, socket, query client, roster, ticket claim, operator role or meeting
permission. Source is newly authored; no external copied logic or dependency
change is introduced. `docs/source/` remains untouched.

## Existing contracts and exact proposed exports

`MeetingClockPort` remains read-only and unchanged: `read()` returns
`{scope,nowMs}` or null, with `subscribe`. `CommunityEntryRelease.places` accepts
scoped existing authoring choices with `id` and `label`. The new entry exports:

```ts
createBrowserAuthorityTimeline(maxObservationGapMs: number, signal: AbortSignal): AuthorityTimeline
createAuthorityTimeline(options: TimelineOptions): AuthorityTimeline
createCommunityAuthority(options: CommunityAuthorityOptions): CommunityAuthority
```

The generic timeline seam is for an explicitly qualified monotonic environment
and deterministic tests. The browser adapter uses `performance.now()` and its
unchanged `performance.timeOrigin` fingerprint. It starts a bounded observation
watchdog and listens for the original request/release abort, hidden visibility, pagehide, BFCache restoration,
freeze and resume. Interruption, backward/nonfinite time, changed origin or an
excessive observation gap permanently withdraws the timeline. A new canonical
request and timeline are required after withdrawal. The leaf borrows its
timeline; the capture owner disposes that timeline separately at teardown.

Root must create the timeline **before canonical request dispatch**, capture
`requestStarted` immediately before the existing canonical load, and capture
`responseReceived` after it resolves. Capturing after asynchronous decoding is
safe because it widens the request interval; inventing a request start from the
receipt or replaying cached Bootstrap data is unsafe. Observations are frozen,
process-local objects; copies, foreign timelines and reversed observations are
rejected. A fresh observation attached to an old Bootstrap is not qualification.

```ts
// Root-owned integration outline, not applied by this owner.
const timeline = createBrowserAuthorityTimeline(qualifiedObservationGapMs, signal)
const requestStarted = timeline.capture()
let bootstrap
try { bootstrap = await canonicalExistingBootstrapLoad(originalScope, signal) }
catch (error) { timeline.dispose(); throw error }
const responseReceived = timeline.capture()
// Both observations must exist, and privacyScope must be captured for THIS
// request, not recaptured afterward to relabel older data.
const authority = requestStarted && responseReceived
  ? createCommunityAuthority({
      bootstrap, scope: capturedPrivacyScope, privacy: canonicalAuth.privacy, signal,
      timeline, requestStarted, responseReceived,
      qualification: { maxAgeMs, uncertaintyMs, maxElapsedRate },
    }) : null
if (!authority) timeline.dispose()
// Borrow authority.clock and authority.places on the original scoped release.
// Release teardown: authority.dispose(); timeline.dispose().
```

The example's limit names are required qualification inputs, not established
platform accuracy values. The source does not silently choose a drift/resolution
bound. The Root implementation owner must supply and document a supported
environment's uncertainty, elapsed-rate envelope and bounded anchor age. Missing
or invalid limits fail closed. No new product permission decision is implied.

Root153 confirmed that it owns all functional consumers and canonical Auth
timing capture. The planned capture is inside the existing `loadBootstrap`,
before its request and after decoded receipt, using the original privacy scope
and request abort signal. That existing-file wiring is still pending and is not
part of these source commits. No other active Meet application writer owns it.

There is currently **no production-qualified clock profile**. For a future
controlled default-App native test only, Root proposed `TEST_ONLY` values:
`maxAgeMs: 30000`, `uncertaintyMs: 1000`, `maxElapsedRate: 1.01`, and
`maxObservationGapMs: 5000`. These are synthetic test inputs, not measured
platform guarantees, production defaults or permission to enable mutations.
The actual provider/profile must remain unavailable when qualification is
missing. Backend expiry and permission checks remain authoritative.

## Interval and expiry semantics

Let `S` be the decoded C03 server timestamp, `a` the request-start performance
observation, `b` its receipt, and `p` the current observation. Server generation
is somewhere inside `[a,b]`. For rate envelope `r >= 1` and combined uncertainty
`u` (explicit clock uncertainty plus the timestamp's decimal precision), expose:

```text
lower = max(0, floor(S - u + (p - b) / r))
upper = ceil(S + u + (p - a) * r)
roundTrip = b - a
clock.read().nowMs = upper
```

`readInterval()` exposes the scope and both bounds for qualification evidence.
The full RTT remains in the upper bound; no midpoint or receipt-only estimate
is treated as expiry authority. Decimal timestamp precision is conservatively
included in both directions. Nonfinite/overflow results, unknown observation
age, maximum request/anchor age, scope/policy change, abort and teardown withdraw
all projections and notify every subscriber. A timer notifies maximum-age
withdrawal even without consumer reads. Withdrawal never reopens under a freshly
captured identity; Root must deliver a new qualified release.

The interval is conditional on its qualified elapsed-rate/resolution envelope
and the server timestamp being generated within the canonical request. Arbitrary
server calendar changes, an unreported platform clock defect or unobserved OS
suspension cannot be proven safe by performance observations alone. This is not
a distributed clock synchronization guarantee. `Date.parse` converts the server
timestamp; `Date.now()` is never consulted for authority. See the primary
[W3C High Resolution Time specification](https://www.w3.org/TR/hr-time-3/) for
monotonic-time and clock-resolution semantics.

## Public-place projection and physical-arrival boundary

The input must be the immutable decoded C03 `ShellBootstrap` with its original
document and matching event/participant binding. The leaf uses the existing C03
decoder and Place schema, retaining exact canonical ID, label, kind,
verification/closure/accessibility data and provenance. `id` is an alias of
`place_id`, never a synthesized landmark. All returned arrays and records are
frozen. Source metadata retains actual map ID/revision/hash; that map revision
is **not** a per-place revision or public catalog revision.

`publicPlaces.read()` returns all actual public map facts, including virtual,
unverified and closed landmarks. `places.read()` returns only open, verified
PHYSICAL choices with reviewed provenance and a safe HTTPS reference, compatible
with Community's minimal authoring port. Virtual concepts remain present only
as virtual facts and cannot become a physical meeting choice. Empty choices are
truthful and leave submission disabled in the existing consumer.

Neither port grants physical arrival, an audience, a beacon revision/expiry,
private meeting permission, rally or pickup authority. No host is invented as a
member, and no operator/ticket field is projected. Photo permission is preserved
as source data; a reviewed place does not grant photo upload/display rights.

The current canonical Bootstrap is World-only. A valid clock or public place
does not upgrade its capabilities: Root must preserve existing unavailable
Meet/mutation behavior. The leaf cannot fill missing `MeetingAuthorityPort`,
per-place revision or authorized audience/meeting sources. Root retains the
existing Meeting places controller and entrypoint wiring outside the explicit
Community hunk transfer.

## Validation plan and evidence limits

Passing tests exercise full-RTT and precision/rate bounds, calendar independence,
unknown/copied/reversed timing, discontinuity/watchdog/lifecycle withdrawal,
identity/event/policy/abort/disposal, maximum-age notification, immutable actual
C03 binding, exact virtual provenance and reviewed/closed/unsafe physical
choices. Positive fixtures use the real C03 Bootstrap decoder and recomputed
asset hashes. Test-only synthetic places do not establish actual venue review.

Allocation `COMMUNITY-AUTHORITY-SOURCE-A-202611041402Z` was accepted at
`2026-11-04T14:04:37Z`, with the original work deadline `14:06:37Z` and hard
cleanup deadline `14:07:37Z`. The qualified source is
`25f473cf572a160c431f69f414e6cb51f7515776`, descending from source preparation
`61ed5cf3`, request-lifetime fix `1b0a1f43`, contract record `c6878fdb`, independent
guardian `85d6c327`, and the guardian's prelaunch object-ID parser fix `26fc8149`.
The guardian is an owned source-validation helper, not application runtime code.

| Check | Actual result |
| --- | --- |
| Focused Vitest, runner config, one thread worker, cache disabled | 39 tests in 3 files passed, exit 0 |
| Full frontend TypeScript, `--noEmit --incremental false` | exit 0 |
| Oxlint on `src/app/community-authority` | exit 0 |
| Patch integrity | `git diff --check`, exit 0 |
| Default-shell/native, real service, multiuser and device | NOT_RUN |

The first attempt's 39 tests passed, then TypeScript reported `TS2542` in the
precision test: `structuredClone` retained the document's read-only index type.
The fix makes that synthetic test copy explicitly mutable; application behavior
did not change. Both attempts and their exact commands/exit codes are retained
under `docs/mcr/evidence/community-authority/source01/` and `source02/`. The retry
completed at `14:05:55.643811Z` on the original lease clock. No failed attempt is
discarded or represented as a pass.

The retained logs are stored as deterministic `.log.gz` archives. Runtime reports
keep their original `.log` names; append `.gz` when reading the stored files.
`log-storage.json` records raw and archive hashes, verified against the exact
`71de884c` handoff. Decompression preserves every original byte, including the
Vitest trailing blank lines that otherwise fail whole-candidate patch checking.

Runtime pins: absolute Node `24.22.2`, binary SHA-256
`b6fb419f0f43bb2fbca273700294dd2f3b685a64b60e8417c295f47646a70e4a`;
both retained lockfiles SHA-256
`47ca44459819ad0e887d2f4f93394fdf3b1053b8f45edc9ceb0236981feee1f6`.
Local/root package hashes and required runtime package versions matched before
acceptance. The pre-existing local physical `node_modules` directory was
explicitly read-only, preserved, and outside this owner's cleanup authority.
No install, dependency copying/link/swap, build, browser, server or database ran.
Vite used the private runner-config/cache/TMP prefix. The earlier assumption
that no dependency directory existed was corrected by actual `lstat` before
acceptance; it was never removed.

The independent guardian sampled at 200ms. Across both attempts the observed
peaks were 4 total PIDs including supervisor, guardian, sampler and Node CLI;
1,454,899,200 bytes aggregate RSS; 4,294,384 bytes whole charged footprint;
1,363,968 bytes private temporary data; and 16,384 bytes new durable output.
Whole accounting includes owned source, retained prior evidence, private Git
metadata/source objects and failed-attempt output. Minimum observed free disk
was 29,210,710,016 bytes. Full release at `14:07:03.518022Z` verified no owned
live processes or temporary/cache prefixes; see `source-release.json`. Retained
logs are bounded evidence, not a retained source slot. Root's separate 505-test
baseline is not evidence for this leaf.

The qualified source handoff goes directly to Root, which owns existing-file
integration. The assigned developer still owns actual default-shell
interaction/negative/privacy/arrival qualification in a separate accepted native
allocation. That proof must distinguish actual World-only unavailable states
from any explicitly synthetic positive fixture, check original-scope/abort/time
withdrawal, retain canonical virtual provenance, and show that a clock/place
projection alone enables no Meet mutation or physical-arrival claim. Real
service, multiuser, device and full FE21/FE22 acceptance remain open until their
own original runtime requirements execute.

### Integration source qualification — 4 November 2026

Source `25ea81e0507add978ad3ba937834e8f8c45d53c7` passed 53 tests across four files, full frontend TypeScript (`noEmit`, incremental disabled), scoped oxlint, and native driver syntax checking under `FE-COMMUNITY-ADOPTION-SOURCE-A-202611041440Z`. Attempt 1 failed because the Node fixture omitted the canonical HTTP origin; the fixture was corrected and both attempts are retained as raw-byte gzip logs. Runtime binding code did not need a qualification fix.

The successful guard measured four total processes, 1,486,602,240 bytes RSS, 9,106,244 bytes whole footprint (9,130,820 bytes conservatively including the retained first attempt), 1,363,968 bytes temporary storage, and 26,472,706,048 bytes minimum free disk. Both temporary prefixes and all owned children were released; pre-existing physical dependencies remained read-only. See `adoption-source-result.json` and `adoption-source01/02`. Native default-shell interaction remains NOT_RUN. Production clock qualification is absent and action bridges remain unavailable without it; synthetic test qualification grants no production capability.

### Native interaction and screenshot assessment

Under `FE-COMMUNITY-ADOPTION-NATIVE-202611041459Z`, source `fc4c5ecf` passed all 12 scenario/width cases and retained 15 screenshots. Actual forms retain session drafts across panels, then withdraw on clock interruption or privacy change; disposal leaves no canvas or socket. Synthetic World-enabled scenarios publish one actual Phaser canvas. The exact committed C03 fixture disables every capability, including World: the legacy `world-only` scenario name preserves those decisions and correctly has zero canvas. This fixture is not evidence of current live-service capability decisions.

All 15 screenshots were inspected. Negative states and retained forms are legible at 360, 390 and 430 pixels. The three enabled base captures show transient `Opening parties` and a generic fallback link, so they do not qualify the settled discovery creation control. Follow-up driver `47fa2013` waits for the actual discovery region and finished read, and targets `#party-create`. The follow-up runs only the three enabled width contexts and retains three settled captures in a fresh allocation with an explicit 18-image aggregate cap; the earlier images remain retained. No application runtime source changed after `25ea81e0`.

Attempts 1-3 retain their exact failures: mobile full-panel navigation visibility, then incorrect World-publication expectations for the disabled canonical fixture (direct Meet and World-first entry). Attempt 4 passed functional interactions. Cleanup verified no owned runtime processes, removed all temporary/cache prefixes, and successfully rebound port 5173. `adoption-native-result.json` records the measured limits and the still-open visual gate.

### Settled discovery control — functional PASS

Fresh allocation `FE-COMMUNITY-SETTLED-CTA-BROWSER-202611041513Z` qualified source `877f2634` with exactly three enabled contexts at 360, 390 and 430 pixels. Each waits for the real `.party-browse` and completed read, then targets `#party-create`; a generic fallback link cannot satisfy this check. All three cases and the three owner-inspected screenshots passed, closing the functional settled-control gap above. The original 15 images and all earlier failures remain retained: 18 images total.

The run used actual default router/providers, whole-character Phaser and synthetic HTTP/C05/TEST_ONLY timing. Each positive case published one canvas, retained its draft across panels, withdrew the form on clock interruption, and disposed to zero canvases/sockets. This is **functional evidence on frozen historical frames**, not approval of broad retro framing or the new post-only visual direction. No production clock policy, real-service mutation, physical-arrival permission or full FE21/FE22 acceptance is claimed.

The independent guardian measured 11 total processes, 2,598,191,104 bytes RSS, 40,202,682 bytes whole footprint, 30,494,720 bytes temporary storage and 266,240 bytes durable output, with 23,024,795,648 bytes minimum free disk. Child work finished at `15:13:19.527231Z`; final cleanup confirmed all owned process groups absent, temporary/cache prefixes removed, and successful port 5173 reuse binding. Exact reports, raw gzip logs, screenshot hashes and assessment are in `adoption-native05/` and `adoption-settled-result.json`. Publication and fresh remote ancestry/payload verification remain pending.
