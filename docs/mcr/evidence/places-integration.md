# Places and help integration — 2026-11-03

FE19's map overview, place details and controls help are integrated as
**MOCK_READY**. The feature reuses the existing camera and canonical session,
keeps place catalog revisions distinct from immutable map revisions, and clearly
separates virtual landmarks from reviewed physical references. No photo or safe
route is invented from source metadata. No decorative left accents were added.

Worker `072c817` plus correction `b528374` became `c903453` and `70d8972`.
Independent reports `458ccf4` and `772059c` became `e58e9e1` and `4dc3bb3`.
All fourteen implementation paths match their assignment. The reviewer changed
only its report; parent applied the implementation into its isolated checkout.

Review reproduced stale targeting after a detail 404/410 or map-version error
followed by Back. Missing targets now stay disabled across navigation and catalog
retries until reconciled, without disabling unrelated places. A rejected map
release disables targeting and camera actions until a new matching release or
explicit event reload. The earlier catalog-only recovery button was replaced
with an actual document reload for map mismatch.

Parent verified all fourteen final delivered files, 53 worker evidence files,
fifteen final independent artifacts and 154 source hashes against the integrated
checkout. Parent ran 61 focused tests, strict build and lint: all exit 0.
Worker native Chrome checks cover 360/390/430px. The independent reviewer replayed
the failing controller probe, added bounded lifecycle assertions and executed
the actual reload at 390px on the integrated FE06 runtime. Fresh document/map
ownership, one canvas, restored targeting, no page errors/overflow and cleanup
pass. Parent inspected the overview screenshot. The retained bundle-size warning
is unchanged. No duplicate broad suite was required.

The [archive index](places-integration/index.json) preserves original and
normalized artifact hashes, source bindings, passing and failed attempts.
Worker receipt: `4ed94eca4252a023fbffa92f68e0b215ef7da887e9304a1ada185ed9c9b11e59`.
Independent receipt: `3b4f275d71cf661213f4f5ae73cc19fb1e106911f1c6dc1d670854463dc75329`.
The worker's broad native run predates three inherited FE06 source changes;
it is not mislabeled as a fresh run on master. The narrow independent browser
run covers the current runtime. Its first import failure is retained as a
harness failure, not an application failure.

Production shell/bootstrap wiring, actual BE08 responses, approved venue facts
and photos, authoritative multiplayer, real devices and whole FE19/QA07 acceptance
remain open. Reload may require sign-in because access tokens remain in memory.
Accepted required tasks remain 11/112; the full project goal remains ACTIVE.
