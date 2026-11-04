"""Offline candidate/reviewed motion fixture coverage; no product registration."""
from pathlib import Path
from jsonschema import Draft202012Validator, FormatChecker


def motion_schema_paths(root):
    return [root / ('contracts/assets/v2/' + version + 'avatar-' + kind + '.schema.json')
            for version in ('', 'reviewed/') for kind in ('config', 'manifest')]


def check_motion_fixtures(root, read, fixture_data, inventory):
    """Classify only exact published fixture roots, with explicit accept/reject."""
    groups = [('contracts/fixtures/assets/motion-v2/', 'contracts/assets/v2/avatar-config.schema.json', 'candidate'),
              ('contracts/fixtures/assets/motion-v2/reviewed/', 'contracts/assets/v2/reviewed/avatar-config.schema.json', 'reviewed')]
    for prefix, schema_path, qualification in groups:
        rule = read(root / schema_path)
        Draft202012Validator.check_schema(rule)
        validator = Draft202012Validator(rule, format_checker=FormatChecker())
        rows = [(path, value) for path, value in fixture_data.items()
                if path.startswith(prefix) and len(Path(path[len(prefix):]).parts) == 2
                and Path(path[len(prefix):]).parts[0] in ('valid', 'invalid')]
        counts = {kind: sum(Path(path).parent.name == kind for path, _ in rows) for kind in ('valid', 'invalid')}
        if counts != {'valid': 15, 'invalid': 16}:
            raise ValueError('motion fixture coverage drift: ' + qualification)
        for path, value in sorted(rows):
            expected = 'accept' if Path(path).parent.name == 'valid' else 'reject'
            if validator.is_valid(value) != (expected == 'accept'):
                raise ValueError('motion fixture expectation mismatch: ' + path)
            inventory.append({'file': path, 'family': 'assets-motion-v2', 'expected': expected,
                              'qualification': qualification + '_SCHEMA_ONLY', 'result': 'PASS'})
