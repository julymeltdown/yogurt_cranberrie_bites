"""Reuse the retained bounded supervisor with this renewed lease's exact source paths.
This file is evidence-only orchestration, not another application/test runtime.
"""
import importlib.util
import sys
from pathlib import Path

ROOT = Path('/Users/lhs/Desktop/mcr-worktrees/fe-ref-map')
SUPERVISOR = ROOT / 'frontend/src/entities/avatar/runtime/__tests__/reviewed-production/qualify.py'
spec = importlib.util.spec_from_file_location('ar03_bounded_source_checks', SUPERVISOR)
if spec is None or spec.loader is None:
    raise RuntimeError('Retained supervisor unavailable')
sys.dont_write_bytecode = True
checks = importlib.util.module_from_spec(spec)
spec.loader.exec_module(checks)
checks.BASE = '3b34d9636a525b4248c43483e9b41a845de025ea'
checks.EVIDENCE = ROOT / 'docs/mcr/evidence/customize-reviewed-production/owner-integration'
checks.NEW_PREFIXES = ['docs/mcr/evidence/customize-reviewed-production/owner-integration/']
checks.EXACT = [
    'frontend/src/features/avatar/motion-adoption/production-owner.ts',
    'frontend/src/features/avatar/motion-adoption/MotionEditor.tsx',
    'frontend/src/features/avatar/__tests__/reviewed-production/owner.test.tsx',
    'docs/mcr/frontend/customize-reviewed-production.md',
]
# Conservatively charge entire retained implementation/tests/evidence, not just new growth.
# This includes every old attempt/log and all current carry under the prior exact source lease.
retained = [ROOT / path for path in checks.EXACT]
retained += [ROOT / path for path in [
    'frontend/src/entities/avatar/runtime/motion-adoption/reviewed-production.ts',
    'frontend/src/entities/avatar/runtime/motion-adoption/runtime.ts',
    'frontend/src/entities/avatar/runtime/motion-adoption/reviewed-manifest.ts',
    'frontend/src/features/avatar/motion-adoption/production-persistence.ts',
    'frontend/src/features/avatar/motion-adoption/controller.ts',
]]
retained_dirs = [ROOT / path for path in [
    'frontend/src/entities/avatar/runtime/__tests__/reviewed-production',
    'frontend/src/features/avatar/__tests__/reviewed-production',
    'docs/mcr/evidence/customize-reviewed-production',
]]
def whole_charged(temporary):
    files = set(retained)
    for directory in retained_dirs:
        files.update(p for p in directory.rglob('*') if p.is_file() or p.is_symlink())
    return checks.tree_bytes(temporary) + sum(p.lstat().st_size for p in files if p.exists())
checks.charged_bytes = whole_charged
if __name__ == '__main__':
    raise SystemExit(checks.main())
