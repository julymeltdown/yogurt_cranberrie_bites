"""Actual runtime SQL paths are independent of copied executable identities."""
from __future__ import annotations
import hashlib
from pathlib import Path


class RuntimeSqlPins:
    def __init__(self, inherited, root, inputs, path):
        self.api, self.root = inherited, root
        value = inherited.json_object(inherited.private_input(path))
        inherited.closed(value, ('services',), 'sessions-runtime-sql-schema')
        inherited.require(set(value['services'])==inherited.SERVICES,
                          'sessions-seven-runtime-sql-closures-required')
        self.services = value['services']
        for service, record in self.services.items():
            inherited.closed(record, ('source_revision','binary_sha256','compile_manifest_path','sql_sha384'),
                             'sessions-runtime-sql-record')
            binary = inputs['binaries'][service]
            inherited.require(record['source_revision']==binary['source']
                and record['binary_sha256']==binary['sha256'], 'sessions-runtime-sql-binary-binding')
            manifest = Path(record['compile_manifest_path'])
            inherited.require(manifest.is_absolute() and manifest.name==service
                and not manifest.is_symlink(), 'sessions-actual-compile-manifest-path-required')
            expected = inherited.git_paths(root,binary['source'],f'backend/services/{service}/migrations')
            inherited.require(set(expected)==set(record['sql_sha384']), 'sessions-runtime-sql-source-inventory')
            for name,digest in record['sql_sha384'].items():
                actual = hashlib.sha384(inherited.source_bytes(root,binary['source'],name)).hexdigest()
                inherited.require(actual==digest,'sessions-runtime-sql-source-bytes')
        self.verify()

    def verify(self):
        for service,record in self.services.items():
            directory = Path(record['compile_manifest_path'])/'migrations'
            prefix = f'backend/services/{service}/migrations/'
            names = {prefix+path.name for path in directory.glob('*.sql')}
            self.api.require(names==set(record['sql_sha384']) and not directory.is_symlink(),
                             'sessions-runtime-sql-live-inventory-drift')
            for name,digest in record['sql_sha384'].items():
                raw,_ = self.api.read_owned(directory/Path(name).name)
                self.api.require(hashlib.sha384(raw).hexdigest()==digest,
                                 'sessions-runtime-sql-live-byte-drift')
