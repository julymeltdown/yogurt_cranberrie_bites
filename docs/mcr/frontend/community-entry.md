# Community route adapters — FE22 / FE23 / FE25 / FE26

Status: **MOCK_READY**. Base: `dc8dee7916a51d3498d2f77ca858b37b97daad44`.
Worktree: `/Users/lhs/Desktop/mcr-worktrees/fe-community-entry`.
Branch: `codex/mcr-fe-community-entry`. This is a new assignment; the former
community-authoring checkout remains stopped. No root composition is delivered
by this patch. IN02, live services, actual devices and formal acceptance are
**NOT_RUN**.

## Ownership and provenance

Only `frontend/src/app/community-entry/**` and this document are authored here.
The adapters use existing feature exports at the base above, including FE22/23/25
and FE26 delivery ancestry `d8778b8` / `941d36c`, and the independently delivered
meeting-entry clock type. C04 public types, generated domain operations, runtime
decoders and fixtures remain owned by their original modules. No backend code,
historical rendering, visual assets, manifests, locks, root providers, routers,
WorldShell or `docs/source/` bytes are changed. All adapter rendering is newly
authored semantic React; existing MCR feature components retain their ownership.

The fixture reuses existing synthetic party data and the C04 giveaway-management
fixture through feature test helpers, but creates one canonical Auth controller
and the two actual discovery controllers per fixture. It imports no backend
implementation. No new dependency is introduced. Browser evidence records hashes
for the adapter and feature source files it exercises. Synthetic places, people,
time and policy must never become production guidance or approved artwork.

## Integration seam sent to delivery before implementation

`CommunityFeatures.tsx` currently owns private `PartiesContext` / `GiftsContext`.
The adapter does not allocate or dispose FE21/FE24 replacements. Its required
`CommunityEntryRelease.parties` and `.gifts` are the exact existing objects.
Delivery can export this small read-only accessor from that reserved file:

```tsx
export function useCommunityControllers() {
  return { parties: useContext(PartiesContext), gifts: useContext(GiftsContext) }
}
```

Alternatively, the existing provider can pass its current owner references
directly. Build the release once per validated shell owner, not on each panel
render. Mount `CommunityEntryProvider` inside that provider and outside panels:

```tsx
<CommunityEntryProvider auth={auth ?? null} release={qualifiedRelease}>
  {children}
</CommunityEntryProvider>
```

Merge exported `communityEntryPanels` into the existing integrator-owned panel
map. The adapter registers no router. It covers M02, M04, M05, G02, G04, G05 and
G06 at the existing specification paths. Existing FE21/FE24 discovery/detail and
MY GIFTS panels continue using their existing controllers. Root composition and
discovery navigation affordances remain delivery-owned.

`ports.ts` is the exact typed contract. Required sources are `scope`, `signal`,
borrowed `parties` / `gifts`, `access: PartyAccessPort` and
`context: GiftContextPort`. Optional qualified inputs are:

| Input | Required owning-source meaning |
|---|---|
| `clock` | Existing `MeetingClockPort`: scoped advancing server-time estimate, with withdrawal notifications. No `Date.now()` fallback. Missing clock prevents all new mutation-controller allocation. |
| `places` | Scoped current reviewed public place choices, with read/subscribe. It is a label projection, never physical-arrival authority. Missing/wrong-scope choices leave authoring submission disabled. |
| `rally(partyId)` | Existing FE23 qualified authority with exact party, meeting revision, explicit authorized description and expiry. Never derive it from a public place or latest unrelated beacon. |
| `pickupPlace(giveawayId?, reservationId?)` | Exact current FE26 place/map revision and expiry authority. Missing authority hides private pickup guidance and code issuance. |
| `pickupMeeting(giveawayId?)` | Exact FE25 qualified private pickup plan, bound to scope/giveaway and expiry. No invented plan. |
| `image(giveawayId?)` | Borrowed FE04 qualified image port for that authoring owner; the image owner retains disposal. No token plumbing or new uploader is created here. |
| `journal` | Optional existing FE26 intent journal. Default FE26 sessionStorage behavior remains fail closed if unavailable. Never supply an in-memory production replacement. |

No current production adapter is claimed to supply these optional sources.
Missing sources remain unavailable; this patch does not upgrade FE19/FE20/C04
authority projections. Capabilities still represent availability, not resource
authorization. Existing server-bound controllers enforce their own actor,
revision, response decoder and mutation checks.

## Lifetimes and recovery

Allocation occurs in effects, including under React StrictMode. Each captured
release retains authoring/management controllers by target across panel closes.
Old route selections never render while a new route selection catches up.
Invalid UUID routes allocate no controller. Owner pickup opens only from an
authorized reservation row while retaining both giveaway and reservation IDs;
the naked pickup route remains recipient-authorized by FE26.

There is one pickup writer per release because FE26's journal is actor/event
scoped. An unresolved/loading/sending/checking pickup cannot be replaced by a
different target. The requested target stays closed and displays explicit
original-command status recovery plus a retry control. Known outcomes permit
replacement. Neither navigation nor reload resends a command.

Release abort or privacy replacement synchronously withdraws the adapter and
disposes only owned FE22/23/25/26 controllers. Borrowed discovery, Auth, query,
media, world and socket owners are untouched. The feature privacy cleanup also
clears old-scope pickup journal metadata. Clock withdrawal after controller
allocation retires the release while preserving unfinished command identity;
the shell must provide a **new qualified release object** to resume. A clock
update never turns local/world presence into arrival.

Controller maps retain drafts and uncertain non-pickup command bindings for the
captured shell lifetime. They are intentionally not evicted on panel navigation;
they are released on shell/privacy withdrawal. Backend status, ownership and
fresh revisions remain authoritative after any retained view becomes stale.

## Verification

Node runtime: `24.22.2`. Existing pnpm `11.7.0` lock resolutions are retained.
Delivery supplied an untracked `frontend/node_modules` symlink. It is preserved
and excluded from the commit. pnpm's automatic dependency check attempted a
noninteractive reinstall and failed before running the script; validation then
used the pinned Node runtime and existing tool entrypoints directly, without
purging or installing into the shared dependency target.

All final commands below exit **0** unless explicitly described otherwise:

| Check | Command / result |
|---|---|
| New adapter tests | `node frontend/node_modules/vitest/vitest.mjs run --root frontend src/app/community-entry/__tests__` — 20 tests / 2 files pass. |
| Combined affected regressions | Same Vitest entrypoint with community-entry, meeting-entry, party-authoring, party-management, giveaway-authoring, gift-pickup, party-discovery, gift-discovery and the domain-request test selector — 217 tests / 15 files pass. |
| Strict TypeScript | `node frontend/node_modules/typescript/bin/tsc -b frontend --pretty false` — pass. |
| Scoped lint | `frontend/node_modules/.bin/oxlint frontend/src/app/community-entry` — pass. |
| Existing application build | `node frontend/node_modules/vite/bin/vite.js build frontend` — pass; existing >500 kB chunk warning remains. This root build does not imply adapter registration. |
| Adapter production bundle | `node frontend/src/app/community-entry/__tests__/build.mjs` — standalone fixture entry includes actual adapter code, then removes its temporary output. |
| Native browser | `node frontend/src/app/community-entry/__tests__/browser.mjs` — Chrome `156.0.8078.80`, widths 360/390/430/1280, no page errors. |
| Planning | `python3 scripts/mcr/check-plan.py` — pass; planning-only scope. |
| Source origin | `node scripts/mcr/check-ui-origin.mjs` — pass; static-origin scope only. |
| Boundary | `python3 scripts/mcr/check-boundaries.py --base dc8dee7 --lane frontend --allow-doc docs/mcr/frontend/community-entry.md` with invocation-only `core.excludesFile` pointing to the committed exact `/frontend/node_modules` local exclusion — pass. Raw invocation reports the provided untracked symlink as unsupported; no source finding is waived. |
| Whitespace | `git diff --check` and staged counterpart — pass. |

Browser report and screenshots: `frontend/src/app/community-entry/__tests__/evidence/browser/`.
Checks exercise retained drafts; all seven panels; missing physical authority;
giveaway-bound owner pickup; unknown command across panel navigation; actual
sessionStorage reload without POST replay; original status reads; privacy journal
scrub; clock withdrawal; and cleanup. Component tests additionally verify stale
IDs, missing/wrong-scope ports, single pickup writer and exact borrowed ownership.
Browser fixtures run via Vite development mode. Actual iOS Safari, Android Chrome,
assistive technology, real two-account backend flows and root integration remain
NOT_RUN. Screenshots do not establish production place provenance or acceptance.

## Handoff

Delivery should inspect this patch's explicit changed paths against the two owned
roots, integrate serially, wire the reserved provider/router from qualified
sources, and run combined root checks. Only delivery pushes master. The commit
ID is reported with the committed handoff; ancestry starts at the pinned base
above. No whole-task INTEGRATED/ACCEPTED or release-readiness claim is made here.
