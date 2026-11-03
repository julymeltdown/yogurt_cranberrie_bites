# IN04 real mural gate

Status: **IN_PROGRESS** for the real qualification objective. Harness source and
negative preflight qualification are delivered separately. Real IN04, including
all actual multiuser mutations and moderation, remains **NOT_RUN**. No service,
operator, browser, database or release acceptance is inferred from this report.

## Scope and provenance

The assigned worktree is `/Users/lhs/Desktop/mcr-worktrees/qa-mural-in04-resume`,
branch `codex/mcr-qa-mural-in04-resume`, with exact handoff parent
`1163b827f9e62390163788bb674d24855816e0ec`. This patch owns only:

- `frontend/e2e/real/mural.spec.ts`
- `frontend/e2e/real/mural-helpers.ts`
- `docs/mcr/qa/mural-in04.md`

Main verified the former turn terminal before transferring ownership. The three
untracked files were recovered byte for byte from the former checkout pinned at
`1455e6a72a160c9c49a9c634499eb7856b041de1`; their original authoring base was
`892ca4de7580eb36c82b54efe44f03941ca68c41`. The supplied recovery receipt is
`/private/tmp/mcr-in04-recovery-1793741374987218000`. These initial hashes were
verified before editing or running the resumed qualification:

| Recovered path | SHA-256 |
| --- | --- |
| `frontend/e2e/real/mural-helpers.ts` | `b7845326b3306318e7d01a0cdc1213b792cde0c00fe0d96514aacae11566b647` |
| `frontend/e2e/real/mural.spec.ts` | `9b1b49d9892c54a0e6f9e2a5bf7e08e9a3f16a97e634ba831977676d9278415f` |
| `docs/mcr/qa/mural-in04.md` | `81704d7d323801e221005742eef4062d2687ea4263551c45c6fd312e38ed7b57` |

C04 dependency originals `a25b85592b906209f4a9d20f9c7279ced8abab02` and
`bff7e53c0638d5462af819bf560468a1f857c88e` are already in this parent. No shared
dependency commits were copied into the resumed branch. The correction makes
C04 0.1.2 authoritative over the initial result-ID assumption. No backend
implementation, root composition, generated projection, manifest, lockfile,
infrastructure or archived source is authored in this patch.

Both TypeScript files are newly authored QA code. They reuse the existing
passive `world-gate/observer.ts` and `world-gate/decoder.ts`, the public operation
registry, generated OpenAPI schemas and C05 schema from the assigned base.
Imports stay within the frontend or public `contracts/`; there are no backend
imports. The non-listening Vite decoder loader is existing IN01 machinery, not
a service or product fixture. Existing dependencies are Playwright, Zod, Vite
and Node. Qualification reused the retained dependency installation through a
temporary nontracked `frontend/node_modules` symlink to
`/Users/lhs/Desktop/mcr/frontend/node_modules`, removed before handoff. No
installation, upgrade, lockfile or shared dependency/cache mutation occurred.
Playwright compilation cache was isolated under `/tmp`.

The specification's IN04 acceptance remains multiple real clients, authoritative
cooldown, pause, restore, final freeze, convergence and unauthorized moderation
denial. QA03 database concurrency, complete QA05 failure/load/background/room
switch coverage, QA09 backup/restart rehearsal and IN05 release qualification are
separate obligations. This gate does not replace them.

## Required real inputs

`MCR_IN04_CONFIG` must name an absolute, current-user-owned regular JSON file,
mode `0600` or stricter, at most 32768 bytes, without a symlink. Its closed keys
are:

| Key | Requirement |
| --- | --- |
| `version` | `1` |
| `isolated`, `allowDestructiveFixtureEndState` | Both explicitly `true`. These declarations alone do not establish isolation. |
| `origin` | Exact HTTPS origin, or HTTP loopback origin; no credentials, path or query. |
| `eventId`, `environmentId` | Canonical UUIDs identifying the owned isolated fixture. |
| `releaseRevision` | Exact deployed candidate, 40 lowercase hexadecimal characters. |
| `members` | Exactly two independent `{userId,email,password}` accounts with real current MEMBER authority. |
| `operator` | A third independent account with real current OPERATOR authority. |
| `contractHashes` | `{domain,realtime}` SHA-256 values for `contracts/generated/mcr-domain.openapi.json` and `contracts/realtime/v1/world.schema.json`. |
| `restoreSnapshotId` | Retained, explicitly owned named baseline snapshot UUID. |
| `maxCooldownMs` | 5000–30000; fixture's real server cooldown must be nonzero and within this bound. |
| `restartObservationTimeoutMs` | 1000–60000 for observing an externally performed, authorized restart. |

Accounts require distinct canonical IDs and case-insensitively distinct emails.
No credentials, bearer values or cookie contents belong in tracked files or
reports. The runner retains the login bearer only in memory, destroys browser
contexts and clears its bearer copies on teardown. A run ending successfully
leaves only this qualified fixture event terminally FROZEN. A failed or unknown
mutation requires operator inspection of its retained receipt; the harness does
not blindly retry, automatically restore, unfreeze or restart anything.

`RuntimeQualificationPort` is deliberately unresolved: `runtimeQualification()`
returns `null`. Delivery confirmed that a reviewed deployed candidate, service,
role and isolation qualification protocol does not exist yet. A future exact
ownership transfer must bind this port to a verifiable read-only runtime source.
A local config, self-declared diagnostic header, invented endpoint/global,
fixture response or successful login cannot implement that authority.

The port must establish the exact deployed source and public-contract hashes,
all six real services, isolation from legacy data, exclusive event ownership,
current actor roles, the owned named snapshot and the deployed own-result read.
Qualification expires within 15 minutes. Restart observation verifies the same
candidate/environment, a fresh external restart and new incumbent C05 epochs
and streams; the gate never executes a restart command. Until this port and the
actual routes/composition exist, a populated private config still exits nonzero
with `REVIEWED_LIVE_DEPLOYMENT_QUALIFICATION_PORT_REQUIRED` before any browser
context or mutation is allocated.

FE33's feature-level implementation (original commit
`1ae55459a2f2b32c5ded777cff28886736827930`, integrated as `36d703d8`) and root
mural provider are present in the current base. However,
`frontend/src/app/world-panels/index.tsx` still passes `release={null}` to
`MuralEntryProvider`. A published mural database kernel and fixture-qualified
consumer do not establish a qualified public Wall. Delivery must connect a
reviewed runtime release with the canonical session, existing world authority,
persistent Phaser scene and real authenticated HTTP producer. The gate observes
the incumbent socket only; it adds no C05 message or second world connection.
No production diagnostic globals or writable application test hooks are
introduced.

## Authored real scenarios

Each required check starts `NOT_RUN`; a failed prerequisite stops subsequent
mutations. Missing inputs remain `NOT_RUN`, assertions/runtime failures become
`FAIL`, and only all required checks passing can produce `PASS`/exit zero.
The single serial test has retries disabled and a ten-minute outer timeout.

| Check | Required evidence |
| --- | --- |
| Configuration and deployment | Private input, exact source/contract hashes, genuine current runtime qualification. |
| Independent actors and baseline | Two separate MEMBER browser contexts plus an OPERATOR context; genuine email logins, actor/event C05 bindings, one active world socket and one incumbent Phaser canvas per member shell, unlocked EDITABLE named fixture baseline. |
| Placement and cooldown | UI confirmation matches exact real response, coordinates/color/revision; two concurrent additional same-actor writes are rejected during server cooldown without changing revision. |
| Different actor and convergence | Second actor can commit independently; both real editor canvases and authenticated snapshots converge to all 16384 authoritative pixels and revision. |
| Own-result recovery | Forward exactly one real UI POST, drop only its real response, retain unknown state, explicitly read status/original result, deny a foreign actor and recover via Check original command without POST replay. |
| Privacy | Closed public delta excludes actor/private cooldown fields; snapshots expose only the caller's observed cooldown (or null after its expiry). |
| Unauthorized moderation | MEMBER pause, lock and restore requests fail with permission denial; authoritative admin state is unchanged. |
| Pause, lock, rejected preview and resume | OPERATOR pause rejects painting; resume and real region lock lead to an actual UI rejection with preview removed and no pixel commit; unlocking permits a confirmed write. |
| Restore | Named original baseline is restored at a strictly newer revision/new snapshot identity; both clients converge to its pixels. |
| Obsolete delta | Hold an unmodified real response containing a committed pixel, restore and explicitly refresh into a newer baseline, release that older response and verify no rollback. A missing refresh seam or undeliverable delayed response cannot certify this case. |
| Offline reconnect | Disconnect one client while another commits; regain one incumbent socket with a new stream and matching actor/event, then converge. |
| Restart epoch | Observe a genuine externally performed restart, new per-client room epochs/streams and convergence without overlapping sockets or invalid frames. |
| Final freeze and teardown | OPERATOR freezes only the qualified fixture; both actors' subsequent writes fail, revision stays fixed, controls disable; contexts/sockets close and transient bearer copies are cleared. |

C04 0.1.2 recovery requires closed `mural.OwnPixelResult`
`{receipt_id,idempotency_key,result:PixelOperation}`. The status receipt must be
retained/unexpired `SUCCEEDED`, operation `placeConcertPixel`, HTTP result 200,
correct captured actor/event/key, no failure and `resource_id == event_id`.
The envelope binds its receipt ID/key; its nested result binds event, receipt
revision and original x/y/color. The original operation UUID is independent of
the aggregate receipt resource UUID. The gate compares the entire recovered
pixel operation, including its original `next_allowed_at`, with the real dropped
server response. Public snapshot/delta equality never establishes an own ACK.
Every API write uses a fresh idempotency key with retries/redirects disabled.
Unknown/pending admin writes stop qualification rather than assuming success.

The private `in04-result.json` journal records each required outcome, source
hashes, candidate/event/environment, qualification, confirmed recovery bindings,
convergence hashes/revisions, original UI command keys/bodies and direct-command
body hashes/status observations. Raw HTTP 200 in the direct-command journal is
labeled `HTTP_200_UNVERIFIED`; scenario assertions establish result validity.
Credentials and full headers are never recorded. Keep real journals private and
outside Git; do not treat a negative test journal as deployed acceptance.

## Executed harness qualification

All results below were requalified on parent `1163b827` in the new assigned
worktree with Node 24.22.2, installed under
`/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin`. No application
build, deployment, real login, service startup or real mutation was executed.

```sh
export PATH=/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin:$PATH
export PWTEST_CACHE_DIR=/tmp/mcr-in04-resume-playwright-cache-01a1a20f
node --experimental-transform-types frontend/e2e/real/mural-helpers.ts --self-check
node frontend/node_modules/typescript/bin/tsc --noEmit --target ES2022 --lib ES2022,DOM,DOM.Iterable --module ESNext --moduleResolution Bundler --strict --noUncheckedIndexedAccess --exactOptionalPropertyTypes --isolatedModules --verbatimModuleSyntax --resolveJsonModule --skipLibCheck --noUnusedLocals --noUnusedParameters --types node --typeRoots frontend/node_modules/@types frontend/e2e/real/mural-helpers.ts frontend/e2e/real/mural.spec.ts
frontend/node_modules/.bin/oxlint frontend/e2e/real/mural-helpers.ts frontend/e2e/real/mural.spec.ts
node frontend/node_modules/@playwright/test/cli.js test frontend/e2e/real/mural.spec.ts --list --workers=1 --retries=0 --reporter=line
python3 scripts/mcr/check-plan.py
git diff --check
```

| Qualification | Exit | Observed result |
| --- | --- | --- |
| Helper self-check | 0 | 69 positive/negative checks: strict configuration, independent identities, proof/candidate/role/isolation expiry, closed public responses, original pixel/receipt/envelope binding, missing-check verdict, private file/permission/symlink/size/JSON and contract-hash checks. Explicit `realIN04: NOT_RUN`. |
| Strict standalone TypeScript | 0 | Both owned TypeScript files and their read-only frontend dependencies; no build-info/cache output. |
| Scoped Oxlint | 0 | No reported errors. |
| Playwright discovery | 0 | Exactly one serial real mural gate discovered. |
| Missing config preflight/gate | 1 (expected) | `MCR_IN04_CONFIG_PRIVATE_ABSOLUTE_PATH_REQUIRED`; gate journal `NOT_RUN`, no contexts or mutations. |
| Correctly shaped private negative-only config with matching local contract hashes, no qualification provider | 1 (expected) for preflight and gate | Configuration passes; deployment is `NOT_RUN` with exact missing-port code; no contexts, no mutations, journal and copied attachment mode `0600`, test-only password absent from output/journal. Input and result directory removed after assertions. |
| Planning provenance | 0 | All 13 archive checksums and original upstream narrative match; required task/IA accounting preserved. No application acceptance. |
| Whitespace/ownership/import inspection | 0 | Only the three assigned QA paths are authored; shared dependencies already belong to the assigned parent. |

Expected nonzero real commands (with the same tool environment) are:

```sh
node --experimental-transform-types frontend/e2e/real/mural-helpers.ts --preflight
node frontend/node_modules/@playwright/test/cli.js test frontend/e2e/real/mural.spec.ts --workers=1 --retries=0 --reporter=line --output=/tmp/mcr-in04-resume-negative-01a1a20f
```

The second negative used temporary `example.invalid` accounts and loopback port
9 solely to prove that no browser/network phase is reached; these values are
not genuine deployment inputs or actor authority. The probe summary labels the
run negative-only; the gate journal records real IN04 `NOT_RUN`. Temporary probes
are not product fixtures, deployment proof or committed gate evidence.

## Outstanding real work

Delivery owns real `/wall` composition, authenticated public own-result and
moderation routing, verifiable deployment/isolation/authority qualification and
its exact adapter ownership. BE34/35/40 owners qualify durable PostgreSQL
invariants, snapshot/delta/barrier semantics and authorized original-result
production. The real service operator must provision independent identities,
owned isolated event/baseline and an authorized external restart window.

After those prerequisites exist, bind the reviewed port within a coordinated
scope, run this exact candidate against those real inputs, resolve any genuine
runtime failures, retain private candidate-bound journals and deliver the
acceptance report. The authored scenarios above remain unexecuted; timing and
composition may need concrete fixes when real execution becomes possible.
The persistent broader IN04 goal stays active; its original checkout wording is
superseded by Main's explicit transfer to this new worktree. Bounded harness
source delivery is complete at its committed handoff; source writes stop there
unless concrete review fixes are assigned. This source handoff does not certify
INTEGRATED, ACCEPTED, QA03/05/09 or release readiness. Only Delivery integrates
and pushes reviewed work to `julymeltdown/mcr/master`.
