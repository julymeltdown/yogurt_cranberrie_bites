"""Offline binding regressions; owning service authorization still needs native proof."""
import copy
import importlib.util
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[3]
SPEC = importlib.util.spec_from_file_location('recovery_contract', ROOT / 'scripts/mcr/validate-domain.py')
domain = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(domain)


def uid(n):
    return f'00000000-0000-4000-8000-{n:012x}'


class RecoveryReads(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.contract = domain.Contract()

    def check_pixel(self, value=None, **changes):
        args = dict(actor_id=uid(2), event_id=uid(1),
                    request_path=f'/api/concert/events/{uid(1)}/mural/commands/{uid(9)}/result',
                    idempotency_key=uid(9), expected_resource_id=uid(3),
                    expected_resource_revision=8,
                    original_command=dict(expected_revision=7, x=4, y=7, color_index=2))
        args.update(changes)
        self.contract.validate_http(value or domain.read(domain.FIXTURES / 'mural-own-result.json'),
                                    'getConcertMuralOwnResult', 200, **args)

    def test_original_pixel_requires_exact_receipt_key_target_and_command(self):
        self.check_pixel()
        for changes in [dict(idempotency_key=uid(10)), dict(expected_resource_id=uid(4)),
                        dict(expected_resource_revision=9), dict(original_command=None),
                        dict(original_command=dict(expected_revision=7, x=5, y=7, color_index=2)),
                        dict(event_id=uid(5)), dict(actor_id=None), dict(request_path=None),
                        dict(request_query='actor_id=' + uid(2))]:
            with self.subTest(changes=changes), self.assertRaises(domain.ContractError):
                self.check_pixel(**changes)

    def test_public_original_result_never_carries_author_or_unregistered_fields(self):
        value = domain.read(domain.FIXTURES / 'mural-own-result.json')
        for field in ('actor_id', 'idempotency_key', 'request_hash', 'receipt'):
            with self.subTest(field=field), self.assertRaises(domain.ContractError):
                self.check_pixel({**value, field: uid(2)})

    def test_total_is_actor_event_bound_and_not_a_page_query(self):
        value = domain.read(domain.FIXTURES / 'notification-unread-total.json')
        args = dict(actor_id=uid(2), event_id=uid(1),
                    request_path=f'/api/concert/events/{uid(1)}/me/notifications/unread-count')
        self.contract.validate_http(value, 'getConcertNotificationUnreadCount', 200, **args)
        for changes in [dict(actor_id=uid(3)), dict(event_id=uid(4)),
                        dict(request_query='limit=20'), dict(request_query='unread=true')]:
            with self.subTest(changes=changes), self.assertRaises(domain.ContractError):
                self.contract.validate_http(value, 'getConcertNotificationUnreadCount', 200,
                                            **{**args, **changes})

    def test_new_reads_cannot_be_replayed_as_commands_or_return_pending_receipts(self):
        for name in ('getConcertMuralOwnResult', 'getConcertNotificationUnreadCount'):
            op = next(o for o in self.contract.operations if o['operationId'] == name)
            self.assertEqual((op['method'], op['idempotency_key'], op['pending_response']),
                             ('get', False, None))
            with self.assertRaises(domain.ContractError):
                self.contract.validate_http(domain.read(domain.FIXTURES / 'command-pending.json'), name, 202)
