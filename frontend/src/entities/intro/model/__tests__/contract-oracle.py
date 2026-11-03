"""Independent frozen JSON Schema oracle; local contract documents only, no network."""
import json
import sys
from pathlib import Path
from jsonschema import Draft202012Validator, FormatChecker
from referencing import Registry, Resource

root = Path(__file__).resolve().parents[6]
documents = [json.loads(p.read_text()) for p in (root / 'contracts').rglob('*.schema.json')]
registry = Registry().with_resources((d['$id'], Resource.from_contents(d)) for d in documents if '$id' in d)
social = next(d for d in documents if d.get('$id') == 'https://contracts.mcr.invalid/domain/v1/social.schema.json')
rows = json.load(sys.stdin)
print(json.dumps([Draft202012Validator({'$ref': social['$id'] + '#/$defs/' + r['definition']}, registry=registry, format_checker=FormatChecker()).is_valid(r['value']) for r in rows]))
