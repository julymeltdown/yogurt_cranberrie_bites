# FE-REF-MEET — production MEET presentation

## Delivery boundary

The production `PartyDiscovery` component adopts the bottom-row second MEET reference layout. This is a bounded **MOCK_READY presentation delivery**, not real-service, full-art, device, or release acceptance. The frontend king owns root composition, the outer titlebar/paper window, shared marching footer, capability/place leases, and authorized avatar binding.

Assignment base: `f837bef676cd26869c26c172a699a91b7842d832`. Branch: `codex/mcr-fe-ref-meet`. The committed handoff records the final SHA and parent. Only assigned party-discovery UI, presentation tests/evidence, this note, and the explicitly permitted feature-index type exports changed. The former QA checkout stayed untouched.

## Reference and provenance

The actual user reference was viewed and its bytes verified SHA-256 `d9a8fae2f96a82253542019ab673589b3938627956eace15c8594c702b5c4617`. Target: the second phone in the bottom row, labeled MEET. Main preserves that same reference at `docs/mcr/design/reference-target-2026-11-04.png`; that file is outside this worker's ownership.

The source component/CSS and existing tests were read at the assignment base. This is newly authored semantic rendering and scoped CSS over the existing canonical controller. No legacy rendering, third-party icon set, new dependency, package installation, avatar derivation, or contract change is included. Small place/member/plus SVG glyphs are authored in the owned UI file. `docs/source/`, supplied art originals, manifests, and lockfiles are unchanged.

Read-only source/contract pins:

| Input | SHA-256 |
| --- | --- |
| `contracts/domain/v1/party.schema.json` | `7cbea6af623668a36d641e7498bf52f4869b9bc44594b46e7f4a3806cc907d03` |
| `contracts/generated/mcr-domain.openapi.json` | `a5fbe73124ed069e6357f5797a264e2d2a1bec93b0179bfc8550bf1122b69fde` |
| `party-discovery/model/controller.ts` | `cf39ceace4d31dd7626629000e118e7dc8722c9cde19acf992c6caea90c27093` |
| `party-discovery/model/contract.ts` | `56978d2a0637e04a6dbffa0ed8ecdefb33aacb93c2f10e0a30d5b48a87a4d7e8` |

The specification, assessment, execution plan and selected character-art guidance were read. Production user imagery remains restricted to the selected pack and reviewed derivatives. Raw fixture avatar IDs are not rendered. The shared static gallery is decorative; its existence does not authorize mapping a preset onto an owner or member identity.

## Product changes and root seam

- Content starts directly with the compact PARTIES / PEOPLE / MY PLAN strip. PARTIES is red when selected; PEOPLE navigates to `/meet/people`. There is no second MEET titlebar, outer paper frame, marching artwork, or shared-style edit.
- Each party uses a compact symmetric bordered paper row: responsive square host-image footprint, bold title, ticket section or reviewed place plus Korea Standard Time, real capacity at top right, and red JOIN at bottom right. The native fixture renders three 110px rows at each required width. No fabricated member count, privacy projection, venue distance, or arrival is added.
- JOIN navigates to `/meet/parties/:partyId` to review and apply. It sends no application and promises no approved membership. Full/closed/cancelled parties remain readable with their true state on the detail action. The original detail/application/cancellation/receipt flow remains in the existing controller.
- Filters and refresh remain usable below the rows. MY PLAN retains independent pagination and original application/cancellation/status controls. Primary controls keep 44px targets and visible keyboard focus; motion is disabled when reduced motion is requested.
- CREATE PARTY stays in the owned content footer. It is a real `/meet/parties/new` link only with both current read access and an explicit creation capability; otherwise a disabled control explains unavailability. The root's separate shared marching footer is not duplicated.

`frontend/src/features/party-discovery/index.ts` additionally exports these presentation-only types, with the existing component exports preserved:

```ts
interface PartyDiscoveryProps {
  readonly controller: PartyController
  readonly presentation?: PartyDiscoveryPresentation
}
interface PartyDiscoveryPresentation {
  readonly renderOwnerPortrait?: (owner: Party['owner']) => ReactNode
  readonly placeLabel?: (placeId: string) => string | null
  readonly canCreate?: boolean
}
```

The frontend king accepted and owns the `app/CommunityFeatures.tsx` binding. The portrait callback must use the exact current authorized owner snapshot and a reviewed selected-pack renderer. The place callback must use a current reviewed-place lease and return null when unavailable. `canCreate` comes from the current actual capability, not a read-permission guess. Callbacks run only for current accessible list rows. No per-row fetch, base64 image, era-triggered refetch, automatic preset mapping or identity fallback is implemented here.

The public party list has no authorized member-avatar projection. The row therefore says **Member portraits unavailable**, without a fake strip or an invented count of portraits. Without a qualified owner renderer, the host footprint explicitly says **Host portrait unavailable** to assistive technology and displays a neutral question mark. If no reviewed place label is supplied, actual ticket-section text is used; absent both, it says **Place unavailable**. This preserves information boundaries while the real integration proceeds.

## Validation

Node 24.22.2 and retained frontend dependencies were used through the authorized temporary dependency symlink. No dependency or browser install was performed. Available disk was checked above 8 GiB before validation. A coordinator disk hold was honored while only source/tests were authored; validation resumed after a fresh passing floor check. All native server/browser contexts are closed by the harness. The owned dependency symlink is removed before handoff.

Commands run from repository root, with the pinned Node directory on PATH:

```sh
node frontend/node_modules/vitest/vitest.mjs run frontend/src/features/party-discovery/__tests__ --maxWorkers=1
node frontend/node_modules/typescript/bin/tsc -b frontend --pretty false
frontend/node_modules/.bin/oxlint frontend/src/features/party-discovery/ui frontend/src/features/party-discovery/index.ts frontend/src/features/party-discovery/__tests__/reference-presentation.test.tsx frontend/src/features/party-discovery/__tests__/reference frontend/src/features/party-discovery/__tests__/visual-adoption/adoption.test.tsx
MCR_MEET_REFERENCE_OUTPUT="<new absolute evidence directory>" node frontend/src/features/party-discovery/__tests__/reference/browser.mjs
python3 scripts/mcr/check-boundaries.py --base f837bef676cd26869c26c172a699a91b7842d832 --lane frontend --allow-doc docs/mcr/frontend/reference-meet.md
git diff --cached --check
```

Observed 2026-11-04:

| Check | Exit / result | What it proves |
| --- | --- | --- |
| Affected Vitest suite | 0 / 71 tests, 5 files PASS | Existing contracts/controller/UI cases plus five new presentation cases; no weakened recovery assertions |
| Application strict TypeScript | 0 | Production component, optional public props, fixtures and tests typecheck |
| Scoped Oxlint | 0, no warnings | Owned changed source/tests pass lint |
| Native Chrome 156.0.8078.80 | 0 / 360, 390, 430 PASS | Production component with frozen contract fixtures; layout and interactions below |
| Planning provenance | 0 / PASS | Original archives unchanged; planning baseline is not an implementation update |
| Frontend boundary / staged whitespace | 0 | Assigned paths and no cross-lane imports; manual ownership/import review also performed |
| Real services, physical devices, full root composition, real portrait/member art | NOT_RUN | Not inferred from fixture results |
| Separate production bundle build | NOT_RUN by this worker | Bounded component browser transform and application typecheck ran; king owns combined integration build |

The first native attempt failed because Vite treated port zero as its occupied default port. The harness now reserves a private ephemeral port and closes that reservation before starting Vite. A second attempt rejected an invalid frozen meeting end time; the fixture was corrected to a valid chronological window. Subsequent runs passed. These harness setup failures are preserved here rather than represented as product defects or successful first attempts.

Native checks at every required width: reference tab order and selected state; three immediately visible dense rows; no fake image/canvas; all visible controls at least 44px; no page/feature horizontal overflow; visible focus; JOIN navigates without POST; browser back restores focus; real PEOPLE and CREATE route dispatch; independent MY PLAN; full-party state; empty list; privacy invalidation removes rows and disables creation; zero page errors. Existing controller tests retain uncertain/pending/idempotent receipt, cancellation, access and capacity coverage. The former broad browser-smoke script was not rerun because this patch uses the bounded affected suite and the new three-width harness.

The committed [native report](../../../frontend/src/features/party-discovery/__tests__/reference/evidence/report.json) pins the actual component/CSS/contract hashes and row bounds. Captures are frozen test data with an explicit fixture caption, not the composed real application:

- [360px parties](../../../frontend/src/features/party-discovery/__tests__/reference/evidence/360-parties.png), [390px parties](../../../frontend/src/features/party-discovery/__tests__/reference/evidence/390-parties.png), [430px parties](../../../frontend/src/features/party-discovery/__tests__/reference/evidence/430-parties.png)
- [360px privacy](../../../frontend/src/features/party-discovery/__tests__/reference/evidence/360-privacy.png), [390px privacy](../../../frontend/src/features/party-discovery/__tests__/reference/evidence/390-privacy.png), [430px privacy](../../../frontend/src/features/party-discovery/__tests__/reference/evidence/430-privacy.png)

All three party captures and a privacy capture were visually inspected against the actual reference. The final pass tightened portrait proportions and moved place/time directly under the title. The compact tabs, row hierarchy, capacity position, JOIN controls and large footer action are present. These are structural comparisons, not a pixel-identity score. Header/footer/paper chrome in the harness is explicitly test-frame composition.

## Remaining integration and visual limits

Owner portraits require a genuine identity-to-reviewed-art binding. A decorative static gallery is insufficient. Member strips require an actual currently authorized member projection. The king must bind the optional ports and verify the component in the actual root window with the root-owned shared footer. Until that occurs, full reference fidelity and real image composition remain open; this delivery does not claim FE21 or IN02 service acceptance. Concrete follow-up fixes stay with this writer, without an extra SL approval gate.

## Exact changed paths

```text
docs/mcr/frontend/reference-meet.md
frontend/src/features/party-discovery/index.ts
frontend/src/features/party-discovery/ui/PartyDiscovery.tsx
frontend/src/features/party-discovery/ui/party-discovery.css
frontend/src/features/party-discovery/__tests__/visual-adoption/adoption.test.tsx
frontend/src/features/party-discovery/__tests__/reference-presentation.test.tsx
frontend/src/features/party-discovery/__tests__/reference/browser.mjs
frontend/src/features/party-discovery/__tests__/reference/preview.css
frontend/src/features/party-discovery/__tests__/reference/preview.html
frontend/src/features/party-discovery/__tests__/reference/preview.tsx
frontend/src/features/party-discovery/__tests__/reference/evidence/report.json
frontend/src/features/party-discovery/__tests__/reference/evidence/360-parties.png
frontend/src/features/party-discovery/__tests__/reference/evidence/390-parties.png
frontend/src/features/party-discovery/__tests__/reference/evidence/430-parties.png
frontend/src/features/party-discovery/__tests__/reference/evidence/360-privacy.png
frontend/src/features/party-discovery/__tests__/reference/evidence/390-privacy.png
frontend/src/features/party-discovery/__tests__/reference/evidence/430-privacy.png
```
