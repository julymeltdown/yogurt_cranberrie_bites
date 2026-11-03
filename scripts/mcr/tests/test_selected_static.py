"""C06 1.1.0 selected source contract; no rendering/persistence acceptance."""
from copy import deepcopy
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import unittest

ROOT = Path(__file__).resolve().parents[3]

def load(name, path):
    spec = importlib.util.spec_from_file_location(name, ROOT / path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module

assets = load("selected_assets", "scripts/mcr/validate-assets.py")
core = load("selected_core", "scripts/mcr/validate-core.py")

class SelectedStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.checks = assets.validators()
        cls.directory = ROOT / "contracts/fixtures/assets/selected-static"
        cls.docs = assets.load_bundle(cls.directory)
        cls.manifest = cls.docs["avatar-manifest.json"]

    def test_six_canonical_configs_round_trip_and_match_exact_art_frames(self):
        self.assertEqual(len(self.docs), 7)
        for i, preset in enumerate(self.manifest["presets"], 1):
            with self.subTest(preset=i):
                expected = f"photo01_front_{i:02}"
                config = self.docs[expected + ".json"]
                self.assertEqual(json.loads(json.dumps(config)), config)
                self.assertEqual(preset["preset_id"], expected)
                self.assertEqual(preset["config"], config)
                self.assertEqual(preset["art_id"], f"photo01-front-{i:02}")
                self.assertEqual(preset["frame"], {"x": (i-1)*64, "y": 0, "width_px": 64, "height_px": 64})
                assets.validate_document(config, self.checks, expected)
        report = assets.validate_bundle(self.docs, self.checks)
        self.assertEqual(report["status"], "CONTRACT_READY")
        self.assertEqual(report["manifest_pin"]["content_hash"], assets.content_hash(self.manifest))
        self.assertEqual(report["resource_bytes_and_hashes"], "NOT_RUN")

    def test_config_rejects_mixed_unknown_url_frame_and_unselected_identity(self):
        baseline = self.docs["photo01_front_01.json"]
        for patch in [{"item_ids": {}}, {"skin_tone_id": "tone_light"}, {"url": "https://private.invalid/x.png"},
                      {"frame": 0}, {"preset_id": "photo01-front-01"}, {"preset_id": "photo01_front_07"},
                      {"manifest_id": "other"}, {"manifest_version": 2}, {"contract_version": "1.0.0"},
                      {"representation": "layered"}, {"unknown": True}]:
            with self.subTest(patch=patch), self.assertRaises(assets.AssetError):
                assets.validate_document(baseline | patch, self.checks, "invalid")
        for key in baseline:
            bad = dict(baseline); del bad[key]
            with self.subTest(missing=key), self.assertRaises(assets.AssetError):
                assets.validate_document(bad, self.checks, "missing")

    def test_manifest_rejects_faked_capabilities_frames_source_and_default(self):
        for mutation in ["walk", "layers", "skin", "frame", "art", "duplicate", "missing", "source", "default", "texture", "config"]:
            bad = deepcopy(self.manifest)
            if mutation == "walk": bad["capabilities"]["poses"].append("walk")
            elif mutation == "layers": bad["layer_order"] = []
            elif mutation == "skin": bad["capabilities"]["skin_tone_customization"] = True
            elif mutation == "frame": bad["presets"][0]["frame"]["x"] = 64
            elif mutation == "art": bad["presets"][0]["art_id"] = "photo01-front-02"
            elif mutation == "duplicate": bad["presets"][1] = deepcopy(bad["presets"][0])
            elif mutation == "missing": bad["presets"].pop()
            elif mutation == "source": bad["source"]["source_pack_revision"] = 2
            elif mutation == "default": bad["default_preset_id"] = "photo01_front_07"
            elif mutation == "texture": bad["atlas"]["texture"]["sha256"] = "sha256:" + "0"*64
            elif mutation == "config": bad["presets"][0]["config"]["preset_id"] = "photo01_front_02"
            bad["content_hash"] = assets.content_hash(bad)
            with self.subTest(mutation=mutation), self.assertRaises(assets.AssetError):
                assets.validate_document(bad, self.checks, "invalid manifest")

    def test_manifest_pin_detects_stale_hash_without_default_substitution(self):
        docs = deepcopy(self.docs)
        docs["avatar-manifest.json"]["default_preset_id"] = "photo01_front_02"
        with self.assertRaisesRegex(assets.AssetError, "content_hash mismatch"):
            assets.validate_bundle(docs, self.checks)

    def test_retained_layered_branches_and_fixture_bytes_are_unchanged(self):
        for file, entry, retained in [("avatar-config.schema.json", "AvatarConfig", "LayeredAvatarConfig"),
                                      ("avatar-manifest.schema.json", "AvatarManifest", "LayeredAvatarManifest")]:
            path = "contracts/assets/v1/" + file
            previous = json.loads(subprocess.check_output(["git", "show", "d8f9f9ac:" + path], cwd=ROOT))
            current = assets.read_json(ROOT / path)
            self.assertEqual(previous["$defs"][entry], current["$defs"][retained])
        for file in ("avatar-manifest.json", "avatar-config.json"):
            path = "contracts/fixtures/assets/" + file
            self.assertEqual((ROOT/path).read_bytes(), subprocess.check_output(["git", "show", "d8f9f9ac:"+path], cwd=ROOT))
        self.assertEqual(assets.validate_bundle(assets.load_bundle(assets.FIXTURES))["contract_version"], "1.0.0")

    def test_c03_put_and_avatar_state_keep_existing_envelopes_with_both_variants(self):
        document = core.generator.build_document()
        state = assets.read_json(ROOT / "contracts/fixtures/core/bootstrap-open.json")["avatar"]
        core.validate(state, "AvatarState", document)
        for i in range(1, 7):
            config = self.docs[f"photo01_front_{i:02}.json"]
            core.validate({"expected_revision": 1, "config": config}, "SetAvatarRequest", document)
            core.validate({"revision": 2, "config": config}, "AvatarState", document)
        self.assertEqual(document["paths"]["/api/concert/events/{eventId}/me/avatar"]["put"]["operationId"], "setConcertAvatar")

    def test_generated_core_domain_mirrors_and_reference_entrypoints(self):
        document = core.generator.build_document()
        content = json.dumps(document, ensure_ascii=False, indent=2) + "\n"
        for file in ["backend/docs/api/mcr-core.openapi.json", "contracts/generated/mcr-core.openapi.json"]:
            self.assertEqual((ROOT/file).read_text(), content)
        domain = load("selected_domain_generator", "backend/scripts/docs/generate-mcr-domain.py")
        content = json.dumps(domain.build_document(), ensure_ascii=False, indent=2) + "\n"
        for file in ["backend/docs/api/mcr-domain.openapi.json", "contracts/generated/mcr-domain.openapi.json"]:
            self.assertEqual((ROOT/file).read_text(), content)
        self.assertEqual(len(document["components"]["schemas"]["AvatarConfig"]["oneOf"]), 2)

if __name__ == "__main__": unittest.main()
