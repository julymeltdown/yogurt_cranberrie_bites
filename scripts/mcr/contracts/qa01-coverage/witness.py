"""Deterministic schema witnesses, not server-response or business-rule evidence."""
from copy import deepcopy
import json


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"), allow_nan=False)


def nodes(value):
    yield value
    if isinstance(value, dict):
        for child in value.values():
            yield from nodes(child)
    elif isinstance(value, list):
        for child in value:
            yield from nodes(child)


class Witnesses:
    def __init__(self, document, validator, corpus):
        self.document, self.validator = document, validator
        self.corpus = list({canonical(v): v for source in corpus for v in nodes(source)}.values())
        self.cache = {}

    def resolve(self, reference):
        if not reference.startswith("#/"):
            raise ValueError("external generated reference: " + reference)
        value = self.document
        for part in reference[2:].split("/"):
            value = value[part.replace("~1", "/").replace("~0", "~")]
        return value

    def merge(self, left, right):
        if left is False or right is False:
            return False
        result = deepcopy(left if left is not True else {})
        for key, value in (right if right is not True else {}).items():
            if key == "required":
                result[key] = sorted(set(result.get(key, [])) | set(value))
            elif key == "properties":
                result.setdefault(key, {})
                for name, child in value.items():
                    result[key][name] = self.merge(result[key].get(name, {}), child)
            elif key == "enum" and key in result:
                result[key] = [v for v in result[key] if v in value]
            elif key in ("minimum", "minLength", "minItems", "minProperties"):
                result[key] = max(result.get(key, value), value)
            elif key in ("maximum", "maxLength", "maxItems", "maxProperties"):
                result[key] = min(result.get(key, value), value)
            else:
                result[key] = deepcopy(value)
        return result

    def expand(self, schema):
        result = deepcopy(schema)
        if isinstance(result, bool):
            return result
        if "$ref" in result:
            reference = result.pop("$ref")
            result = self.merge(self.expand(self.resolve(reference)), result)
        for branch in result.pop("allOf", []):
            result = self.merge(result, self.expand(branch))
        return result

    def valid(self, schema, value):
        return self.validator(schema).is_valid(value)

    def make(self, schema, depth=0):
        key = canonical(schema)
        if key in self.cache:
            return deepcopy(self.cache[key])
        if depth > 32:
            raise ValueError("schema witness recursion bound")
        expanded = self.expand(schema)
        if expanded is False:
            raise ValueError("unsatisfiable response schema")
        if expanded is True:
            expanded = {}
        if "const" in expanded:
            candidates = [expanded["const"]]
        elif "enum" in expanded:
            candidates = expanded["enum"]
        else:
            candidates = []
        for value in candidates:
            if self.valid(schema, value):
                self.cache[key] = value
                return deepcopy(value)
        # Fixture nodes are candidates only: every selected value is independently
        # checked against the full target schema, including sibling assertions.
        kind = expanded.get("type")
        required = set(expanded.get("required", []))
        for value in self.corpus:
            if required and (not isinstance(value, dict) or not required <= value.keys()):
                continue
            if kind == "object" and not isinstance(value, dict):
                continue
            if kind == "array" and not isinstance(value, list):
                continue
            if self.valid(schema, value):
                self.cache[key] = value
                return deepcopy(value)
        for union in ("oneOf", "anyOf"):
            if union in expanded:
                base = {k: v for k, v in expanded.items() if k != union}
                for branch in expanded[union]:
                    try:
                        value = self.make(self.merge(base, self.expand(branch)), depth + 1)
                    except ValueError:
                        continue
                    if self.valid(schema, value):
                        self.cache[key] = value
                        return deepcopy(value)
                raise ValueError("no deterministic witness for union: " + key[:160])
        kinds = kind if isinstance(kind, list) else [kind]
        for item in kinds:
            try:
                value = self.construct(expanded, item, depth)
                if self.valid(schema, value):
                    self.cache[key] = value
                    return deepcopy(value)
                if "if" in expanded and self.valid(expanded["if"], value):
                    value = self.make(self.merge({k: v for k, v in expanded.items()
                                                  if k not in ("if", "then", "else")},
                                                 expanded.get("then", {})), depth + 1)
                    if self.valid(schema, value):
                        self.cache[key] = value
                        return deepcopy(value)
            except ValueError:
                continue
        raise ValueError("no deterministic witness for schema: " + key[:220])

    def construct(self, schema, kind, depth):
        if kind == "object" or (kind is None and "properties" in schema):
            properties = schema.get("properties", {})
            names = list(schema.get("required", []))
            for name in properties:
                if len(names) >= schema.get("minProperties", 0):
                    break
                if name not in names:
                    names.append(name)
            return {name: self.make(properties.get(name, {}), depth + 1) for name in names}
        if kind == "array":
            return [self.make(schema.get("items", {}), depth + 1)
                    for _ in range(schema.get("minItems", 0))]
        if kind == "null" or kind is None:
            return None
        if kind == "boolean":
            return False
        if kind in ("number", "integer"):
            return schema.get("minimum", 0)
        if kind == "string":
            length = max(1, schema.get("minLength", 0))
            candidates = ["x" * length, "0" * length, "a" * 43, "0" * 64,
                          "sha256:" + "0" * 64, "#000000", "000000",
                          "00000000-0000-4000-8000-000000000001", "2026-11-07T07:00:00Z",
                          "2026-11-07", "fixture@example.test", "https://example.test/",
                          "world/fixture.png", "world/fixture.json"]
            uuid = "00000000-0000-4000-8000-000000000001"
            candidates += [f"/api/concert/events/{uuid}/media/{uuid}/{suffix}"
                           for suffix in ("bytes", "content")]
            for value in candidates:
                if self.valid(schema, value):
                    return value
        raise ValueError("unsupported deterministic witness: " + canonical(schema)[:180])
