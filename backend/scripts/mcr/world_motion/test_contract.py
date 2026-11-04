"""Motion negotiation/anchor negatives against the real C05 oracle."""
import copy
import importlib.util
import json
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[4]
spec = importlib.util.spec_from_file_location('motion_contract', ROOT / 'scripts/mcr/validate-realtime.py')
rt = importlib.util.module_from_spec(spec)
spec.loader.exec_module(rt)

class ContractTests(unittest.TestCase):
    def setUp(self):
        self.contract = rt.Contract()
        self.fixture = json.loads((ROOT / 'contracts/fixtures/realtime/motion-v1/negotiated.json').read_text())

    def trace(self, fixture=None):
        f = fixture or self.fixture
        t = rt.Trace(self.contract, f['actor_id'])
        for row in f['messages']:
            t.consume(row['direction'], row['message'])
        return t

    def test_real_fixture_catalog_including_legacy_negatives(self):
        self.assertGreaterEqual(rt.validate_fixtures(), 31)

    def test_unnegotiated_profile_is_rejected(self):
        del self.fixture['messages'][0]['message']['motionProfileVersion']
        with self.assertRaises(rt.WireError): self.trace()

    def test_requested_but_unavailable_has_authoritative_only_fallback(self):
        s = self.fixture['messages'][1]['message']
        del s['motionProfile']; del s['selfMotion']
        self.assertIsNone(self.trace().motion)

    def test_tick_position_duration_and_command_metadata_are_rejected(self):
        for key, value in [('serverTick', 999), ('position', {'x': 999, 'y': 999}), ('inputAppliedTicks', 1), ('heldInput', {'seq':1,'axisX':0,'axisY':0,'remainingHoldMs':0}), ('interact', True)]:
            f = copy.deepcopy(self.fixture)
            f['messages'][1]['message']['selfMotion'][key] = value
            with self.subTest(key=key), self.assertRaises(rt.WireError): self.trace(f)

    def test_negotiated_delta_requires_full_current_anchor(self):
        t = self.trace(); s = self.fixture['messages'][1]['message']
        d = {key: copy.deepcopy(s[key]) for key in ('protocolVersion','streamId','roomEpoch','serverTick','ackInputSeq')}
        d.update(type='delta',streamSeq=1,upserts=[],removes=[])
        with self.assertRaises(rt.WireError): t.consume('server',d)
        d['selfMotion'] = copy.deepcopy(s['selfMotion'])
        t.consume('server',d)
        self.assertEqual(t.motion['position'], s['selfMotion']['position'])

if __name__ == '__main__': unittest.main()
