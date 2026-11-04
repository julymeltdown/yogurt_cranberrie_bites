#!/usr/bin/env python3
"""Run the source-bound private World remote invalidation controller."""
from pathlib import Path
import runpy
runpy.run_path(str(Path(__file__).resolve().parents[5] / "backend/scripts/mcr/world_remote_invalidation_smoke.py"),run_name="__main__")
