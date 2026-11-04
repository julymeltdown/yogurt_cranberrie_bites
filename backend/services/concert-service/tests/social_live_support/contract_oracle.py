"""Use the retained offline C04 oracle on actual captured HTTP responses.

No schema is fetched, no transport is opened, and no private value is logged.
"""
from __future__ import annotations
import hashlib
import importlib.util
import json
from pathlib import Path
from urllib.parse import urlsplit


class ContractOracle:
    def __init__(self, repo: Path):
        path = repo / 'scripts/mcr/validate-domain.py'
        spec = importlib.util.spec_from_file_location('social_frozen_contract', path)
        self.module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(self.module)
        self.contract = self.module.Contract()
        paths = [path, Path(__file__).resolve()]
        for group in ('domain', 'core', 'assets'):
            paths += sorted((repo / 'contracts' / group / 'v1').glob('*.schema.json'))
        paths += [repo / 'contracts/domain/v1' / name for name in
                  ('operations.json', 'policy.json', 'inherited-map.json')]
        self.source_sha256 = {str(p.relative_to(repo)): hashlib.sha256(p.read_bytes()).hexdigest()
                              for p in paths}

    def target(self, operation, url, event):
        target = urlsplit(url)
        self.contract.validate_target(operation, target.path, target.query, event_id=event)

    def response(self, raw, operation, status, actor, event, url):
        value = json.loads(raw, object_pairs_hook=self.module.unique_pairs,
                           parse_constant=self.module.reject_constant)
        target = urlsplit(url)
        self.contract.validate_http(value, operation, status, actor_id=actor,
                                    event_id=event, request_path=target.path,
                                    request_query=target.query)
        return value
