# Producer validation checkpoint

Worktree: `be-avatar-motion-v2`; branch: `codex/mcr-be-avatar-motion-v2`.
Base: `980fc4d85020f297b36fee8195e1ca92079ff33f` (published master).
Schema/catalog checkpoint: `ae41db9d`.

| Check | Result | Exit |
|---|---|---|
| `python3 docs/mcr/contracts/avatar-motion-v2/build_catalog.py --check` | PASS: 34 deterministic outputs, 20 originals, 10 handoff pins | 0 |
| `python3 docs/mcr/contracts/avatar-motion-v2/validate.py` | PASS: both schemas, 15 valid/16 invalid configs, 38 pinned resource occurrences, 6 presets/28 items, source/direction/frame/palette/cuff and actual PNG/rear-eye checks, 9 manifest tamper refusals | 0 |
| `rustfmt +1.88.0 --check --edition 2021 backend/crates/concert-domain/src/avatar_motion_v2/mod.rs backend/crates/concert-domain/tests/avatar_motion_v2.rs` | PASS, new files only | 0 |
| `python3 scripts/mcr/check-plan.py` | PASS: planning/provenance/routing only | 0 |
| Allocated Cargo `avatar_motion_v2` test | NOT_RUN, fresh shared target allocation pending | — |
| Public HTTP/WebSocket/persistence/production integration | NOT_RUN, outside producer ownership | — |
| Art review/production admission | NOT_RUN; candidate admission explicitly denied | — |

The initial unqualified `rustfmt` attempt exited by macOS dynamic-loader failure
because the machine's default stable toolchain lacks its expected rustc driver
library. The project-pinned `+1.88.0` formatter succeeds; no global toolchain or
machine setting was changed. Cargo execution will use the explicitly allocated
project target/toolchain and be recorded here before final producer handoff.

## Frozen hashes

| File | Bytes | SHA256 |
|---|---:|---|
| `backend/releases/selected-character-motion-v2/manifest.json` | 95128 | `9a555f726a9e421f14ed876117be87bb1bb8fa128ac401ab38f8927760776fd6` |
| `contracts/assets/v2/avatar-config.schema.json` | 3543 | `3cbfd3185113d7f4913de89b0e04d67046dbab64b3f90c0b5f14a261afc6a78b` |
| `contracts/assets/v2/avatar-manifest.schema.json` | 102672 | `8fc206f00d263ed9465cad932768075a1a0ca61d927e98ccc2aa8e9e7ef0d54f` |

The additional Rust source does not amend these frozen bytes. Source-art originals,
old 1.0.0/1.1.0 schemas, frontend implementation and existing parsers/proto/registries
are unchanged. The only existing central change is the granted one-line
`pub mod avatar_motion_v2;` export.
