# Current service build receipt and qualification report

Status: **BUILD_VERIFIED; CURRENT RUNTIME NOT_RUN**. The six retained MCR services have been successfully compiled from exact published base `683484baaf78792292337ad0c197a5d574536db3`, copied into a permission-guarded bundle, and bound to an exact build receipt satisfying `in01_provision.py` constructor validation. The recorded checks cover compilation, byte/source binding and executable help. They do not establish startup, migrations, readiness, current authentication or World behavior; those require the separately assigned current-runtime execution.

## 1. Scope and checkout boundary

- **Checkout**: `/Users/lhs/Desktop/mcr-worktrees/qa-current-service-build`
- **Branch**: `codex/mcr-qa-current-service-build`
- **Source Revision**: `683484baaf78792292337ad0c197a5d574536db3`
- **Sole Tracked Writable Path**: `docs/mcr/qa/current-service-build.md`
- **Private Build Target**: `/Users/lhs/Desktop/mcr-build/current-service-683484b/` (isolated private target, copy-on-write cloned from retained baseline; retained target remained untouched).

---

## 2. Retained service binaries & hashes

The six services were compiled with `cargo build --locked -j 1` and placed into immutable bundle `/Users/lhs/Desktop/mcr-build/current-service-683484b/bundle` (mode `0700`):

| Service | Binary Path | SHA-256 |
|---|---|---|
| `api-gateway` | `/Users/lhs/Desktop/mcr-build/current-service-683484b/bundle/api-gateway` | `eed2fb4cc06a2f4cf88b7b790e00bd83cca8e68ab74ff5f696f536603739a9ea` |
| `auth-service` | `/Users/lhs/Desktop/mcr-build/current-service-683484b/bundle/auth-service` | `a7ca963053b6734a0db34b89125dac7a25b182703844fefad1c5492bb033dcbb` |
| `profile-service` | `/Users/lhs/Desktop/mcr-build/current-service-683484b/bundle/profile-service` | `f278d58621c4922e12a8f3ac84f7d9268c73a004fbfa5697c7cb75fbf5f86ec6` |
| `friend-service` | `/Users/lhs/Desktop/mcr-build/current-service-683484b/bundle/friend-service` | `9250eccdf74664c1dda3b2cda77b04eed6d953a392256cd5a3bb0be817d70ef9` |
| `post-service` | `/Users/lhs/Desktop/mcr-build/current-service-683484b/bundle/post-service` | `47e5b34092ba2b6112006b31dfb434925ecb6dab2aca8f2ca136e5383692d805` |
| `notification-service` | `/Users/lhs/Desktop/mcr-build/current-service-683484b/bundle/notification-service` | `6e5b9949d0fde226398bb5b9bc03022d25acfcc39a57a5e1cc2f310fd588e957` |

---

## 3. Build receipt specification

- **Receipt Path**: `/Users/lhs/Desktop/mcr-build/current-service-683484b/build_receipt.json` (mode `0600`)
- **Receipt SHA-256**: `beed9cfaad633a9c9e00cd85cbf4af21b9572a1eae03c94680cd067969ac27dc`
- **Contents**:
  - `exit_code`: 0
  - `source_revision`: `683484baaf78792292337ad0c197a5d574536db3`
  - `binary_sha256`: All 6 binaries mapped exactly.
  - `source_files_sha256`: 1,286 tracked repository-relative backend files mapped to exact SHA-256 checksums.

---

## 4. Verification against `in01_provision.py` constructor

The bundle and receipt were verified directly against `in01_provision.py::validate_binaries`:
- `source_revision`: `683484baaf78792292337ad0c197a5d574536db3`
- `current_source_drift`: 0 (zero drift detected)
- `qualification_scope`: `SOURCE_HASHES_MATCH_BUILD_RECEIPT`
- Pre-flight execution check: all 6 binaries cleanly execute with `--help` returning usage and migration flags.

