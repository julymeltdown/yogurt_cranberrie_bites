# Meet CSS foreground parity

Task: CSS-PARITY-MEET. Status: MOCK_READY for the bounded foreground CSS, published on master. Full photo parity remains open. The objective is unfinished.

The task owns only `frontend/src/features/party-discovery/ui/party-discovery.css`, this document and `docs/mcr/evidence/css-parity-meet/**`. Root153 alone integrates and publishes. No technical approval stage applies. The isolated branch is `codex/mcr-fe-css-parity-meet`, initially fast-forwarded to `54320d71c4ae0bff3524d1b500cea772e0ba42e0`, then to `29a42ca1` after fetching origin/master before edits.

Root153 granted the exclusive foreground whole-file lease after COM01 stopped writing. Root explicitly supplied its qualified input from `91058462`: 9145 bytes, SHA256 `487c1b051bfc246b2f3e76e6f31174f96d3da7960be30b7489e8189acf0f3051`. This exact input was materialized before foreground authoring. The final summary focus selector with `outline-offset: -3px` is Root/COM01's supplied fix, not this worker's authored fix. Preserve it during integration. The first foreground source commit is `fedb12cb7ceb651910641d0e15f6e43381a66151`.

Before native admission the task branch was rebased onto current published `baad4944f4cf3e6ee3156436d8044c543a780d44`, which includes the qualified Root focus fix and a disjoint Landing handoff. Rebase completed without conflicts; the worker's current product diff no longer includes the Root focus append. The earlier source SHA above is historical provenance, not the current rebased delivery SHA.

## Reference and comparison

The current human photo is `1-Photo-1.jpg` from attachment `AB983BDC-49A0-4F5E-A56F-9F45A5560B6D` in frontend visual delivery chat `01a1a385-9f32-73c2-8715-ff54bd1e0497`. Root supplied SHA256 `2b3988d546f87b813d78ca13bf267730a6dd19118bd6535a2d3c3032b6f12d23`. The original is read-only. The evidence crop selects `(260,450)` to `(501,848)` in the 1280×853 photo; it is comparison evidence, never product artwork. Use the actual product DOM through `frontend/src/app/__tests__/cozy-backgrounds-v1/preview.html?entry=%2Fmeet&era=PARADE`, not a recreated static screen. This fixture uses current selected whole-character host projections and the default router/renderer, with synthetic HTTP/C05 and test map admission.

The Meet panel contains three compact party rows, square host portraits, bold condensed titles, location/time metadata, member portraits, capacity at upper right and Join at lower right. The three tabs and centered Create party action remain real controls. Primary controls retain at least 44px target dimensions even where the photo's scaled painted control is smaller.

## Exact non-CSS blockers

- `DiscoveryRow` in `frontend/src/features/party-discovery/ui/PartyDiscovery.tsx` hardcodes `Member portraits unavailable`. There is no authorized member portrait strip or renderer in the presentation interface. CSS cannot create member identities or art. An implementation owner must supply current authorized projections and markup.
- `PartiesPanel` in `frontend/src/app/CommunityFeatures.tsx` supplies a selected whole-character owner renderer when a runtime exists, but only accepts validated current whole-character configs. The older `reference-integrated` fixture uses synthetic config metadata and shows unavailable host portraits. Root's cozy fixture binds current selected whole-character configs; use it for current parity evidence. Neither fixture authorizes fabricating real host identity or member portraits.
- Root supplies neither `placeLabel` nor `canCreate` to the current panel. Reference place names/times and enabled creation cannot be fabricated by styling. Creation remains disabled with its explanation until the owning implementation supplies current authority.
- Shared titlebar/chrome/footer geometry is outside this worker's CSS scope. Material shared-frame differences remain a separate owner request.

These requests were sent directly to Root153. No TSX, assets, background paint, selected characters, World, shared shell, manifests, lockfiles or backend changes are authorized here.

## Validation and delivery

Native browser rounds must be admitted by resource21d. Required: current 390px foreground comparison, 360/430px captures, overflow/reachability, keyboard focus, tabs/filter/detail navigation, long-title wrapping and truthful full/closed/unavailable states. Fixture checks do not establish live-service, device or full visual acceptance. Evidence and final commands/exit codes will be recorded here with the committed handoff.

The first source pass set 118px minimum party rows, added 6px copy separation, placed metadata in the title link's clear lower hit area while retaining at least 44px, and narrowed Create party to 216px. Native iteration subsequently made row height and copy separation responsive and removed spare footer padding. Decorative background declarations and every portrait rule remain unchanged.

## Historical admission checkpoint

Resource21d has the concrete READY packet. Its latest queue report says roughly 14 GiB free is below the 16 GiB start threshold. No named offer has been received and no native browser, test, build or compiler job has run. Node and retained dependency lock paths were inspected read-only; both checkout and retained lock hashes are `47ca44459819ad0e887d2f4f93394fdf3b1053b8f45edc9ceb0236981feee1f6`. The driver uses the pinned Node 24.22.2 runtime, one private Chrome/Vite workflow, a private cache/temp directory, the existing root fixture and explicit 32/128/160 MiB durable/temp/whole caps. Baseline restores only the exact qualified Root stylesheet input; candidate uses the worker stylesheet. No background or character substitutions are involved.

The same admission blocker was revalidated across three consecutive goal turns. The third turn's read-only `df -k .` reported 14,708,788 KiB available (below 16 GiB); the coordinator remained active without issuing a named offer. The checkout remained clean at `a80b8aadd5e357d378ee73a7e00005c2b60d01e1` before this documentation receipt, and `frontend/node_modules` was absent: no dependency link, private browser, Vite server or output workflow has been started. The persistent goal is blocked rather than complete. Resume the existing queued workflow after fresh admission; do not treat these source commits as validated delivery or visual acceptance.

## Native execution and visual iteration

The later human-authorized continuation resumed the same preserved checkout and objective. The branch was reconciled onto published `20682b5736838a4acbdc79e000350834cc553961`; accepted clean source was `002184eeb93e6a471f8e132400594a63379337f9`. During the lease only this worker's CSS changed. No shared, character, backend or fixture product source was edited.

Resource offer `CSS-MEET-FIRST-202611041147Z` was explicitly accepted at **2026-11-04 11:47:36 UTC**. Fresh free space was 20,048,203,776 bytes, both lock pins matched, port 5298 was free, and the checkout dependency link was absent. A supervisor existed before native children and enforced the absolute 11:51:36 work cutoff, 11:52:36 release deadline, 8 GiB continuous floor and byte caps. The workflow used `/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node`, its bin directory first in child PATH, retained read-only dependencies, private Vite cache/TMP/Chrome, and `configFile:false` with the existing React/Tailwind plugins.

Commands used the same absolute Node executable and `docs/mcr/evidence/css-parity-meet/capture.mjs TAG` from this checkout. `MCR_MEET_BASELINE=1` applied only to `baseline-01`.

| Round | Widths | Exit | Finding and response |
|---|---|---|---|
| `baseline-01` | 390 | 0 | Exact Root `91058462` stylesheet; reference comparison baseline. |
| `candidate-01` | 390, 360, 430 | 0 | Harness passed, but visual inspection found third-row clipping near the action area. Passing assertions were not treated as parity. |
| `candidate-02` | 390, 360, 430 | 0 | Removed 18px spare footer padding and moved copy toward the reference column. The 390px card was fully visible; 360px still clipped the last row. |
| `candidate-03` | 390, 360, 430 | 0 | Responsive row heights 109/118/130px, copy inset 6–12px and a 4px small-screen outer gap keep all three cards visible. This is the delivered CSS postimage. |

Final CSS SHA256: `b6e0523eea6105e700d01d905b39027026feae6e5e21d4ad945bf9afa45d7f29`, matching the saved native report. Chrome was `156.0.8078.80`; Node was `v24.22.2`. The original photo was read back and matched the supplied SHA256. Saved analysis is in `docs/mcr/evidence/css-parity-meet/saved-analysis.json`.

At all three widths: no horizontal overflow or visible controls below 44px; all three Join links were 68×44px, hit-testable and focusable; Enter opened party details without a mutation; MY PLAN and filter keyboard interactions worked; long titles wrapped without horizontal overflow; the forced-colors summary retained a visible 3px ring with -3px inset. The 390px baseline/final host canvas RGBA hashes, 64×64 render dimensions, object fit and pixel rendering were identical. Selected host art was retained at 360/430 as well. The guarded unavailable creation state and its explanation remained visible. Full/closed party runtime fixtures were not added or run; their action logic and styling were not changed.

Native captures were inspected at reference aspect ratio, with an additional 390×844 capture. The final foreground title/metadata column, responsive card cadence and 216px action width move toward the photo. The following material gaps remain and were requested directly from Root153:

- Member portrait strips require authorized data, presentation API and markup. The unavailable text remains truthful.
- The supplied fixture repeats one selected host config; the photo has distinct hosts. CSS must not replace selected characters or change their framing/scale.
- Current authorized place labels, reference meeting times and creation capability are absent from the root presentation. No fabricated content or enabled action was introduced.
- The root-owned titlebar has additional controls and a different height/treatment. The root parade footer is a separate sibling after the panel body, while the photo places Create party over the parade near the bottom. At 390px the actual action begins near y496, whereas the scaled photo action begins near y573. Reproducing that composition requires the shared shell/footer owner; fixed-position escape or shared overrides were not introduced. At 360px the existing filter/refresh controls remain reachable by scrolling even when not in the initial view.
- Original background paint remains intentionally outside this foreground task. Actual devices, live services and full visual acceptance are NOT_RUN/open.

The local optional `candidate-04` request was rejected before child launch by the original clock guard. No fourth browser or source/planning-check job ran, and no implicit extension was used. Its NOT_RUN receipt is retained; the unrun driver addition was removed. `python3 scripts/mcr/check-plan.py` was NOT_RUN in this CSS round. `git diff --check` passed with exit 0 after source authoring.

## Release and committed handoff

The supervisor ended with FULLRELEASE at **11:51:36.093943 UTC**, before the release deadline. Port 5298 was free; owned native processes were terminal; the dependency link, private TMP/profile and Vite cache were absent. Peak temporary/durable/whole use was 7,152,106 / 26,086,464 / 33,238,570 bytes. Minimum free space was 19,761,586,176 bytes. Durable evidence after cleanup was 3,850,637 bytes. See `lease-01.json`; failed visual iterations are retained.

Publishing route remains directly to Root153 (`01a1a153-1227-7e72-801f-a2a6b91981e1`). The cumulative handoff starts after `20682b5736838a4acbdc79e000350834cc553961`; the final commit alone is incremental and does not replace the earlier CSS changes. Root owns normal integration/publication and affected combined checks. Publication/remote ancestry will be recorded after actual delivery. No king/SL review or acknowledgment is required.

Root published the cumulative worker handoff `bdb10a2f3b8e047df77220c602f07a2190c0950d` as `02fdd6a1e2c2125918c0f25037be653771290305` on `origin/master`. The implementation owner fetched the remote and read back every one of the 69 handed-off Git file objects: all bytes matched, including the final CSS SHA256 above. `git merge-base --is-ancestor 02fdd6a1e2c2125918c0f25037be653771290305 origin/master` returned 0. The remote tip at readback was that same delivery commit. `remote-delivery.json` records per-file SHA256 and ancestry. This proves publication of the bounded patch, not full visual or live-service acceptance. No resource was reacquired for this Git-object readback.

The same persistent goal object was preserved throughout. Its API still reported BLOCKED on explicit resumption and exposes no resume operation; no replacement goal or false completion was created. Implementation work and native validation resumed under the existing objective. Publication is proved; the goal is not complete while the exact content/shared-shell limits remain unresolved.
