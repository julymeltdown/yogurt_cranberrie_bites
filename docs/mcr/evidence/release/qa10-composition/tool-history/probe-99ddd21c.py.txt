#!/usr/bin/env python3
"""Inventory actual published QA10 inputs; never upgrades historical evidence.

Only explicit canonical task evidence and the existing release receipt directory
are read. Source content, private paths, application commands and screenshots
are not executed or reviewed. Git ownership/touch links are provenance, not
proof that a task's behavior passed. Missing current producer bindings stay gaps.
"""
import argparse
import gzip
import hashlib
import json
from pathlib import Path
import sys
import tomllib

ROOT = Path(__file__).resolve().parents[5]
sys.path.insert(0, str(ROOT / 'scripts/mcr/release_checks'))
import check as source
from composition.model import AUTHORITY, GATES, SCHEMA, evaluate, owned
from composition.reports import unittest_counts

REGISTER = 'docs/mcr/task-progress-2026-11-04.json'
HISTORICAL_FE = 'docs/mcr/evidence/release/qa10/candidate-980fc4d8/receipt.json'
VISUAL_REUSE = 'docs/mcr/evidence/visual/qa06/current-reflow/coverage-reuse.json'


class Inventory:
    def __init__(self, root, candidate):
        self.root, self.candidate = root, candidate
        self.repo = source.Repository(root)
        self.read_bytes = 0
        self.cache = {}
        self.tree = {}
        for row in self.repo.git('ls-tree', '-r', '-z', candidate).split(b'\0'):
            if row:
                meta, path = row.split(b'\t', 1)
                mode, kind, blob = meta.decode().split()
                self.tree[path.decode()] = {'mode': mode, 'kind': kind, 'git_blob': blob}

    def read(self, path, limit=2 * 1024 ** 2):
        source.relative_path(path)
        source.require(not source.sensitive_path(path), 'inventory.secret-path-refused')
        file = self.repo.path(path)
        source.require(path in self.tree and self.tree[path]['mode'] in ('100644', '100755'),
                       'inventory.not-published-regular-file')
        if file.exists():
            source.require(file.stat().st_size <= limit, 'inventory.file-limit')
            data = file.read_bytes()
        else:
            # This retained backend checkout is sparse. Read authoritative Git
            # bytes without materializing another lane or changing sparse rules.
            size = int(self.repo.git('cat-file', '-s', self.tree[path]['git_blob']).decode())
            source.require(size <= limit, 'inventory.file-limit')
            data = self.repo.git('show', self.candidate + ':' + path)
        self.read_bytes += len(data)
        source.require(self.read_bytes <= 128 * 1024 ** 2, 'inventory.total-read-limit')
        # Check actual working bytes against this candidate's immutable Git blob.
        blob = hashlib.sha1(b'blob ' + str(len(data)).encode() + b'\0' + data).hexdigest()
        source.require(blob == self.tree[path]['git_blob'], 'inventory.changed-input')
        return data

    def document(self, path):
        return source.decode(self.read(path))

    def reference(self, path):
        if path in self.cache:
            return self.cache[path]
        result = {'path': path}
        try:
            data = self.read(path)
            result.update(self.tree[path])
            result.update({'sha256': source.digest(data), 'bytes': len(data),
                'input_source': 'WORKTREE_MATCHED_GIT' if (self.root / path).exists() else 'PINNED_GIT_BLOB_NOT_MATERIALIZED',
                'last_evidence_change_commit': self.repo.git('log', '-1', '--format=%H', self.candidate,
                                                            '--', path).decode().strip(),
                'qualification': 'PUBLISHED_REFERENCE_ONLY'})
            if path.endswith('.json'):
                doc = source.decode(data)
                if isinstance(doc, dict):
                    candidate = doc.get('candidate')
                    candidate = candidate.get('commit') if isinstance(candidate, dict) else candidate
                    if isinstance(candidate, str) and source.HEX40.fullmatch(candidate):
                        result['recorded_candidate'] = candidate
                        result['qualification'] = 'CURRENT_CANDIDATE_RECORD_UNQUALIFIED' if candidate == self.candidate else 'HISTORICAL_CANDIDATE_ONLY'
        except (source.Rejected, OSError, ValueError, TypeError) as error:
            result['qualification'] = 'NOT_RUN'
            result['finding'] = str(error) if isinstance(error, source.Rejected) else 'inventory.unavailable'
        self.cache[path] = result
        return result

    def historical_frontend(self):
        receipt = self.document(HISTORICAL_FE)
        candidate = receipt['candidate']['commit']
        pins = {**receipt['source_pins'], **receipt['contract_pins']}
        same, changed, absent = [], [], []
        for path, pin in pins.items():
            if path not in self.tree:
                absent.append(path)
            elif self.tree[path]['git_blob'] == pin['git_blob']:
                same.append(path)
            else:
                changed.append(path)
        # Stream the large retained archive only to verify its declared digest.
        # Parsing its full JSON graph would exceed this small SOURCE RSS budget.
        folder = str(Path(HISTORICAL_FE).parent)
        digest = hashlib.sha256()
        expanded = 0
        packed = self.repo.path(folder + '/vitest.json.gz')
        with gzip.open(packed, 'rb') as stream:
            while data := stream.read(1024 ** 2):
                expanded += len(data)
                self.read_bytes += len(data)
                source.require(expanded <= source.VITEST_LIMIT and self.read_bytes <= 128 * 1024 ** 2,
                               'inventory.archive-read-limit')
                digest.update(data)
        source.require(digest.hexdigest() == receipt['artifact_pins']['vitest.json'], 'artifact.hash-mismatch')
        node = self.read(folder + '/node-tests.log')
        source.require(source.digest(node) == receipt['artifact_pins']['node-tests.log'], 'artifact.hash-mismatch')
        commands = receipt['commands']
        source.require(len(commands) == 10 and all(c['attempted'] is True and c['exit_code'] == 0
                       and c['status'] == 'PASS' for c in commands), 'inventory.historical-command-shape')
        return {'receipt': self.reference(HISTORICAL_FE), 'recorded_candidate': candidate,
            'qualification': 'HISTORICAL_HASH_CHECKED_NOT_CURRENT', 'current_candidate_qualified': False,
            'same_source_blobs': len(same), 'changed_source_blobs': len(changed), 'absent_source_paths': len(absent),
            'changed_paths': changed, 'absent_paths': absent,
            'vitest_archive_sha256_matches': True, 'vitest_expanded_bytes': expanded,
            'vitest_count_basis': 'OWNER_RECORDED_NOT_REPARSED',
            'recorded_command_counts': {c['id']: c['nonempty_count'] for c in commands},
            'node_counts_from_actual_retained_tap': source.tap_counts(node),
            'retained_production_assets': 'ABSENT_FROM_THIS_FRONTEND_SCHEMA',
            'limitations': ['No command rerun.', 'Archive hash equality is not a current-candidate test result.',
                            'Historical audit/logs cannot rescan missing production bytes.']}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--root', type=Path, required=True)
    parser.add_argument('--candidate', required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    root, out = args.root.resolve(), args.out.resolve()
    out.relative_to(root / 'docs/mcr/evidence/release/qa10-composition')
    inventory = Inventory(root, args.candidate)
    repo = inventory.repo
    source.require(repo.git('rev-parse', 'HEAD').decode().strip() == args.candidate, 'candidate.not-current-head')
    source.require(not repo.git('status', '--porcelain', '--untracked-files=no').strip(), 'candidate.worktree-dirty')
    candidate = {'commit': args.candidate, 'tree': repo.git('rev-parse', args.candidate + '^{tree}').decode().strip(), 'dirty': False}
    plan = inventory.document('generated/TASKS_EN.json')
    ia = inventory.document('generated/IA_EN.json')
    register = inventory.document(REGISTER)
    required = {row['id']: row for row in plan['tasks'] if row['priority'] == 'P0'}
    screens = {row['id']: row for row in ia['screens'] if row['priority'] == 'P0'}
    rows = {row['id']: row for row in register['tasks']}
    source.require(len(required) == 112 and len(screens) == 61 and set(rows) == set(required), 'inventory.original-scope')
    out.mkdir(exist_ok=False)

    def save(name, data):
        target = out / name
        target.write_text(json.dumps(data, indent=2, sort_keys=True) + '\n')
        return {'path': target.relative_to(root).as_posix(), 'sha256': source.digest(target.read_bytes())}

    traces, records = [], []
    for identity, task in required.items():
        row, paths = rows[identity], []
        for path in inventory.tree:
            if not source.sensitive_path(path) and owned(path, task['owned_paths']):
                paths.append(path)
        revised = row['revised_plan']
        references = list(dict.fromkeys([p for p in row.get('evidence', []) + revised.get('current_scope_evidence', [])
            if isinstance(p, str) and p.startswith('docs/mcr/') and '\n' not in p]))
        owned_patterns = []
        for pattern in task['owned_paths']:
            first, sep, tail = pattern.partition('/')
            owned_patterns.append(source.ALIASES.get(first, first) + (sep + tail if sep else ''))
        last = repo.git('log', '-1', '--format=%H', args.candidate, '--', *owned_patterns).decode().strip()
        traces.append({'id': identity, 'canonical_recorded_state': row.get('recorded_state'),
            'canonical_revised_state': revised['state'], 'canonical_owner_role': revised['owner_role'],
            'canonical_owner_chats': revised.get('known_current_owner_chats', []),
            'original_owned_paths': task['owned_paths'], 'latest_candidate_touch_commit': last or None,
            'source_files': [{'path': p, **inventory.tree[p]} for p in sorted(paths)],
            'evidence_references': [inventory.reference(p) for p in references],
            'candidate_qualification': 'DEFERRED_BY_USER' if identity == 'QA08' else 'NOT_RUN',
            'provenance_limit': 'Git ownership/touch and published references do not certify behavior or acceptance.'})
        state = 'DEFERRED_BY_USER' if identity == 'QA08' else 'BLOCKED' if revised['state'] in ('WAITING_INPUT', 'BLOCKED') else 'NOT_RUN'
        records.append({'id': identity, 'status': state, 'owner': revised['owner_role'],
            'reason': 'QA08.USER_DEFERRAL' if identity == 'QA08' else 'CURRENT_CANDIDATE_OWNER_RECEIPT_NOT_SUPPLIED',
            'commit': last or args.candidate, 'paths': paths, 'receipt_ids': []})
    history = inventory.historical_frontend()
    self_validation_path = 'docs/mcr/evidence/release/qa10-composition/self-validation.json'
    self_validation = inventory.document(self_validation_path)
    matched_tool_files = []
    for path, expected in self_validation['source_sha256'].items():
        source.require(source.digest(inventory.read(path)) == expected, 'inventory.tool-validation-source-changed')
        matched_tool_files.append(path)
    tool_log = inventory.read('docs/mcr/evidence/release/qa10-composition/round-02/focused-unittest.log')
    source.require(source.digest(tool_log) == self_validation['rounds'][1]['files']['focused-unittest.log']['retained_sha256'],
                   'inventory.tool-validation-log-changed')
    tool_validation = {'record': inventory.reference(self_validation_path),
        'executed_source_candidate': self_validation['source_candidate'], 'current_source_candidate': args.candidate,
        'all_recorded_tool_source_hashes_equal': True, 'matched_paths': matched_tool_files,
        'actual_verbose_test_counts': unittest_counts(tool_log),
        'qualification': 'UNCHANGED_TOOL_IMPLEMENTATION_EVIDENCE_REUSED',
        'limit': 'These genuine parser tests do not qualify application, live, device, operator or release outcomes.'}
    version_paths = ['.node-version', 'frontend/package.json', 'frontend/pnpm-lock.yaml',
                     'backend/rust-toolchain.toml', 'backend/Cargo.toml', 'backend/Cargo.lock',
                     'contracts/generated/mcr-core.openapi.json', 'contracts/generated/mcr-domain.openapi.json']
    versions = {'candidate': candidate, 'published_input_pins': [inventory.reference(p) for p in version_paths],
        'declared_node': inventory.read('.node-version').decode().strip(),
        'declared_rust_toolchain': tomllib.loads(inventory.read('backend/rust-toolchain.toml').decode()).get('toolchain', {}),
        'declared_frontend_package_manager': inventory.document('frontend/package.json').get('packageManager'),
        'checker_runtime': {'python': sys.version.split()[0], 'platform': sys.platform,
                            'git': repo.git('--version').decode().strip()},
        'actual_application_binary_versions': 'NOT_RUN',
        'current_candidate_genuine_test_receipts': 'NOT_RUN',
        'production_mock_scan': 'NOT_RUN', 'production_secret_scan': 'NOT_RUN',
        'production_reason': 'CURRENT_RETAINED_BUILD_BYTES_NOT_SUPPLIED',
        'qualification_limit': 'Manifest/lock/schema pins are published source identity, not executed application versions.'}
    release_records = []
    for path in sorted(inventory.tree):
        if path.startswith('docs/mcr/evidence/release/qa10/') and path.endswith('/receipt.json'):
            release_records.append(inventory.reference(path))
    visual = inventory.document(VISUAL_REUSE)
    reused = []
    for record in visual['reuse']:
        path = record.get('sourceDocument')
        if isinstance(path, str) and path.startswith('docs/mcr/'):
            ref = inventory.reference(path)
            expected = record.get('documentSha256')
            reused.append({'reference': ref, 'declared_sha256_matches': ref.get('sha256') == expected,
                           'qualification': 'EXISTING_OWNER_REUSE_RECORD_ONLY'})
        for ref in record.get('sourceDocuments', []):
            if isinstance(ref, dict) and isinstance(ref.get('path'), str):
                actual = inventory.reference(ref['path'])
                reused.append({'reference': actual, 'declared_sha256_matches': actual.get('sha256') == ref.get('sha256'),
                               'qualification': 'EXISTING_OWNER_REUSE_RECORD_ONLY'})
    receipts = []
    for category in ('backend', 'frontend', 'service', 'device', 'operator'):
        receipts.append(save(category + '-not-run.json', {'schema': 'mcr.qa10.composition-evidence.v1',
            'id': category + '-candidate', 'candidate': candidate, 'status': 'NOT_RUN',
            'owner': 'Responsible ' + category + ' implementation/evidence owner',
            'reason': 'No compatible exact-candidate producer envelope in the explicit published release input set.'}))
    authority = inventory.read(AUTHORITY)
    def pending(identity, owner='Responsible live/device integration implementation owner'):
        state = rows.get(identity, {}).get('revised_plan', {}).get('state', 'OPEN')
        return {'id': identity, 'status': 'DEFERRED_BY_USER' if identity == 'QA08' else 'BLOCKED' if state in ('WAITING_INPUT', 'BLOCKED') else 'NOT_RUN',
                'owner': owner, 'reason': 'QA08.USER_DEFERRAL' if identity == 'QA08' else 'CURRENT_CANDIDATE_OWNER_RECEIPT_NOT_SUPPLIED',
                'commit': args.candidate, 'paths': [], 'receipt_ids': []}
    document = {'schema': SCHEMA, 'candidate': candidate, 'source_manifest': None, 'receipts': receipts,
        'tasks': records, 'ia': [pending(identity, 'Responsible UI/live/device implementation owner')
                               for identity in screens], 'gates': [pending(identity) for identity in [*GATES, 'QA08']],
        'deferrals': [{'task_id': 'QA08', 'status': 'DEFERRED_BY_USER', 'blocking': False,
            'authority': {'path': AUTHORITY, 'git_blob': inventory.tree[AUTHORITY]['git_blob'], 'sha256': source.digest(authority)}}]}
    input_ref = save('input.json', document)
    # IA IDs are not task IDs; all are explicitly pending, never inferred accepted.
    report = evaluate(root, args.candidate, input_ref['path'])
    save('candidate-report.json', report)
    save('task-provenance.json', {'candidate': candidate, 'registry': inventory.reference(REGISTER), 'tasks': traces,
        'canonical_dispositions_preserved': True, 'acceptance_reassessment': 'NOT_PERFORMED'})
    save('existing-evidence.json', {'historical_frontend': history, 'release_receipts': release_records,
        'qa10_tool_validation': tool_validation,
        'visual_reuse_index': inventory.reference(VISUAL_REUSE), 'visual_owner_references': reused,
        'qualification_limit': 'Existing owner evidence retained; no current-candidate live/device acceptance inferred.',
        'read_bytes': inventory.read_bytes})
    save('version-and-production-inputs.json', versions)
    source.require(report['status'] == 'INCOMPLETE' and report['qa08'] == 'DEFERRED_BY_USER'
                   and report['task_coverage']['required'] == 112 and report['ia_coverage']['required'] == 61
                   and report['release_decision'] == 'NOT_ASSESSED', 'inventory.unexpected-candidate-result')
    source.require(evaluate(root, '0' * 40, input_ref['path'])['findings'] == ['candidate.not-current-head'],
                   'inventory.stale-candidate-not-refused')
    save('stale-frontend-input.json', {**document, 'receipts': [{'path': HISTORICAL_FE,
         'sha256': source.digest(inventory.read(HISTORICAL_FE))}]})
    stale = evaluate(root, args.candidate, (out / 'stale-frontend-input.json').relative_to(root).as_posix())
    source.require(stale['status'] == 'FAIL' and 'candidate.mismatch' in stale['findings'], 'inventory.stale-receipt-not-refused')
    save('stale-frontend-refusal.json', stale)
    print(json.dumps({'candidate': args.candidate, 'input': input_ref, 'composition_checks': report['composition_checks'],
        'tasks': report['task_coverage'], 'ia': report['ia_coverage'], 'qa08': report['qa08'],
        'product_release_state': 'NOT_READY', 'release_decision': 'NOT_ASSESSED',
        'checks': ['actual-input-built', 'original-112-61-preserved', 'qa08-deferred', 'stale-head-refused',
                   'actual-historical-frontend-refused'], 'report': 'candidate-report.json'}))
    return 0


if __name__ == '__main__':
    sys.exit(main())
