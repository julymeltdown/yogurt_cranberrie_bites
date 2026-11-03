# Email-account and theme checkpoint — 2026-11-03

Reviewed frontend delivery is integrated on master. This checkpoint qualifies
local account interoperability and a synthetic-resource theme consumer; it does
not accept production consent, artwork, multiplayer or release readiness.

| Scope | Worker commits | Integrated commits |
|---|---|---|
| Actual email browser journey and nonempty stale-cookie proof | `0cca6d2`, `9591fe5` | `2f398a3`, `bed5ffa` |
| Three themes and reentrant texture commit correction | `40d86dc`, `4cb28b5` | `4f4df6d`, `5710363` |
| Independent theme review and evidence correction | `44cd8d1`, `46478ee` | `777d0bc`, `4bab070` |

## Account flow

The actual local six-service bundle built at `600582d` passes 17 native Chrome
checks through the real frontend forms and native fetch: signup, delivered email
verification, wrong-code refusal, sign-in, two-tab invalidation, logout, actual
delivered reset-link navigation/scrubbing, password reset and canonical identity
preservation. Both revoked-session probes require a nonempty captured HttpOnly
cookie, verify restoration and its exact outgoing Cookie header, then require
the actual server 401. Missing cookies cannot satisfy the assertions.

The final receipt SHA-256 is
`e482f0173dece132284cfdb0e36834e02fd8d46772a00cc37a19e1a1466d015a`.
Parent verified 38 referenced files, 105 frontend source hashes, committed runner
bytes, all 1,108 declared backend inputs and six executed binary hashes. Migration
ledgers and cleanup pass. Existing 247 focused auth regressions, strict build,
entry typecheck and lint remain applicable: the final correction changes only
the test helper and documentation. Parent inspected both masked screenshots.
The [archive index](real-email-flow/index.json) preserves 28 evidence references,
including superseded and failed attempts; it excludes the private ready/mail data.

Production signup remains blocked. Test-only eligibility/consent is explicitly
synthetic. Local HTTP is not Secure-cookie/TLS qualification. Actual bootstrap
failure remains visible; no world is opened or response substituted. OAuth,
production policy and full FE09–FE11 acceptance remain open.

## Themes

The consumer provides scoped palettes, keyboard-accessible selection, local
user/event persistence, reduced motion and atomic replacement on one existing
Phaser instance. All skins retain the exact immutable map and avatar release;
the neutral Tiled image marker is never fetched. The real PNG remains independently
allowlisted and hash checked. Failed/superseded loads release staged resources.

Review found a reentrant texture callback could commit a new visual skin, then
dispose its image while runtime still reported the old skin. `4cb28b5` serializes
reentry across the synchronous commit and reconciles failed newer requests to the
actually visible fallback. Caller cancellation cannot discard committed ownership.

Worker evidence is sealed by receipt
`0d5c461349041de612a8149022ef0537d7db2e89e3125be34eefadeca64d9503`.
Parent verified 20 changed paths, four configuration hashes and 11 artifacts.
Historical pre-commit source matching is chronology-based, explicitly not a
contemporaneous hash snapshot. Parent then ran the integrated candidate itself:
262 focused theme/runtime/movement tests, strict build and lint all exit 0;
installed Chrome 156 at 360/390/430px exits 0, with no page errors. Source hashes
were captured before that native run and remained identical afterward. Real
texture-add reentry, failed newer-load fallback, camera/movement/canvas continuity,
keyboard selection and zero owned resources after teardown pass. The retained
544.82kB build warning remains. Parent inspected the phone screenshot.

The initial independent QA report contained incorrect artifact hashes and a
nonexistent result file; those claims are retracted. Its broad custom probe failed
on an unhandled harness rejection, so it is not passing evidence. The corrected
seven actual artifacts are hash verified. The initial R1 failure was observed in
the execution stream; the on-disk result was overwritten by the final PASS run.
QA also accidentally wrote the corrected document into the integration checkout
via a relative patch path. Writes were stopped; parent verified exact equality to
the committed QA correction and preserved it as `4bab070`. Future delegated writes
must name absolute paths inside their assigned worktree. No application code was
mixed across lanes. The [theme archive](theme-integration/index.json) includes
the corrected QA files and fresh parent checks.

FE06 remains MOCK_READY: production AR04 textures, shell composition, server
preferences, real character/room identity and device/multiuser gates are open.
Accepted counts remain 11/112 required and 11/118 total. FE19 map/place/help work
continues independently; backend HTTP and owner composition remain under review.
