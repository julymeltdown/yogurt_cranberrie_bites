# FE33-ENTRY / FE37 bounded regression repair

This delivery contributes to the original FE33 and FE37 tasks. It adds no task,
acceptance count or scope reduction. Status: **MOCK_READY** for the scoped
consumer/route behavior. The historical unfiltered broad diagnostic remains
**NOT GREEN**; no current global-suite result is inferred from these checks.

## Ownership and provenance

Writer: chat `01a1a26e-1de2-7b41-a294-c9a79cfded9b`, preserved isolated
`fe-ref-customize` checkout, branch `codex/mcr-fe-mural-admin-regressions`.
Assignment base: `7ea11efdbdc1e2a447ece669a660c10f12b43463`. The unbudgeted goal
created at epoch `1793756572` covers diagnosis, fixes, validation, documentation
and direct safe publication. Community allocated the two existing tests, this new
document and new `docs/mcr/evidence/mural-admin-regressions/**`. After the source
diagnosis, Community explicitly added only the Schema/keyword/`oneOf` evaluation
hunks of `frontend/src/features/admin/reports/model/validate.ts`.

Source commit: `ccd71cb4b7c4557fa2463ae8057b853a9b877866`, parent assignment
base. Reconciliation merge: `813684f9f2ce1b4128b63f252bbff075f987a60e`, preserving
published peer tip `145d0e21b5ce92c55a04c31dc447267f78014a99`. Its changes include
the incumbent shell/world input work, so affected routes, types and the default
build were qualified again after reconciliation. The 65 native moderation source
hashes and all nine retained image pins still match; unchanged native source was
not rerun merely because unrelated peer commits arrived.
The later documentation-only peer update at
`16fde48ab9842b3a5e9b5e4aab3719c926191ac9` is preserved by merge
`c33c6e21e03619b622055a7175730ff1e549a002`; application/contract bytes did not
change, so no further execution was needed for that merge.
The final incoming QA07 evidence / FE17 test / coordination update at
`ef1b6c60afdd06a1847f49c91d805421904552fc` is preserved by merge
`a25c9147182d556288501e8bc4cd00fa2ccd72cb`. All 32 pinned inputs to this
qualification and 65 native moderation source hashes remain identical. Full
strict TypeScript was rerun successfully for the incoming test files; no other
retest was required by unchanged application/contract bytes. See
`reconciliation.json` and `final-peer-typecheck.log` in the evidence directory.

The exact three source/test paths changed are:

- `frontend/src/app/mural-entry/__tests__/routes.test.tsx`
- `frontend/src/features/admin/reports/__tests__/controller.test.ts`
- `frontend/src/features/admin/reports/model/validate.ts`

All other production code, fixtures, schemas/generated outputs, manifests/locks,
root composition, backend, source archive and artwork are unchanged by this
writer. No former Customize/avatar ownership is inherited. The source is an
incremental repair of current MCR modules, with no copied legacy rendering or
new dependencies. Exact source, fixture, contract and artifact hashes are in
[validation.json](../evidence/mural-admin-regressions/validation.json).

## Diagnosis of the six reported residuals

The owner packet is `docs/mcr/frontend/production-refresh.md`; the frozen broad
run used source `cb9abce48d78227bbdedf7ea59ce95e9e0270c4a` and was published with
`62b4bf80`. Its existing `broad-diagnostic.log` is preserved. Fresh scoped
`baseline.log` reproduced **all six failures**, 46 passing / 52 tests, exit 1.

| Reported residual | Actual cause and repair |
| --- | --- |
| Wall/world/other-panel navigation + reconnect cannot find `Confirmed revision 10` | Test presentation drift. Layout commit `7b567acb6026d0bab6b5afefce6bc704e4440f2a` replaced that text with `REV 10 · EDITABLE`; the named synchronization status still reports the verified revision. The actual owner/snapshot was healthy. The test now requires named ready status, actual sync revision/state/pixels, the current metadata and 512px canvas. It still requires the identical owner and non-null unknown command, one POST, no replay, reconnect/replacement canvas withdrawal, and one incumbent game/connection. No mural production fix was needed. |
| Privacy closure cannot find the same old revision text | Same stale assertion, not demonstrated privacy failure. Before closure the test now verifies the complete scene-sink RGBA payload against the fixture's 128×128 pixels/palette. Closure must synchronously remove protected status/DOM, zero the retained DOM canvas, clear snapshot/texture/object/listeners/RGBA and refuse a replacement owner for the expired scope. |
| Exact members command reaches `rowKey(undefined)` | Product decoder incompatibility: valid member list rejected before row retention. Fix the decoder, retaining command actor/event/target/revision/key/no-replay assertions. The test prepare helper now explicitly checks ready phase and one row before selecting, exposing the underlying failure directly. |
| Direct member route retains `null` selected row | The actual outgoing request already contains the correct frozen `user_id`; the failure is selected state after response rejection, not a missing filter. Existing query and minimal-profile assertions remain. |
| Reverse members action reaches `rowKey(undefined)` | Same decoder cause before suspension reversal. Existing explicit UNSUSPEND at revision 4 → ACTIVE revision 5 coverage remains. |
| Separately reviewed intent cannot obtain a fresh key after refresh | Same list rejection, not a key-generation defect. Existing assertions still require two separately reviewed actions, keys 800/801 and final ACTIVE revision 3. |

Canonical contract revision `3398d8331863775ecc0180e5eccebddd179fedd5` made
`AvatarConfig` a `oneOf` of layered 1.0 and whole-character 1.1 configurations.
`UserReview.profile` references full `PublicProfile`, including `AvatarState` and
that union. The resolved domain OpenAPI carries it. The old FE37 restricted
interpreter (`b786e648dd889e9bdf064c0e4e637a1dc4075a70`) rejected the unknown
keyword even for the unchanged valid layered fixture. Changing fixture data,
making `rowKey` tolerate absent rows, skipping avatar validation or stripping the
profile before validation would conceal the defect.

The production repair adds `oneOf` to the local Schema/keyword whitelist and
requires **exactly one** recursively valid branch. Existing shared node budget,
depth bound, reference checks, closed fields, formats and minimization remain.
This interpreter remains restricted to its canonical reachable DTOs; it is not
a general JSON Schema implementation. No contract regeneration was required.

Eleven new regressions check both valid wire variants through list validation,
minimal retained profile and an exact explicit SUSPEND command; reject hybrid,
unreviewed preset/manifest, mixed version, missing field/avatar and private extra
fields; and distinguish schema-valid foreign identity from an authorized member.
Invalid reads retain no row or private marker and issue no command. The canonical
whole input comes directly from the selected-static golden fixture. With these
tests added before the product fix, the positive member cases failed and all
three mural cases passed (`regressions-before-fix.log`, exit 1). After repair,
all 63 focused tests pass. No existing behavioral assertion was removed.

## Validation and evidence

All commands use retained Node **24.22.2** and locked dependencies, without an
install. `NODE` below denotes the pinned executable. Commands run from
`frontend/`, except the planning check. Logs and the native report are retained
under the new evidence directory; empty successful type/lint logs have explicit
exit receipts in `validation.json`.

| Check | Exit and result |
| --- | --- |
| `NODE node_modules/vitest/vitest.mjs run src/app/mural-entry/__tests__/routes.test.tsx src/features/admin/reports/__tests__/controller.test.ts --configLoader runner --no-cache --maxWorkers=1` | 0, 63 tests / 2 files |
| `NODE node_modules/vitest/vitest.mjs run src/app/mural-entry/__tests__ src/features/mural src/world/mural src/features/admin/reports/__tests__/controller.test.ts --configLoader runner --no-cache --maxWorkers=1` | 0, 263 tests / 17 files; repeated after peer reconciliation |
| `NODE node_modules/typescript/bin/tsc --noEmit --incremental false --pretty false` | 0, strict frontend types; repeated after reconciliation |
| `NODE node_modules/oxlint/bin/oxlint src/app/mural-entry/__tests__/routes.test.tsx src/features/admin/reports/__tests__/controller.test.ts src/features/admin/reports/model/validate.ts` | 0 |
| Default Vite production build with `configLoader: runner`, private `cacheDir`/`build.outDir` | 0; repeated after reconciliation; retained large-chunk advisories only |
| `MCR_FE37_EVIDENCE=<owned-output> NODE src/features/admin/reports/__tests__/browser.mjs` | 0, installed Chrome 156.0.8078.80; nine reports/members/content × 360/390/430 configurations |
| `python3 scripts/mcr/check-plan.py` from repository root | 0; planning provenance only |
| Scoped path/import inspection, `git diff --check`, source and screenshot pin verification | 0 |

The unchanged native runner builds the real moderation components with synthetic
HTTP/Auth/role/media fixtures. It covers keyboard review/confirm/cancel focus,
zero write on cancel, explicit pending/unknown status GET without replay, failed
receipt/reason retention, verified success and reverse member/content actions,
permission/denied cleanup, no browser exceptions, 44px controls and no horizontal
overflow. The 390px member confirmation capture was inspected. Nine pinned
confirmation PNGs and the original report remain; the generated site and 36
supplemental captures were removed. The runner's embedded old `assignmentBase`
is historical runner provenance; this assignment's base/source hashes are recorded
separately, not rewritten to misrepresent that evidence.

The inherited sparse checkout omitted two tracked sign-in PNG imports, causing
the first default build failure (`build-sparse.log`). Under an exact operational
exception only those two assets were materialized and verified byte-for-byte
against HEAD; no asset diff was staged. Successful builds then used the ordinary
application entry. Local sparse includes preserve prior patterns plus the owned
evidence path. A temporary allocated `frontend/node_modules` symlink borrowed the
retained installation read-only; only that exact owned link is removed before
handoff. Build/cache/browser outputs are private and cleaned. Early development
corrections were an extra parent segment in a new fixture import and an inferred
string instead of the `ACTIVE` literal; both were fixed before final qualification.

## Limits and delivery

Mural evidence here uses actual React routing/Auth/privacy/controllers and a
synthetic incumbent world/scene/HTTP server. DOM canvas contexts are stubbed in
jsdom; exact RGBA comparison qualifies the real sink into its in-memory scene
port, not native browser raster presentation. Existing native moderation evidence
likewise does not prove live server authorization or backend mutations.

Live backend, actual root-to-world propagation, multiple real users, physical
iOS/Android, FE33/FE37 as whole tasks, IN04/IN05, full ten-screen visual fidelity,
the unrelated privacy Node-test collection mismatch and the unfiltered broad
suite are **NOT_RUN** by this patch. The historical broad failure remains honest.
Server authorization/privacy/idempotency duties remain authoritative.

Delivery is committed, non-force published to `julymeltdown/mcr master` after
fresh fetch/reconciliation, and independently checked for remote ancestry. The
published commit/remote receipt and returned exact leases are reported to
Community when the source writer stops cleanly. No king/SL technical review or
extra acceptance gate is imposed.
