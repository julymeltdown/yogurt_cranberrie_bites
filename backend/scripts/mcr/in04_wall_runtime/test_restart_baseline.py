"""Protocol tests with synthetic observations; no product/runtime qualification."""
import copy
from pathlib import Path
import sys
import time
import unittest
from unittest.mock import Mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from in04_wall_runtime.observer import Observations
from in03_stack_runtime import InputRefusal


class BaselineCases(unittest.TestCase):
    def setUp(self):
        self.observer = Observations.__new__(Observations)
        self.observer._restart_stage = None
        self.observer.expires_at = time.time() + 30
        self.observer.guard = Mock()
        self.generation = {"systemIdentifier": "123456", "concertPid": 42,
                           "postmasterStartedAt": "2026-11-04T20:00:00Z"}
        self.binding = {"fixture": "closed-binding"}
        self.observer.observe = Mock(return_value={"generation": copy.deepcopy(self.generation)})

    def test_unarmed_notification_cannot_observe_or_queue(self):
        with self.assertRaises(InputRefusal):
            self.observer.record_restart_baseline(self.binding, self.generation)
        self.observer.observe.assert_not_called()

    def test_arm_is_once_and_has_no_observation_effect(self):
        self.observer.arm_restart_baseline()
        self.observer.observe.assert_not_called()
        with self.assertRaises(InputRefusal):
            self.observer.arm_restart_baseline()

    def test_stale_pid_or_postmaster_or_cluster_is_refused(self):
        self.observer.arm_restart_baseline()
        for key, value in (("concertPid", 43), ("postmasterStartedAt", "2026-11-04T19:00:00Z"),
                           ("systemIdentifier", "654321")):
            with self.subTest(key=key), self.assertRaises(InputRefusal):
                self.observer.record_restart_baseline(self.binding, {**self.generation, key: value})
        self.assertIsNone(self.observer.take_restart_baseline())

    def test_extra_fields_cannot_carry_commands(self):
        self.observer.arm_restart_baseline()
        with self.assertRaises(InputRefusal):
            self.observer.record_restart_baseline(self.binding, {**self.generation, "restart": True})
        self.observer.observe.assert_not_called()

    def test_actual_binding_validation_precedes_record(self):
        self.observer.arm_restart_baseline()
        self.observer.observe.side_effect = InputRefusal("synthetic-foreign-binding")
        with self.assertRaises(InputRefusal):
            self.observer.record_restart_baseline({"fixture": "foreign"}, self.generation)
        self.assertIsNone(self.observer.take_restart_baseline())

    def test_record_is_detached_and_single_consumption(self):
        self.observer.arm_restart_baseline()
        receipt = self.observer.record_restart_baseline(self.binding, self.generation)
        receipt["generation"]["concertPid"] = 999
        self.generation["concertPid"] = 888
        result = self.observer.take_restart_baseline()
        self.assertEqual(result["concertPid"], 42)
        self.assertIsNone(self.observer.take_restart_baseline())
        with self.assertRaises(InputRefusal):
            self.observer.record_restart_baseline(self.binding, result)

    def test_expiry_between_record_and_owner_poll_refuses(self):
        self.observer.arm_restart_baseline()
        self.observer.record_restart_baseline(self.binding, self.generation)
        self.observer.expires_at = 0
        with self.assertRaises(InputRefusal):
            self.observer.take_restart_baseline()


if __name__ == "__main__":
    unittest.main()
