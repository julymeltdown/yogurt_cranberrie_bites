# FE40 public archive root composition

Status: **IN_PROGRESS**. Source and qualification harness are committed. Root's
producer and default PublicBoundary seams are present as committed technical
inputs. All validation remains **NOT_RUN** pending a fresh named21d offer and
explicit acceptance; source composition alone is not runtime evidence.

Assignment base `9e7952fcd89360f7b4fe43c69f22060b9aa55c74`; isolated
`fe-archive-root-204`, branch `codex/mcr-fe-FE40-archive-root`. Initial goal read
was null; new unbudgeted goal `createdAt=1793780227` on chat
`01a1a204-c583-7670-965a-4891b8b1289e` was actually ACTIVE on immediate readback.
It covers implementation, validation, committed handoff, concrete fixes, Root
publication, fresh independent remote proof and clean STOP/exact lease return.

Exclusive writes: new `frontend/src/app/archive-entry/**`, new
`frontend/src/app/__tests__/archive-entry/**`, this report and new
`docs/mcr/evidence/archive-root-composition/**`. No old checkout or returned
Auth/Meet/FE32 lease is reused. Root retains ports/main/PublicBoundary/router.

The adapter borrows the existing ArchivePage/controller/context/public reader.
It consumes explicit canonical `ports.eventId`, with absent/invalid input mapping
to unavailable without GET. Canonical `ports.session` is observed only for
generation cleanup, never to select an event or authorize a public request.
Subscriptions are allocated after commit and disposed controller-before-context;
StrictMode replay creates a fresh owner. A render fence hides the prior event or
generation before passive cleanup. No media producer is supplied, so unavailable
archived images remain truthful; no private/live media fallback is introduced.

## Exact Root-owned integration proposal

Root confirmed the current base has no public event port. Root owns adding
`readonly eventId?: string | null` to ShellPorts and a fourth
`publicEventId: string | null = null` argument to createProductionPorts, returning
that value in both Auth and anonymous branches. Main qualifies configuredEvent
once and passes the same explicit value to Auth and this public producer. No
Auth-derived public event or query fallback is allowed. These shared files are
not edited by this worker.

Minimal PublicBoundary changes, applied only by Root after this entry exists:

```tsx
import { lazy, Suspense } from 'react'
const ArchiveRoute = lazy(() => import('./archive-entry').then(module => ({ default: module.ArchiveRoutePanel })))
// Before the placeholder fallback, inside PublicBoundary:
if (location.pathname === '/archive') return <Suspense fallback={<p role="status">Opening archive…</p>}><ArchiveRoute /></Suspense>
```

No router or publicPaths change is needed. Default-route qualification must use
Root's actually applied shared cohort, not a fixture override or an isolated
ArchiveRoutePanel render relabeled as production composition. The frozen202 App
replay candidate and current Wall/producer jobs remain independent.

BE41 real publication/deletion, qualified public media, physical devices and
release acceptance remain separate. Original archive module MOCK_READY is
retained; this work supplies the missing public client entry.

## Committed provenance and qualification boundary

Worker source commits are `931bc26d71ef6912e36b1ef362031e5c496bf3a7` and
`f7c4b89922322c678227119f31b70e9bc897062f`. Root consumed the first as
`904121f0`; the second is the next worker-owned intake. These use the already
audited archive module in place, without copying legacy rendering or changing
the frozen contract, manifests, lockfile, shared transport or source artwork.

Root explicitly supplied two separately committed shared inputs:
`16c29d91e8fdceab8847c07ca7640a2fa0301636` (ports/main), consumed locally as
`246e9be3`; and `0a47638e453308d801b467d6277c0e386b79c697`
(PublicBoundary), consumed locally as `67264947`. Those are Root-owned inputs,
excluded from this worker's ownership and handoff commit list. Root implemented
the proposal above; this worker did not edit the three shared files.

The new route suite calls actual default `createAppRoutes` and PublicBoundary,
with synthetic public contract responses. Its separately labeled isolated
adapter case checks event replacement and unmount. It covers anonymous producer
selection, loading/empty/errors, pagination and focus, no inferred event, no
private bootstrap/world, strict projection rejection, withdrawal, cancellation
and stale generations. Unit canvas stubs do not prove native pixels.

The finite native runner uses that same actual default root, a test-only public
HTTP fixture, one Chrome/Vite workflow and four serial viewports: 360/390/430 by
844, and 390 by 420. It does not transform Root's source or override route
composition. Planned checks include frozen native pixels, pagination/focus,
public-only request metadata, media unavailability, generation reload, safe
errors/withdrawal/empty, target sizes and overflow. Captures are synthetic
contract evidence, not real BE41 publication, production replay or physical
device evidence.

The closed archive projection rejects extra private fields, but cannot decide
whether an arbitrary permitted text string semantically contains a private
handle. Producer redaction and current deletion enforcement require real BE41
evidence. No qualified public media adapter is supplied; the existing explicit
unavailable state remains intentional. No endpoint, media authority or deleted
content is invented by the client entry.

## Requested finite checks

Request `FE40-ARCHIVE-ROOT-204` names the following sequential checks under
Node 24.22.2. At authoring time every result is **NOT_RUN**. An offer is not
acceptance; no dependency link, install, source check or browser has started.

| Check | Command (from frontend unless stated) | Result |
| --- | --- | --- |
| Route/module regression | `vitest run src/app/__tests__/archive-entry/route.test.tsx src/features/archive/__tests__/archive.test.ts src/app/__tests__/public-entry.test.tsx --cache=false --maxWorkers=1` | NOT_RUN |
| Application types | `tsc --noEmit --incremental false --project tsconfig.json` | NOT_RUN |
| Owned source lint | `oxlint src/app/archive-entry src/app/__tests__/archive-entry` | NOT_RUN |
| Runner syntax | `node --check src/app/__tests__/archive-entry/native.mjs` | NOT_RUN |
| Native default route | `MCR_NATIVE_SLOT=<accepted fresh offer> MCR_ARCHIVE_ROOT_HEAD=<qualified HEAD> node src/app/__tests__/archive-entry/native.mjs` | NOT_RUN |
| Planning baseline | `python3 scripts/mcr/check-plan.py` from repository root | NOT_RUN |

Requested resource bounds: one workflow, at most six active minutes, 0.2 GiB
whole output ceiling (temporary peak estimate at most 0.15 GiB, retained at most
0.01 GiB), 16 GiB free at start and 8 GiB stop floor. Dependencies may only be
linked after acceptance into a task-local directory, leaving generated caches
local and removable. No install, production build, code generation, backend
process, device session or shared output cleanup is included. Publication and
independent remote proof remain unfinished while this report is IN_PROGRESS.
