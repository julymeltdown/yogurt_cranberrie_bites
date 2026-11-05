"""Generate the Notes-only public wire projection from a pinned qualified API.

Authoring tool; no backend implementation, service authority, or schema edits.
"""
from pathlib import Path
import hashlib
import json
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[5]
DEST = Path(__file__).resolve().parent
SOURCE = 'a378329c242f950f36b868ff50c9ef19bf6ed6c4'
API = 'backend/docs/api/paradise-city-v1/place-notes.openapi.json'
EXPECTED = '5b14c48c7b288ba4a1c7a68c5e0d84c16cc9805ae87e98b18c091957c2f24f1e'

def read():
    # The producer's private source commit is provenance, not a clone prerequisite.
    path = ROOT / API
    raw = path.read_bytes() if path.exists() else subprocess.check_output(
        ['git', 'show', '8d4561d9291274f867fc921ba8ed4c28790a85a7:' + API], cwd=ROOT)
    if hashlib.sha256(raw).hexdigest() != EXPECTED:
        raise ValueError('Pinned Notes API bytes do not match')
    return json.loads(raw)

def literal(value):
    return json.dumps(value, ensure_ascii=True, separators=(',', ':'))

def type_of(schema):
    if '$ref' in schema:
        return schema['$ref'].removeprefix('#/components/schemas/')
    if 'const' in schema:
        return literal(schema['const'])
    if 'enum' in schema:
        return '(' + ' | '.join(map(literal, schema['enum'])) + ')'
    for union in ('oneOf', 'anyOf'):
        if union in schema:
            return '(' + ' | '.join(type_of(s) for s in schema[union]) + ')'
    kind = schema.get('type')
    if kind in ('integer', 'number'):
        return 'number'
    if kind in ('string', 'boolean', 'null'):
        return kind
    if kind == 'array':
        return 'readonly (' + type_of(schema['items']) + ')[]'
    if kind == 'object':
        required = schema.get('required', [])
        return '{ ' + '; '.join('readonly ' + literal(k) + ('' if k in required else '?') + ': ' + type_of(v)
                                 for k, v in schema.get('properties', {}).items()) + ' }'
    raise ValueError('Unsupported schema type: ' + repr(schema))

def main():
    api = read()
    schemas = api['components']['schemas']
    supported = {'$ref', 'type', 'const', 'enum', 'oneOf', 'anyOf', 'not', 'properties', 'required',
                 'additionalProperties', 'items', 'minItems', 'maxItems', 'minLength', 'maxLength',
                 'minimum', 'maximum', 'pattern', 'format', 'x-maxUtf8Bytes', 'description', 'default'}
    def supported_schema(schema):
        if set(schema) - supported:
            raise ValueError('Unsupported wire keywords: ' + repr(set(schema) - supported))
        if '$ref' in schema and schema['$ref'].removeprefix('#/components/schemas/') not in schemas:
            raise ValueError('Unknown local schema reference')
        if schema.get('format') not in (None, 'uuid', 'date-time'):
            raise ValueError('Unsupported format')
        for child in schema.get('properties', {}).values(): supported_schema(child)
        for key in ('oneOf', 'anyOf'):
            for child in schema.get(key, []): supported_schema(child)
        for key in ('items', 'not'):
            if key in schema: supported_schema(schema[key])
    for schema in schemas.values(): supported_schema(schema)
    operations = {}
    inputs = {}
    responses = {}
    for path, methods in api['paths'].items():
        for method, op in methods.items():
            name = op['operationId']
            properties = {}
            required = []
            parameters = []
            for param in op.get('parameters', []):
                key = 'idempotencyKey' if param['in'] == 'header' and param['name'].lower() == 'idempotency-key' else param['name']
                properties[key] = param['schema']
                if param.get('required'):
                    required.append(key)
                parameters.append(dict(key=key, name=param['name'], location=param['in'], required=param.get('required', False)))
            body = op.get('requestBody')
            if body:
                properties['body'] = body['content']['application/json']['schema']
                if body.get('required'):
                    required.append('body')
            input_schema = dict(type='object', additionalProperties=False, properties=properties, required=required)
            supported_schema(input_schema)
            success = {status: value['content']['application/json']['schema']
                       for status, value in op['responses'].items() if status.startswith('2')}
            errors = {status: value['content']['application/json']['schema']
                      for status, value in op['responses'].items() if not status.startswith('2')}
            operations[name] = dict(method=method.upper(), path=path, parameters=parameters,
                                    input=input_schema, success=success, errors=errors)
            inputs[name] = type_of(input_schema)
            responses[name] = ' | '.join(dict.fromkeys(type_of(s) for s in success.values()))
    lines = ['// Generated from the exact source contract; run generate.py to reproduce.',
             '// Do not edit generated public DTOs or import backend runtime implementation.',
             'export const notesProvenance = ' + literal(dict(source=SOURCE, path=API, sha256=EXPECTED)) + ' as const',
             'export const notesSchemas = ' + literal(schemas) + ' as const',
             'export const notesOperations = ' + literal(operations) + ' as const']
    lines += ['export type ' + name + ' = ' + type_of(schema) for name, schema in schemas.items()]
    lines += ['export interface NotesInputs {'] + [literal(name) + ': ' + value for name, value in inputs.items()] + ['}']
    lines += ['export interface NotesResponses {'] + [literal(name) + ': ' + value for name, value in responses.items()] + ['}']
    lines += ['export type NotesOperationId = keyof NotesInputs', '']
    output = '\n'.join(lines)
    if '--check' in sys.argv:
        assert (DEST/'notes.generated.ts').read_text() == output, 'Notes generated wire drift'
    else:
        (DEST/'notes.generated.ts').write_text(output)
    subprocess.run([sys.executable, '-B', str(ROOT/'scripts/mcr/generate-domain-transport.py'), '--venue-only',
                    *(['--check'] if '--check' in sys.argv else [])], cwd=ROOT, check=True)
    print(json.dumps(dict(source=SOURCE, sha256=EXPECTED, schemas=list(schemas),
                          operations={name:dict(method=op['method'], path=op['path'], input=inputs[name], response=responses[name])
                                      for name, op in operations.items()})))

if __name__ == '__main__':
    main()
