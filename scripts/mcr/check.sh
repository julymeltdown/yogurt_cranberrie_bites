#!/usr/bin/env bash
set -euo pipefail

mcr_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
mcr_scope="${1:-foundation}"

check_plan() {
  python3 "$mcr_root/scripts/mcr/check-plan.py"
}

check_frontend() {
  (
    cd "$mcr_root/frontend"
    if [[ "$(node --version)" != 'v24.22.2' ]]; then
      echo 'Frontend checks require Node 24.22.2 from .node-version.' >&2
      exit 1
    fi
    echo 'Frontend scope: frozen install, strict type/build, and lint.'
    corepack pnpm@11.7.0 install --frozen-lockfile
    corepack pnpm@11.7.0 run build
    corepack pnpm@11.7.0 run lint
    node "$mcr_root/scripts/mcr/check-ui-origin.mjs" --root "$mcr_root"
  )
}

check_backend() {
  (
    cd "$mcr_root/backend"
    echo 'Backend scope: locked workspace build; no database/runtime acceptance.'
    CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}" cargo +1.88.0 build --locked --workspace
  )
}

check_guards() {
  python3 -B -m unittest discover -s "$mcr_root/scripts/mcr/tests" -p test_boundaries.py
  node --test "$mcr_root/scripts/mcr/tests/ui-origin.test.mjs"
  node "$mcr_root/scripts/mcr/check-ui-origin.mjs" --root "$mcr_root"
}

case "$mcr_scope" in
  plan) check_plan ;;
  frontend) check_frontend ;;
  backend) check_backend ;;
  guards) check_guards ;;
  foundation) check_plan; check_guards; check_frontend; check_backend ;;
  *) echo 'Usage: scripts/mcr/check.sh [plan|frontend|backend|guards|foundation]' >&2; exit 2 ;;
esac
