"""Frozen C03/C04 oracle for the original 16-case read-path counterexample."""
import json
import sys
from pathlib import Path
from jsonschema import Draft202012Validator, FormatChecker
from referencing import Registry, Resource

root = Path(__file__).resolve().parents[7]
docs = [json.loads(p.read_text()) for p in (root / 'contracts').rglob('*.schema.json')]
registry = Registry().with_resources((d['$id'], Resource.from_contents(d)) for d in docs if '$id' in d)
print(json.dumps([Draft202012Validator({'$ref': row['ref']}, registry=registry, format_checker=FormatChecker()).is_valid(row['value']) for row in json.load(sys.stdin)]))
