import { createHash, randomUUID } from 'node:crypto';
import { chmod, lstat, mkdtemp, readFile, rm, symlink, writeFile } from 'node:fs/promises';
import { isAbsolute, join } from 'node:path';
import { tmpdir } from 'node:os';
import { pathToFileURL } from 'node:url';
import assert from 'node:assert/strict';
import { z } from 'zod';
import type { APIRequestContext, Page } from '@playwright/test';
export class NotRun extends Error {
    constructor(readonly code: string) { super(`IN04 NOT_RUN: ${code}`); }
}
export function need(value: unknown, code: string): asserts value { if (!value)
    throw new NotRun(code); }
export const hash = (value: string | Uint8Array) => createHash('sha256').update(value).digest('hex');
const uuid = z.string().regex(/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/);
const sha = z.string().regex(/^[0-9a-f]{64}$/);
const utc = z.string().datetime({ offset: false }).refine(s => s.endsWith('Z'));
const account = z.strictObject({ userId: uuid, email: z.email(), password: z.string().min(8).max(512) });
const configSchema = z.strictObject({ version: z.literal(1), isolated: z.literal(true), allowDestructiveFixtureEndState: z.literal(true),
    origin: z.string().refine(s => { try {
        const u = new URL(s);
        return u.origin === s && !u.username && !u.password && (u.protocol === 'https:' || u.protocol === 'http:' && ['127.0.0.1', 'localhost', '[::1]'].includes(u.hostname));
    }
    catch {
        return false;
    } }),
    eventId: uuid, releaseRevision: z.string().regex(/^[0-9a-f]{40}$/), environmentId: uuid,
    members: z.tuple([account, account]), operator: account,
    contractHashes: z.strictObject({ domain: sha, realtime: sha }),
    restoreSnapshotId: uuid, maxCooldownMs: z.number().int().min(5000).max(30000),
    restartObservationTimeoutMs: z.number().int().min(1000).max(60000),
});
export type GateConfig = z.infer<typeof configSchema>;
export function parseConfig(value: unknown): GateConfig {
    const parsed = configSchema.safeParse(value);
    need(parsed.success, 'INVALID_PRIVATE_CONFIGURATION');
    const c = parsed.data, actors = [...c.members, c.operator];
    need(new Set(actors.map(a => a.userId)).size === 3 && new Set(actors.map(a => a.email.toLowerCase())).size === 3, 'THREE_INDEPENDENT_AUTHENTICATED_ACTORS_REQUIRED');
    return c;
}
export async function loadConfig(path = process.env.MCR_IN04_CONFIG): Promise<GateConfig> {
    need(path && isAbsolute(path), 'MCR_IN04_CONFIG_PRIVATE_ABSOLUTE_PATH_REQUIRED');
    try {
        const st = await lstat(path);
        need(st.isFile() && !st.isSymbolicLink() && st.uid === process.getuid?.() && !(st.mode & 0o077) && st.size <= 32768, 'CONFIG_MUST_BE_PRIVATE_OWNED_REGULAR_FILE');
        let value: unknown;
        try {
            value = JSON.parse(await readFile(path, 'utf8'));
        }
        catch {
            throw new NotRun('PRIVATE_CONFIG_UNREADABLE_OR_INVALID_JSON');
        }
        return parseConfig(value);
    }
    catch (e) {
        if (e instanceof NotRun)
            throw e;
        throw new NotRun('PRIVATE_CONFIG_UNREADABLE');
    }
}
export interface DeploymentProof {
    environmentId: string;
    eventId: string;
    isolated: true;
    legacyData: false;
    candidate: string;
    domainHash: string;
    realtimeHash: string;
    expiresAt: string;
    actors: readonly {
        userId: string;
        role: 'MEMBER' | 'OPERATOR';
    }[];
    services: readonly {
        name: 'gateway' | 'auth' | 'profile' | 'concert' | 'world' | 'postgres';
        ready: true;
        candidate: string;
    }[];
    ownResultOperation: string;
    restoreSnapshotId: string;
    exclusiveEvent: true;
}
/** Delivery must bind this to a reviewed, read-only deployment/authority source.
 * Configuration statements and local fixtures are NOT an implementation of this port.
 * Restart is external operator activity: this port observes it, never executes a command.
 */
export interface RuntimeQualificationPort {
    inspect(config: GateConfig): Promise<DeploymentProof>;
    observeRestart(config: GateConfig, before: {
        roomEpoch: string;
        streamId: string;
    }): Promise<{
        candidate: string;
        environmentId: string;
        restartedAt: string;
    }>;
}
/** No reviewed deployment identity/role/isolation protocol exists at the assigned pin.
 * Keep absent instead of inventing an endpoint, header, diagnostic global or authority.
 */
export function runtimeQualification(): RuntimeQualificationPort | null { return null; }
export function verifyProof(c: GateConfig, p: DeploymentProof, now = Date.now()) {
    need(p.isolated === true && p.legacyData === false && p.exclusiveEvent === true && p.environmentId === c.environmentId && p.eventId === c.eventId, 'LIVE_ISOLATION_BINDING_REQUIRED');
    need(p.candidate === c.releaseRevision && p.domainHash === c.contractHashes.domain && p.realtimeHash === c.contractHashes.realtime, 'DEPLOYED_CANDIDATE_CONTRACT_MISMATCH');
    need(Number.isFinite(Date.parse(p.expiresAt)) && Date.parse(p.expiresAt) > now && Date.parse(p.expiresAt) <= now + 15 * 60000, 'FRESH_DEPLOYMENT_QUALIFICATION_REQUIRED');
    need(p.actors.length === 3 && new Set(p.actors.map(a => a.userId)).size === 3, 'UNAMBIGUOUS_LIVE_ACTOR_ROLES_REQUIRED');
    for (const a of c.members)
        need(p.actors.some(x => x.userId === a.userId && x.role === 'MEMBER'), 'QUALIFIED_MEMBER_AUTHORITY_REQUIRED');
    need(p.actors.some(x => x.userId === c.operator.userId && x.role === 'OPERATOR'), 'QUALIFIED_OPERATOR_AUTHORITY_REQUIRED');
    for (const name of ['gateway', 'auth', 'profile', 'concert', 'world', 'postgres'])
        need(p.services.some(s => s.name === name && s.ready === true && s.candidate === c.releaseRevision), 'CURRENT_REAL_SERVICES_REQUIRED');
    need(p.restoreSnapshotId === c.restoreSnapshotId && p.ownResultOperation === 'getConcertMuralOwnResult', 'AUTHORIZED_NAMED_SNAPSHOT_AND_OWN_RESULT_REQUIRED');
}
export interface Operation {
    operationId: string;
    method: string;
    path: string;
    response: string;
    request: string | null;
}
type Rule = Record<string, unknown>;
const object = (v: unknown): v is Rule => !!v && typeof v === 'object' && !Array.isArray(v);
/** Strict runtime checking against the generated public contract, including private-field rejection. */
export class Contracts {
    constructor(readonly operations: Operation[], readonly schemas: Record<string, unknown>) { }
    operation(id: string) { const found = this.operations.find(o => o.operationId === id); need(found, 'FROZEN_OPERATION_REQUIRED'); return found; }
    path(id: string, eventId: string, parameters: Record<string, string> = {}) {
        let path = this.operation(id).path.replace('{eventId}', uuid.parse(eventId));
        for (const [k, value] of Object.entries(parameters))
            path = path.replace(`{${k}}`, uuid.parse(value));
        need(!/[{}]/.test(path) && path.startsWith(`/api/concert/events/${eventId}/`), 'BOUND_PUBLIC_OPERATION_PATH_REQUIRED');
        return path;
    }
    decode<T>(name: string, value: unknown): T {
        const valid = (input: unknown, v: unknown, depth = 0): boolean => {
            if (!object(input) || depth > 30)
                return false;
            const r = input;
            if (!Object.keys(r).every(k => ['type', '$ref', 'const', 'enum', 'properties', 'required', 'additionalProperties', 'items', 'minItems', 'maxItems', 'uniqueItems', 'minLength', 'maxLength', 'pattern', 'format', 'minimum', 'maximum', 'anyOf', 'description', 'title', 'default'].includes(k)))
                return false;
            if (typeof r.$ref === 'string')
                return /^#\/components\/schemas\/[A-Za-z0-9_]+$/.test(r.$ref) && valid(this.schemas[r.$ref.split('/').at(-1)!], v, depth + 1);
            if (Array.isArray(r.anyOf))
                return r.anyOf.some(x => valid(x, v, depth + 1));
            if ('const' in r)
                return v === r.const;
            if (Array.isArray(r.enum))
                return r.enum.includes(v);
            if (r.type === 'null')
                return v === null;
            if (r.type === 'boolean')
                return typeof v === 'boolean';
            if (r.type === 'integer')
                return typeof v === 'number' && Number.isSafeInteger(v) && v >= Number(r.minimum ?? Number.MIN_SAFE_INTEGER) && v <= Number(r.maximum ?? Number.MAX_SAFE_INTEGER);
            if (r.type === 'string')
                return typeof v === 'string' && v.length >= Number(r.minLength ?? 0) && v.length <= Number(r.maxLength ?? 10000)
                    && (typeof r.pattern !== 'string' || new RegExp(r.pattern, 'u').test(v)) && (r.format === undefined || r.format === 'uuid' && uuid.safeParse(v).success || r.format === 'date-time' && utc.safeParse(v).success);
            if (r.type === 'array')
                return Array.isArray(v) && v.length >= Number(r.minItems ?? 0) && v.length <= Number(r.maxItems ?? 16384) && v.every(x => valid(r.items, x, depth + 1));
            if (r.type === 'object')
                return object(v) && object(r.properties) && Array.isArray(r.required) && r.additionalProperties === false
                    && r.required.every(k => typeof k === 'string' && Object.hasOwn(v, k)) && Object.entries(v).every(([k, x]) => object(r.properties) && Object.hasOwn(r.properties, k) && valid(r.properties[k], x, depth + 1));
            return false;
        };
        assert(valid(this.schemas[name], value), 'STRICT_CONTRACT_RESPONSE_REQUIRED');
        return value as T;
    }
}
export async function loadContracts(c?: GateConfig) {
    const [domain, realtime, ops] = await Promise.all([
        readFile(new URL('../../../contracts/generated/mcr-domain.openapi.json', import.meta.url)),
        readFile(new URL('../../../contracts/realtime/v1/world.schema.json', import.meta.url)),
        readFile(new URL('../../../contracts/domain/v1/operations.json', import.meta.url)),
    ]);
    if (c)
        need(hash(domain) === c.contractHashes.domain && hash(realtime) === c.contractHashes.realtime, 'LOCAL_FROZEN_CONTRACT_HASH_MISMATCH');
    const bundle = JSON.parse(domain.toString()) as {
        components: {
            schemas: Record<string, unknown>;
        };
    };
    return new Contracts(JSON.parse(ops.toString()) as Operation[], bundle.components.schemas);
}
export interface Snapshot {
    snapshot_id: string;
    event_id: string;
    revision: number;
    state: 'EDITABLE' | 'PAUSED' | 'FROZEN';
    width: 128;
    height: 128;
    palette: string[];
    pixels: number[];
    server_time: string;
    next_allowed_at: string | null;
    cooldown_ms: number;
}
export interface Pixel {
    operation_id: string;
    event_id: string;
    revision: number;
    x: number;
    y: number;
    color_index: number;
    committed_at: string;
    next_allowed_at: string;
}
export interface OwnPixelResult {
    receipt_id: string;
    idempotency_key: string;
    result: Pixel;
}
export interface Delta {
    event_id: string;
    from_revision: number;
    to_revision: number;
    reset_required: boolean;
    operations: Omit<Pixel, 'next_allowed_at'>[];
}
export interface Admin {
    event_id: string;
    state: Snapshot['state'];
    revision: number;
    snapshot_id: string;
    locks: {
        x_min: number;
        y_min: number;
        x_max_exclusive: number;
        y_max_exclusive: number;
    }[];
}
export interface Receipt {
    receipt_id: string;
    actor_id: string;
    event_id: string;
    operation_id: string;
    idempotency_key: string;
    state: string;
    resource_id: string | null;
    resource_revision: number | null;
    result_status: number | null;
    failure_reason: string | null;
    created_at: string;
    expires_at: string;
}
export type PixelBody = {
    expected_revision: number;
    x: number;
    y: number;
    color_index: number;
};
export function bindPixel(p: Pixel, body: PixelBody, eventId: string) {
    assert.equal(p.event_id, eventId);
    assert.equal(p.revision, body.expected_revision + 1);
    assert.equal(p.x, body.x);
    assert.equal(p.y, body.y);
    assert.equal(p.color_index, body.color_index);
    assert(Date.parse(p.next_allowed_at) >= Date.parse(p.committed_at));
}
export function bindReceipt(r: Receipt, actor: string, eventId: string, key: string) {
    assert.equal(r.actor_id, actor);
    assert.equal(r.event_id, eventId);
    assert.equal(r.operation_id, 'placeConcertPixel');
    assert.equal(r.idempotency_key, key);
    assert(Date.parse(r.expires_at) > Date.parse(r.created_at));
    assert.equal(r.resource_id, eventId);
}
export function bindOwnResult(envelope: OwnPixelResult, receipt: Receipt, body: PixelBody, actor: string, eventId: string, key: string, serverTime: string) {
    bindReceipt(receipt, actor, eventId, key);
    assert.equal(receipt.state, 'SUCCEEDED');
    assert.equal(receipt.result_status, 200);
    assert.equal(receipt.failure_reason, null);
    assert(Date.parse(receipt.expires_at) > Date.parse(serverTime));
    assert.equal(envelope.receipt_id, receipt.receipt_id);
    assert.equal(envelope.idempotency_key, key);
    bindPixel(envelope.result, body, eventId);
    assert.equal(envelope.result.revision, receipt.resource_revision);
    return envelope.result;
}
export interface RecordedCommand {
    actor: string;
    operation: string;
    key: string;
    bodyHash: string;
    outcome: string;
}
export class Actor {
    private token = '';
    readonly commands: RecordedCommand[] = [];
    constructor(readonly page: Page, readonly api: APIRequestContext, readonly account: GateConfig['operator'], readonly config: GateConfig, readonly contracts: Contracts) { }
    async login() {
        const response = await this.page.goto(`${this.config.origin}/login?returnTo=%2Fwall`);
        need(response?.ok(), 'REAL_PRODUCT_ORIGIN_REQUIRED');
        await this.page.getByLabel('Email', { exact: true }).fill(this.account.email);
        await this.page.getByLabel('Password', { exact: true }).fill(this.account.password);
        const login = this.page.waitForResponse(r => new URL(r.url()).pathname === '/api/auth/login' && r.request().method() === 'POST');
        await this.page.getByRole('button', { name: 'Sign in with email', exact: true }).click();
        const result = await login;
        need(result.ok(), 'REAL_PROVISIONED_LOGIN_REQUIRED');
        const body: unknown = await result.json();
        need(object(body) && body.userId === this.account.userId && typeof body.accessToken === 'string' && body.accessToken.length > 0, 'CANONICAL_LOGIN_IDENTITY_REQUIRED');
        this.token = body.accessToken; // Transient memory only; never report response bodies, credentials or headers.
    }
    async call(id: string, body?: unknown, parameters: Record<string, string> = {}, query: Record<string, string> = {}, key = randomUUID()) {
        const op = this.contracts.operation(id), method = op.method.toUpperCase();
        const url = new URL(this.contracts.path(id, this.config.eventId, parameters), this.config.origin);
        for (const [k, value] of Object.entries(query))
            url.searchParams.set(k, value);
        const command = method === 'GET' ? null : { actor: this.account.userId, operation: id, key, bodyHash: hash(JSON.stringify(body)), outcome: 'UNKNOWN' };
        if (command)
            this.commands.push(command);
        const response = await this.api.fetch(url.href, { method, headers: { Authorization: `Bearer ${this.token}`, Origin: this.config.origin, 'X-MCR-Intent': 'browser', ...(command ? { 'Idempotency-Key': key } : {}) }, ...(body === undefined ? {} : { data: body }), maxRetries: 0, maxRedirects: 0, timeout: 15000 });
        assert.equal(response.headers()['cache-control']?.includes('no-store'), true);
        const value: unknown = await response.json();
        if (command)
            command.outcome = response.status() === 200 ? 'HTTP_200_UNVERIFIED' : response.status() === 202 || response.status() >= 500 ? 'UNKNOWN' : 'REJECTED';
        return { status: response.status(), value, key };
    }
    async snapshot() { const r = await this.call('getConcertMuralSnapshot'); assert.equal(r.status, 200); const s = this.contracts.decode<Snapshot>('Domain_mural_MuralSnapshot', r.value); assert.equal(s.event_id, this.config.eventId); return s; }
    async delta(after: number) { const r = await this.call('getConcertMuralDelta', undefined, {}, { after_revision: String(after) }); assert.equal(r.status, 200); const d = this.contracts.decode<Delta>('Domain_mural_MuralDelta', r.value); assert.equal(d.event_id, this.config.eventId); assert.equal(d.from_revision, after); return d; }
    async admin() { const r = await this.call('getConcertAdminMural'); assert.equal(r.status, 200); const state = this.contracts.decode<Admin>('Domain_mural_MuralAdminState', r.value); assert.equal(state.event_id, this.config.eventId); return state; }
    clear() { this.token = ''; }
}
export async function selectPixel(page: Page, body: Pick<PixelBody, 'x' | 'y' | 'color_index'>) {
    await page.getByLabel('X', { exact: true }).fill(String(body.x));
    await page.getByLabel('Y', { exact: true }).fill(String(body.y));
    await page.getByRole('button', { name: 'Preview pixel', exact: true }).click();
    await page.getByRole('button', { name: new RegExp(`^Color ${body.color_index} #[0-9A-F]{6}$`) }).click();
}
export async function rendered(page: Page, snapshot: Snapshot) {
    const wall = page.locator('section.mcr-mural');
    assert.equal(await wall.getByText(`Confirmed revision ${snapshot.revision}`, { exact: true }).count(), 1);
    await wall.getByRole('button', { name: 'Fit wall', exact: true }).click();
    // FE32 fits 128² raster into a 512² canvas. Sample cell centers, excluding selection overlays.
    const colors = await wall.getByRole('group', { name: 'Mural pixel preview', exact: true }).locator('canvas').evaluate(canvas => {
        const c = (canvas as HTMLCanvasElement).getContext('2d')!, bytes = c.getImageData(0, 0, 512, 512).data, values: string[] = [];
        for (let y = 0; y < 128; y++)
            for (let x = 0; x < 128; x++) {
                const p = ((y * 4 + 2) * 512 + x * 4 + 2) * 4;
                if (bytes[p + 3] !== 255)
                    throw new Error('Opaque confirmed raster required');
                values.push('#' + [bytes[p], bytes[p + 1], bytes[p + 2]].map(v => v!.toString(16).padStart(2, '0')).join('').toUpperCase());
            }
        return values;
    });
    assert.deepEqual(colors, snapshot.pixels.map(p => snapshot.palette[p]));
}
export const scenarios = ['configuration', 'deployment', 'independentActors', 'baseline', 'confirmedPlacement', 'parallelCooldown', 'differentActorConvergence', 'rejectedPreview', 'ownResultRecovery', 'privacy', 'unauthorizedModeration', 'pause', 'regionLock', 'resume', 'restore', 'obsoleteDelta', 'slowReconnect', 'restartEpoch', 'finalFreeze', 'teardown'] as const;
export type Scenario = typeof scenarios[number];
export type Outcome = {
    status: 'NOT_RUN' | 'PASS' | 'FAIL';
    code?: string;
};
export function resultOf(checks: Record<Scenario, Outcome>) { return scenarios.some(k => checks[k]?.status === 'FAIL') ? 'FAIL' : scenarios.every(k => checks[k]?.status === 'PASS') ? 'PASS' : 'NOT_RUN'; }
async function selfCheck() {
    let checks = 0;
    const reject = (f: () => unknown) => { assert.throws(f); checks++; };
    for (const v of [undefined, {}, { version: 1, isolated: false }])
        reject(() => parseConfig(v));
    const contracts = await loadContracts();
    const u = (n: number) => `00000000-0000-4000-8000-${String(n).padStart(12, '0')}`;
    const sample = { version: 1, isolated: true, allowDestructiveFixtureEndState: true, origin: 'http://127.0.0.1:4000', eventId: u(1), environmentId: u(8), releaseRevision: 'a'.repeat(40), members: [{ userId: u(2), email: 'a@example.invalid', password: 'self-check-only' }, { userId: u(3), email: 'b@example.invalid', password: 'self-check-only' }], operator: { userId: u(4), email: 'o@example.invalid', password: 'self-check-only' }, contractHashes: { domain: 'b'.repeat(64), realtime: 'c'.repeat(64) }, restoreSnapshotId: u(9), maxCooldownMs: 10000, restartObservationTimeoutMs: 10000 };
    const c = parseConfig(sample);
    checks++;
    for (const patch of [{ isolated: false }, { allowDestructiveFixtureEndState: false }, { origin: 'http://unsafe.invalid' }, { origin: 'https://user:secret@example.invalid' }, { members: [sample.members[0], sample.members[0]] }, { operator: sample.members[0] }, { releaseRevision: 'master' }, { mock: true }, { maxCooldownMs: 0 }])
        reject(() => parseConfig({ ...sample, ...patch }));
    const proof: DeploymentProof = { environmentId: c.environmentId, eventId: c.eventId, isolated: true, legacyData: false, exclusiveEvent: true, candidate: c.releaseRevision, domainHash: c.contractHashes.domain, realtimeHash: c.contractHashes.realtime, expiresAt: new Date(Date.now() + 60000).toISOString(), actors: [...c.members.map(a => ({ userId: a.userId, role: 'MEMBER' as const })), { userId: c.operator.userId, role: 'OPERATOR' }], services: (['gateway', 'auth', 'profile', 'concert', 'world', 'postgres'] as const).map(name => ({ name, ready: true, candidate: c.releaseRevision })), ownResultOperation: 'getConcertMuralOwnResult', restoreSnapshotId: c.restoreSnapshotId };
    verifyProof(c, proof);
    checks++;
    for (const patch of [{ candidate: 'd'.repeat(40) }, { eventId: u(10) }, { isolated: false }, { legacyData: true }, { exclusiveEvent: false }, { expiresAt: '2020-01-01T00:00:00Z' }, { actors: [] }, { services: [] }, { domainHash: 'd'.repeat(64) }, { restoreSnapshotId: u(10) }, { actors: [...proof.actors, proof.actors[0]] }, { ownResultOperation: 'not-a-qualified-own-result' }])
        reject(() => verifyProof(c, { ...proof, ...patch } as DeploymentProof));
    const pixel = { operation_id: u(7), event_id: c.eventId, revision: 2, x: 1, y: 2, color_index: 3, committed_at: '2026-11-04T00:00:00Z', next_allowed_at: '2026-11-04T00:00:10Z' };
    contracts.decode('Domain_mural_PixelOperation', pixel);
    checks++;
    reject(() => contracts.decode('Domain_mural_PublicPixelOperation', pixel));
    const body = { expected_revision: 1, x: 1, y: 2, color_index: 3 };
    bindPixel(pixel, body, c.eventId);
    checks++;
    for (const patch of [{ revision: 1 }, { x: 2 }, { event_id: u(10) }, { color_index: 4 }, { next_allowed_at: '2026-11-03T00:00:00Z' }])
        reject(() => bindPixel({ ...pixel, ...patch }, body, c.eventId));
    const receipt = { receipt_id: u(6), actor_id: u(2), event_id: c.eventId, operation_id: 'placeConcertPixel', idempotency_key: u(5), state: 'SUCCEEDED', resource_id: c.eventId, resource_revision: 2, result_status: 200, failure_reason: null, created_at: pixel.committed_at, expires_at: '2026-11-05T00:00:00Z' };
    bindReceipt(receipt, u(2), c.eventId, u(5));
    checks++;
    for (const patch of [{ actor_id: u(10) }, { event_id: u(10) }, { operation_id: 'restoreConcertMural' }, { idempotency_key: u(10) }, { resource_id: pixel.operation_id }])
        reject(() => bindReceipt({ ...receipt, ...patch }, u(2), c.eventId, u(5)));
    const envelope = { receipt_id: receipt.receipt_id, idempotency_key: receipt.idempotency_key, result: pixel };
    contracts.decode('Domain_mural_OwnPixelResult', envelope);
    checks++;
    bindOwnResult(envelope, receipt, body, u(2), c.eventId, u(5), pixel.committed_at);
    checks++;
    for (const patch of [{ receipt_id: u(10) }, { idempotency_key: u(10) }, { result: { ...pixel, revision: 3 } }, { result: { ...pixel, event_id: u(10) } }, { result: { ...pixel, x: 2 } }, { result: { ...pixel, color_index: 4 } }])
        reject(() => bindOwnResult({ ...envelope, ...patch }, receipt, body, u(2), c.eventId, u(5), pixel.committed_at));
    for (const patch of [{ state: 'PENDING' }, { state: 'RETIRED' }, { state: 'FAILED' }, { result_status: 202 }, { failure_reason: 'MURAL_COOLDOWN' }, { resource_revision: 3 }, { expires_at: pixel.committed_at }])
        reject(() => bindOwnResult(envelope, { ...receipt, ...patch }, body, u(2), c.eventId, u(5), pixel.committed_at));
    reject(() => contracts.decode('Domain_mural_OwnPixelResult', pixel));
    reject(() => contracts.decode('Domain_mural_OwnPixelResult', { ...envelope, actor_id: u(2) }));
    reject(() => contracts.decode('Domain_mural_OwnPixelResult', { ...envelope, result: { ...pixel, actor_id: u(2) } }));
    const { next_allowed_at: _private, ...publicPixel } = pixel;
    contracts.decode('Domain_mural_PublicPixelOperation', publicPixel);
    checks++;
    void _private;
    const states = Object.fromEntries(scenarios.map(k => [k, { status: 'PASS' as const }])) as Record<Scenario, Outcome>;
    assert.equal(resultOf({} as Record<Scenario, Outcome>), 'NOT_RUN');
    checks++;
    assert.equal(resultOf(states), 'PASS');
    states.restartEpoch = { status: 'NOT_RUN' };
    assert.equal(resultOf(states), 'NOT_RUN');
    states.restore = { status: 'FAIL' };
    assert.equal(resultOf(states), 'FAIL');
    checks++;
    await assert.rejects(loadConfig(''), /MCR_IN04_CONFIG_PRIVATE_ABSOLUTE_PATH_REQUIRED/);
    checks++;
    const dir = await mkdtemp(join(tmpdir(), 'mcr-in04-config-negative-'));
    try {
        const file = join(dir, 'private.json'), link = join(dir, 'linked.json');
        await writeFile(file, JSON.stringify(sample), { mode: 0o600 });
        assert.deepEqual(await loadConfig(file), c);
        checks++;
        await symlink(file, link);
        await assert.rejects(loadConfig(link), /CONFIG_MUST_BE_PRIVATE_OWNED_REGULAR_FILE/);
        checks++;
        await chmod(file, 0o644);
        await assert.rejects(loadConfig(file), /CONFIG_MUST_BE_PRIVATE_OWNED_REGULAR_FILE/);
        checks++;
        await chmod(file, 0o600);
        await writeFile(file, 'invalid-json');
        await assert.rejects(loadConfig(file), /PRIVATE_CONFIG_UNREADABLE_OR_INVALID_JSON/);
        checks++;
        await writeFile(file, ' '.repeat(32769));
        await assert.rejects(loadConfig(file), /CONFIG_MUST_BE_PRIVATE_OWNED_REGULAR_FILE/);
        checks++;
        await assert.rejects(loadConfig(dir), /CONFIG_MUST_BE_PRIVATE_OWNED_REGULAR_FILE/);
        checks++;
        await assert.rejects(loadContracts(c), /LOCAL_FROZEN_CONTRACT_HASH_MISMATCH/);
        checks++;
    }
    finally {
        await rm(dir, { recursive: true, force: true });
    }
    console.log(JSON.stringify({ scope: 'IN04 harness qualification only; no service/browser/auth authority', checks, status: 'PASS', realIN04: 'NOT_RUN' }));
}
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
    try {
        if (process.argv.includes('--self-check'))
            await selfCheck();
        else if (process.argv.includes('--preflight')) {
            const c = await loadConfig();
            await loadContracts(c);
            const port = runtimeQualification();
            need(port, 'REVIEWED_LIVE_DEPLOYMENT_QUALIFICATION_PORT_REQUIRED');
            verifyProof(c, await port.inspect(c));
        }
        else
            throw new NotRun('USE_SELF_CHECK_OR_PREFLIGHT');
    }
    catch (error) {
        console.error(error instanceof NotRun ? error.message : 'IN04 harness qualification FAIL');
        process.exitCode = 1;
    }
}
