"""Wall B closed contract and actor/original-command correlation regressions."""
import copy
import importlib.util
from pathlib import Path
import unittest

SPEC = importlib.util.spec_from_file_location('wall_b_contract', Path(__file__).parents[1] / 'validate-domain.py')
domain = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(domain)


class WallBContracts(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.contract = domain.Contract()
        cls.action = domain.read(domain.FIXTURES / 'wall-tools-b/own-fill-256.json')
        cls.tools = domain.read(domain.FIXTURES / 'wall-tools-b/tools-qualified.json')

    def test_registered_fixtures_and_closed_legacy_shapes(self):
        domain.validate_fixtures(self.contract)
        body = self.action['command']['body']
        self.contract.validate(body, 'mural.PlacePixel')
        for field, value in [('tool', 'fill'), ('cells', []), ('action_id', self.action['action_id'])]:
            with self.assertRaises(domain.ContractError):
                self.contract.validate(dict(body, **{field: value}), 'mural.PlacePixel')

    def test_changed_count_reset_and_exact_revision(self):
        for field, value in [('changed_cells', 257), ('changed_cells', 0), ('reset_required', False), ('revision', 3)]:
            bad = dict(self.action, **{field: value})
            with self.assertRaises(domain.ContractError):
                self.contract.validate(bad, 'mural.OwnAction')

    def test_no_foreign_author_or_private_before_colors(self):
        bad = copy.deepcopy(self.tools)
        bad['latest_action']['actor_id'] = self.action['event_id']
        with self.assertRaises(domain.ContractError):
            self.contract.validate(bad, 'mural.ToolCapabilities')
        with self.assertRaises(domain.ContractError):
            self.contract.validate(dict(self.action, cells=[{'index': 0, 'before': 1, 'after': 2}]), 'mural.OwnAction')

    def test_scope_and_original_key_body_required_for_recovery(self):
        a = self.action
        context = dict(actor_id=a['actor_id'], event_id=a['event_id'], idempotency_key=a['idempotency_key'],
                       original_command=a['command'], request_path=f"/api/concert/events/{a['event_id']}/mural/actions/{a['idempotency_key']}")
        self.contract.validate_http(a, 'getOwnConcertMuralAction', 200, **context)
        for field, value in [('actor_id', a['event_id']), ('idempotency_key', a['action_id']), ('original_command', None)]:
            with self.assertRaises(domain.ContractError):
                self.contract.validate_http(a, 'getOwnConcertMuralAction', 200, **dict(context, **{field: value}))
        changed = copy.deepcopy(a['command'])
        changed['body']['color_index'] = 3
        with self.assertRaises(domain.ContractError):
            self.contract.validate_http(a, 'getOwnConcertMuralAction', 200, **dict(context, original_command=changed))

    def test_only_qualified_capabilities_and_unexpired_undo(self):
        for update in [{'background_index': None}, {'editable': False}, {'remaining_cells': 513}]:
            with self.assertRaises(domain.ContractError):
                self.contract.validate(dict(self.tools, **update), 'mural.ToolCapabilities')
        bad = copy.deepcopy(self.tools)
        bad['undo']['expires_at'] = bad['server_time']
        with self.assertRaises(domain.ContractError):
            self.contract.validate(bad, 'mural.ToolCapabilities')


if __name__ == '__main__':
    unittest.main()
