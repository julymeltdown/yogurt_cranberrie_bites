# FE38 event operations

Date: 2026-11-04, Asia/Seoul. Status: **MOCK_READY**. The bounded frontend
implementation and deterministic browser/controller qualification are delivered.
Real operator authorization, production publication, native devices and closure
execution remain **NOT_RUN**. This is not an IN05 or whole-project acceptance.

## Assignment and provenance

The resumed writer uses only
`/Users/lhs/Desktop/mcr-worktrees/fe-event-operations-resume`, branch
`codex/mcr-fe-event-operations-resume`, parent
`707ea68d585324430144c971724efbf5c73167d6`. The prior FE38 writer was interrupted
and its checkout was preserved read-only by the recovery owner. The unfinished,
aligned persistent FE38 goal was inspected and reused; it was not reset or
falsely completed. The recovery instruction transferred only:

- `frontend/src/features/admin/event/**`
- `frontend/src/features/admin/places/**`
- `frontend/src/features/admin/audit/**`
- This newly assigned `docs/mcr/frontend/event-operations.md`.

The former planned `operations.md` had not been created. No source-archive, shared
contract/API/Auth/provider, admin registry, root/router, backend, mural/closure,
manifest, lockfile or character-art path was modified. The integration owner is
now the main chat; the former delivery chat is archived. Only that publishing
integrator integrates/pushes master.

All rendering and controller code is newly authored in this chat. The recovered
uncommitted starting source came from the earlier FE38 base
`072ea86768ab3606854e81a67ab923c78c7ca71d`; its requested temporary receipt path
was unavailable on inspection. Final provenance therefore pins the new parent
and actual final source hashes, rather than asserting an independently checked
recovery-receipt hash. Source originals under `docs/source/` are untouched.

Read-only dependencies are the retained canonical Auth/session/privacy/HTTP
modules, generated domain operation metadata, the existing FE19 headless place
schema, React/Zod and the shared `ConcertWindow`. No legacy UI or new dependency
was copied. Tests use newly authored synthetic fixture ports and the canonical
Auth implementation. No credentials, private media or actual venue facts are
embedded. The native report pins every feature source file, relevant shared
source, contract and screenshot with SHA-256.

The exact ten-screen reference was viewed at
`docs/mcr/design/ui-reference-audit-2026-11-04/user-reference.jpg`, SHA-256
`2b3988d546f87b813d78ca13bf267730a6dd19118bd6535a2d3c3032b6f12d23`.
These operations screens use the shared symmetric concert window, compact
English headings and plain forms/tables. No character artwork, decorative left
accent, private media preview or unsupported game embellishment is introduced.
The reference does not establish venue truth or real operational readiness.

## Implemented surfaces

| IA | Export | Behavior |
| --- | --- | --- |
| O01 `/admin` | `EventOperations` with `summaryOnly` | Health/degraded summary, report/job counts, current settings/phase and reviewed capability switches. |
| O02 `/admin/event` | `EventOperations` | Five feature flags, bounded TTL/grace/cooldown limits, reasons, CAS phase changes, source-stamped notice create/edit/publish/withdraw. |
| O03 `/admin/places` | `PlacesOperations` | Bounded place pages, actual place revision separate from map revision, source/review/author/license/photo-permission metadata, verification, closure, accessibility and reviewed-source forms. |
| O08 `/admin/audit` | `AuditOperations` | Actor/operation/outcome/UTC-range filters, explicit cursor pages, minimal list and locally selected detail, no invented detail endpoint. |

The settings and event revision must agree. Phase commands allow only
`DRAFT → OPEN` and `OPEN → LIVE`, as in `SetEventPhase`. Read-only, freeze,
archive, expiry, cleanup and START/RESUME closure belong to FE39/BE41; these
screens explain that boundary and offer no bypass. Feature switches require
server enforcement by each owning service; hiding a client control proves no
server gate adoption.

Risky commands require a review of the operation, exact target, expected
revision, changed values and reason/source information, followed by a separate
Confirm button. Cancel sends nothing and restores keyboard focus. Inputs remain
drafts across a definite conflict and explicit refresh; the new revision must
be reviewed again. Controls are locked while reading/sending or while a command
record is unresolved. An unrelated audit read cannot unlock a settings retry.

Notices require an HTTPS source without embedded credentials and a valid,
nonfuture UTC confirmation time. Publication and withdrawal repeat source/time
validation and use the registered CreateNotice/PublishNotice shapes. The UI
renders source/content as text, never arbitrary media, raw HTML or an automatic
external request. Withdrawn notices remain visible only in the authorized admin
projection; actual public-feed removal and durable delivery are server duties.

Audit data is limited to IDs, known operation label, outcome, receipt and time.
Unknown operation text is replaced with `Other operation`; extra/private fields
or foreign-event/filter-mismatched rows are rejected. Filters and cursor bounds
follow C04, including actor, time, outcome and page ordering. Detail uses only
the currently authorized page and is cleared on page/filter/permission changes.
Raw report text, action reasons, notice bodies and private media are absent.

## Authority and lifecycle

`createOperationsController` accepts the existing canonical Auth `session`,
`privacy` and `request`; it creates no identity, HTTP client, QueryClient,
socket or world instance. All reads/writes use the frozen operation registry
and captured actor/event/privacy generation. Private work runs through the
canonical privacy coordinator and is fenced again before results are retained.

`OperatorAuthorityPort` and `OperationsClockPort` are narrow external owner
ports, not invented HTTP DTOs. A role lease must be `OPERATOR`, bind the actor,
event and privacy scope, have a positive policy revision, nonfuture checked time
and exclusive expiry. This frontend additionally imposes a conservative
60-second maximum checked-age ceiling. The server-time estimate must be scoped
and monotonic; there is no `Date.now` fallback. Bootstrap flags and Boolean
admin checks never substitute for a lease. MEMBER/MODERATOR, absent, stale,
expired, rebound or throwing producers fail closed before private requests.

Identity/event/policy changes, role withdrawal/change, stale clock, expiry and
disposal synchronously abort owned work, scrub snapshots/confirmation inputs and
release authority/clock/place subscriptions. Late results cannot repopulate the
new scope. A withdrawn controller cannot silently reopen under a new identity
or authority; composition creates a new owner with newly qualified inputs.

Place writes additionally require an explicit `PlaceWritePort` with a current
scope/map revision/expiry and target-specific current place revision. No default
place PUT transport is installed. A closure-only edit must preserve all current
immutable provenance. Other edits must exactly match a place supplied through
an independently reviewed new-release target. This port must be supplied by the
current-map/current-state owner, not derived from a public catalog revision.
The release is checked again at confirmation and after response; withdrawal
aborts/fences an in-flight write and preserves uncertainty. Form-entered review
names/timestamps alone never create this authorization.

## Receipts and failure semantics

Each reviewed command has one UUID key and immutable body. Confirm dispatches
once. There is no automatic replay, polling, offline queue or replacement-key
recovery. Server 403 clears private state. CAS conflict, rate-limit/read-only
rejection and missing producers have safe feedback; raw server text is never
rendered. Network/5xx/invalid response, inaccessible target and idempotency
conflict remain uncertain.

Direct success requires a bound authoritative DTO with the intended values,
target and next revision. HTTP 202 is pending only. Explicit status reads bind
actor/event/operation/key, target, status and receipt retention. `SUCCEEDED`
metadata alone cannot mark an action applied: a fresh authorized resource read
must match the receipt revision/target and intended action. If the current state
has moved on or the target is absent from the selected bounded page, the outcome
stays unresolved. The operator may explicitly page and check again; no automatic
collection scan is performed.

Missing, expired, retired and recorded-FAILED receipts never unlock replay or a
new key. Refreshes and panel navigation retain unresolved command locks. Locks
are memory-only in the retained controller. Composition must keep that owner
outside panel navigation; this handoff does not establish reload persistence or
recovery after a new browser process. Notification delivery and closure-step
completion are not inferred from local action metadata.

## Contract pins and integration seam

The base uses C04 policy candidate `0.1.2`; FE38 consumes the unchanged selected
admin/settings/place/notice/audit operations. The newer closure schema is outside
this assignment. Current SHA-256 pins include:

| Input | SHA-256 |
| --- | --- |
| `contracts/domain/v1/admin.schema.json` | `953e78a482d998e35c9612c30467600adcead3c9e9d4fb6ab7679a2164aef640` |
| `contracts/domain/v1/feed.schema.json` | `590cce6afc00c7a869b5a9711104550ba280ffa5875c9dc49a0753d272320801` |
| `contracts/domain/v1/operations.json` | `622c0e0eba6e1c835c5e953a3acd6825f97ed43aff9bc32b982c207fdfe31839` |
| `contracts/domain/v1/policy.json` | `81b0da4c007543765f120c4ea58163cc1eadea41e10d24db54e1d177ef4e5fbb` |

The publishing integrator owns route registration and role/map producer wiring.
Use one controller across these panels, supplied with canonical Auth and actual
qualified role/clock ports; dispose on shell/scope withdrawal. Import public
components from the three assigned `index.ts` modules. The missing role query
is not filled with a guessed endpoint. Until trustworthy producers are wired,
the default production operations entry must remain unavailable. Existing FE37
ownership and shared role/provider semantics remain unchanged.

## Validation and evidence

Node `24.22.2` is invoked directly from
`/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node`. The retained
pnpm `11.7.0` dependency installation is used through an explicitly authorized
temporary `frontend/node_modules` symlink; no install or upgrade is performed.
The link and owned scratch outputs are removed before handoff. Runner config
loads in memory; browser/build use `configFile: false` and owned cache directories
so final checks do not write a shared Vite cache. No root config is modified.

Commands below use frontend cwd unless indicated. `NODE` denotes the exact
absolute executable above. Evidence is under
`frontend/src/features/admin/event/__tests__/evidence/`.

| Command | Exit/result |
| --- | --- |
| `NODE node_modules/vitest/vitest.mjs run --config src/features/admin/event/__tests__/runner.ts --configLoader runner` | 0; **72 tests / 2 files**; `tests.log` |
| `NODE node_modules/typescript/bin/tsc --noEmit --incremental false` | 0; whole current frontend strict check, no shared incremental cache |
| `NODE node_modules/oxlint/bin/oxlint src/features/admin/event src/features/admin/places src/features/admin/audit` | 0 |
| `NODE src/features/admin/event/__tests__/build.mjs` | 0; production Vite transform of the FE38 fixture/dependency graph; `build.log`; bundle-size warning retained; not a default-route build/acceptance |
| `NODE src/features/admin/event/__tests__/browser.mjs` | 0; Chrome version and source/screenshot hashes in `native/report.json`; 360/390/430px |
| `python3 scripts/mcr/check-plan.py` from repo | 0; planning/source checks only |
| `NODE scripts/mcr/check-ui-origin.mjs` from repo | 0; no violations |
| `python3 scripts/mcr/check-boundaries.py --base 707ea68d585324430144c971724efbf5c73167d6 --lane frontend --allow-doc docs/mcr/frontend/event-operations.md` from repo | 0; manual exact path/import review also performed |
| `git diff --cached --check` from repo | 0 |

Controller checks cover nonoperator/absent/stale/future/expired/cross-scope roles,
CAS/403/read-only/rate/unavailable outcomes, explicit confirm/cancel, both phase
edges, source and time validation, pending/lost/malformed/foreign/retired/expired
receipts, no replay, failed-receipt retention, delayed identity/policy results,
current-map place provenance/fencing, audit redaction/filter/pagination and
cleanup. Contract tests bind field sets/methods/statuses to the actual C04 inputs.

Native Chrome runs all three feature surfaces with real React/components and
canonical Auth/privacy/request plus conspicuously synthetic HTTP, role, clock,
event and place-write ports. Every width checks denied zero reads/writes,
keyboard focus/cancel, 44px targets, no document overflow, conflict draft
retention, pending navigation without replay, source-stamped notice publication
and withdrawal, provenance-preserving closure, audit detail/redaction and
role-expiry/identity cleanup. The 390px captures were visually inspected. No
production router or real operator/backend/device journey is claimed.

The original interrupted run found a consumed fixture request-body assertion;
the recovered fixture now retains a clone and uses distinct command keys. The
first native run exposed Cancel focus loss when disabling its opener; focus is
now captured before disable and restored. Strict typing caught readonly test
fixtures; the final tests use explicit mutable copies. Final logs retain the
successful runs; scratch/cache files are excluded. Real server gates, public
notice propagation, full place-release integration, operator-role producer,
native devices and integration/QA acceptance remain separate obligations.
