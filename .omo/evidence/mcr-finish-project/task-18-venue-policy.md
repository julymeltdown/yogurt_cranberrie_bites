# Task 18 - venue policy freeze

Date: 2026-11-06
Worktree: `/Users/lhs/Desktop/mcr-worktrees/exec-w3-policy`
Branch: `codex/mcr-exec-w3-policy`
Base: `0dfbf3d8b`
Commit: `0a08d55a5432b4924710267eb2b27ed9fc45c2e8` `docs(venue): freeze location, invitation and notes policy`

## Deliverables

- `docs/mcr/contracts/paradise-city-v1/policy-freeze.md` (315 lines, new):
  41 policy rows plus a field coverage map, the three required invariants,
  W04 and external-provenance owner inputs, and deployment-input rules.
- `backend/docs/api/paradise-city-v1/location.openapi.json`: descriptions only.
- `backend/docs/api/paradise-city-v1/meeting.openapi.json`: descriptions only.
- `backend/docs/api/paradise-city-v1/place-notes.openapi.json`: descriptions only.
- `docs/mcr/contracts/paradise-city-v1/location.md`: replaced the stale
  single-direction friend caveat with the frozen `VENUE_FRIEND_V1` predicate.
- `scripts/mcr/paradise-city-v1/*_transport.py`: repinned the three contract
  hashes; the location script now prefers the checked-out contract file and
  falls back to its historical commit, matching the meeting/notes pattern.
- `frontend/src/shared/api/domain-operations.generated.ts`: regenerated
  registry; the diff is limited to the three contract-input and three script
  hash lines. No frontend application code was touched.

## Decisions recorded

- `VENUE_FRIEND_V1`: both live follow edges in the Friend service, both current
  event participants, no live block either direction, OPEN/LIVE event, and a
  current session for the requesting actor. The counterpart need not be online.
  Friendship is an eligibility gate, never a location audience or GPS consent.
- Location visibility requires an explicit owner-selected viewer in an ACTIVE,
  unexpired lease. A location request is metadata only; approval can add a
  requester only to an already active lease and never extends it.
- Freshness: `fresh_until = observed_at + policy.fresh_ms`;
  `last_known_ms` bounds LAST_KNOWN; `coordinate_retention_ms` bounds
  persistence; policy ordering violations are 503. One replaceable fix per owner,
  never a movement history.
- Pause/revoke: immediate, same transaction, empty viewer list, precise
  phone/check-in state deleted. No cached or in-flight fix restores visibility.
- Event time: `concert_events.timezone` (currently `Asia/Seoul` only by CHECK).
  `end_of_day` is the next local midnight strictly after server time, capped by
  an earlier approved event cutoff or access deadline. `VENUE_END_DAY` uses
  that absolute instant and is immutable.
- INVITE requires a current reviewed fixed landmark and its full revision tuple;
  missing is 400, unknown/unreviewed/closed/stale is 404/403.
- MESSAGE requires a current `VENUE_FRIEND_V1` pair and event context, with no
  landmark, invitation or acceptance prerequisite. `invitationId` is optional
  historical context and never permission.
- W04 virtual party-room invitation remains a pending owner input. Its proposal
  (host to accepted party members/current room, five-minute expiry/revoke,
  capacity-checked admission, no reservation) is explicitly not frozen. No
  L/M/N route, producer, capability or mapping may consume it; affected behavior
  fails closed with 404/503.

## Checks

| Command | Exit | Result |
| --- | --- | --- |
| `python3 scripts/mcr/generate-domain-transport.py --check` | 0 | `Domain transport metadata current: 140 operations` |
| `python3 scripts/mcr/check-plan.py` | 0 | plan report `result: PASS`; 7 checks; 118 tasks / 64 IA |
| JSON parse of all three venue OpenAPI files | 0 | all parse |
| `git diff --check` | 0 | no whitespace errors |
| `grep -n TBD docs/mcr/contracts/paradise-city-v1/policy-freeze.md` | 1 | no matches (`none`) |
| Wire-shape comparison against HEAD (paths, operation IDs, schema names, enums, required, scalar keywords) | 0 | 0 differences in all three files |
| `git status --short` after commit | 0 | empty |

The wire-shape comparison is a local JSON projection check: for each file it
compares every non-description structural value (including `enum`,
`required`, `const`, `type`, bounds and patterns) against
`git show HEAD~1:<path>`. Result: `location 0`, `meeting 0`,
`place-notes 0`.

## Constraint proof

- No Rust business logic, no migration and no protobuf change.
- No frontend application code. The only frontend-path change is the generated
  contract registry required by `--check`; its diff is six hash lines and no
  operation metadata.
- Only the task worktree was written. No merge, push or master update.
- The sparse checkout did not contain `docs/source` or the generated registry;
  both were materialized locally only to run the required checks. The committed
  tree contains the new contract, OpenAPI descriptions, transport pins, the
  regenerated registry and the location contract clarification.

## Open owner inputs

- **W04 virtual party-room invitation policy**: pending existing human question.
  Not decided here; fail-closed default recorded.
- **External provenance**: authoritative georeferencing/calibration, current
  permitted meeting locations/access/closure reviewer and validity, and any
  third-party physical-source rights. These block physical claims only
  (projection unavailable, landmark `can_meet`/`can_check_in` false or absent,
  404/503) and do not weaken L/M/N privacy rules.
- **Deployment policy values**: numeric freshness/lease/retention/TTL values are
  operator configuration, not open product decisions. Semantics and invariants
  are frozen; missing/stale/invalid policy is 503.

## Risks / limitations

- The current Friend owner RPC exposes a directional `following` flag, not a
  bilateral friend read. L/M/N therefore need a new fenced bilateral owner read;
  until it exists, the friend gate must return 503. This is recorded in the
  freeze rather than hidden by a cached or one-way substitute.
- The end-day cap uses an approved event cutoff when present; the current
  `concert_meeting_settings.event_cutoff` is the available operator-supplied
  absolute boundary. Without a cutoff, the next `Asia/Seoul` midnight is used.
- The generated registry was regenerated in this sparse worktree; a full
  checkout must see the same six hash lines, which this commit supplies.
- Comment-checker flagged the generated registry header comments; they are the
  generator's pre-existing preamble, not authored comments, and were left
  unchanged.

## Attempt 2 - adapter pin and drift-probe repair

Date: 2026-11-06
Commits: `29c448edbfbbc06b5e81e68e98602925356f5797` `fix(venue): refresh adapter pins and local drift probe`;
`bc452a9cfdf14f9f6024fed572f0c2450d5c585e` `docs(venue): align adapter README with canonical drift check`.
Branch: `codex/mcr-exec-w3-policy` (same worktree, on top of `0a08d55a5`).

Verification found that the first commit changed the venue OpenAPI files but
left the frontend venue adapter pins and one drift probe stale.

Changes:

- `frontend/src/shared/api/paradise-city-v1/generate.py`: `EXPECTED` refreshed
  to the new place-notes SHA-256
  `246d802956084e85969227ed7ce8549107169fddd99273c3edb1b53a845a9027`.
- `frontend/src/shared/api/paradise-city-v1/notes.generated.ts`: regenerated
  from the pinned contract. The diff is the new provenance hash plus the new
  schema descriptions; no enum or type shape changed.
- `frontend/src/shared/api/paradise-city-v1/__tests__/location-http.test.ts`:
  the expected projection input hash is now the new location SHA-256
  `acbd0feaa10bdc742c8ce906050bf6a463b037f4a52c2c2e151827e78cefa32e`. The
  `hashDrift` probe now patches `pathlib.Path.read_bytes` (the actual local
  read path) instead of the retired `subprocess.check_output` call, and still
  returns `b'{}'` so the projection must reject non-matching bytes atomically.
  The `atomicRejections.hashDrift === true` assertion remains.
- `frontend/src/shared/api/paradise-city-v1/generate.py`: the shared-registry
  subprocess now runs the canonical full check instead of `--venue-only`. The
  tracked `domain-operations.generated.ts` is the full projection at this base;
  the `--venue-only` projection is a historical comparison that no committed
  artifact matches. The full check still verifies every operation and every
  input hash, so no drift assertion was removed or loosened.
- `frontend/src/shared/api/paradise-city-v1/README.md`: updated the current
  working-tree Notes hash and corrected the now-stale statement that
  `generate.py` invokes `--venue-only`; the canonical full check is the drift
  gate and `--venue-only` remains a historical comparison.
- `meeting.test.ts` was not edited. It hashes the pinned git object
  `0981774d...:backend/docs/api/paradise-city-v1/meeting.openapi.json`, which
  still matches its `MEETING_CONTRACT.openapiSha256`; it passed in the suite.

Commands and results on the committed state:

| Command | Exit | Result |
| --- | --- | --- |
| `python3 frontend/src/shared/api/paradise-city-v1/generate.py --check` | 0 | `Domain transport metadata current: 140 operations`; notes SHA `246d8029...`; no drift |
| `cd frontend && corepack pnpm@11.7.0 exec vitest run src/shared/api/paradise-city-v1` | 0 | 8 test files passed; 185 tests passed |
| `python3 scripts/mcr/generate-domain-transport.py --check` | 0 | `Domain transport metadata current: 140 operations` |
| `git diff --check` | 0 | no whitespace errors |
| `git status --short` after commit | 0 | empty |

The location probe was also confirmed to exercise the new path: patching
`pathlib.Path.read_bytes` to `b'{}'` produces a hash mismatch and the
projection raises `ValueError` before mutating the supplied dictionaries, so
the test's atomic-rejection assertion is a real failure path, not a bypass.

Wire-shape audit for the Notes projection: the regenerated `notesSchemas`
diff is descriptions plus provenance only; all generated `NotesInputs`,
`NotesResponses` and exported type names/enums are unchanged.

## Cleanup

No temporary worktrees, servers or committed files were created beyond the two
task commits. The worktree-local `frontend/node_modules` installed only to run
Vitest is ignored and was removed after the recorded run; `/tmp/venue-gen*.out`,
`/tmp/venue-vitest*.out` and the plan-check outputs are transient.
