# Original FE41 lifecycle completion

Status: IN_PROGRESS — client change source/native qualified; awaiting designated publication. This assignment retains the original FE41 scope; it does not
reopen completed FE17 or AR03, qualify real backend/device behavior, or remove any
original release criterion. The new unbudgeted goal covers implementation,
relevant developer validation and fixes, documentation, committed handoff and
normal publication by Root153.

## Source and custody

The exclusive `fe-ref-map` checkout uses branch
`codex/mcr-fe-FE41-original-completion`, based on public
`ba314a2c96639b05b6c15c1ead57353719004ffa`. Existing FE17 `8ca330d8` and AR03
`defdc8fc` refs remain unchanged. All thirteen formerly dirty read-only dependency
materializations match the public base byte for byte. A normal switch initially
refused them; an exact-path private stash preserves their original materialized
state, and the subsequent normal switch preserved every file's bytes. No force
clean, stash drop, clone, worktree or target was created. See
`evidence/fe41-original-completion/checkout-provenance.json` for full pins.

Root153 explicitly transferred the listed error/connection presentation leaves,
new `app/errors/fe41-lifecycle/**`, relevant existing error tests, and this new
document/evidence prefix. Shared edits require Root's exact additional hunks.
The earlier FE17 grants remain returned. Contracts, generated files, other
features, CSS, manifests, lockfiles and source originals are read-only.

## Original scope and retained evidence

The specification FE41 row requires offline/reconnect/maintenance/version/404
states, stale labels, foreground resync and render throttling. Acceptance requires
bounded waits, no silent offline pickup queue, and usable keyboard/reconnect
navigation. Specification S02/S03 also require safe unavailable routes and no
silent reservation replay. Current functional ordering defers QA08 performance;
it does not waive functional or real integration criteria.

| Criterion | Existing authoritative client evidence | Disposition |
| --- | --- | --- |
| Offline, reconnect, reload, keyboard navigation, safe 404, panel coverage and teardown | `frontend-combined-adoption/default-retry01/fe41/2026-11-04T15-34-52-198Z-native.json`, eighteen captures at 360/390/430, actual main/default ports/router with controlled HTTP/socket admission | Reuse unchanged. The prior failed internal-heading expectation remains retained. |
| Bounded pending phases and stale authority | Existing observer/default-entry source tests; original source cohort reported 150 cases in eleven files plus full types/lint at `c5dd793b` | Reuse. The eight-second null-view deadline already meets bounded-wait behavior; faster local failure alone does not justify an artificial scope increment. |
| Foreground recovery, privacy fences, one game and one connection, no old world intent | Published FE17 original-completion source and changed five-scenario native evidence, plus retained actual-App/Phaser three-width fault journey | Reuse. Client/controlled-peer evidence does not certify OS backgrounding or Rust/Auth recovery. |
| Offline pickup and uncertain non-replayable commands | Actual unchanged gift-pickup controller regression in `features/connection-status/__tests__/pickup-no-replay.test.ts`; original FE26 journal/reconciliation evidence in `community-authoring.md` | Reuse. Online, load, claim and reconstruction never silently issue a second claim POST. Unknown outcomes are not success. |
| Low power and reduced motion | `runtime-preferences-root-integration.md`, `world-runtime-preferences.md`, `fe34-runtime-preferences.md`; affected 35-case source cohort and retained controlled default-runtime result | Reuse. No performance or physical-device claim. |
| Operational maintenance or minimum application version | Observer accepts only a validated exact-release decision. World20f confirms no such frozen C03/C05 producer exists in its lane. | Preserve truthful unavailable behavior and explicit missing producer; never infer maintenance from HTTP 503, navigator, disabled capability, protocol version or motion-profile version. |
| Typed map-version admission failure and terminal version presentation | Current source maps initial ticket mismatch to generic connection; composition also destroys the mismatch view before the global observer can consume it. | Concrete uncovered version path; narrowly scoped fix and focused validation required. |

Historical `degraded-states.md` leading default-insertion NOT_RUN text is not the
current disposition. The later qualified controlled default-root result in
`combined-root-adoption.md` supersedes that absence. Historical evidence remains
unchanged and is not relabeled as live integration.

## Concrete version-path finding

`TransportEngine.TicketAcquisitionFailure` currently maps validated HTTP 409
`MAP_VERSION_MISMATCH` to generic unavailable/4500. Replication consequently
publishes a connection problem, allowing a reconnect action instead of the
existing reload-only version presentation. Close code 4409 alone cannot identify
a map mismatch because it also represents resynchronization failures.

Separately, replication publishes to its composition sink before notifying
readers. Composition sees a map-version problem and calls `fail('release')`,
which destroys/withdraws replication before the default observer can retain the
typed version reason. A local exact-release terminal projection must preserve
that validated distinction without copying protected roster/credentials or
inventing server maintenance. Generic release/runtime failures remain unavailable.

## Implementation and source qualification

The implemented path preserves only validated canonical HTTP 409 ticket
`MAP_VERSION_MISMATCH` and typed replication map-version errors. An exact-release
local lifecycle projection retains the terminal version reason after composition
withdraws replication. The existing default observer presents reload-only recovery;
generic release/runtime/503 errors remain unavailable. The optional lifecycle port
and its subscriptions retire with the captured release, including abort, privacy
withdrawal and disposal. No second identity, game or connection is introduced.

Source slot `FE41-CHANGED-CHAIN-20261104T1901Z` qualified 42 distinct cases
incrementally: eleven new chain/authority cases and thirty-one affected existing
cases. Attempt `source01` passed 32 cases but omitted the retained test-library
alias needed by the new version suite. The immutable failure remains. Configuration
successor `49df4add` ran only the ten unexecuted version cases, all passing, then
full application types, scoped deny-warning lint, native/config syntax,
whitespace and planning checks, all exit 0. Application bytes were unchanged.

The guardian completed at 19:04:33.573055Z, within the original allocation clock.
An independent census recorded FULL release with no matching live PID, temporary
root, borrowed cache, dependency link, external target or open temporary handle.
See `source-full-release.json` and both attempt supervisors. The release receipt
was written after the deadline following context recovery; the recorded workload
and cleanup had already finished before work stop.

## Changed native qualification

Slot `FE41-CHANGED-390-NATIVE-20261104T1912Z` accepted at
19:11:10.881223Z, work stop 19:14:10.881223Z and hard FULL deadline
19:15:10.881223Z. Native attempt `native-retry02` passed at
19:13:02.682366Z on `a1317c4d`. Only the new 390x844 viewport journey ran;
the existing eighteen-image matrix and settled source cohort were not repeated.

The actual default `main.tsx`, login UI, production loader, selected source art,
HTTP decoder, transport, replication, composition and connection observer ran
with the retained HTTP fixture. Its exact world-ticket branch alone was replaced
in memory with canonical 409 `MAP_VERSION_MISMATCH`; the report records original
fixture SHA and the exact transformation. No product module was transformed.
The visible version state offers Reload page and safe navigation, hides reconnect
and in-place world retry, and has no remaining canvas or socket. Offline/online
hints do not issue a second ticket. A visible minimum 44px reload target accepts
keyboard Enter; actual navigation reaches the sign-in access gate without replaying
credentials or admission. Page errors are empty and horizontal overflow is absent.
The 390px capture was visually inspected for this state, not full reference acceptance.

Two immutable fixture failures precede the successful run: `native01` used a
CommonJS query-library entry, and `native-retry01` omitted Phaser prebundling,
causing lazy world loading to fail. Successors changed only the new native
harness/readset: browser package resolution and private-cache prebundling, then
Phaser inclusion and failure diagnostics. All product TypeScript bytes and settled
source tests remained unchanged. The final harness executes successfully; its
prior lint/syntax result is from the source-qualified predecessor, not a claim of
rerunning that lint on later harness bytes.

An independent FULL census at 19:13:22.759425Z confirmed zero matching live
PIDs, temporary roots, external Chrome Singleton targets, dependency links,
borrowed caches and open temporary handles. Port 64379 rebinds with no listener.
See `native-full-release.json`, all three supervisor reports, the successful
`native/report.json` and `390-map-version.png`. Across attempts, peaks remain
within the accepted process/RSS/WHOLE/entries/NEW bounds; resource custody is zero.

## Disposition and delivery

The concrete client version-path gap is implemented and qualified. Original
unchanged client evidence is reused with its original limits. Root153 remains the
sole designated publisher of the exact ownership manifest, on normal non-force
master delivery. No king or SL review, acknowledgment or approval is required.
No publication receipt has yet been claimed at this checkpoint.

FE41 is not marked fully ACCEPTED: a real operational maintenance/minimum-client
producer is absent from current public contracts, and real Rust/Auth, multiuser,
physical-device, OS-backgrounding and full visual acceptance remain open. This
records a missing integration input and original acceptance limits, not a scope
cut, invented authority, or backend implementation claim.
