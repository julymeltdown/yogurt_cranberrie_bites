# FE03 form and intro policy helpers

Status: IN_PROGRESS for FE03 integration/acceptance; bounded implementation and focused validation complete for committed delivery; integration and task acceptance remain with delivery evidence. No live social, verification, publication or release acceptance is claimed.

## Assignment and goal

Worktree `fe-form-policy-helpers`, branch `codex/mcr-fe-FE03-helpers`, exact base `aedbb020e637993fe9ef1831e31bd6ae5316ae0c`. The existing unbudgeted FE03 persistent goal remains the sole goal. Its completion criteria were extended by Main's explicit ownership transfer to include the production Unicode correction, consumer regression, native width checks, documentation and committed handoff. The goal API supports status changes only; no replacement goal was created. Final actual goal status is reported with the handoff.

Original ownership: new `frontend/src/shared/validation/**`, new `frontend/src/entities/intro/model/**`, and this document. Additional transferred ownership: `frontend/src/features/intro/contract.ts`, `IntroEditor.tsx`, new `__tests__/unicode-length.test.tsx` and narrowly named `__tests__/evidence/unicode-length/**`. Main confirmed the former writer stopped at clean `3477041257187806392119a25a8c15218d2a84e5`. Both transferred consumer files were independently verified byte-identical at that revision, the initial probe revision and this base. No controller, existing tests, people/profile/age feature, API/privacy/Auth, contract, central manifest, lockfile, root composition or backend modification is included.

## Delivered behavior

- `shared/validation/text.ts`: audited code-point length, bounded text, trim-then-validate tags and duplicate-aware tag arrays. Code points include combining marks and joiners; they are not grapheme clusters. Input is never normalized or truncated. Tag bounds apply before and after trimming, duplicates are case-sensitive after trim, and JavaScript trim behavior is retained.
- `shared/validation/tags.ts`: audited comma parsing, one leading hash removal, stable exact deduplication and limit. This intentionally retains the source's `# space` result (` space`); callers validate with `tagArray`. Limits are caller-supplied nonnegative integers. Tags are utilities only; no C04 intro field was added.
- `entities/intro/model/age.ts`: required paired-null or paired-number bounds, `14 <= minimum <= 100`, `15 <= maximum_exclusive <= 101`, and strictly increasing half-open endpoints. Unknown age matches only unrestricted discovery conditions and never a sharing condition. These arithmetic predicates establish neither verification nor authorization.
- `model/contract.ts`: strict C04 SetIntro, owner/public intro and AgeStatus validators with code-point limits, canonical UUID spelling, current revisions/timestamps and C03/C06 public profile/avatar shapes. Public extra age, bounds, expiry, owner, handle and claim fields are rejected; owner/status DTOs also reject extra handle/claim/age fields. Parsing a status never creates a verified badge. Structural decoding is separate from policy and authorization.
- `model/errors.ts`: current intro-write reason enum derived from the frozen generated operation registry, without inventing legacy age/contact error reasons.
- `model/publication-contact.ts`, `policy.ts`: contact normalization/detection is confined to intro publication policy. Ordinary text/tag helpers allow links/contact-like prose. Advisory BE25 checks include empty drafts versus nonempty publication/closed text, no NUL, nil identity rejection, creation time, event cutoff and future optional expiry bounded by cutoff. Contact policy applies when publishing or already published, matching BE25; closed content is not newly subjected to publication contact rules. The server still owns all publication/access/identity/consent decisions.

The new policy detector uses Unicode `Alphabetic` plus `N` for BE25 alphanumeric classification (including alphabetic combining marks) and adapts the source's domain neighbors to whole code points rather than UTF-16 halves and uses Unicode `White_Space`, matching Rust BE25. For example, `𝒜x.com`/`x.com𝒜` remain ordinary text, `💡x.com` is detected, NEL inside a URL is collapsed and BOM is not. Policy nonempty checks use the same whitespace semantics. Existing consumer whitespace handling is unchanged.

## Production correction and preserved baseline

The original actual-module probe is preserved byte-for-byte at `frontend/src/shared/validation/__tests__/evidence/prerequisite-probe-572473f.json`. It comes from `572473fe344fe0f2a933f352cc6a409fd70aceaa`: the actual old draft schema accepted 500 emoji and rejected 501/1000/1001; C04 accepted 500/501/1000 and rejected 1001. The old editor's `maxLength=1000` additionally imposed UTF-16 truncation.

The authorized correction replaces only the draft and owner text validators with `boundedText(0, 1000)`, removes native `maxLength`, and adds a described code-point count plus `aria-invalid` and an over-limit field alert. The complete draft, including over-limit and composition input, stays editable. The unchanged controller blocks an invalid save through the canonical draft schema and sends no write; valid 1,000-emoji drafts and owner responses now round-trip unchanged. Receipt/reconciliation, privacy, publication and expected-revision behavior remain unchanged.

One existing UI limitation remains: after a successful save, `command.message` takes precedence over a subsequent local validation message in the global status paragraph. The visible described field alert explicitly says to shorten the draft; the native check confirms the rejected save adds no write. The first native assertion incorrectly expected the global status to change, failed, and was corrected to verify the actual field alert and write count. This patch does not alter receipt status precedence. The initial failure is preserved explicitly in `__tests__/evidence/unicode-length/initial-native-status-assertion.json`.

Read-only remaining consumers still use their original text validators: people/profile public intro text and profile display text were not transferred. Consumers adopting the new strict projection/policy helpers require separate scoped integration; no claim is made that all frontend validators now use code-point semantics. The actual intro draft/owner consumer gap assigned here is corrected.

## Source and dependency provenance

Source repository: `julymeltdown/bigyeting`, pinned revision `5eecdcaffd1a9d505b9ab2a974f8d5623217e90a`. All five inventoried source files and three regression references were verified against both Git objects and on-disk bytes, including inventory blob IDs and SHA-256. Exact paths/hashes and consumer byte comparisons are recorded in `frontend/src/shared/validation/__tests__/evidence/source-verification.json`.

| Selected source | Destination and adaptation |
| --- | --- |
| `frontend/src/lib/api/real/canonical-social-intro-schema-core.ts` | `shared/validation/text.ts`: codePointLength, boundedText, tagSchema, tagArray; English messages only. `entities/intro/model/publication-contact.ts`: contactBypass renamed and Unicode neighbor/whitespace correction above. `model/age.ts`: paired-null half-open visibility algorithm with gender removed and required C04 key names preserved. |
| `frontend/src/features/social-intro/social-intro-model.ts` | `shared/validation/tags.ts`: parseTagInput only; no provider model, persistence or UI imported. |
| C04 common/social, C03 PublicProfile and C06 AvatarConfig | Newly authored `model/contract.ts` and `errors.ts`; canonical IDs/statuses/current error reasons replace old prefixed IDs/provider-specific fields. Existing public headless C06 schema evaluator is reused; no avatar rendering is imported. |
| BE25 intro/age policy and its evidence document at the base | Read-only compatibility reference for newly authored advisory policy/age predicates. No Rust/backend imports or runtime dependencies. |

The other inventoried validation-policy files and source regression files were byte-verified as audit references, not copied wholesale. No old rendering, media workflow, provider-specific identity, Auth-return helper or safe-return-target code is imported. New runtime dependencies remain retained Zod and existing public headless contract utilities. Consumer UI uses retained React/router. Tests use retained Vitest, Testing Library/jsdom and Python `jsonschema` 4.26.0 with local `referencing` resources, with no network schema resolution or dependency installation. Browser evidence uses retained Playwright against installed Google Chrome. Node is `/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node` (24.22.2).

## Validation

Commands below run from `frontend/` unless explicitly marked repository root. `NODE` denotes the retained absolute Node binary above. Every final command exited 0; after those runs, one trailing blank line was removed from `text.ts` for a clean diff check, with exact tested/delivery hashes recorded in browser/build evidence; executable text is unchanged; initial test-only type/native/build-harness failures were corrected before the final run.

| Command/check | Result and scope |
| --- | --- |
| `NODE node_modules/vitest/vitest.mjs run --configLoader runner --no-cache src/shared/validation/__tests__/text.test.ts src/entities/intro/model/__tests__/policy.test.ts src/features/intro/__tests__/unicode-length.test.tsx src/features/intro/__tests__/intro.test.ts` | Focused helper, actual UI/save/own-decode and existing intro receipt/privacy tests. 70/70 tests across four files passed before peer feedback; the final 41/41 policy-only rerun includes three additional alphabetic-combining-mark regressions (73 distinct focused tests total). Unrelated passing tests were not repeated. |
| `NODE node_modules/typescript/bin/tsc --noEmit --incremental false --pretty false` | Whole frontend strict types; no shared TypeScript cache writes. |
| `NODE node_modules/oxlint/bin/oxlint src/shared/validation src/entities/intro/model src/features/intro/contract.ts src/features/intro/IntroEditor.tsx src/features/intro/__tests__/unicode-length.test.tsx src/features/intro/__tests__/evidence/unicode-length` | Affected lint. |
| `NODE src/features/intro/__tests__/evidence/unicode-length/production.mjs` | Production build explicitly includes every new helper entry and the actual editor/controller fixture; 197 modules. No central/root entry was edited. All cache/output paths are private temporary directories. |
| `NODE src/features/intro/__tests__/evidence/unicode-length/native.mjs` | Installed Google Chrome 156.0.8078.80 at 360/390/430/768/1280/1440: 500/501/1000 accepted, 1000 saved unchanged, 1001 retained and rejected without an extra write, described count/error, no horizontal overflow or page errors. Twelve screenshots and source-bound report adjacent to runner. Manual image inspection covered phone over-limit and desktop valid states. The additional requested 430-width check used `MCR_FE03_WIDTHS=430` and source-hash-verified merging of earlier passing width evidence; unrelated checks were not repeated. |
| Repository root: `NODE scripts/mcr/check-ui-origin.mjs` | C08 PASS, zero violations. `reviewed_extracts: 0` is reported honestly: exact-byte/static-import checking does not register adapted extracts or certify acceptance. |
| Repository root: `python3 scripts/mcr/check-plan.py` | PASS for planning provenance/structure/exporter/routing only. `docs/source/` unchanged. |

The independent Python JSON Schema oracle checks the actual frozen contract against synthetic SetIntro and public/private projections. It verifies 500/501/1000/1001 emoji, combining marks, mixed/BMP text and positive/negative projection cases. It reads only `contracts/`; tests never import old UI or backend files. Strictly increasing age bounds add the required semantic invariant that JSON Schema alone does not encode.

Composition events are exercised in jsdom with the full draft preserved and recovery to a valid save. Real operating-system IME, physical devices, backend routes/database, multiuser flows, load, full root-shell integration and release checks are NOT_RUN. Browser fixture calls are synthetic and cannot certify live acceptance.

## Targeted peer review

A read-only peer review of the five pure policy/contract helpers against frozen C04/C03 and BE25 found one P2: category `L` misses alphabetic combining marks included by Rust `is_alphanumeric()`. The detector now uses `[\p{Alphabetic}\p{N}]` consistently for the filter and both domain neighbors. New U+0345 before/after-domain and obfuscated-name regressions pass in the final 41-test policy run. The peer confirmed all three classifier changes and the specified regressions close the finding. No additional projection secrecy or age-bound/unknown-semantic findings were reported. The peer did not review final provenance packaging, root integration or live behavior.

## Delivery integration

`frontend/src/shared/validation/__tests__/evidence/allowlist-proposal.json` contains exact source/destination SHA-256, symbols and regression paths for the four audited extracts. It is a proposal, not registration or approval. The integrator owns `docs/mcr/reuse-allowlist.json` and may record the actual reviewer and merge these entries without another main-chat approval gate; no worker changed the allowlist or audit inventory. C08 currently passes adapted bytes without registration, so its PASS must not be confused with a populated review record.

Integrate this bounded commit serially, compare the changed paths against the transfer above, rerun affected combined checks, and update central task/acceptance evidence separately. Final committed ancestry and actual goal status are supplied in the handoff rather than a self-referential commit hash in this file. Only the integrator pushes master. The temporary retained dependency symlink is removed before the clean committed handoff.
