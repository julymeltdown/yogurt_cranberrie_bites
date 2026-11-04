"""Closed Operations access producer/consumer wire fixtures; not runtime authority."""
import copy
import importlib.util
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[3]
SPEC = importlib.util.spec_from_file_location("operations_access_contract", ROOT / "scripts/mcr/validate-domain.py")
domain = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(domain)
OP = "getOwnConcertOperationsAccess"
ACTOR = "00000000-0000-4000-8000-000000000001"
EVENT = "00000000-0000-4000-8000-000000000002"
DECISION = "00000000-0000-4000-8000-000000000003"


class AccessContract(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.contract = domain.Contract()
        cls.op = next(o for o in cls.contract.operations if o["operationId"] == OP)

    def fixture(self):
        return {"server_time": "2026-11-04T17:00:00.000Z", "access": {
            "actor_id": ACTOR, "event_id": EVENT, "role": "OPERATOR",
            "role_revision": 7, "decision_id": DECISION,
            "checked_at": "2026-11-04T17:00:00.000Z", "expires_at": "2026-11-04T17:01:00.000Z"}}

    def test_self_read_does_not_accept_a_caller_role_or_command(self):
        self.assertEqual((self.op["actor"], self.op["write_policy"], self.op["cache_control"]), ("SELF", "READ", "no-store"))
        self.assertEqual((self.op["request"], self.op["idempotency_key"], self.op["pending_status"]), (None, False, None))
        path = f"/api/concert/events/{EVENT}/me/operations-access"
        self.assertEqual(self.contract.validate_target(OP, path, "", event_id=EVENT)["query"], {})
        for query in ("role=OPERATOR", "actor_id=" + ACTOR, "revision=7", "limit=1"):
            with self.subTest(query=query), self.assertRaises(domain.ContractError):
                self.contract.validate_target(OP, path, query, event_id=EVENT)
        with self.assertRaises(domain.ContractError):
            self.contract.validate_target(OP, path, "", event_id=ACTOR)

    def test_current_role_and_explicit_no_assignment_are_closed_responses(self):
        for role in ("MODERATOR", "OPERATOR"):
            value = self.fixture(); value["access"]["role"] = role
            self.contract.validate_http(value, OP, 200)
        self.contract.validate_http({"server_time": self.fixture()["server_time"], "access": None}, OP, 200)
        with self.assertRaises(domain.ContractError):
            self.contract.validate_http(self.fixture(), OP, 202)

    def test_reject_private_fields_invalid_revision_role_and_non_utc_time(self):
        invalid = []
        for field, value in (("role", "MEMBER"), ("role_revision", 0), ("role_revision", 9007199254740992),
                             ("checked_at", "2026-11-04T17:00:00+09:00"), ("actor_id", "not-an-id")):
            item = self.fixture(); item["access"][field] = value; invalid.append(item)
        for field in ("token", "email", "reporter_id", "grant", "event_revision", "phase"):
            item = self.fixture(); item["access"][field] = "private"; invalid.append(item)
        invalid.append({**self.fixture(), "grant": True})
        for item in invalid:
            with self.subTest(item=item), self.assertRaises(domain.ContractError):
                self.contract.validate_http(item, OP, 200)

    def test_versioned_policy_fixes_clock_identity_fence_and_withdrawal(self):
        rule = self.contract.policy["rules"][self.op["rule"]]
        self.assertEqual(rule["contract_version"], 1)
        text = " ".join(rule["preconditions"] + rule["result"])
        for required in ("SAME PostgreSQL clock_timestamp", "checked_at+60", "request-start", "monotonic", "BEFORE replacement", "HTTP body terminal", "decision_id", "getConcertSettings"):
            self.assertIn(required, text)


if __name__ == "__main__":
    unittest.main()
