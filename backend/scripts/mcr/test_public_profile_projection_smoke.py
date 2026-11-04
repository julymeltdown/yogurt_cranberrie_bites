"""Artifact selection regressions; no Cargo/PG is invoked."""
import copy
import unittest
from pathlib import Path
import public_profile_projection_smoke as smoke

class Artifacts(unittest.TestCase):
    def fixtures(self):
        return [{"reason":"compiler-artifact","executable":"/tmp/unused-"+name,
            "manifest_path":str(smoke.ROOT/"backend"/package/"Cargo.toml"),
            "target":{"name":name},"profile":{"test":True,"debuginfo":0}}
            for name,package in smoke.TARGETS.items()]
    def test_exact_artifacts_only(self):
        self.assertEqual(set(smoke.select_artifacts(self.fixtures())),set(smoke.TARGETS))
    def test_explicit_profile_only_mode_cannot_claim_a_projection_artifact(self):
        self.assertEqual(set(smoke.select_artifacts(self.fixtures()[:1],profile_only=True)),{"concert_public_profile"})
    def test_missing_duplicate_wrong_manifest_or_debug_are_rejected(self):
        original=self.fixtures()
        bad=[original[:1],original+original[:1]]
        for field,value in [("manifest_path","/foreign/Cargo.toml"),("executable",None)]:
            changed=copy.deepcopy(original);changed[0][field]=value;bad.append(changed)
        for field,value in [("test",False),("debuginfo",2)]:
            changed=copy.deepcopy(original);changed[0]["profile"][field]=value;bad.append(changed)
        for items in bad:
            with self.subTest(items=items),self.assertRaises(ValueError):smoke.select_artifacts(items)
    def test_similarly_named_foreign_artifact_cannot_replace_exact(self):
        items=self.fixtures();items[0]["target"]["name"]+="_old"
        with self.assertRaises(ValueError):smoke.select_artifacts(items)
    def test_joined_requires_exact_lib_artifact_and_cannot_be_profile_only(self):
        items=self.fixtures()
        unit={"reason":"compiler-artifact","executable":"/tmp/unused-concert-unit",
              "manifest_path":str(smoke.ROOT/"backend/services/concert-service/Cargo.toml"),
              "target":{"name":"concert_service","kind":["lib"]},"profile":{"test":True,"debuginfo":0}}
        self.assertEqual(set(smoke.select_artifacts(items+[unit],joined=True)),set(smoke.TARGETS)|{"concert_service"})
        with self.assertRaises(ValueError):smoke.select_artifacts(items,joined=True)
        with self.assertRaises(ValueError):smoke.select_artifacts(items+[unit],joined=True,profile_only=True)
        wrong=copy.deepcopy(unit);wrong["target"]["kind"]=["test"]
        with self.assertRaises(ValueError):smoke.select_artifacts(items+[wrong],joined=True)
    def test_source_pin_excludes_build_targets_and_private_runtime_files(self):
        pins=smoke.source_pins()
        self.assertTrue(any(p.endswith("concert_public_profile.proto") for p in pins))
        self.assertTrue(any("public_profile/mod.rs" in p for p in pins))
        self.assertFalse(any("/target/" in p or "private-restart" in p for p in pins))
if __name__=="__main__":unittest.main()
