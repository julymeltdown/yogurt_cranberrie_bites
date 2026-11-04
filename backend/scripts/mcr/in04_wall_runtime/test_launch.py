"""Root stage tests use synthetic controls; no real restart or role proof."""
from pathlib import Path
import sys
from types import SimpleNamespace
import unittest
from unittest.mock import Mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from in03_stack_runtime import InputRefusal
from in04_wall_runtime.launch import ManagedProfile


class StageCases(unittest.TestCase):
    def setUp(self):
        self.profile = ManagedProfile()
        self.profile.config = {"restart_stage": "POSTGRES_CONCERT_AFTER_BASELINE"}
        self.profile.guard = Mock()
        self.profile.stack = object()
        self.baseline = {"systemIdentifier": "123456", "concertPid": 42,
                         "postmasterStartedAt": "2026-11-04T20:00:00.123456Z"}
        self.controls = Mock()
        self.controls.binding.return_value = {"fixture": "owner-binding"}
        self.controls.inspect.return_value = {"generation": {**self.baseline,
            "postmasterStartedAt": "2026-11-04 20:00:00.123456+00",
            "concertStartedAt": "fixture-start", "ownerEpochs": []}}
        self.controls.request_postgres_concert_restart.return_value = {"state": "QUEUED"}
        self.controls.checkpoint.return_value = {"state": "COMPLETE"}
        self.observations = Mock()
        self.observations.take_restart_baseline.return_value = None
        self.attachment = SimpleNamespace(stack=self.profile.stack,
            controls=self.controls, observations=self.observations)

    def poll(self):
        self.profile.poll(attachment=self.attachment, context={}, args=None)

    def test_ready_only_arms_and_poll_without_notification_never_requests(self):
        self.profile.attachment_ready(attachment=self.attachment, context={}, args=None)
        self.observations.arm_restart_baseline.assert_called_once_with()
        self.poll()
        self.controls.inspect.assert_not_called()
        self.controls.request_postgres_concert_restart.assert_not_called()

    def test_nonrestart_window_does_not_arm(self):
        self.profile.config["restart_stage"] = "NONE"
        self.profile.attachment_ready(attachment=self.attachment, context={}, args=None)
        self.observations.arm_restart_baseline.assert_not_called()

    def test_external_owner_reobserves_then_queues_once_with_its_full_generation(self):
        self.observations.take_restart_baseline.return_value = dict(self.baseline)
        self.poll()
        args = self.controls.request_postgres_concert_restart.call_args.args
        self.assertEqual(args[0], self.controls.binding.return_value)
        self.assertEqual(args[2], self.controls.inspect.return_value["generation"])
        self.assertIn("ownerEpochs", args[2])
        self.poll()
        self.controls.request_postgres_concert_restart.assert_called_once()
        self.controls.checkpoint.assert_called_once()

    def test_rebound_pid_cluster_or_time_never_queues(self):
        for key, value in (("concertPid", 99), ("systemIdentifier", "999999"),
                           ("postmasterStartedAt", "2026-11-04T19:00:00Z")):
            with self.subTest(key=key):
                self.observations.take_restart_baseline.return_value = {**self.baseline, key: value}
                with self.assertRaises(InputRefusal):
                    self.poll()
                self.controls.request_postgres_concert_restart.assert_not_called()

    def test_failed_owner_checkpoint_stops_without_retrying_restart(self):
        self.observations.take_restart_baseline.return_value = dict(self.baseline)
        self.poll()
        self.controls.checkpoint.return_value = {"state": "FAILED"}
        with self.assertRaises(InputRefusal):
            self.poll()
        self.controls.request_postgres_concert_restart.assert_called_once()


if __name__ == "__main__":
    unittest.main()
