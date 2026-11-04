# FE40 public archive root composition

Status: **IN_PROGRESS**. Source authoring only; validation and actual default
PublicBoundary composition remain **NOT_RUN** pending the Root-owned producer
seams and a fresh named21d offer plus explicit acceptance for output checks.

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
