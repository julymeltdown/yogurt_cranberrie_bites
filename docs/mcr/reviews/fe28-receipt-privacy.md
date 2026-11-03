# FE27 / FE28 targeted receipt and privacy review

Review date: 2026-11-04 (Asia/Seoul). QA branch: `codex/mcr-qa-FE28`.
Report-only ownership: `docs/mcr/reviews/fe28-receipt-privacy.md`.
Application source, contracts, manifests, lockfiles, fixtures and source archives
were read-only. Temporary probes used `frontend/.qa-fe28/` outside tracked
source; no existing tests or screenshots were overwritten. Temporary probes,
outputs and the correction snapshot are disposable and are not delivery paths.
The two defect reproductions are preserved inline in this report.

## Decision and source binding

Two P2 cache-resurrection defects were reproduced on the assigned candidate
and corrected in owner commit `7fecec0`. Independent execution against that exact
committed snapshot passed 32/32 focused checks, including both original failures.
At the baseline, the 23 existing focused tests passed and 15 of 17 independent
cases passed; the two race cases failed. The baseline deterministic browser
journey passed at 360/390/430 CSS pixels. No new P1 receipt-replay defect was
observed within this scope. This review does not grant FE28 or IN03 acceptance.

| Input | Pinned revision / evidence |
|---|---|
| Assigned worktree and report parent | `5bc00554c54c670b83a99d3b52823eba4de56bc2` |
| FE27 original / integrated | `97ef9fb39d72e4dfdffbc294759e4510aa874653` / `9f72a71a99091fe89411b180af3404be85d963f1` |
| FE28 original / integrated | `7e4f533b48c999860ec3ed2ad579131b979479f9` / `5bc00554c54c670b83a99d3b52823eba4de56bc2` |
| Latest requested root wiring, read separately | `8bb878c979138d0f6b3a4b880edb394f41cf99be` |
| Parallel authority | `f0e44bd1bba943b02319f9d9a42433e253e8de6f`; local document SHA-256 matches that revision |
| C04 candidate | v0.1.0 generated domain OpenAPI; SHA-256 `c47e767505e660d8f5eb69bb9759eae1f0aaee6472e4fb963edc82e060385f51` |
| Feed schema | `contracts/domain/v1/feed.schema.json`; SHA-256 `9c8d991df0bb707d7a7a2d43d4b132e7fadb878a52633e1c1f338afc9407f8ed` |
| Receipt schema | `contracts/domain/v1/common.schema.json`; SHA-256 `303f3019611c636679282d6c29106deab83d7c0c9717bd8b9ba526d2ff865a35` |

`git diff --exit-code` confirms the respective original and integrated feature
trees are equal. The newer `fc930a2` ordering/render seam and `b7d4203` protected
image consumer do not change either affected controller. They are not treated as
qualified media composition by this report. No source code was copied for product
use, and no dependency was added. Local offline installation reused 199 packages
without downloading or modifying the retained lockfile.

## Findings

### QA-FE28-01 — P2: revoked linked target returns after delayed pagination

At `frontend/src/features/event-feed/controller.ts:18`, `loadFeed(true)` captures
`previous`. `removeLink()` at lines 79–81 only rewrites current state; the pending
read later builds items from the captured previous rows at lines 29–33. This
restores a target removed in the meantime. The controller explicitly promises
not to restore an old preview. The documented destination-denial callback may
call `removeLink()` without a global relationship-policy generation change.

Reproduction: load a first page containing one PARTY link and an older cursor;
start and delay the older-page GET; call `removeLink(target)` and verify an empty
links array; resolve that GET with a valid older post. The first row again
contains `PARTY / 00000000-0000-4000-8000-000000000301`.

Impact: stale target identifiers and the related-content button reappear after
local denial/revocation. This is a frontend cache defect; destination server
authorization bypass was neither observed nor claimed. Fence stale reads or
apply scoped removal tombstones to both captured rows and incoming pages.
Cover pagination and refresh with a nonempty link. The original removal test
loops over `public-post.json.links`, which is empty, so its eviction assertions
never exercise an actual removed target.

### QA-FE28-02 — P2: withdrawn notice returns after delayed notice pagination

At the same controller's lines 38 and 47–51, `loadNotices(true)` has an equivalent
captured-list race. A concurrent `selectNotice(id)` returning 404 correctly
removes the withdrawn notice at lines 65–67. Settling the delayed page restores
the old notice object, including title, text and source URI, into list state.

Reproduction: first notices page contains a notice and an older cursor; start and
delay the older-page GET; resolve detail for that notice as 404 and verify list
items are empty; settle the page as `{ items: [], next_cursor: null }`. The
withdrawn notice returns to the list. Detail remains unavailable; no successful
forbidden detail response was needed. Fence or filter both old and incoming
notice pages after authoritative withdrawal/denial.

Both findings were sent directly to FE owner
`01a1a1fe-1ab7-79b0-adbc-d4c025701acb` and delivery
`01a1a1fc-7f3d-7ff1-9416-e3435c8a6f08`, with Social SL
`01a1a1fe-ca12-7cc3-9a1d-62bbb2689498` informed. No main/SL approval is required.
Both corrections are independently verified below against the exact owner commit.

## Focused evidence

All frontend commands use the assigned worktree's `frontend/` directory and
`PATH=/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin:$PATH`.
`pnpm` below means `node /Users/lhs/Desktop/mcr-tools/pnpm-11.7.0/package/bin/pnpm.cjs`.

| Command / probe | Result and evidence boundary |
|---|---|
| `pnpm install --offline --frozen-lockfile` | Exit 0; Node 24.22.2, pnpm 11.7.0; 199 reused, zero downloads |
| `pnpm exec vitest run src/features/event-feed/__tests__/controller.test.ts src/features/feed-thread/__tests__/controller.test.ts src/features/feed-thread/__tests__/ui.test.tsx` | Exit 0; 23/23; pinned feature candidate, no full frontend suite |
| `pnpm exec vitest run .qa-fe28/receipt-privacy.test.ts` | Exit 1; 14/16 pass; exactly the two P2 regressions above fail |
| `pnpm exec vitest run .qa-fe28/navigation.test.tsx` | Exit 0; 1/1; jsdom callback/denial proof, not production navigation |
| `node .qa-fe28/browser-smoke.mjs` | Exit 0; Chrome 156.0.8078.80; widths 360/390/430; 42 deterministic same-origin fixture requests, three writes per viewport; zero page errors or horizontal overflow |
| `python3 scripts/mcr/check-plan.py` from worktree root | Exit 0; original archive checksums match, planning only |
| `git diff --check` from worktree root | Exit 0 |

The browser runner is the committed FE28 smoke runner with only its evidence
output directory redirected to `.qa-fe28/evidence`. It exercises keyboard publish,
committed-but-503 outcome, disabled resubmission, explicit GET receipt recovery,
post/comment/reaction, privacy eviction and read-only feed. Its fixture's linked
and report callbacks are no-ops, so it proves no linked destination/report flow.
An initial pnpm invocation from the repository root failed because it has no
package manifest; it was corrected to frontend cwd. This is not a product failure.

The independent receipt/privacy probes use the canonical session, actual feature
controllers and HTTP transport with controlled fetch responses. They check:

- Actor, event, operation, key, expiry-order and success-shape receipt mismatches
  retain draft/key without another write; a known-resource reaction receipt
  cannot confirm another post.
- A 202 pending receipt survives same-identity read-only transition; selection
  and submission remain locked; status uses the original key and only GET,
  then clears the draft on confirmed success. A terminal receipt in a 202
  response remains uncertain. RETIRED never claims success or replays a write.
- Policy change and identity replacement synchronously remove private draft and
  command state; an already-started status reply cannot restore either.
- A denied detail reread evicts parent/comments and reply draft and blocks reply
  dispatch. Invalid detail IDs fail before any network request.
- The jsdom probe uses a real nonempty PARTY link; clicking it forwards only the
  typed target. Report entry forwards only POST/COMMENT type and ID. No direct
  href is created. A subsequent 403 reread removes those controls and comments.

Temporary suite SHA-256: receipt/privacy
`a91fd3f76165c3f980cc7f12af5914c1051b8c8b91791f3279006049cdba430a`;
navigation `bb9790a59850a1c390b610a51658adbd38ba60d9d239d1e7752e585aaad8577a`;
redirected browser runner
`ae690a2509a93224515f591ac28985a4c4ad21f579c8037002c2de407c83848e`.
The two failure repros are preserved inline below, independent of temporary files.

## Reachable wiring and unrun gates

Read-only inspection of root `8bb878c` shows `main.tsx` calling
`createAppRouter(ports)` without feature panel overrides. `router.tsx:16–24`
wires rooms, nearby, party and giveaway panels. It does not attach FEED, THREAD,
COMPOSER, NOTICE or REPORT feature components. These paths are registered in
`app/routes.ts` and pass the shell's canonical-ID checks, but fall back to
`PanelScreen`'s unavailable content. This is a documented integration gap, not
proof that routes do not exist and not a new blanket review gate.

Production shell retention of uncertain commands across back/tab/detail changes,
actual linked-target reauthorization and removal callbacks, FE36 report submission,
qualified FE04 upload/media composition, live BE31/BE32/BE24 services, two-account
IN03 privacy propagation, real devices, load, database and deployment are
`NOT_RUN` in this review. Source checks and deterministic responses establish no
server inventory, authorization or multiuser guarantee. No Cargo, PostgreSQL,
new build target, full frontend build or unrelated integration gate was run.

## Reproduce the two regression cases

At the pinned candidate, save the following as a temporary frontend
`.qa-fe28/cache-races.test.ts`, then run
`pnpm exec vitest run .qa-fe28/cache-races.test.ts`. Both expectations fail on
`5bc0055`; application source and tracked tests remain untouched.

```ts
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { createFeedThreadController } from '../src/features/feed-thread/controller'
import { deferred, eventId, id, post, setup, userId, notice } from '../src/features/event-feed/__tests__/fixtures'
const cleanups: (() => void)[] = []
beforeEach(() => vi.stubGlobal('location', { origin: 'https://mcr.example' }))
afterEach(() => { cleanups.splice(0).forEach(f => f()); vi.unstubAllGlobals() })
function fixture(handler: (r: Request) => Response | Promise<Response>) {
  const fetch = vi.fn<typeof globalThis.fetch>(async input => handler(input as Request)), base = setup(fetch)
  const thread = createFeedThreadController({ auth: base.auth, access: base.access })
  cleanups.push(() => { thread.dispose(); base.dispose() }); thread.start()
  return { ...base, thread, fetch }
}
it('does not restore a revoked nonempty link from an already-started pagination read', async () => {
  const older = deferred<Response>(), target = { target_type: 'PARTY', target_id: id(301) }
  const linked = { ...post, links: [target] }
  const s = fixture(r => {
    if (r.url.includes('/notices')) return Response.json({ items: [], next_cursor: null })
    if (new URL(r.url).searchParams.has('cursor')) return older.promise
    return Response.json({ items: [linked], next_cursor: 'older' })
  })
  s.controller.start(); await vi.waitFor(() => expect(s.controller.read().feed.phase).toBe('ready'))
  expect(s.controller.read().feed.data!.items[0]!.links).toHaveLength(1)
  const page = s.controller.loadFeed(true); await vi.waitFor(() => expect(s.fetch).toHaveBeenCalledTimes(3))
  s.controller.removeLink(target)
  expect(s.controller.read().feed.data!.items[0]!.links).toEqual([])
  older.resolve(Response.json({ items: [{ ...post, post_id: id(302), created_at: '2020-01-01T00:00:00Z' }], next_cursor: null }))
  await page
  expect(s.controller.read().feed.data!.items[0]!.links).toEqual([])
})

it('does not restore a withdrawn notice from an already-started notice page', async () => {
  const older = deferred<Response>()
  const s = fixture(r => {
    const url = new URL(r.url)
    if (url.pathname.endsWith(`/notices/${notice.notice_id}`)) return Response.json({}, { status: 404 })
    if (url.pathname.endsWith('/notices')) return url.searchParams.has('cursor') ? older.promise : Response.json({ items: [notice], next_cursor: 'older' })
    return Response.json({ items: [], next_cursor: null })
  })
  s.controller.start(); await vi.waitFor(() => expect(s.controller.read().notices.phase).toBe('ready'))
  const page = s.controller.loadNotices(true); await vi.waitFor(() => expect(s.fetch).toHaveBeenCalledTimes(3))
  await s.controller.selectNotice(notice.notice_id); expect(s.controller.read().notices.data!.items).toEqual([])
  older.resolve(Response.json({ items: [], next_cursor: null })); await page
  expect(s.controller.read().notices.data!.items).toEqual([])
})
```

## Correction verification and delivery

Owner fix: `7fecec0c69e6a0ce9012070782aebecd93f7aa91`, parent `ee41a3b`.
Its three changed paths are `frontend/src/features/event-feed/controller.ts`,
`frontend/src/features/event-feed/__tests__/controller.test.ts` and
`docs/mcr/frontend/social-feed.md`. Scoped removed-link and unavailable-notice
sets now filter both captured rows and incoming pages before publication. They
survive same-scope filter/refresh and clear on privacy-scope cleanup. The old
vacuous fixture now contains a real PARTY target. Four committed regressions
cover delayed pagination and refresh for both findings.

QA archived the exact committed `frontend/src`, `frontend/vite.config.ts`,
`frontend/package.json` and `contracts` into
`frontend/.qa-fe28/candidate/`, linked the already-installed owned node_modules,
and copied the two unchanged independent probe files into that snapshot.
No tracked application file was checked out, patched or rewritten. The archived
controller SHA-256 is
`0d528342c21b6d1e3c9a7038c6a243d2fb4d32e9f2e00beda6ac7c3091d83c5b`,
matching `git show 7fecec0:frontend/src/features/event-feed/controller.ts`.

Executed from the assigned root with the pinned PATH:

```sh
node /Users/lhs/Desktop/mcr-tools/pnpm-11.7.0/package/bin/pnpm.cjs \
  -C frontend/.qa-fe28/candidate/frontend exec vitest run \
  src/features/event-feed/__tests__/controller.test.ts \
  .qa-fe28/receipt-privacy.test.ts .qa-fe28/navigation.test.tsx
```

Result: exit 0, three files, **32/32 tests passed**: 15 committed FE27 cases,
16 independent receipt/privacy cases and one independent callback/denial case.
The same two original failing expectations now pass without changing their
assertions. QA-FE28-01 and QA-FE28-02 are corrected at this tested feature level.
Exact verification was sent directly to FE owner, delivery and Social SL.
The browser journey was run on the baseline, not rerun on the fix; the 32-test
snapshot run is the correction evidence. Newer media qualification remains a
separate owner's deliverable.

This report completes its bounded QA evidence and correction review; it is not
full FE28/IN03 acceptance. The persistent unbudgeted goal is completed only after
the report is committed and handed off. The report commit descends directly from
assigned `5bc0055`; it does not contain the application fix. Delivery must preserve
`7fecec0` or its reviewed integration ancestry separately. Only this report path
is handed off by QA, and only delivery publishes master. QA does not assert a
remote report SHA before delivery integrates it.
