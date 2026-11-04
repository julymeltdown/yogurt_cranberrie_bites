# FE40 public archive root composition

Client qualification: **PASS**. Root has integrated the entry and public producer
in unpublished source; designated publication is pending. This closes the missing
original FE40 public client composition against frozen C04, not real BE41,
qualified public media, physical-device or release acceptance. The existing
archive module's historical MOCK_READY evidence remains separate.

## Scope and behavior

Specification: original FE40/X01 and public archive requirements in
`MCR_IMPLEMENTATION_PLAN_V2_EN.md`; required assessment/execution documents were
read before implementation. Assignment base:
`9e7952fcd89360f7b4fe43c69f22060b9aa55c74`, isolated `fe-archive-root-204`, branch
`codex/mcr-fe-FE40-archive-root`. No prior Auth/Meet/FE32 lease was reopened.

Exclusive new paths:

- `frontend/src/app/archive-entry/**`
- `frontend/src/app/__tests__/archive-entry/**`
- `docs/mcr/frontend/archive-root-composition.md`
- `docs/mcr/evidence/archive-root-composition/**`

`ArchiveRoutePanel` reuses the existing public reader/controller/context and
ArchivePage in place. No legacy rendering was copied. It consumes only explicit,
canonical `ShellPorts.eventId`; absent/invalid input becomes unavailable without
GET. Auth readiness, session event and query parameters cannot select an event.
The one canonical session is observed only for generation cleanup. No new Auth
identity, private reader/cache, Phaser instance or world connection is created.

Owners are allocated after commit, so StrictMode replay gets a fresh controller
and context. A render fence hides old event/session/generation content before
passive cleanup. Disposal runs controller before context; canceled/ignored-abort
work cannot repopulate a new owner. The public reader keeps registered C04 GET,
credentials omitted, no Authorization/Cookie, no-store and redirect-error policy.
The final mural is frozen; only read navigation remains. No private or write
controls, live media fallback, handle field or physical beacon is introduced.

No qualified public media producer is supplied: the existing explicit unavailable
message is intentional. Closed projection validation rejects extra private
fields, and a subsequent invalid/410 public response clears prior posts and wall.
Arbitrary permitted text cannot be semantically certified as redacted by the
client. Real producer redaction, current deletion enforcement and freshness are
BE41 obligations; no live deletion/subscription guarantee is claimed here.

## Root-owned producer and composition provenance

The assignment base had no public event port. Root supplied and exclusively
owned these shared patches; the worker consumed their committed postimages:

| Root source input | Local technical-input commit | Paths |
| --- | --- | --- |
| `16c29d91e8fdceab8847c07ca7640a2fa0301636` | `246e9be31e895173cdf74cc1777f36dbb05a0434` | `frontend/src/app/ports.ts`, `frontend/src/main.tsx` |
| `0a47638e453308d801b467d6277c0e386b79c697` | `67264947497ea5e5c417a098135b7fa2c21dbd8d` | `frontend/src/app/PublicBoundary.tsx` |

Root added backward-compatible `eventId?: string | null`, returned the explicit
fourth public event argument in both production factory branches, and qualified
configuredEvent once in main for both Auth and public producer. No public event
is borrowed from Auth. Root applied this minimal proposal:

```tsx
import { lazy, Suspense } from 'react'
const ArchiveRoute = lazy(() => import('./archive-entry').then(module => ({ default: module.ArchiveRoutePanel })))
// Inside PublicBoundary, before the fallback:
if (location.pathname === '/archive') return <Suspense fallback={<p role="status">Opening archive…</p>}><ArchiveRoute /></Suspense>
```

Router/publicPaths already registered `/archive`; neither needed a change.
The worker did not directly edit those shared files. Both technical-input commits
are excluded from the owned handoff. Root intake already mapped `931bc26d` to
`904121f0`, `f7c4b899` to `19e4c22e`, and report-only `3f69299b` to `ca7909b8`.
Later evidence/fix commits follow normally; publication and independent remote
readback are separate from this source receipt.

## Qualification and evidence

Source checks qualified `3f69299bc1e1132c474139e5a11f3bfb6d8ff8e5` on pinned
Node v24.22.2. Product adapter, TS tests and Root inputs remained byte-identical
through final native qualification at
`7909a45a6f0344299550d9c54ecd9d4fd1243fd2`. Later runner changes affect only
fixture probes, offer recording, resource accounting and provenance inputs.

| Check | Command from frontend unless stated | Result |
| --- | --- | --- |
| Default route/module regression | `vitest run src/app/__tests__/archive-entry/route.test.tsx src/features/archive/__tests__/archive.test.ts src/app/__tests__/public-entry.test.tsx --cache=false --maxWorkers=1` | exit 0; 84 tests / 3 files |
| Full application types | `tsc --noEmit --incremental false --project tsconfig.json` | exit 0 |
| Owned source lint | `oxlint src/app/archive-entry src/app/__tests__/archive-entry` | exit 0 |
| Final runner syntax | `node --check src/app/__tests__/archive-entry/native.mjs` | exit 0 |
| Final native route | `MCR_NATIVE_SLOT=ARCHIVE-ROOT-204-202611040909Z MCR_ARCHIVE_ROOT_HEAD=7909a45a6f0344299550d9c54ecd9d4fd1243fd2 node src/app/__tests__/archive-entry/native.mjs` | exit 0; 4 views |
| Planning baseline, repository root | `python3 scripts/mcr/check-plan.py` | exit 0; planning provenance only |

The new suite exercises actual default `createAppRoutes`/PublicBoundary without
route overrides: explicit anonymous production factory, no inferred event,
loading/empty/safe HTTP failures, pagination/focus, no private bootstrap/world,
strict privacy projection, withdrawal and canonical-generation cancellation.
Its one isolated adapter case is labeled separately and checks event replacement,
unmount and late completion. Unit canvas stubs do not certify native pixels.

The final one-attempt Chrome 156.0.8078.80/Vite workflow used synthetic public
HTTP, actual default root composition and four serial viewports: 360×844,
390×844, 430×844 and 390×420. All four full-page PNGs were visually inspected.
All 29 source pins and four PNG hashes matched. Native checks passed frozen
pixel/tap invariance, one archive canvas, zero world mounts, no write forms,
44px targets/no horizontal overflow, pagination/keyboard focus, honest media
unavailability, generation reload, withdrawal/privacy/offline errors and empty
state. Every captured application request was the explicit event's public GET
with credentials omitted and no Authorization/Cookie. Application socket count
was zero before and after flows. Only the Vite fixture protocol `vite-hmr` is
classified separately; no socket URLs or dev tokens are recorded.

Evidence under `docs/mcr/evidence/archive-root-composition/`:

- `source-qualification.json`, `logs/`: initial source results and commands.
- `native/report.json`, four PNGs: final passing workflow, source/PNG hashes,
  request metadata, protocols, checks and sampled resource limits.
- `round-qualification.json`, `native-final-release.json`: exact final round,
  supervisor deadlines, attempt/exit/source and whole-owned cleanup receipt.
- `provenance.json`: base, owned commits/paths, Root inputs, scope/import audit
  and four actual visual inspections.
- Initial failures below remain preserved. Log normalization replaces only the
  assigned absolute worktree prefix with `<worktree>`; messages/exits stay intact.

## Allocation, failures and cleanup

No checks or dependency links were created before a fresh offer and explicit
acceptance. Retained dependencies were read-only; each round created an absent
owned directory of package symlinks, excluding shared caches, and removed that
owned directory afterward. No install/build/codegen/backend/device/peer cleanup
occurred. Browser profile/subprocess TMP, Vite cache, local dependency links/cache
and all carried evidence count toward the whole output bound.

| Offer | Actual accept → full release (UTC) | Result |
| --- | --- | --- |
| `ARCHIVE-ROOT-204-202611040852Z` | 08:50:21 → 08:51:43 | 84 tests, TS/lint/syntax/plan passed; native exit 1 before Vite/Chrome because guard incorrectly required FE40 in the issued name. Failure retained in `logs/native.log`. |
| `ARCHIVE-ROOT-204-202611040859Z` | 08:59:16 → 09:00:35 | syntax passed; public archive rendered/world mount zero, then blanket socket probe counted Vite HMR. Native exit 1; no certified view or PNG. Preserved in `logs/native-qualified.log`, `failures/native-development-socket.json`, `native-second-release.json`. |
| `ARCHIVE-ROOT-204-202611040909Z` | 09:09:21 → 09:11:37 | final syntax/native exit 0, four views; one attempt of the two allowed, no repeat of unaffected source suites. |

The two earlier rounds were released without automatic retries. Their owned
fixes were committed before the next fresh allocation. The final usable round
allowed repairs/retries within its unchanged six-minute scope/caps; none was
needed. Authorization is the actual offer plus explicit acceptance, not words
in a naming token. No main/king/SL technical approval or acknowledgment gate
applies. The designated publisher still exclusively owns master publication.

Final round carried 11,498 bytes; sampled peak whole output was 29,507,967 bytes,
temporary 29,283,547 bytes; minimum free disk 27.86028 GiB and post-cleanup
27.98547 GiB. All durable evidence at release was 239,421 bytes, comfortably
within 0.01 GiB; whole cap was 214,748,364 bytes, temporary 0.15 GiB and disk
16 GiB start/8 GiB stop. Actual release was 136 seconds, before the 09:14:21
active and 09:15:21 whole-round supervisor deadlines. Browser/server/profile,
TMP/cache, owned dependency directory and runtime processes were gone. The
first source round did not separately measure its dependency/cache peak; its
reported final bytes and free space are retained without inventing a peak.

## Delivery boundary

The same unbudgeted goal, createdAt `1793780227` on chat
`01a1a204-c583-7670-965a-4891b8b1289e`, remains ACTIVE through designated
publication, independent fresh remote ancestry/all-payload proof and clean STOP
with all four leases returned. Source commits and current evidence alone do not
complete that delivery step. Preserve old branches/checkouts/artifacts.

BE41 real publication/deletion/redaction, qualified public media, production App
replay, physical devices and release acceptance remain separate NOT_RUN here.
No Auth renewal/duration gate, archived-202 diversion or broader acceptance is
added. Frozen-contract client evidence is not relabeled as a real multiuser flow.
