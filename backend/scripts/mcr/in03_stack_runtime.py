#!/usr/bin/env python3
"""Owned original-IN03 stack inputs; the existing frontend harness owns the journey.

Native startup is never a fallback for missing qualified binaries or feature
configuration. This module's pure input guards do not start a service on import.
"""
from __future__ import annotations

from dataclasses import dataclass
import hashlib
import os
from pathlib import Path
import re
import stat
import time
from typing import Mapping


SERVICES = frozenset({"api-gateway", "concert-service", "post-service",
                      "notification-service", "auth-service", "friend-service",
                      "profile-service"})
ADVANCED_SERVICES = frozenset({"api-gateway", "concert-service", "post-service",
                               "notification-service"})


class InputRefusal(ValueError):
    """Public diagnostics contain only fixed guard labels, never input values."""


def require(condition: bool, label: str) -> None:
    if not condition:
        raise InputRefusal(label)


def source_revision(value: str) -> str:
    require(isinstance(value, str) and re.fullmatch(r"[0-9a-f]{40}", value) is not None,
            "source-revision-required")
    return value


@dataclass(frozen=True)
class BinaryPin:
    path: Path
    sha256: str
    source: str

    def validate(self) -> dict:
        source_revision(self.source)
        require(isinstance(self.sha256, str) and re.fullmatch(r"[0-9a-f]{64}", self.sha256) is not None,
                "binary-hash-required")
        require(self.path.is_absolute() and not self.path.is_symlink(), "explicit-binary-path-required")
        try:
            info = self.path.stat()
            require(stat.S_ISREG(info.st_mode) and os.access(self.path, os.X_OK), "binary-not-executable")
            digest = hashlib.file_digest(self.path.open("rb"), "sha256").hexdigest()
        except OSError:
            raise InputRefusal("binary-unavailable") from None
        require(digest == self.sha256, "binary-hash-mismatch")
        return {"sha256": digest, "source": self.source,
                "identity": [info.st_dev, info.st_ino, info.st_size, info.st_mtime_ns]}


def validate_binary_cohort(pins: Mapping[str, BinaryPin]) -> dict:
    require(set(pins) == SERVICES, "exact-seven-service-cohort-required")
    require(len({pin.path for pin in pins.values()}) == len(SERVICES), "distinct-service-binaries-required")
    return {name: pins[name].validate() for name in sorted(SERVICES)}


def private_input(path: Path) -> bytes:
    require(path.is_absolute() and not path.is_symlink(), "explicit-private-input-required")
    try:
        info = path.stat()
        require(stat.S_ISREG(info.st_mode) and info.st_uid == os.getuid()
                and info.st_mode & 0o077 == 0, "private-input-custody-required")
        require(info.st_size <= 1024 * 1024, "private-input-too-large")
        return path.read_bytes()
    except OSError:
        raise InputRefusal("private-input-unavailable") from None


@dataclass(frozen=True)
class WorkDeadline:
    work_monotonic: float
    release_monotonic: float

    def check(self, *, cleanup: bool = False) -> None:
        require(time.monotonic() < (self.release_monotonic if cleanup else self.work_monotonic),
                "allocation-release-expired" if cleanup else "allocation-work-expired")

