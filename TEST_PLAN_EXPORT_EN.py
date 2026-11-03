#!/usr/bin/env python3
"""Regression probes for the planning exporter, not application tests."""
import copy
import importlib.util
import json
import tempfile
from pathlib import Path

root = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('plan_export', root / 'MCR_EXPORT_PLAN_EN.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
source = root / 'PLAN_TABLES_EN.md'
text = source.read_text(encoding='utf-8')
results = []

def probe(name, mutate, expected):
    tasks, screens = module.parse(text)
    probe_text = mutate(tasks, screens) or text
    try:
        module.validate(probe_text, tasks, screens)
    except module.PlanError as exc:
        if expected not in str(exc):
            raise AssertionError(f'{name}: wrong rejection: {exc}') from exc
        results.append({'test': name, 'result': 'PASS', 'expected_failure_detected': str(exc)})
    else:
        raise AssertionError(f'{name}: invalid input was accepted')

probe('duplicate_task_id', lambda tasks, screens: tasks[1].update(id=tasks[0]['id']), 'Duplicate task')
probe('missing_dependency', lambda tasks, screens: tasks[0]['depends_on'].append('BE99'), 'unknown dependency')
probe('dependency_cycle', lambda tasks, screens: tasks[0]['depends_on'].append('C02'), 'Dependency cycle')
probe('unknown_ia_task', lambda tasks, screens: screens[0]['task_ids'].append('FE99'), 'unknown task')
probe('uncovered_ia', lambda tasks, screens: screens[0].update(task_ids=[]), 'no implementation coverage')
probe('frontend_backend_coupling', lambda tasks, screens: next(t for t in tasks if t['id']=='FE01')['depends_on'].append('BE01'), 'frontend waits on backend')
probe('zero_task_false_green', lambda tasks, screens: tasks.clear(), 'Expected 118 tasks')
probe('non_english_source', lambda tasks, screens: text + chr(0xac00), 'Hangul')
with tempfile.TemporaryDirectory() as directory:
    a, b = Path(directory)/'a', Path(directory)/'b'
    module.export(source, a, 118, 64)
    module.export(source, b, 118, 64)
    for path in a.iterdir():
        assert path.read_bytes() == (b/path.name).read_bytes(), path.name
    results.append({'test': 'deterministic_export', 'result': 'PASS'})
report = {
    'scope': 'Planning-exporter regression probes only; no application test executed.',
    'result': 'PASS',
    'test_count': len(results),
    'tests': results,
}
(root/'generated'/'EXPORTER_TESTS_EN.json').write_text(json.dumps(report, indent=2)+'\n', encoding='utf-8')
print(json.dumps(report, indent=2))
