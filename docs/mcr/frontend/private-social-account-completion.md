# Original private social, inbox and account completion

Status: IN_PROGRESS, waiting for canonical publication. Developer source fixes,
focused reproduction and affected regression validation are complete. This
continues original FE31, FE35 and FE36; it does not
replace those tasks or certify the live IN03/provider/account-deletion journey.
The persistent unbudgeted development goal is active. C07 remains complete.

## Source and exclusive ownership

Base: `ff96d53eb817c93dd1bafb5c4f125630f995413b` from canonical
`julymeltdown/mcr` master. Branch:
`codex/mcr-fe-private-social-account-completion`, using the existing exclusive
`be-avatar-motion-v2` worktree. Root153 is the publisher.

Root153 explicitly stopped and transferred inbox model contract/controller/targets
and UI Inbox; safety contract/controller/panels/Dialog; account
contract/controller/panel; and NEW `__tests__/completion.test.tsx` in those three
features. The new document and `docs/mcr/evidence/private-social-account-completion/`
are assigned here. Root153 subsequently received the historical sharing writer's
explicit STOP/return and transferred the four social-sharing model/UI leaves
plus its NEW completion test. CSS, existing tests/fixtures, Auth, Feed, Profile, shared root,
HTTP/contracts/generated files, manifests and lockfiles remain read-only.

Existing implementation/evidence is retained: FE31 `61136cd`/`981b399`,
`21e157f`/`abaf9fd`; FE35 `fb44efb`/`021a191`/`e677f792`;
FE36 `705da35a`/`abae824d` through `69d4a0ac`. New code is authored locally;
no source rendering, third-party code or dependencies are copied. Frozen
`docs/source/` remains unchanged.

## Original criteria and delivered frontend behavior

| Original task | Existing source behavior | Fresh completion work |
| --- | --- | --- |
| FE31 | Private handle versions; explicit opt-in and recipient decision; bound no-store reveal; revoke/expiry/privacy cleanup; no auto-follow | Current default-entry sharing adapter uses the qualified canonical sensitive-read port and clears reveal on navigation; existing consent, reveal/expiry/no-store and invalidation tests pass in the final affected suite; no social-sharing production patch was needed |
| FE35 | Dedicated server unread total; bounded dedup pages; original read/all-read key recovery; fresh protected target checks; privacy cleanup | Reproduced stale mounted rows after a broken earlier observer; all publish/outage/privacy-clear notifications now isolate each subscriber; original command/count/target tests pass |
| FE36 | Canonical Auth/logout/deletion port; report authorization; block/unblock policy invalidation; truthful eight-target progress and unknown/partial failure | Reproduced confirmations surviving changed target/controller/account; confirmation now binds its original owner and target/revision or canonical scope; new consent is required after replacement |

## Fresh qualification and provenance

Production source changed only:

- `frontend/src/features/inbox/model/controller.ts`: isolated notification of a
  stable observer snapshot for publication, producer-outage clearing and privacy
  teardown. A broken callback cannot suppress other mounted subscribers.
- `frontend/src/features/safety/panels.tsx`: block confirmation binds controller
  and user; unblock binds controller, user and currently loaded revision.
  Owner/target changes discard local confirmation; dispatch rechecks its binding.
- `frontend/src/features/account/panel.tsx`: sign-out/delete confirmation binds
  controller, canonical Auth owner and full privacy scope. Replacing the account
  cannot inherit consent. Existing controller authorization remains authoritative.

New regression files belong to the three assigned feature `__tests__/completion.test.tsx`
paths. No CSS, root composition, Auth, backend, contract, manifest, lockfile,
original test, original fixture, source archive or product art changed.
Source fix commit is `a76870bc`; final qualified source/test/harness commit is
`b2b3abd6da2001931d73ae85e16cf4ec604cdd27`, descended from the base above.

| Fresh check | Result / exit | Retained evidence |
| --- | --- | --- |
| Focused baseline | 8 failures / 1 | `redA.log`, `redA.json` |
| Fixed focused behavior | 8 passes / 0 | `greenA.log`, `greenA.json` |
| All four original features plus Social composition and default account routes | 384 passes, 15 files / 0 | `regressionF.log`, `regressionF.json` |
| Strict feature source/test roots and complete imported type graph | 39 roots, 750 files, zero diagnostics / 0 | `ownedTypesC.log`, `ownedTypesC.json` |
| Oxlint, warnings denied, all assigned inbox/safety/account source and tests | PASS / 0 | `lintB.log`, `lintB.json` |
| Planning provenance/export/delivery checker | PASS / 0 | `planA.log`, `planA.json` |
| Full application TypeScript | FAIL / 2, unrelated fixture diagnostics retained | `typesA.log`, `typesA.json` |

Evidence paths above are under
`docs/mcr/evidence/private-social-account-completion/`. Every job receipt records
exact argv, source HEAD and per-feature source SHA256 pins, original shared
clock, resource peaks and cleanup. The final combined test invocation uses the
retained Vitest with `--root frontend --configLoader runner --no-cache --maxWorkers 1`
and the four feature test directories plus Social composition. The NEW account
completion file configures a five-second Testing Library async wait, then imports
all six unchanged default account-entry route assertions. They execute once via
that wrapper; none is omitted or substituted. This resolves the initial one-second
cold Settings lazy-import timeout without changing application behavior or old tests.

The original full application type command was retained unchanged:
`tsc --noEmit --incremental false --project frontend/tsconfig.json`.
Its diagnostics are the pre-existing Feed fixture `vite.config.ts:4` TS5097
and Gift detail native `preview.tsx:38` TS2339 for `Blob | MediaSource`.
They were routed directly to Root153/responsible implementation owners. The
supplementary type program parses the original strict compiler options, preserves
its config resolution context, includes every file under all four feature roots
and checks every imported dependency. It is reported separately; it does not
turn the recorded full-program failure into a pass.

Initial owned harness attempts B-E failed before assertions because of retained
pnpm peer resolution, browser URL handling and an external Vite-config/module
root mismatch. Their logs are retained. The final harness uses the original Vite
configuration and an owned same-environment wrapper. Initial scoped types also
preserved its missing config-resolution-context failure before repair. The first
red supervisor incorrectly counted its own completed `ps` sampler as a residual
child; physical readback showed that PID absent. The corrected sampler excludes
itself, and every subsequent receipt shows zero residual children.

## Resource release and remaining boundaries

The corrected SourceA offer `PRIVATE-SOCIAL-ACCOUNT-SOURCE-A-20261104T1743Z`
was actually accepted at 17:44:11.400866 UTC on 4 November 2026. All commands,
repairs and reruns shared immutable `acceptanceA.json`, work cutoff 17:54:11 and
hard cleanup 17:55:11. `FULL.json` records actual release at 17:52:02.578160:
zero owned processes, dependency link, TMP/cache or ports, and 26,251,870,208 free
bytes. No resource is held for documentation or publication.

Whole custody conservatively includes the entire checkout, old frontend/backend/
art/evidence, all shared Git objects, worktree metadata and the retained read-only
dependency logical root. Peaks were 2,733,207,958 whole bytes / 26,160 files;
33,297,733 positive-growth bytes; 29,884,196 TMP bytes; 509,517 new durable bytes;
four processes and 1,671,069,696 aggregate RSS bytes. All stayed within the named
4GiB/32,000-file/160MiB-new/128MiB-TMP/16MiB-durable/16-process/3GiB-RSS limits.
The first insufficient 20,000-file offer was declined before admission or children.
No install, build, browser, server, service or C07/backend job ran.

This fresh proof qualifies the deterministic frontend criteria and cleanup fixes.
Prior native captures remain historical; there is no new 360/390/430 visual
acceptance, physical-device, live two-account IN03, real provider or completed real
deletion claim. Those are separately owned actual flows, not invented blanket
frontend dependencies. All 112 required tasks and 61 required IA entries remain.
Root153 publishes this validated handoff and performs relevant combined checks;
the canonical publisher receipt closes publication without king/SL review or ACK.
