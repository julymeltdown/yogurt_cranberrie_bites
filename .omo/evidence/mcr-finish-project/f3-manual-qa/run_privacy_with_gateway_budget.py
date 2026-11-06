#!/usr/bin/env python3
"""F3 privacy run wrapper: apply the documented development gateway request budget.

The stack driver builds service environments explicitly, so a parent-shell
GATEWAY_REQUEST_TIMEOUT_MS is not inherited. This wrapper applies the same
5000 ms transport budget the closure/in02 drivers already set for the product's
own 2000 ms command budget, and records that override in the report. It does not
change product code, service timeouts, or any owner budget.
"""
from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(ROOT / "backend/scripts/mcr"))

import dev_stack  # noqa: E402

_original = dev_stack.development_environments
OVERRIDE_MS = "5000"


def development_environments(*args, **kwargs):
    envs = _original(*args, **kwargs)
    envs["api-gateway"]["GATEWAY_REQUEST_TIMEOUT_MS"] = OVERRIDE_MS
    return envs


dev_stack.development_environments = development_environments

import in03_social_gate  # noqa: E402

raise SystemExit(in03_social_gate.main(sys.argv[1:]))
