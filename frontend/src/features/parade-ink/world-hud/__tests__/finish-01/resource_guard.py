"""Actual absolute-argv guardian wrapper; old guard source remains read-only."""
import runpy
import sys
from pathlib import Path
import resource_snapshot

sys.dont_write_bytecode = True
sys.modules['membership'] = resource_snapshot
runpy.run_path(str(Path(__file__).resolve().parents[7] / 'frontend/src/features/gift-discovery/__tests__/parade-ink/guardian.py'), run_name='__main__')
