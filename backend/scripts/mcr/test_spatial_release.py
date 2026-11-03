#!/usr/bin/env python3
"""Developer-owned spatial source/candidate regression tests; no production admission."""
import copy
import importlib.util
import json
from pathlib import Path
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('spatial_builder', Path(__file__).with_name('build_spatial_release.py'))
builder = importlib.util.module_from_spec(spec)
spec.loader.exec_module(builder)


class SpatialReleaseTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.outputs = builder.build()
        cls.proposal = builder.assets.read_json(builder.ROOT / 'frontend/public' / builder.BASE / 'geometry-proposal.json')

    def test_real_geometry_and_place_qualification_preserved(self):
        manifest = self.outputs['map-manifest.json']
        self.assertEqual(manifest['geometry'], self.proposal['geometry'])
        self.assertEqual(manifest['places'], self.proposal['places'])
        self.assertEqual((manifest['geometry']['width_cells'], manifest['geometry']['height_cells']), (48, 81))
        self.assertEqual(manifest['geometry']['collision_id'], 'reference_scene_v2_proposal_collision')
        self.assertTrue(all(p['verification_state'] == 'CONCEPT_ONLY' and p['provenance']['review_state'] == 'UNREVIEWED' for p in manifest['places']))
        builder.assets.validate_document(manifest, builder.assets.validators(), 'real spatial candidate')
        builder.assets.validate_map(manifest)

    def test_resealed_foreign_geometry_cannot_replace_frozen_artist_source(self):
        proposal = copy.deepcopy(self.proposal)
        proposal['geometry']['collision_grid'][37][29] = 1
        proposal['geometry_hash'] = builder.assets.digest(proposal['geometry'])
        with self.assertRaisesRegex(ValueError, 'geometry pin'):
            builder.candidate_map(proposal)
        proposal = copy.deepcopy(self.proposal)
        proposal['geometry_hash'] = 'sha256:' + '0' * 64
        with self.assertRaisesRegex(ValueError, 'geometry pin'):
            builder.candidate_map(proposal)

    def test_candidate_construction_never_mutates_source_or_guesses_tilemap(self):
        proposal = copy.deepcopy(self.proposal)
        original = copy.deepcopy(proposal)
        candidate = builder.candidate_map(proposal)
        candidate['places'][0]['label'] = 'Changed destination'
        self.assertEqual(proposal, original)
        self.assertEqual(candidate['tilemap'], {'format': 'TILED_JSON', 'resource_state': 'NOT_PROVIDED', 'path': None, 'sha256': None})

    def test_actual_runtime_bytes_dimensions_and_budgets_are_truthful(self):
        binding = self.outputs['source-binding.json']
        resources = binding['runtime_resources']
        for pin in resources:
            raw = builder.published_input(pin['path'])
            self.assertEqual((builder.sha(raw), len(raw)), (pin['sha256'], pin['bytes']))
            self.assertEqual(builder.png_size(raw), (pin['width_px'], pin['height_px']))
        budget = self.outputs['readiness.json']['scenery_budget']
        self.assertEqual((budget['bytes'], budget['pixels']), (3179667, 2567844))
        self.assertGreater(budget['bytes'], budget['max_bytes'])
        self.assertGreater(budget['pixels'], budget['max_pixels'])
        with self.assertRaises(ValueError):
            builder.png_size(b'not a PNG')

    def test_source_checkout_drift_is_rejected_without_editing_original(self):
        with patch.object(builder.subprocess, 'check_output', return_value=b'changed published source'):
            with self.assertRaisesRegex(ValueError, 'source changed'):
                builder.published_input(builder.BASE + 'geometry-proposal.json')

    def test_missing_skins_and_review_gates_cannot_be_reported_as_admitted(self):
        report = self.outputs['readiness.json']
        self.assertEqual(report['state'], 'NOT_ADMITTED')
        self.assertEqual(report['skins'], {'PARADE': 'SOURCE_PROPOSAL', 'REVENGE': 'UNAVAILABLE', 'DANGER': 'UNAVAILABLE'})
        for reason in ['MISSING_PAINTED_REVENGE', 'MISSING_PAINTED_DANGER', 'UNREVIEWED_SOURCE_PROVENANCE', 'TILED_RESOURCE_NOT_PROVIDED', 'C06_INTEGER_SCENERY_PROJECTION_REQUIRED']:
            self.assertIn(reason, report['blockers'])
        self.assertIsNone(report['bootstrap'])
        self.assertIsNone(report['theme_manifests'])
        for name, expected in self.outputs.items():
            actual = json.loads((builder.ROOT / 'backend/releases/reference-scene-v2' / name).read_text())
            self.assertEqual(actual, expected)


if __name__ == '__main__':
    unittest.main()
