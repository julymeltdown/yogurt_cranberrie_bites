#!/usr/bin/env bash
set -euo pipefail
mcr_release_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
exec python3 -B "$mcr_release_root/scripts/mcr/release_checks/check.py" --root "$mcr_release_root" "$@"
