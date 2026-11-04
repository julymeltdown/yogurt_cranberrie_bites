"""Fixed-label failures, bounded numeric evidence and private input handling."""
from __future__ import annotations

import hashlib
import json
import math
import os
from pathlib import Path
import re
import stat
import sys
import time

ROOT = Path(__file__).resolve().parents[4]
MAX_PRIVATE_INPUT = 8 * 1024 * 1024


class Failure(Exception):
    def __init__(self, label):
        if not isinstance(label, str) or not re.fullmatch(r"[a-z][a-z0-9-]{0,79}", label):
            label = "invalid-failure-label"
        super().__init__(label)
        self.label = label


def check(condition, label):
    if not condition:
        raise Failure(label)


def number(value, minimum, maximum, label):
    check(type(value) in (int, float) and math.isfinite(value)
          and minimum <= value <= maximum, label)
    return value


def integer(value, minimum, maximum, label):
    check(type(value) is int and minimum <= value <= maximum, label)
    return value


def fields(value, required, optional=()):
    check(isinstance(value, dict) and set(required) <= value.keys()
          and value.keys() <= set(required) | set(optional), "input-fields")


def json_value(raw):
    def pairs(items):
        result = {}
        for key, value in items:
            check(key not in result, "duplicate-json-key")
            result[key] = value
        return result

    try:
        if isinstance(raw, bytes):
            raw = raw.decode("utf-8")
        return json.loads(raw, object_pairs_hook=pairs,
                          parse_constant=lambda _: (_ for _ in ()).throw(Failure("nonfinite-json")))
    except Failure:
        raise
    except (ValueError, UnicodeError, RecursionError):
        raise Failure("invalid-json") from None


def read_private(path=None, stream=None):
    """Never echo an input path, parser exception, body, token or identifier."""
    if path is None:
        raw = (stream or sys.stdin.buffer).read(MAX_PRIVATE_INPUT + 1)
    else:
        check(Path(path).is_absolute(), "private-input-path")
        flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_NONBLOCK", 0)
        fd = os.open(path, flags)
        with os.fdopen(fd, "rb") as handle:
            info = os.fstat(handle.fileno())
            check(stat.S_ISREG(info.st_mode) and stat.S_IMODE(info.st_mode) == 0o600
                  and info.st_uid == os.getuid() and info.st_nlink == 1, "private-input-mode")
            raw = handle.read(MAX_PRIVATE_INPUT + 1)
    check(len(raw) <= MAX_PRIVATE_INPUT, "private-input-size")
    return json_value(raw)


class Samples:
    """Bounded retained samples with exact count/sum/extrema for the full run."""
    def __init__(self, limit=4096):
        self.limit, self.values = limit, []
        self.count, self.total, self.maximum = 0, 0.0, 0.0

    def add(self, value):
        number(value, 0, 1e12, "numeric-measurement")
        self.count += 1
        self.total += value
        self.maximum = max(self.maximum, value)
        if len(self.values) < self.limit:
            self.values.append(value)

    def public(self):
        values = sorted(self.values)
        def percentile(fraction):
            return values[max(0, math.ceil(len(values) * fraction) - 1)] if values else None
        return {"count": self.count, "mean": self.total / self.count if self.count else None,
                "max": self.maximum if self.count else None,
                "retained_samples": len(values), "sample_policy": "first-bounded-window",
                "retained_p50": percentile(.5), "retained_p95": percentile(.95)}


def source_pins():
    paths = ["scripts/mcr/validate-realtime.py", "scripts/mcr/validate-core.py",
             "scripts/mcr/validate-assets.py", "scripts/mcr/requirements-contracts.txt", "backend/scripts/docs/generate-mcr-core.py",
             "backend/scripts/mcr/world_two_principal_probe.py", "backend/docs/api/mcr-core.openapi.json"]
    paths += [str(p.relative_to(ROOT)) for base in ["contracts/realtime/v1", "contracts/core/v1",
              "contracts/assets/v1", "contracts/assets/v2/reviewed"]
              for p in sorted((ROOT / base).glob("*.json"))]
    paths += [str(p.relative_to(ROOT)) for p in sorted((ROOT / "backend/scripts/mcr/world_capacity_support").glob("*.py"))]
    paths += ["backend/scripts/mcr/world_capacity_probe.py", "backend/scripts/mcr/world_live_failure_probe.py"]
    return {name: hashlib.sha256((ROOT / name).read_bytes()).hexdigest() for name in paths}


def observations(value):
    """Only a producer-supplied hook can supply server/oracle measurements."""
    allowed = {"server_cpu_seconds", "server_rss_bytes", "memberships", "prepared_transactions"}
    check(isinstance(value, dict) and bool(value) and value.keys() <= allowed, "oracle-fields")
    for item in value.values():
        number(item, 0, 1e18, "oracle-number")
    for name in allowed - {"server_cpu_seconds"}:
        if name in value:
            integer(value[name], 0, 10**18, "oracle-integer")
    return {"status": "OBSERVED_PRODUCER_HOOK", **value}


def new_report(scope, fixture):
    return {"result": "FAIL", "scope": scope,
            "transport_boundary": "IN_MEMORY_FIXTURE" if fixture else "ACTUAL_PUBLIC_HTTP_WS",
            "acceptance": "NOT_CLAIMED", "credentials_recorded": False,
            "identities_frames_endpoints_recorded": False,
            "server_queue_metrics": "NOT_RUN", "server_metrics_and_cleanup_oracle": {"status": "NOT_RUN"},
            "physical_mobile_rendering": "NOT_RUN", "chosen_production_caps": "NOT_RUN",
            "source_pins": source_pins(), "steps": [], "cleanup": {"status": "NOT_RUN"}}


def elapsed(start):
    return round(time.monotonic() - start, 6)
