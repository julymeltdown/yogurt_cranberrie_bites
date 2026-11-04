#!/usr/bin/env python3
"""Developer-owned spatial source/candidate regression tests; no production admission."""
import copy
import argparse
import hashlib
import importlib.util
import json
import re
from pathlib import Path
import unittest
import signal
import subprocess
import sys
import tempfile
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

    def test_candidate_preserves_source_and_binds_published_tilemap(self):
        proposal = copy.deepcopy(self.proposal)
        original = copy.deepcopy(proposal)
        candidate = builder.candidate_map(proposal)
        candidate['places'][0]['label'] = 'Changed destination'
        self.assertEqual(proposal, original)
        self.assertEqual(candidate['tilemap'], {'format': 'TILED_JSON', 'resource_state': 'PROVIDED', 'path': builder.TILED_PATH, 'sha256': builder.TILED_HASH})

    def test_actual_runtime_bytes_dimensions_and_budgets_are_truthful(self):
        binding = self.outputs['source-binding.json']
        expected = {'REVENGE': 1820481, 'PARADE': 1839826, 'DANGER': 1819166}
        atlas_hashes = set()
        for scene in binding['scenes']:
            resources = scene['runtime_resources']
            for pin in resources:
                raw = builder.published_input(pin['path'], builder.ART_COMMIT)
                self.assertEqual((builder.sha(raw), len(raw)), (pin['sha256'], pin['bytes']))
                self.assertEqual(builder.png_size(raw), (pin['width_px'], pin['height_px']))
            budget = self.outputs['readiness.json']['scenery_budget'][scene['theme_id']]
            self.assertEqual((budget['bytes'], budget['pixels']), (expected[scene['theme_id']], 2043904))
            self.assertLessEqual(budget['bytes'], budget['max_bytes'])
            self.assertLessEqual(budget['pixels'], budget['max_pixels'])
            atlas_hashes.add(resources[1]['sha256'])
        self.assertEqual(len(atlas_hashes), 3)
        with self.assertRaises(ValueError):
            builder.png_size(b'not a PNG')

    def test_source_checkout_drift_is_rejected_without_editing_original(self):
        with patch.object(builder.subprocess, 'check_output', return_value=b'changed published source'):
            with self.assertRaisesRegex(ValueError, 'source changed'):
                builder.published_input(builder.BASE + 'geometry-proposal.json')

    def test_all_three_source_skins_do_not_waive_review_and_operator_gates(self):
        report = self.outputs['readiness.json']
        self.assertEqual(report['state'], 'NOT_ADMITTED')
        self.assertEqual(report['skins'], {theme: 'CONTRACT_READY' for theme in ['PARADE', 'REVENGE', 'DANGER']})
        for reason in ['PRODUCT_BOOTSTRAP_NOT_CONFIGURED', 'ART_FIDELITY_NOT_ACCEPTED']:
            self.assertIn(reason, report['blockers'])
        self.assertNotIn('UNREVIEWED_SOURCE_PROVENANCE', report['blockers'])
        self.assertEqual(self.outputs['map-manifest.json']['provenance']['review_state'], 'REVIEWED')
        self.assertTrue(all(p['provenance']['review_state'] == 'UNREVIEWED' for p in self.outputs['map-manifest.json']['places']))
        review = self.outputs['engineering-source-review.json']
        self.assertEqual(review['map']['map_revision'], 3)
        self.assertEqual(review['virtual_places']['physical_selection'], 'INELIGIBLE')
        binding_path = builder.ROOT / 'backend/releases/reference-scene-v2/source-binding.json'
        self.assertEqual(review['source_binding_sha256'], builder.sha(binding_path.read_bytes()))
        self.assertEqual(report['fractional_depth_count'], 0)
        self.assertFalse(any(b.startswith('C06_') for b in report['blockers']))
        self.assertEqual(report['art_commit'], builder.ART_COMMIT)
        self.assertIsNone(report['bootstrap'])
        self.assertIsNone(report['theme_manifests'])
        for name, expected in self.outputs.items():
            actual = json.loads((builder.ROOT / 'backend/releases/reference-scene-v2' / name).read_text())
            self.assertEqual(actual, expected)



class QualityV6Tests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.outputs = builder.build(builder.QUALITY_V6)

    def test_new_generation_preserves_places_and_exact_canonical_grids(self):
        old = builder.build()['map-manifest.json']
        new = self.outputs['map-manifest.json']
        self.assertEqual(new['map_revision'], 4)
        self.assertEqual(new['places'], old['places'])
        self.assertNotEqual(new['geometry_hash'], old['geometry_hash'])
        self.assertEqual(new['geometry_hash'], builder.QUALITY_V6.geometry_hash)
        self.assertNotEqual(new['tilemap']['sha256'], old['tilemap']['sha256'])
        for name, document in self.outputs.items():
            raw = (builder.ROOT / 'backend/releases/reference-scene-v2/quality-v6' / name).read_bytes()
            self.assertEqual(json.loads(raw), document)

    def test_new_three_era_runtime_budgets_and_common_source_pins(self):
        expected = {'REVENGE': 1707562, 'PARADE': 1727221, 'DANGER': 1712974}
        for scene in self.outputs['source-binding.json']['scenes']:
            budget = self.outputs['readiness.json']['scenery_budget'][scene['theme_id']]
            self.assertEqual((budget['bytes'], budget['pixels']), (expected[scene['theme_id']], 2043904))
            self.assertEqual(len(scene['runtime_resources']), 2)
            common = [pin for pin in scene['scene_sources'] if '/sources/' in pin['path']]
            self.assertEqual(len(common), 5)
            for pin in scene['runtime_resources']:
                raw = builder.published_input(pin['path'], builder.QUALITY_V6.art_commit)
                self.assertEqual(builder.sha(raw), pin['sha256'])
                self.assertEqual(builder.png_size(raw), (pin['width_px'], pin['height_px']))

    def test_scene_source_review_qualifies_only_exact_supplied_inputs(self):
        packet = self.outputs['scene-source-qualification.json']
        binding = self.outputs['source-binding.json']
        self.assertEqual(packet['admission'], 'NOT_ADMITTED')
        self.assertEqual(packet['source_binding_sha256'], self.outputs['engineering-source-review.json']['source_binding_sha256'])
        self.assertEqual(len(packet['scenes']), 3)
        for qualified, source in zip(packet['scenes'], binding['scenes']):
            self.assertEqual(qualified['theme_id'], source['theme_id'])
            self.assertEqual(qualified['scenery_source'], source['scenery_source'])
            self.assertEqual(qualified['runtime_resources'], source['runtime_resources'])
            self.assertIn(qualified['artist_provenance_source'], source['scene_sources'])
            self.assertTrue(qualified['artist_provenance_source']['path'].endswith('/provenance.json'))
            provenance = qualified['source_provenance']
            self.assertEqual(provenance['review_state'], 'REVIEWED')
            self.assertIn(source['scenery_source']['sha256'], provenance['source_revision'])
            self.assertIn(packet['source_binding_sha256'], provenance['source_revision'])
            self.assertNotIn('avatar_manifest', qualified)
        self.assertIn('No supplied ThemeManifest or Bootstrap', packet['limits'])

    def test_native_receipt_rejects_missing_or_wrong_candidate_observations(self):
        expected = {'map_revision': 4, 'geometry_hash': builder.QUALITY_V6.geometry_hash}
        valid = 'test native ... MCR_SPATIAL_CANDIDATE ' + json.dumps(expected) + '\n'
        self.assertEqual(verify_native_candidate(valid, expected), expected)
        for invalid in ['', valid + valid, 'MCR_SPATIAL_CANDIDATE {"map_revision":3}\n']:
            with self.assertRaisesRegex(ValueError, 'exact requested candidate'):
                verify_native_candidate(invalid, expected)

    def test_old_tiled_cannot_be_resealed_as_new_generation(self):
        original = builder.published_input
        def swapped(path, revision=builder.SOURCE_COMMIT):
            if path == builder.QUALITY_V6.tiled_path:
                return original(builder.TILED_PATH, builder.TILED_COMMIT)
            return original(path, revision)
        # Even after changing the declared byte pin, geometry comparison rejects
        # a well-formed, published old Tiled file of the same length/dimensions.
        mixed = builder.QUALITY_V6._replace(tiled_hash=builder.TILED_HASH)
        with patch.object(builder, 'published_input', side_effect=swapped):
            with self.assertRaisesRegex(ValueError, 'Tiled differs'):
                builder.build(mixed)


class QualityV8Tests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.outputs = builder.build(builder.QUALITY_V8)

    def test_actual_new_revision_preserves_old_runtime_and_adds_exact_collision_cells(self):
        old = builder.build(builder.QUALITY_V6)['map-manifest.json']
        new = self.outputs['map-manifest.json']
        self.assertEqual(new['map_revision'], 5)
        self.assertEqual(new['map_id'], old['map_id'])
        self.assertEqual(new['places'], old['places'])
        self.assertEqual(new['geometry_hash'], builder.QUALITY_V8.geometry_hash)
        self.assertEqual(new['tilemap']['sha256'], builder.QUALITY_V8.tiled_hash)
        changes = [(a, b) for before, after in zip(old['geometry']['collision_grid'], new['geometry']['collision_grid'])
                   for a, b in zip(before, after) if a != b]
        self.assertEqual(changes, [(0, 1)] * 72)
        for name, value in self.outputs.items():
            self.assertEqual(json.loads((builder.ROOT / 'backend/releases/reference-scene-v2/quality-v8' / name).read_bytes()), value)

    def test_all_three_actual_source_descriptors_and_review_limits(self):
        budgets = {'REVENGE': 1735321, 'PARADE': 1746508, 'DANGER': 1732813}
        binding = self.outputs['source-binding.json']
        for scene in binding['scenes']:
            source = json.loads(builder.published_input(scene['scenery_source']['path'], builder.QUALITY_V8.art_commit))
            self.assertEqual(len(source['objects']), 34)
            budget = self.outputs['readiness.json']['scenery_budget'][scene['theme_id']]
            self.assertEqual((budget['bytes'], budget['pixels']), (budgets[scene['theme_id']], 2043904))
            self.assertTrue(any(p['path'].endswith('/spatial-v4/revision-evidence.json') for p in scene['scene_sources']))
        packet = self.outputs['scene-source-qualification.json']
        self.assertEqual(packet['admission'], 'NOT_ADMITTED')
        self.assertEqual(len(packet['scenes']), 3)
        self.assertTrue(all(p['source_provenance']['source_id'].startswith('mcr_quality_v8_') for p in packet['scenes']))
        self.assertIsNone(self.outputs['readiness.json']['bootstrap'])
        self.assertTrue(all(p['provenance']['review_state'] == 'UNREVIEWED' for p in self.outputs['map-manifest.json']['places']))

    def test_v8_source_evidence_cannot_hide_collision_removal_or_prop_move(self):
        proposal = json.loads(builder.published_input(builder.QUALITY_V8.geometry_path, builder.QUALITY_V8.source_commit))
        evidence = json.loads(builder.published_input(builder.BASE + 'spatial-v4/revision-evidence.json', builder.QUALITY_V8.art_commit))
        builder.verify_v8_revision(proposal, evidence)
        changed = copy.deepcopy(evidence); changed['changed_cells'].pop()
        with self.assertRaisesRegex(ValueError, 'collision evidence'):
            builder.verify_v8_revision(proposal, changed)
        changed = copy.deepcopy(proposal)
        changed['placements'][0]['y'] += 1
        with self.assertRaisesRegex(ValueError, 'frozen props'):
            builder.verify_v8_revision(changed, evidence)
        changed = copy.deepcopy(proposal)
        changed['geometry']['overdraw_layer'][0][0] = 1 - changed['geometry']['overdraw_layer'][0][0]
        with self.assertRaisesRegex(ValueError, 'frozen terrain'):
            builder.verify_v8_revision(changed, evidence)

    def test_map4_tiled_cannot_be_resealed_under_map5(self):
        original = builder.published_input
        def swapped(path, revision=builder.SOURCE_COMMIT):
            if path == builder.QUALITY_V8.tiled_path:
                return original(builder.QUALITY_V6.tiled_path, builder.QUALITY_V6.tiled_commit)
            return original(path, revision)
        with patch.object(builder, 'published_input', side_effect=swapped):
            with self.assertRaisesRegex(ValueError, 'Tiled differs'):
                builder.build(builder.QUALITY_V8._replace(tiled_hash=builder.QUALITY_V6.tiled_hash))

    def test_png_crc_and_complete_pixel_stream_are_checked(self):
        def chunk(kind, data):
            return builder.struct.pack('>I', len(data)) + kind + data + builder.struct.pack('>I', builder.zlib.crc32(kind + data))
        def image(pixels):
            return (b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', builder.struct.pack('>IIBBBBB', 1, 1, 8, 6, 0, 0, 0))
                    + chunk(b'IDAT', builder.zlib.compress(pixels)) + chunk(b'IEND', b''))
        builder.decode_png(image(b'\0\xff\0\0\xff'))
        damaged = bytearray(image(b'\0\xff\0\0\xff')); damaged[-1] ^= 1
        with self.assertRaisesRegex(ValueError, 'CRC'):
            builder.decode_png(damaged)
        with self.assertRaisesRegex(ValueError, 'pixel stream'):
            builder.decode_png(image(b'\0\xff'))
        with self.assertRaisesRegex(ValueError, 'row filter'):
            builder.decode_png(image(b'\x05\xff\0\0\xff'))


class QualityV10Tests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.outputs = builder.build(builder.QUALITY_V10)

    def test_new_reserved_revision_changes_only_exact_terrain_cells(self):
        old = builder.build(builder.QUALITY_V8)['map-manifest.json']
        new = self.outputs['map-manifest.json']
        self.assertEqual(new['map_revision'], 6)
        self.assertEqual(new['places'], old['places'])
        self.assertEqual({k: v for k, v in new['geometry'].items() if k != 'terrain_layer'},
                         {k: v for k, v in old['geometry'].items() if k != 'terrain_layer'})
        changes = [(a, b) for before, after in zip(old['geometry']['terrain_layer'], new['geometry']['terrain_layer'])
                   for a, b in zip(before, after) if a != b]
        self.assertEqual(len(changes), 560)
        self.assertEqual(set(changes), {(1, 2), (2, 1)})
        for name, value in self.outputs.items():
            self.assertEqual(json.loads((builder.ROOT / 'backend/releases/reference-scene-v2/quality-v10' / name).read_bytes()), value)

    def test_exact_frontend_descriptors_sources_and_runtime_budgets(self):
        expected = {'REVENGE': 1752996, 'PARADE': 1764183, 'DANGER': 1750488}
        compatibility = self.outputs['frontend-source-compatibility.json']
        self.assertEqual(compatibility['source_input_pins_verified'], 44)
        self.assertIsNone(compatibility['frontend_candidate_revision_at_pin'])
        self.assertEqual(compatibility['frontend_metadata_candidate_revision'], 6)
        self.assertEqual(compatibility['frontend_metadata_commit'], builder.V10_METADATA_COMMIT)
        for scene in self.outputs['source-binding.json']['scenes']:
            paths = [self.outputs['source-binding.json'][key]['path']
                     for key in ('geometry_source', 'tilemap_source')]
            paths += [scene['scenery_source']['path']]
            paths += [pin['path'] for group in ('scene_sources', 'runtime_resources') for pin in scene[group]]
            self.assertEqual(len(paths), len(set(paths)), 'retained loader rejects source/runtime path aliases')
            self.assertEqual(len(scene['scene_sources']), 23)
        for scene in compatibility['scenes']:
            self.assertEqual((scene['world_objects'], scene['map_objects']), (34, 39))
            self.assertEqual(len(scene['runtime_resources']), 2)
            budget = self.outputs['readiness.json']['scenery_budget'][scene['theme_id']]
            self.assertEqual((budget['bytes'], budget['pixels']), (expected[scene['theme_id']], 2043904))
        packet = self.outputs['scene-source-qualification.json']
        self.assertEqual(packet['admission'], 'NOT_ADMITTED')
        self.assertTrue(all(s['source_provenance']['source_id'].startswith('mcr_quality_v10_') for s in packet['scenes']))
        self.assertIsNone(self.outputs['readiness.json']['bootstrap'])
        self.assertIsNone(self.outputs['readiness.json']['theme_manifests'])
        self.assertTrue(all(p['provenance']['review_state'] == 'UNREVIEWED' for p in self.outputs['map-manifest.json']['places']))

    def test_terrain_evidence_cannot_hide_collision_or_world_prop_changes(self):
        proposal = json.loads(builder.published_input(builder.QUALITY_V10.geometry_path, builder.QUALITY_V10.source_commit))
        evidence = json.loads(builder.published_input(builder.BASE + 'spatial-v5/revision-evidence.json', builder.QUALITY_V10.art_commit))
        builder.verify_v10_revision(proposal, evidence)
        changed = copy.deepcopy(evidence); changed['terrain_changed_cells'].pop()
        with self.assertRaisesRegex(ValueError, 'terrain evidence'):
            builder.verify_v10_revision(proposal, changed)
        changed = copy.deepcopy(proposal)
        changed['geometry']['collision_grid'][0][0] = 1 - changed['geometry']['collision_grid'][0][0]
        with self.assertRaisesRegex(ValueError, 'frozen collision'):
            builder.verify_v10_revision(changed, evidence)
        changed = copy.deepcopy(proposal); changed['placements'][0]['x'] += 1
        with self.assertRaisesRegex(ValueError, 'frozen WORLD props'):
            builder.verify_v10_revision(changed, evidence)

    def test_valid_map5_tiled_cannot_be_resealed_under_map6(self):
        original = builder.published_input
        def swapped(path, revision=builder.SOURCE_COMMIT):
            if path == builder.QUALITY_V10.tiled_path:
                return original(builder.QUALITY_V8.tiled_path, builder.QUALITY_V8.tiled_commit)
            return original(path, revision)
        with patch.object(builder, 'published_input', side_effect=swapped), patch.object(builder, 'verify_v10_reservation', return_value={}):
            with self.assertRaisesRegex(ValueError, 'Tiled differs'):
                builder.build(builder.QUALITY_V10._replace(tiled_hash=builder.QUALITY_V8.tiled_hash))

    def test_revision_cannot_be_an_alias_of_existing_map5_or_unallocated_frontend(self):
        with self.assertRaisesRegex(ValueError, 'actual committed revision6'):
            builder.verify_v10_reservation(builder.QUALITY_V10._replace(map_revision=5))
        reservation = builder.verify_v10_reservation(builder.QUALITY_V10)
        self.assertEqual(reservation['map_revision'], 6)
        self.assertIsNone(reservation['frontend_candidate_revision_at_pin'])

    def test_historical_pending_binding_cannot_replace_allocated_current_metadata(self):
        original = builder.published_input
        def swapped(path, revision=builder.SOURCE_COMMIT):
            if path == builder.BASE + 'runtime/spatial-v5/source-binding.json' and revision == builder.V10_METADATA_COMMIT:
                return builder.subprocess.check_output(
                    ['git', 'show', builder.QUALITY_V10.tiled_commit + ':frontend/public/' + path], cwd=builder.ROOT)
            return original(path, revision)
        with patch.object(builder, 'published_input', side_effect=swapped):
            with self.assertRaisesRegex(ValueError, 'allocation binding drift'):
                builder.verify_v10_frontend(builder.QUALITY_V10,
                    builder.verify_v10_reservation(builder.QUALITY_V10), self.outputs['source-binding.json']['scenes'])


def verify_native_candidate(stdout, expected):
    rows = re.findall(r'MCR_SPATIAL_CANDIDATE (\{[^\n]*\})', stdout)
    builder.require(len(rows) == 1 and json.loads(rows[0]) == expected,
                    'native binary did not observe the exact requested candidate')
    return json.loads(rows[0])


def frozen_bootstrap(metadata, pinned_binaries):
    import bootstrap_smoke as bootstrap
    import migration_smoke as migration
    selected = []
    finished = False
    for line in metadata.read_text().splitlines():
        row = json.loads(line)
        if row.get('reason') == 'build-finished':
            bootstrap.require(row.get('success') is True, 'Cargo build failed')
            finished = True
        if (row.get('reason') == 'compiler-artifact' and row.get('executable')
                and row['target']['name'] == 'bootstrap' and 'test' in row['target']['kind']
                and Path(row['manifest_path']) == builder.ROOT / 'backend/services/concert-service/Cargo.toml'):
            executable = Path(row['executable'])
            bootstrap.require(executable.is_absolute(), 'absolute compiler metadata required')
            selected.append(executable)
    bootstrap.require(finished and len(set(selected)) == 1, 'exact service bootstrap artifact required')
    receipt = json.loads(pinned_binaries.read_text())
    matches = [row for row in receipt['artifacts']
               if row['package'] == 'concert-service'
               and Path(row['manifest_path']) == builder.ROOT / 'backend/services/concert-service/Cargo.toml'
               and Path(row['compiler_executable']) == selected[0]]
    bootstrap.require(len(matches) == 1, 'exact frozen compiler artifact required')
    row = matches[0]
    binary = migration.executable(Path(row['snapshot']))
    bootstrap.require(hashlib.sha256(binary.read_bytes()).hexdigest() == row['sha256'], 'frozen checksum mismatch')
    return binary, receipt, row


class FrozenBootstrapTests(unittest.TestCase):
    def test_released_compiler_paths_are_metadata_only_and_snapshot_is_pinned(self):
        import migration_smoke as migration
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            metadata, receipt_path = root / 'cargo.jsonl', root / 'receipt.json'
            shared = root / 'released-target' / 'bootstrap-exact-compiler-id'
            snapshot = root / 'bootstrap-frozen'
            snapshot.write_bytes(b'hermetic metadata fixture; never execute')
            snapshot.chmod(0o700)
            artifact = {'reason': 'compiler-artifact', 'target': {'name': 'bootstrap', 'kind': ['test']},
                        'manifest_path': str(builder.ROOT / 'backend/services/concert-service/Cargo.toml'),
                        'executable': str(shared)}
            finished = {'reason': 'build-finished', 'success': True}
            metadata.write_text('\n'.join(map(json.dumps, [artifact, finished])))
            row = {'package': 'concert-service', 'manifest_path': artifact['manifest_path'],
                   'compiler_executable': str(shared), 'snapshot': str(snapshot),
                   'sha256': hashlib.sha256(snapshot.read_bytes()).hexdigest()}
            receipt_path.write_text(json.dumps({'artifacts': [row]}))
            original_stat = Path.stat
            original_resolve = Path.resolve
            def guarded_stat(path, *args, **kwargs):
                if path == shared or shared.parent in path.parents:
                    raise AssertionError('released target stat')
                return original_stat(path, *args, **kwargs)
            def guarded_resolve(path, *args, **kwargs):
                if path == shared or shared.parent in path.parents:
                    raise AssertionError('released target resolve')
                return original_resolve(path, *args, **kwargs)
            with patch.object(Path, 'stat', guarded_stat), patch.object(Path, 'resolve', guarded_resolve):
                binary, _, _ = frozen_bootstrap(metadata, receipt_path)
            self.assertEqual(binary, snapshot.resolve())
            snapshot.write_bytes(b'drift')
            with self.assertRaises(migration.SmokeError) as error:
                frozen_bootstrap(metadata, receipt_path)
            self.assertEqual(error.exception.detail['diagnostic'], 'frozen checksum mismatch')
            metadata.write_text(json.dumps(artifact))
            with self.assertRaises(migration.SmokeError) as error:
                frozen_bootstrap(metadata, receipt_path)
            self.assertEqual(error.exception.detail['diagnostic'], 'exact service bootstrap artifact required')
            metadata.write_text('\n'.join(map(json.dumps, [artifact, {**finished, 'success': False}])))
            with self.assertRaises(migration.SmokeError) as error:
                frozen_bootstrap(metadata, receipt_path)
            self.assertEqual(error.exception.detail['diagnostic'], 'Cargo build failed')


def run_native(metadata, pg_bin, pinned_binaries, inputs=builder.BASELINE):
    # Reuse the existing marker/inode-owned cluster and exact native-test runner.
    # Original compiler paths are metadata only after the shared target release.
    import bootstrap_smoke as bootstrap
    import migration_smoke as migration
    binary, receipt, row = frozen_bootstrap(metadata, pinned_binaries)
    tools = {name: migration.executable(pg_bin / name) for name in migration.PG_TOOLS}
    outputs = builder.build(inputs)
    expected = {**outputs['readiness.json']['map'],
                'source_commit': inputs.source_commit, 'art_commit': inputs.art_commit}
    cluster = migration.Cluster(tools)
    cluster.env['MCR_SPATIAL_TEST_CANDIDATE'] = next(name for name, value in builder.CANDIDATES.items() if value == inputs)
    report = {'state': 'FAIL', 'commands': {}, 'source_commit': receipt['source_commit'],
              'binary_sha256': row['sha256'], 'geometry_hash': inputs.geometry_hash, 'map_revision': inputs.map_revision,
              'art_commit': inputs.art_commit, 'admission': 'NOT_ADMITTED'}
    def interrupted(number, _frame):
        raise migration.SmokeError('signal', f'interrupted by signal {number}')
    def run_selected(name):
        env = {**cluster.env, 'MCR_BOOTSTRAP_ROOT': str(cluster.root),
               'MCR_BOOTSTRAP_TOKEN': cluster.token, 'MCR_BOOTSTRAP_INODE': str(cluster.identity[1]),
               'MCR_BOOTSTRAP_DATABASE_URL': cluster.url(bootstrap.DATABASE)}
        result = migration.execute([str(binary), '--ignored', '--exact', name, '--test-threads=1', '--nocapture'],
                                   env=env, phase=name, cwd=cluster.root, timeout=60)
        bootstrap.require(re.search(r'test result: ok\. 1 passed; 0 failed; 0 ignored;', result.stdout),
                          'native test missing/skipped')
        observed = verify_native_candidate(result.stdout, expected)
        report.setdefault('observed_candidates', {})[name] = observed
        report['commands'][name] = result.returncode
    previous = {number: signal.signal(number, interrupted) for number in (signal.SIGINT, signal.SIGTERM)}
    try:
        cluster.init(); cluster.start()
        report['postgres_version'] = cluster.sql('postgres', 'show server_version;')
        bootstrap.require(report['postgres_version'].startswith('16.14'), 'PostgreSQL16.14 required')
        cluster.sql('postgres', 'create database mcr_concert_bootstrap template template0;')
        report['migration_sha384'] = migration.migration_manifest(builder.ROOT / 'backend', 'concert-service')
        for path in sorted((builder.ROOT / 'backend/services/concert-service/migrations').glob('*.sql')):
            if not path.name.endswith('.down.sql'):
                cluster.sql(bootstrap.DATABASE, path.read_text())
                report['commands'][path.name] = 0
        run_selected('native_spatial_candidate_transactions')
        before = cluster.assert_postmaster(); cluster.stop(); cluster.start()
        after = cluster.assert_postmaster()
        bootstrap.require(before != after, 'postmaster did not restart')
        report['restart'] = {'before_pid': before, 'after_pid': after}
        run_selected('native_spatial_candidate_restart')
        report['native_tests'] = 2
        report['state'] = 'PASS'
    except (migration.SmokeError, OSError, ValueError, subprocess.SubprocessError) as error:
        report['error'] = error.detail if isinstance(error, migration.SmokeError) else type(error).__name__
    finally:
        try:
            cluster.cleanup()
            bootstrap.require(not cluster.root.exists(), 'owned cluster remains')
            report['cleanup'] = 'PASS'
        except (migration.SmokeError, OSError) as error:
            report['state'] = 'FAIL'; report['cleanup'] = 'FAIL'
            report['cleanup_error'] = error.detail if isinstance(error, migration.SmokeError) else type(error).__name__
        for number, handler in previous.items():
            signal.signal(number, handler)
    return report


if __name__ == '__main__':
    if '--native' in sys.argv:
        parser = argparse.ArgumentParser(description=__doc__)
        parser.add_argument('--native', action='store_true')
        parser.add_argument('--candidate', choices=list(builder.CANDIDATES), default='baseline')
        parser.add_argument('--build-metadata', type=Path, required=True)
        parser.add_argument('--pg-bin', type=Path, required=True)
        parser.add_argument('--pinned-binaries', type=Path, required=True)
        args = parser.parse_args()
        report = run_native(args.build_metadata, args.pg_bin, args.pinned_binaries,
                            builder.CANDIDATES[args.candidate])
        print(json.dumps(report, sort_keys=True))
        raise SystemExit(0 if report['state'] == 'PASS' else 1)
    unittest.main()
