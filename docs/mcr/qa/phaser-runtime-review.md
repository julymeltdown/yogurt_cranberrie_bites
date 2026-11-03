# FE14 independent runtime handoff review

Date: 2026-11-03. Status: **review complete — all three P2 findings cleared on
`9b9307c922facb7c8786595b2d0a78c6086e084a`; final parent disposition pending.** This
report reviews the bounded synthetic runtime foundation, not acceptance of FE14
as a whole, production artwork, real world admission, or IN01/IN05.

## Candidate, authority and ownership

QA began clean at integrated `26f5e4fd6b822f81d82dd340363b8666ad20d066`, branch
`codex/mcr-qa-fe-runtime`, worktree `/Users/lhs/Desktop/mcr-worktrees/qa-fe-runtime`.
Worker ancestry is `244d35a85dd884655e270f7c6af6a90657fabfbd` → `a7dcb792` →
`a1243ffd` → `1ffff588`. QA compared all fifteen worker paths with the integrated
checkout: every file is byte-identical. Git also confirms the C06 schemas and
golden fixtures are unchanged from the worker base.

Authorities read: root and frontend `AGENTS.md`, specification FE14 and its
world/lifecycle/stack requirements, `docs/mcr/project-assessment.md`,
`docs/mcr/execution-plan.md`, and `docs/mcr/frontend/phaser-runtime.md`.
The sole QA-authored tracked path is this new report. The parent subsequently
authorized an exact cherry-pick of the worker correction for local qualification;
QA did not reauthor application files. No contract, golden fixture, manifest,
lockfile, backend, or source-archive edits were performed. Only the parent
integrates/publishes. The previous QA checkout is stopped.

The exact fifteen reviewed paths are:

- `docs/mcr/frontend/phaser-runtime.md`
- `frontend/src/world/runtime.ts`
- `frontend/src/world/map/json.ts`
- `frontend/src/world/map/schema.ts`
- `frontend/src/world/map/formats.ts`
- `frontend/src/world/map/manifest.ts`
- `frontend/src/world/map/tiled.ts`
- `frontend/src/world/map/resources.ts`
- `frontend/src/world/scenes/MapScene.ts`
- `frontend/src/world/__tests__/runtime.test.ts`
- `frontend/src/world/__tests__/runtime-browser-smoke.mjs`
- `frontend/src/world/__tests__/fixtures/png.ts`
- `frontend/src/world/__tests__/fixtures/synthetic.ts`
- `frontend/src/world/__tests__/fixtures/runtime-preview.html`
- `frontend/src/world/__tests__/fixtures/runtime-preview.tsx`

## F1 [P2] — inherited teardown can lose the host reservation

Parent-origin finding, independently reproduced by QA. In the initial
`runtime.ts:53–60`, a generation without its own engine releases its host claim
immediately, even when it inherited a pending teardown drain. Reproduction uses
the actual runtime with a supported injected engine whose `closed` promise is
deliberately deferred:

1. Await owner A's first ready engine, then stop A; leave its engine unclosed.
2. Start A again, reserving a replacement generation that awaits the old drain.
3. Stop, dispose, or externally abort that replacement before the drain resolves.
4. Start owner B against the same host before closing A's original engine.

All three cancellation variants return B `ready` with **two engine allocations
before the old engine closes**. The no-engine replacement deletes its claim;
B has its own already-resolved drain and can allocate over the outstanding
engine. This violates the runtime's claimed ownership fence. The existing
synchronous-close replacement test does not cover this sequence.

Required correction: retain the host reservation through the inherited drain,
including no-engine replacement stop/dispose/abort, without allowing an older
release to delete a newer generation's claim. Include a deferred-close regression
that proves B remains host-busy until release and can subsequently acquire safely.
Evidence: temporary `inherited-drain.json`. Disposition: **CLEARED by exact
`9b9307c`**, as independently verified below.

## F2 [P2] — a visibility callback can publish readiness after disposal

QA-origin finding in initial `runtime.ts:98–100`. Startup checks `current()`
before calling `owned.engine.setVisible(visible)`, then publishes `ready` without
checking again. A supported injected lease can synchronously call its owner's
`stop()` or `dispose()` inside that callback.

QA ran both variants with actual `createWorldRuntime`, a validated synthetic map
and a minimal injected lease. In each case the engine is destroyed once, the map
is disposed once, and the host has zero children, but `await start()` and `read()`
report **ready**. In the dispose variant the disposed flag remains set, so later
start calls cannot repair the false state. This is an injected-engine lifecycle
reproduction; QA does not claim the current native no-input Phaser scene invokes
this callback reentrantly.

Required correction: check generation/active ownership after the visibility
callback before publishing readiness, with stop/dispose/restart regressions.
Evidence: temporary `reentrant-visible.json`. Disposition: **CLEARED by exact
`9b9307c`**, as independently verified below.

## F3 [P2] — IPv-literal provenance differs from the pinned C06 checker

QA-origin finding in initial `map/formats.ts:29–30`. The hand-written IPvFuture
case and WHATWG `new URL()` IPv6 branch disagree with the actual pinned
`rfc3986-validator==0.1.1` path. Full map/theme manifests were rehashed, references
updated, and their approved release pins supplied before both validators ran.

| Provenance source URI | Client manifest | Actual pinned Python manifest |
|---|---|---|
| `http://[::ffff:192.168.001.1]/` | Rejects invalid | Accepts, exit 0 |
| `http://[::ffff:192.168.01.1]/` | Rejects invalid | Accepts, exit 0 |
| `x://[V1.ab:cd]/` | Accepts | Rejects schema, exit 1 |
| Canonical embedded IPv4, lowercase `v`, `::`, URN controls | Accepts | Accepts |

The consequence is inconsistent manifest acceptance, including failure to load
metadata accepted by the contract owner. Neither edge is permission to fetch a
provenance URL; runtime resources use an independent allowlist. This finding
concerns parity with the selected checker, not a claim that its edge semantics
establish ideal or exhaustive RFC conformance.

Required correction: align the bounded IP-literal predicate with the selected
owner checker and add differential full-manifest positives/negatives. Do not
silently amend C06 schemas or resource URL policy. Evidence: `uri-parity.json`
and three rehashed bundle JSONs in the QA temporary root. Disposition:
**CLEARED by exact `9b9307c`**, as independently verified below.

## Source review and evidence boundary

The resource path permits exact relative `world/...json|png` entries only;
composition copies/freezes allowlist entries. Fetch omits credentials and
referrer, uses same-origin mode and rejects redirects, mismatched response URL,
media type, declared/streamed size, and byte hash. Raw JSON is parsed before
duplicate-key or numeric lexical information is lost. Canonical integer-only
ASCII bytes, codepoint key ordering, release/content/geometry pins, local-only
schema references, collision/spawn/reachability and theme sets were inspected.
PNG header and decoded size checks precede renderer use. Tiled accepts a finite
normalized subset and cannot discover external tilesets, images or object layers.
Consumer bounds are explicit; they do not amend C06.

Runtime reservation, generation/abort checks, late load/import disposal,
reentrant allocation, timer/RAF/observer cleanup, detached mounts and engine/image
teardown were inspected. F1/F2 above are concrete gaps in those fences. Phaser
creation is dynamically imported after resource validation. Terrain and overdraw
use row-major tile indices, nearest filtering, integer zoom, bounded camera and
default cell-center foot coordinates; no avatar, movement or arrival is inferred.

Installed Phaser **3.90.0** source was inspected directly: `Game.js` sets pending
destruction through `destroy()` and processes it at the beginning of `step()`;
`runDestroy()` emits destruction synchronously before renderer/loop destruction
finishes. The runtime's promise continuation runs after that synchronous call.
`VisibilityHandler.js` adds an anonymous document listener and replaces blur/focus
properties, without corresponding removal in `runDestroy()`. The owned subclass
intercepts only synchronous `super.start()` document registration, restores the
exact prior method/own-property state in `finally`, removes its recorded callback
and capture flag, restores blur/focus only while still owned, and deletes only
its own debug game reference. The fixed no-input/no-extra-plugin scene matters
to this containment; changed Phaser/plugin/input configuration needs new review.
ScaleManager and TimeStep teardown source was also inspected.

Imports stay within the frontend module, declared Phaser dependency, public C06
schemas and type-only public fixture projections. No backend implementation,
server configuration, secrets, production router or character art is imported.
The fixture UI explicitly labels synthetic scope. QA visually inspected the
worker's 390px ready/invalid screenshots: simple grid, readable status and controls,
ordinary symmetric button borders and an accessible focus outline, with no
prohibited decorative left accent.

## Executed and inspected validation

QA used Node24.22.2 from the prescribed toolchain and pnpm11.7.0. Frozen offline
installation in the owned checkout exited 0 without lockfile changes. QA ran:

```sh
MCR_FE14_PYTHON=/tmp/mcr-fe14-format-venv/bin/python \
  corepack pnpm@11.7.0 --dir frontend test src/world/__tests__/runtime.test.ts
python3 -B scripts/mcr/check-plan.py
```

Results: **116 targeted runtime cases PASS**, one file, exit 0; planning exit 0.
The opt-in Python differential case executed with actual registered URI,
date-time and UUID checkers. QA also ran the three independent reproduction
groups above via Vite SSR/JSDOM or direct Node, without starting a browser/server
listener or modifying application/test sources. Reproduction commands exited 0
after recording the defective outcomes, not after asserting product acceptance.
No Rust, database, Docker or global services were used.

Inspected worker evidence at `/tmp/mcr-fe14-evidence/`: full 853-test output,
build/lint logs, browser harness and report, synthetic hashes, planning/boundary
outputs and the screenshots named above. These were **inspected, not rerun by
QA**. The browser report describes Chrome156.0.8078.80 at 360/390/430/1280 widths,
real tile pixels, resize, repeated mounts, asleep/early-boot teardown, missing/hash/
slow resources, retained prior/later callbacks, no persistent owned listeners/
RAF/blob URLs/observers and public-entry isolation. It lists zero unexpected page
errors and one deliberately injected `FE14_SYNC_START_THROW`. The harness checks
production public routes do not request Phaser/world/API/fixture bundles.
Passing initial suites did not expose F1–F3.

QA temporary evidence root:
`/var/folders/j4/41h_5mjj7j9d8f2j2bzcsngw0000gn/T/mcr-fe14-qa-hljKKQ`.
`source-pins.json` records SHA-256 for all fifteen reviewed files, owning installed
Phaser source and the worker browser report. Selected anchors:

| Initial source/evidence | SHA-256 |
|---|---|
| `runtime.ts` | `d24a2208c5df9e454931b310f2cd99cefe36792bea39681709d0e3b1cc732b3c` |
| `map/formats.ts` | `4605d687a75bb72cc59cd7dfcbc34d8c0594573aa3a35384f921b496cd979a4b` |
| `scenes/MapScene.ts` | `0431b7e4e48cfab8a59daf701407e5adb43a4a7a81db24c6b21acb6d63996d96` |
| `runtime.test.ts` | `8aa312229bd77f004a3b594e73f217e8f3f5036d4dd13facf5e704901b51bcb9` |
| Browser harness | `cc64403538d9c8d3a0e5072f590fd11f0177f1f464abc3a0b95b0ba9f0f9bc7a` |
| Worker browser `report.json` | `7119b7d2ae3c467921068c5b8e20ed8794a3a2e36baedb974f4981922448e30d` |
| Installed Phaser `Game.js` | `3f04fde6307e194518cc1704cc75ab53873c4012ec8d74f858065deb867d6e11` |
| Installed Phaser `VisibilityHandler.js` | `3c8ece6e566a1c735cd684c76e57976c0778d59040eebd7e464d8a7432628bd5` |

## Exact correction and final independent disposition

Reviewed worker correction: **`9b9307c922facb7c8786595b2d0a78c6086e084a`**, parent
`1ffff588d44ce7a7aa82a574aaae163cbcd1248c`. Per explicit parent authorization,
QA cherry-picked it unchanged after the initial report commit `6480707`;
the local application commit is `afa6a977bc329ace65f9f3b80e1827fa4fa84f2d`.
This local ancestry is not an application patch authored or submitted by QA.
All fifteen worker files now match exact `9b9307c` bytes. The correction changes
only six originally assigned paths; C06 schemas and golden fixtures remain
byte-identical to the original base.

- **F1 CLEARED:** the no-engine branch chains release behind `drain`, preserving
  inherited teardown lifetime. The same standalone probe now gets `host-busy`
  and one allocation before old close for each stop/dispose/abort variant; B
  reaches ready only after close. The targeted deferred-close regressions also
  check zero B loads/factory calls while blocked, peak live engines of one, and
  that obsolete A cleanup cannot evict the subsequent B/C owner.
- **F2 CLEARED:** a second `current()` guard immediately after `setVisible()`
  prevents stale publication. The unchanged probe returns stopped/disposed
  without any ready publication for those actions. Restart returns loading for
  the old start and publishes exactly one ready state for its replacement.
  Targeted tests also verify later starts on disposed owners remain disposed.
- **F3 CLEARED:** provenance IPv6 now uses an independently authored bounded
  group/unit counter, not WHATWG normalization. Decimal dotted tails preserve
  the selected checker's leading-zero behavior; uppercase IPvFuture `V` is
  rejected consistently. The same seven rehashed full-manifest comparisons now
  agree. An additional independent **655-case scalar IP-literal matrix** has
  zero client/actual-Python mismatches, covering expanded/compressed group
  counts, dotted-tail positions, octet width/range, zone/colon errors and
  IPvFuture controls. This finite matrix is not exhaustive RFC certification.

QA independently reran the targeted runtime file on the corrected bytes with
`MCR_FE14_PYTHON` set: **134/134 tests PASS**, exit 0. This includes twelve
additional full map/theme Python differential cases with critical positives
and invalid octets, misplaced tails, zones and compression. The same temporary
`review-probe.mjs` was executed against initial and corrected source and its
recorded final outcomes were asserted separately; both drivers exited 0.
`combined-initial.json` and `combined-final.json` retain the before/after results.

Inspected, not independently rerun: worker final `unit-tests-r1-r2-r3.txt`
(871/871), build/lint logs and `review-r1-r2-r3/report.json`. The final native
report adds real-Phaser wrappers for inherited held-close and reentrant
visibility stop/dispose/restart while retaining the four-width matrix and zero
unexpected page errors. The held-close native test delays the lease's contract
completion after Phaser has destroyed its engine; it does not claim two native
engines remained physically alive. QA inspected the changed fixture/harness
assertions. The parent owns final combined/native qualification.

`final-source-pins.json` in the QA temporary root records all fifteen exact
corrected file hashes and evidence hashes. The six corrected-path pins are:

| Final source | SHA-256 |
|---|---|
| `docs/mcr/frontend/phaser-runtime.md` | `bb6195ad98dabadb07c0ba83b27991187d46a75deb2da469bc639526cb2b46a9` |
| `frontend/src/world/runtime.ts` | `822b3fcd9d3c380bd9d781b9ca908184714b0e2810f09c8aff4d82a1ca7ec679` |
| `frontend/src/world/map/formats.ts` | `0e31dc185d0e3c0fa148c70443d2b2a9b34c96d3b43dc72e2070fce62cb98770` |
| `frontend/src/world/__tests__/runtime.test.ts` | `7cd429511d2fd04815ac04a91ef3451d3bdf16e40a9882c57f220260491796b7` |
| `frontend/src/world/__tests__/runtime-browser-smoke.mjs` | `98e9e1b329004c5f38c457d66b6b959239fc577915d04609ec11c901c3ed26e4` |
| `frontend/src/world/__tests__/fixtures/runtime-preview.tsx` | `2cd3f574642965547718a04915ce6b3706f625e910e1be8ff1cac2cb92567e66` |
| QA `review-probe.mjs` | `cda455e8d583950ce98295d0909e253acc7379c0b999a1d72868bdaec8640af9` |
| QA `combined-initial.json` | `82093261390e7d80a14b993aab9db3ce004037b5c5e734e7f91230a9ea601737` |
| QA `combined-final.json` | `bcf497958a31a08b1c78ce3f92e633dc8401d2e38fabe243ab289a169489423b` |
| QA `final-ip-literal-matrix.json` | `2dc02344a750cddedfe9cfe5e496b209bfd6331deaf11af35a12b88f86214ea7` |
| QA `final-targeted-tests.log` | `00aa3cda7ca8a7ec3a89fbb65e5e4351440445da5f17418fdcdcae357a347643` |
| Inspected final worker native `report.json` | `5f012373b0be81c1f27f654f9adab3b80b12f490337c80d02e92aa729277d1db` |

Finding labels differ from the worker's documentation: this report's F1/F2/F3
map to worker R1/R3/R2 respectively. **No actionable finding remains in this
bounded correction review.** The sole QA-authored final patch updates this report;
the parent should integrate QA report commits, not QA's local application cherry-pick.

## Qualification limits and goal

No real multiuser, shell integration, socket/admission, user/event private-cache,
character/production map/venue, physical mobile Safari/device, load or release
qualification follows. Public-entry isolation is inspected native evidence,
not an independent QA browser rerun. The pinned timestamp plugin's unsupported
leap seconds and the normalized Tiled subset remain explicit limitations.

The previous C06 audit goal was COMPLETE. This assignment's fresh unbudgeted
goal was created and actually returned ACTIVE. It stays active through exact
fix verification, sole report commit, parent review fixes and disposition.
