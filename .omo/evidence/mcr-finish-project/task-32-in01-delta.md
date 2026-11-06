# Task 32 - IN01 delta re-run for changed features

Status: **GATE NOT_RUN** on candidate `937ab6699167735995661d91329a238797096ea5` (branch
`codex/mcr-exec-w5-in01`, base master `937ab6699`). The changed-feature delta list is
enumerated below with its commits, and every delta was attempted through the repository's
real stack driver. No delta could reach the two-client world gate: the current canonical
enrollment/admission policy has no operator publication in an isolated stack, so the
gateway refuses browser signup (`422 ACKNOWLEDGEMENTS_REQUIRED`), returns
`503 POLICY_UNAVAILABLE` from `/api/concert/events/{event}/rooms`, and the bootstrap read
that precedes any world socket returns `503 DEPENDENCY_UNAVAILABLE`. The unmodified
`dev_stack.py` additionally fails its retained readiness schema at `friend-service`.
Nothing in this record is marked PASS from a fixture.

## 1. Baseline and candidate

- Historical IN01 acceptance: commits `92989fd7` -> `1bb911c3` (2026-11-04 17:40 KST),
  real two-account journey attempt-11 in `docs/mcr/frontend/in01-browser-journey.md`,
  accepted in `docs/mcr/scrum/world.md` (`IN01 | DONE | 92989fd7 -> 1bb911c3 published`).
- Delta range: `git rev-list --count 1bb911c3c..HEAD` = 1263 commits.
- Candidate binaries were compiled from this checkout into
  `/Users/lhs/Desktop/mcr-build/exec-w5-in01-target/debug` (target dir copy-on-write cloned
  from `mcr-build/exec-w2-closure`, then rebuilt from this source). Toolchain
  `cargo +1.88.0` (repository `rust-toolchain.toml`), `CARGO_BUILD_JOBS=3`,
  `CARGO_INCREMENTAL=0`.

| Binary | SHA-256 |
| --- | --- |
| api-gateway | `69aa66f36b3c132c670996fdfee5bba48ca55e96e08926b50a791ff98e104fef` |
| auth-service | `07ba7313fb11ccedf335ba71baf9f7d6eab4963c5463b7590ad50135bad41dd3` |
| profile-service | `55f96b2f15e3e05da9dd268b241543dfc864bc41c60c112c14f99b6e58826730` |
| friend-service | `f7d6dcb89bfb8528db02be9ae56cc321c558687972f9582503047cfa8e8f8882` |
| post-service | `508c162b96e2065153ece46bbdf153028b43150258df0b592afcd64c08b3db6e` |
| notification-service | `e783d45db589c1bb4a70cff4bdd54fa9cf3929b8aeb6c8d6b34a231319428193` |
| concert-service | `98c501aac9e378a8e407a2a15a7e752891f87494a431f1d7f912d18e8210eb8e` |

## 2. Delta table

Every item names its commits since the accepted IN01 commit, the gate assertion that would
prove it, the candidate attempt, and the status. `NOT_RUN` means no check executed; no
socket frame was captured because no world socket was opened.

| # | Delta | Commits since `1bb911c3c` | Gate assertion | Candidate attempt | Status |
| --- | --- | --- | --- | --- | --- |
| D1 | Admission: world tickets, membership and canonical enrollment admission | 18 world commits (`51af7f7a8`, `4143e9494`, `c2544b69d`, `8364b25cc`, `b9244f869`, `930380dee`, `8fa2db1c0`, `4bcb1d9eb`, ...) and 4 enrollment commits (`c093d8a5c`, `c988365ca`, `2f7992d43`, `a3d547f64`) | Two authenticated accounts receive tickets, join one room on distinct streams with mutual membership, move authoritatively, and disconnect cleanly (`world_two_principal_probe`) | Two real accounts created and logged in (200), profile writes 200, then `/rooms` 503 `POLICY_UNAVAILABLE` and bootstrap 503 | NOT_RUN |
| D2 | Policy frames: explicit Social policy baseline and invalidation | 2 (`930380dee`, `b9244f869`) | `policy.baseline` follows the world snapshot on the same socket; `policy.invalidate` withdraws the private view; no second socket | No world socket established in the candidate attempt | NOT_RUN |
| D3 | Feed: producer, read composition and Feed/Thread presentation | 26 server (`d4224d1ef`, `c5fe8473e`, `5088d3173`, `2ab8e50f5`, `11004823e`, ...) and 17 UI (`10b1afe68`, `827877828`, `8e5cc757f`, `8fea3b43d`, `7eac13ce3`, ...) | Feed panel opens with authorized state, keeps the same host/canvas, opens no socket, and suppresses world movement while focused | Feed panel unreachable: the app cannot reach authorized readiness without the enrollment policy | NOT_RUN |
| D4 | Theme: bound era palettes in the production shell | 4 (`3321b3e19`, `eacbaab50`, `99b5bbf93`, `8e5cc757f`) | Era selection changes the rendered skin, keeps room/map/avatar/position and socket invariants, persists per identity and event | Theme controls unreachable: the shell never mounts without admission | NOT_RUN |
| D5 | Panels: shell navigation, host lifetime and panel chrome | 17 (`cb44bb77c`, `09be648b8`, `56a0cb7f0`, `78886ffa9`, `0a27b70d7`, `6b6c94d24`, `7ee21c586`, `bf2ed2fa3`, `3f81baea9`, `c45ebce00`, ...) | Six panels per client with identical host/canvas nodes, unchanged socket count and peak one | Panels unreachable: no authorized world for either client | NOT_RUN |

The complete per-path commit lists are in `task-32-in01-delta/delta-commits.txt`.

## 3. Execution receipts

### 3.1 Candidate build

Command (from `backend/`):

```
CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/exec-w5-in01-target CARGO_BUILD_JOBS=3 \
CARGO_INCREMENTAL=0 cargo +1.88.0 build --locked -p api-gateway -p auth-service \
  -p profile-service -p friend-service -p post-service -p notification-service -p concert-service
```

Original compile 2026-11-06T10:06:35Z -> 10:07:48Z, `Finished dev profile ... in 1m 13s`,
exit 0. Verification rerun exit 0 (`task-32-in01-delta/build-verify.log`).

Frontend dependencies: `corepack pnpm@11.7.0 install --offline --frozen-lockfile
--ignore-scripts` with Node `v24.22.2` (from `/tmp/mcr-node24/bin`), exit 0.

### 3.2 Documented dev stack start (`backend/scripts/mcr/dev_stack.py`)

```
python3 dev_stack.py start --bin-dir /Users/lhs/Desktop/mcr-build/exec-w5-in01-target/debug \
  --pg-bin-dir /opt/homebrew/bin --redis-binary /opt/homebrew/bin/redis-server \
  --openssl-binary /opt/homebrew/bin/openssl --browser-origin http://127.0.0.1:5173 \
  --duration-seconds 30 --ready-file /tmp/mcr-in01-delta/devstack-ready.json \
  --report /tmp/mcr-in01-delta/devstack-report.json
```

Exit 1. `result: FAIL`, `failure: {phase: development, diagnostic: SmokeError}`,
`cleanup: PASS`. The failure is the retained readiness validator in
`backend/scripts/mcr/runtime_smoke.py`: `friend-service` (the first started service with
peers) reports the required check `concert-owner-authority`, which `PEERS`/`readiness_value`
do not expect, so `/readyz` is classified malformed. `profile-service` and
`notification-service` were validated before it; `friend-service`, `post-service`,
`auth-service` and `api-gateway` carry `readiness: null` in the report.

Same-stack `/readyz` check names captured with the current in-tree validator
(`browser_refresh.current_readiness`), exit 0:

| Service | HTTP | checks (all required unless noted) |
| --- | ---: | --- |
| friend-service | 200 | `concert-owner-authority`, `database`, `friend->profile` |
| auth-service | 200 | `auth->friend` (not required), `auth->notification` (not required), `auth->post` (not required), `auth->profile` (not required), `concert-owner-authority`, `database` |

`world_in01_live.py` is the in-tree driver that already patches this validator
(`patch.object(dev.runtime, "readiness_value", prep.current_readiness)`), so the stack was
started through that driver for the delta attempt below. No helper was edited and no
suite was weakened.

### 3.3 Current two-account world attempt (`world_in01_live.py`, profile `in01`)

```
cd backend/scripts/mcr
python3 world_in01_live.py --bin-dir .../debug --concert-bin .../debug/concert-service \
  --report /tmp/mcr-in01-delta/world-live-report.json \
  --browser-origin http://127.0.0.1:5173 --world --hold-seconds 900
```

Exit 1. All six services plus the concert child reach listener readiness; concert
migrations exit 0/0 with a 60-entry ledger; then the first real account signup through the
browser gateway route is refused:

```
statuses: {"principal-1-signup": 422}
failure:  principal-1-signup:status
cleanup:  PASS
```

`task-32-in01-delta/world-live-report.json` is the raw report.

### 3.4 Account and world reachability on the same candidate

Because the browser signup route is hard-refused, a temporary harness started the same
candidate stack and created two real accounts through Auth's documented local-development
application route (`AUTH_PUBLIC_APPLICATION_ROUTES_ENABLED=true`), then logged both in
through the public gateway. This does not replace the gate; it measured how far the
candidate's real services allow the journey to proceed. Commands and observed statuses:

| Operation | Status | Detail |
| --- | ---: | --- |
| Auth signup (dev application route) x2 | 200 | two distinct real identities |
| Gateway `/api/auth/login` x2 | 200 | both access tokens accepted |
| Gateway `/api/profile/{user}` PUT x2 | 200 | profiles initialized |
| Gateway `/api/auth/enrollment/policy` | 503 | `POLICY_UNAVAILABLE` |
| Gateway `/api/auth/email/signup` (browser) | 422 | `ACKNOWLEDGEMENTS_REQUIRED` |
| Gateway `/api/auth/enrollment/admission` (bearer) | 200 | decision `POLICY_UNAVAILABLE`, `policyRevision: null` |
| Gateway `/api/concert/events/{event}/rooms` (bearer) | 503 | `POLICY_UNAVAILABLE`, "Global enrollment policy unavailable" |
| Gateway `/api/concert/events/{event}/bootstrap` (bearer) | 503 | `GATEWAY_503` / `DEPENDENCY_UNAVAILABLE` |

Harness exit 0 (the wrapper does not propagate the driver's integer result); the driver
report inside says `result: FAIL`, `failure: principal-1-bootstrap:status`,
`world_two_client: NOT_RUN`, `cleanup: PASS`
(`task-32-in01-delta/world-direct-report.json`, `direct-run.log`).

The enrollment policy store is an explicit operator publication
(`backend/services/auth-service/src/enrollment_policy/store.rs`, "Explicit operator action
only. There is no public policy-publication route."). No isolated stack supplies a
publication, and synthesizing one would fabricate a legal/publication approval, so it was
not done. The browser IN01 driver (`frontend/e2e/real/world/in01/run.mjs`) requires the
private descriptor that `world_in01_live.py` writes only after bootstrap; that step never
completes, so no browser context, panel, theme, feed or socket observation exists for this
candidate. No fixture was substituted for them.

## 4. Blocking services and unrun deltas

| Blocker | Exact failure | Deltas affected |
| --- | --- | --- |
| Retained dev-stack readiness schema | `dev_stack.py` rejects `friend-service` `/readyz` because of the required `concert-owner-authority` check; `auth-service` has the same check. The in-tree `browser_refresh.current_readiness` is needed. | Stack start through the unpatched documented driver |
| Enrollment policy not published | Gateway browser signup `422 ACKNOWLEDGEMENTS_REQUIRED`; `/api/auth/enrollment/policy` and `/api/concert/.../rooms` `503 POLICY_UNAVAILABLE`; admission decision `POLICY_UNAVAILABLE`; bootstrap `503 DEPENDENCY_UNAVAILABLE`. Visible refusal at `api-gateway`, missing prerequisite at `auth-service` enrollment policy store. | D1, D2, D3, D4, D5 and the reconnect scenario |

## 5. Cleanup and limits

- Every owned stack report recorded `cleanup: PASS`; no listener of this run remains:
  `pgrep` for the candidate binaries, `world_in01_live`, `direct_provision` and
  `dev_stack.py` is empty, and port 5173 has no listener. The three
  `/private/tmp/mcr-pg-*` clusters present on the host started at 11:08, 15:23 and 16:16,
  before this task, and were not touched.
- Build target retained at `/Users/lhs/Desktop/mcr-build/exec-w5-in01-target` (9.6 GiB
  logical, copy-on-write). Free disk after the runs: 39 GiB.
- No repository source, spec or suite was changed; this record and its raw captures are the
  only additions. No fixture, mock, injected cookie or fabricated identity was used for any
  status above.
- Not claimed: two-client join, movement, policy frames on the wire, panels, theme, feed
  presentation, single-socket invariants, reconnect recovery, or IN01 re-acceptance on this
  candidate. They remain unrun until a real enrollment policy publication (and provisioned
  admitted accounts) exists for an isolated or deployed release.
