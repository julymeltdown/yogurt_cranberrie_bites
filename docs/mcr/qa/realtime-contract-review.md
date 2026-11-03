# C05 independent review and freeze — 2026-11-03

Both user-supplied visible chats reviewed the integrator's C05 candidate read-only:
backend `01a19f83-12cc-7f21-8ad3-55749f012d9b` for server authority/admission,
frontend `01a19f83-2c2a-7fa0-a98d-1c4f5f65cb3c` for consumer recovery/privacy.
Neither reviewer edited the shared contract. Final reviewed logic candidate:
`d43b89572101e7c8d7618ede0fcc1c7cac2d2035`.

| Finding | Correction and regression |
|---|---|
| P2: room switching changed map/manifest identity without reconnect | `2af7a3f`: same event and asset tuple required; mismatched fields reject. |
| P2: queued old-stream deltas rejected while resync was pending | `2af7a3f`: validate then discard without advancing the frozen baseline. |
| P2: snapshot retained old roster after removals/revocation | `2af7a3f`: single roster, metadata-only snapshot, no retained admission ticket, full revoke cleanup. |
| P2: replacement snapshots extended access expiry | `82ec896`: exact deadline invariant across RESYNC, ROOM_SWITCH and EPOCH_CHANGED; all three have regressions. |
| P2: asset mismatch resumed using stale assets | `82ec896`: mismatch/backpressure terminate the trace; in-place resync rejects. |
| P3: imported AvatarState revision exceeded safe integers | `82ec896`: generic wire-number bound without duplicating C03. `d43b895` also handles huge literals without Python overflow/parser errors escaping WireError. |

Both final rechecks report **no remaining findings within that scope**. Each ran
31 regressions and 24 indexed fixtures, exit 0. Backend checked atomic expiry
rejection and large integers; frontend additionally probed 401-, 1,001- and
5,000-digit inputs. All fail with bounded WireError results.

Reviewed SHA-256 values:

| Path | SHA-256 |
|---|---|
| `docs/mcr/realtime-contract.md` | `3fa2abe8588026148ff646e924436d50ffd6ffcb364209ed200fa2713f1fcde4` |
| `contracts/realtime/v1/world.schema.json` | `e886e60bcb417149eadf1f32abc06547d2ab27f2c51e5f54aaea6777bfb3ff88` |
| `contracts/realtime/v1/policy.json` | `82568708eec232e1b4e84facaa11b8ed4401ac89876e55b1ab82c488de60ab8d` |
| `contracts/realtime/v1/operations.json` | `400d28e861ca5a87c54fde447f77b1680f7ea58e3047ee43b46db25816af6da0` |
| `scripts/mcr/validate-realtime.py` | `0934aedbd1c7004398b4521c6f304271e104a6e7d3b5119d8118ff0b5ae7d40e` |
| `scripts/mcr/tests/test_realtime.py` | `b06e720391f90143de1ef9aa691f9e52a17b7af3d2af99eae268469b51a8aebf` |

The freeze descendant changes only documentation/state/title metadata, not the
reviewed fields or validation logic. Parent previously ran the combined contract
command: inherited/core generation drift, asset/core/realtime fixtures, 21 asset
and 23 core tests plus the then-25 realtime tests passed. After corrections the
focused realtime suite passes **31**. C05 is accepted as the specified contract
task and protocol v1.0.0 is frozen for producer/consumer work.

Real ticket atomicity, Auth/Concert checks, takeover/switch races, network queues,
timers, privacy propagation and measured capacity remain **NOT_RUN**. This review
does not grant BE11–BE14, FE16–FE17, QA08, IN01 or release acceptance. C07 must still
register the HTTP inputs in the final inherited OpenAPI assembler and qualify
generated consumers.
