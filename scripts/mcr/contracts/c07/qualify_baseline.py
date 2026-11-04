#!/usr/bin/env python3
"""Run unchanged QA01 logic against an explicitly audited temporary pin candidate.

Never writes the active pin file and never enables --freeze or disables checks.
The ordinary public QA01 CLI and its full regressions must run after publication
of a qualified forward baseline.
"""
import argparse
import importlib.util
import json
from pathlib import Path
import sys
from types import SimpleNamespace

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[4]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--pins', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    spec = importlib.util.spec_from_file_location('qa01_forward_candidate', ROOT / 'scripts/mcr/contracts/qa01-coverage/run.py')
    qa = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(qa)
    if args.pins.resolve() == qa.PIN.resolve():
        raise ValueError('Forward preflight requires an external temporary candidate')
    qa.PIN = args.pins.resolve()
    report = {'task': 'QA01_FORWARD_PREFLIGHT', 'candidate': qa.git('rev-parse', 'HEAD'),
              'pins_sha256': qa.sha(qa.PIN.read_bytes()), 'active_pins_unchanged': True}
    try:
        qa.run(SimpleNamespace(probe=None, freeze=False), report)
        report['result'] = 'PASS'
    except Exception as error:
        report.update(result='FAIL', error=f'{type(error).__name__}: {error}')
    args.output.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({key: report[key] for key in ('task', 'result', 'error') if key in report}))
    return 0 if report['result'] == 'PASS' else 1


if __name__ == '__main__':
    raise SystemExit(main())
