# FE17 headless replication independent review

Date: 2026-11-03. **Bounded review complete: no actionable defect found in the checked
headless FE17 contract, lifecycle, and ordering cases at `bd202ad`. All 7
delivered files match sealed receipt `dd0bd78f`. Independent adversarial probe
confirms delta suppression during resync, 64-input queue saturation recovery,
throwing sink termination, room-switch snapshot ordering, and zero-replay
visibility cycling. No actionable defects identified.** Implementation status
remains MOCK_READY.

## Scope and authority

QA worktree: `/Users/lhs/Desktop/mcr-worktrees/qa-replication`, branch
`codex/mcr-qa-FE17`, assigned base `bd202adb4888ef0f92fec915ec2226a6c9ec6334`;
worker comparison base `4dc3bb3dd8015abcf26839c34954f7ca91870ceb`.
Evaluated worker commit: `bd202adb4888ef0f92fec915ec2226a6c9ec6334`.
Sole tracked writable file is this report at its absolute path:
`/Users/lhs/Desktop/mcr-worktrees/qa-replication/docs/mcr/qa/replication-review.md`.
No application code, shared API modules, contracts, manifests, or locks were edited.
No Cargo, PostgreSQL, or push operations were performed.

Verified handoff receipt: `/tmp/mcr-fe17-handoff/receipt.json`
(12,240 bytes, SHA-256 `dd0bd78fe8896cfb95377443ed2f902dff25ebd65c0bd16742fac870a3d4e851`).
All 7 delivered files in `bd202ad` match the receipt byte-for-byte and hash-for-hash:
- `docs/mcr/frontend/world-replication.md` (11,214 B, `32d6bcce22cc87edc884c076c1de47e5dda378ca78d58a6177a0cea6e16d90a8`)
- `frontend/src/world/entities/replicated.ts` (2,435 B, `05a3222c8ae68be52a7118f3c04c0d0bce686916328422b50134d535e5f26700`)
- `frontend/src/world/network/__tests__/fixtures/replication-browser.html` (297 B, `7c9d194dc4b25d07b01af84131f03ae96f4ae2e889c3f2e58e9af8a980075c8e`)
- `frontend/src/world/network/__tests__/fixtures/replication-browser.ts` (2,509 B, `f33a6363787eafe8086eaa52be9a2373cbeedecc5f108b5e674d15932ab6e96b`)
- `frontend/src/world/network/__tests__/replication-browser.mjs` (8,476 B, `5176e98a8ff930e2e8344bed479af91559f0bd0a6874d0dab38fb47417e4d255`)
- `frontend/src/world/network/__tests__/replication.test.ts` (17,029 B, `7c9f18e2e0dc5c8d7f991b8fe39493dfe3dff4ecdcbd398bac1d476e593e25e5`)
- `frontend/src/world/network/replication.ts` (13,389 B, `568dda7c23f6d66f9cf5c8239bacf1f58ff1744b0746e07d6cc72812f76f1dd2`)

## Invariant and lifecycle evaluation

1. **Snapshot, Delta & Ghost Removal**:
   - `INITIAL`, `RESYNC`, `ROOM_SWITCH`, and `EPOCH_CHANGED` snapshots call
     `entities.reset(players, selfUserId, now)` which clears the map and installs
     the fresh roster. `snapshot.players` is discarded from the baseline.
   - Gap recovery: On `resync-needed`, `clear(true, true)` empties displayed
     entities immediately while awaiting the replacement snapshot, preventing stale ghosts.
   - Remote entity interpolation is strictly bounded to 100ms (`policy.delta_hz_max_target`)
     with linear interpolation clamped to `[0, 1]`; there is zero spatial extrapolation.
2. **Local Input Ledger & Coalescing**:
   - Axes coalesce to the 20Hz simulation target. One-shot interaction is consumed once.
   - Idle neutral intent sends once, then installs no timer while idle.
   - Ledger holds at most 64 sequence numbers (`policy.outgoing_queue_frames`); hitting 64
     triggers `requestResync('PROTOCOL_RECOVERY')` instead of growing memory or replaying.
   - Acks filter `pending = pending.filter(seq => seq > ack)`. The ledger stores only
     integers, preventing command replay.
3. **Room Switching & Microtask Ordering**:
   - `switchRoom` preserves old room baseline until a replacement snapshot arrives.
   - A microtask fence defers `live` publication when transport emits `state: live`
     before the replacement snapshot, preventing the old room from being committed as new baseline.
4. **Visibility & Connection Ownership**:
   - `setVisible(false)` withdraws transport, clears entities and pending inputs, and
     stops all timers.
   - `setVisible(true)` reconnects with a fresh ticket, resetting sequences and sending
     a fresh neutral input `seq: 1` with zero replay of old held movement or interactions.
5. **Sink Reentrancy & Exception Safety**:
   - In `publish()`, `try { options.sink.replace(view) } catch { terminate('sink'); return }`
     guarantees that a throwing sink immediately terminates the controller, clears all timers,
     and disposes the transport permanently.
6. **Spatial Prediction Boundary**:
   - Correctly exposes `spatialPrediction: 'unavailable'`. Frozen C05 v1.0.0 lacks an
     authoritative physics/collision model; FE15 local values are not authoritative BE12 parameters.

## Independent QA adversarial probe

An independent probe script was executed via Node 24.22.2 through Vite SSR
(`/tmp/mcr-qa-fe17-agy8u3i5/adversarial-probe.mjs`, 8,312 bytes, SHA-256
`7904cb7366026fe8e318c193df8e3e468d7be58ff7772b2923b7fe49a7885bcb`):

| Scenario | Assertion verified | Result |
|---|---|---|
| Resync delta suppression | Deltas arriving while in `resync` phase are dropped; snapshot recovers to clean roster | PASS |
| Queue saturation recovery | Holding movement accumulates 64 inputs; 65th triggers `PROTOCOL_RECOVERY` resync and resets pending inputs | PASS |
| Throwing sink termination | Crashing sink transitions controller to `unavailable`/`problem: 'sink'`, clears all timers, disposes transport | PASS |
| Room switch snapshot ordering | `switchRoom` replacement snapshot updates baseline cleanly without old-room microtask clobbering | PASS |
| Visibility teardown & zero replay | Hiding cancels timers/entities; foregrounding connects with fresh ticket and baseline seq 1 with zero replay | PASS |

Probe output saved to `/tmp/mcr-qa-fe17-agy8u3i5/adversarial-result.json`
(206 bytes, SHA-256 `f5b9bacd80c3427acc02efd99876331e1aec8ac6ed9593efbdeedd0d5eb287c1`).

## Toolchain and gate verification

- Focused test suite: `corepack pnpm@11.7.0 --dir frontend exec vitest run src/world/network/__tests__/replication.test.ts`
  exited 0; all 23 focused tests pass (425ms).
- Boundary check: `python3 scripts/mcr/check-boundaries.py --base 4dc3bb3dd8015abcf26839c34954f7ca91870ceb --lane frontend --allow-doc docs/mcr/frontend/world-replication.md --allow-doc docs/mcr/qa/replication-review.md`
  exited 0; 8 changes vs base.
- Planning gate: `python3 scripts/mcr/check-plan.py` exited 0 (118 tasks, 64 IA entries).
- UI Origin check: `node scripts/mcr/check-ui-origin.mjs` exited 0.
- Worker build/lint/native evidence: Sealed in receipt `dd0bd78f` (Chrome 156.0.8078.80 native report).

## Disposition and limitations

The checked headless FE17 cases passed; this is not proof of all possible interleavings.
The persistent goal remains **ACTIVE** awaiting parent review and disposition.

Limitations:
- Headless replication only; FE12 character animation, sprite compositing, and visual rendering are NOT_RUN.
- Spatial prediction remains unavailable pending authoritative BE12 physics/collision integration.
- Production network/backend WebSocket cluster, load, and IN01 two-user gates are NOT_RUN. Worker native evidence uses an actual loopback socket with synthetic server frames.
- Implementation status is **MOCK_READY**.

