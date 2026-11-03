import spec from '../../../../../../contracts/generated/mcr-domain.openapi.json';
import { canonicalUuid } from '../../../auth/model/contract';
/** Restricted headless C04 interpreter adapted from party-authoring/model/contract.ts
 * at 98c2c793. Only description is ignored as an annotation; assertions fail closed. */
type Schema = {
    $ref?: string;
    type?: string;
    const?: unknown;
    enum?: unknown[];
    anyOf?: Schema[];
    properties?: Record<string, Schema>;
    required?: string[];
    additionalProperties?: boolean;
    items?: Schema;
    minItems?: number;
    maxItems?: number;
    minLength?: number;
    maxLength?: number;
    minimum?: number;
    maximum?: number;
    format?: string;
    pattern?: string;
    description?: string;
};
const schemas = spec.components.schemas as unknown as Record<string, Schema>;
export function adminValue<T>(name: string, value: unknown): T {
    const budget = { remaining: 100000 };
    function valid(v: unknown, s: Schema, depth = 0): boolean {
        if (--budget.remaining < 0 || depth > 30 || Object.keys(s).some(k => !['$ref', 'type', 'const', 'enum', 'anyOf', 'properties', 'required', 'additionalProperties', 'items', 'minItems', 'maxItems', 'minLength', 'maxLength', 'minimum', 'maximum', 'format', 'pattern', 'description'].includes(k)))
            return false;
        if (s.$ref) {
            const n = s.$ref.replace('#/components/schemas/', '');
            return s.$ref === '#/components/schemas/' + n && !!schemas[n] && valid(v, schemas[n]!, depth + 1);
        }
        if (s.anyOf)
            return s.anyOf.some(t => valid(v, t, depth + 1));
        if ('const' in s && v !== s.const || s.enum && !s.enum.includes(v))
            return false;
        if (s.type === 'null')
            return v === null;
        if (s.type === 'object') {
            if (!v || typeof v !== 'object' || Array.isArray(v) || s.additionalProperties !== false)
                return false;
            return (s.required ?? []).every(k => Object.hasOwn(v, k)) && Object.entries(v).every(([k, x]) => !!s.properties?.[k] && valid(x, s.properties[k]!, depth + 1));
        }
        if (s.type === 'array')
            return Array.isArray(v) && !!s.items && v.length >= (s.minItems ?? 0) && v.length <= (s.maxItems ?? 100) && v.every(x => valid(x, s.items!, depth + 1));
        if (s.type === 'integer')
            return typeof v === 'number' && Number.isSafeInteger(v) && v >= (s.minimum ?? 0) && v <= (s.maximum ?? Number.MAX_SAFE_INTEGER);
        if (s.type === 'boolean')
            return typeof v === 'boolean';
        if (s.type === 'string')
            return typeof v === 'string' && Array.from(v).length >= (s.minLength ?? 0) && Array.from(v).length <= (s.maxLength ?? 10000) && (!s.pattern || new RegExp(s.pattern, 'u').test(v)) && (!s.format || s.format === 'uuid' && canonicalUuid(v) || s.format === 'date-time' && /^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d(?:\.\d+)?Z$/.test(v) && Number.isFinite(Date.parse(v)) && new Date(v).toISOString().slice(0, 19) === v.slice(0, 19));
        return 'const' in s || !!s.enum;
    }
    if (!schemas[name] || !valid(value, schemas[name]!))
        throw new Error('Invalid moderator contract');
    return structuredClone(value) as T;
}
