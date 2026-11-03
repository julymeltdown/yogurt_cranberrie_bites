import { readFile, writeFile } from 'node:fs/promises';
import { test, expect, type BrowserContext, type Route } from '@playwright/test';
import { Actor, NotRun, bindOwnResult, bindPixel, hash, loadConfig, loadContracts, need, rendered, resultOf, runtimeQualification, scenarios, selectPixel, verifyProof, type Admin, type DeploymentProof, type GateConfig, type Outcome, type OwnPixelResult, type Pixel, type PixelBody, type Receipt, type Scenario, type Snapshot } from './mural-helpers';
import { loadDecoder } from './world-gate/decoder';
import { WorldObserver } from './world-gate/observer';
test.describe.configure({ mode: 'serial', retries: 0 });
test('IN04 real isolated two-client mural, recovery, moderation and terminal freeze', async ({ playwright }, info) => {
    test.setTimeout(600000);
    const checks = Object.fromEntries(scenarios.map(k => [k, { status: 'NOT_RUN', code: 'PREVIOUS_PREREQUISITES_REQUIRED' }])) as Record<Scenario, Outcome>;
    const report = { kind: 'mcr-in04-real-mural-v1', status: 'NOT_RUN' as 'NOT_RUN' | 'PASS' | 'FAIL', checks,
        sourceBase: '1163b827f9e62390163788bb674d24855816e0ec', recoveredAuthoringBase: '892ca4de7580eb36c82b54efe44f03941ca68c41', recoveredCheckoutPin: '1455e6a72a160c9c49a9c634499eb7856b041de1', contractDependencies: ['a25b85592b906209f4a9d20f9c7279ced8abab02', 'bff7e53c0638d5462af819bf560468a1f857c88e'], candidate: '', eventId: '', environmentId: '', sourceHashes: {} as Record<string, string>,
        mutations: [] as Actor['commands'], browserPlacements: [] as {
            actor: string;
            key: string;
            body: PixelBody;
            outcome: string;
        }[],
        restoreSnapshotId: '', finalState: 'NOT_RUN', lastRevision: 0,
        qualification: null as DeploymentProof | null,
        recoveries: [] as { receipt: Receipt; envelope: OwnPixelResult }[],
        convergence: [] as { revision: number; state: Snapshot['state']; pixelsHash: string; snapshotId: string }[],
        restart: 'NOT_RUN', scope: 'Real deployment only; no fixture fallback, synthetic ACK, automatic mutation replay, service startup or restart command.',
        recovery: 'On failure preserve this private journal, inspect command status/original results, and have the qualified operator restore the named baseline. Do not replay writes automatically.' };
    const contexts: BrowserContext[] = [], actors: Actor[] = [], observers: WorldObserver[] = [];
    let browser: Awaited<ReturnType<typeof playwright.chromium.launch>> | undefined, c: GateConfig | undefined;
    let healthy = true, baseline: Snapshot | undefined, opState: Admin | undefined;
    const ownPixels = new Map<number, Pixel>();
    let held: Route | undefined, releaseHeld: (() => Promise<void>) | undefined;
    const step = async (name: Scenario, work: () => Promise<void>) => {
        if (!healthy && name !== 'teardown')
            return;
        try {
            await work();
            checks[name] = { status: 'PASS' };
        }
        catch (error) {
            checks[name] = error instanceof NotRun ? { status: 'NOT_RUN', code: error.code } : { status: 'FAIL', code: 'ASSERTION_OR_RUNTIME_FAILURE' };
            healthy = false;
        }
    };
    const member = (i: number) => actors[i]!, operator = () => actors[2]!;
    const page = (i: number) => member(i).page;
    const fresh = async () => { const s = await member(0).snapshot(); report.lastRevision = s.revision; return s; };
    const converge = async () => {
        const s = await fresh();
        for (let i = 0; i < 2; i++) {
            await expect.poll(async () => { try {
                await rendered(page(i), s);
                return true;
            }
            catch {
                return false;
            } }, { timeout: 15000 }).toBe(true);
            const peer = await member(i).snapshot();
            expect(peer.revision).toBe(s.revision);
            expect(peer.pixels).toEqual(s.pixels);
        }
        report.convergence.push({revision:s.revision,state:s.state,pixelsHash:hash(JSON.stringify(s.pixels)),snapshotId:s.snapshot_id});
        return s;
    };
    const ready = async (i: number) => {
        const s = await member(i).snapshot(), wait = s.next_allowed_at ? Date.parse(s.next_allowed_at) - Date.parse(s.server_time) : 0;
        need(wait <= c!.maxCooldownMs, 'BOUNDED_SERVER_COOLDOWN_REQUIRED');
        if (wait > 0)
            await page(i).waitForTimeout(wait + 100);
        return member(i).snapshot();
    };
    const pixelBody = (s: Snapshot, x: number, y: number): PixelBody => ({ expected_revision: s.revision, x, y, color_index: (s.pixels[y * 128 + x]! + 1) % 16 });
    const checkPixel = async (i: number, body: PixelBody) => {
        const result = await member(i).call('placeConcertPixel', body);
        expect(result.status).toBe(200);
        const p = member(i).contracts.decode<Pixel>('Domain_mural_PixelOperation', result.value);
        bindPixel(p, body, c!.eventId);
        ownPixels.set(i, p);
        return p;
    };
    const reason = (value: unknown) => (value as {
        details?: {
            reason?: unknown;
        };
    })?.details?.reason;
    const reject = async (i: number, body: PixelBody, why: string) => { const r = await member(i).call('placeConcertPixel', body); expect([403, 409, 429]).toContain(r.status); expect(reason(r.value)).toBe(why); };
    const control = async (action: 'PAUSE' | 'RESUME' | 'FREEZE') => {
        const before = await operator().admin();
        const r = await operator().call('controlConcertMural', { expected_revision: before.revision, action, reason: 'IN04 isolated gate' });
        // Pending/unknown operator transitions stop the run; never infer completion from requested action.
        expect(r.status).toBe(200);
        opState = operator().contracts.decode<Admin>('Domain_mural_MuralAdminState', r.value);
        expect(opState.event_id).toBe(c!.eventId);
        expect(opState.revision).toBeGreaterThan(before.revision);
        expect(opState.state).toBe(action === 'PAUSE' ? 'PAUSED' : action === 'FREEZE' ? 'FROZEN' : 'EDITABLE');
        return opState;
    };
    try {
        await step('configuration', async () => {
            for (const p of ['./mural.spec.ts', './mural-helpers.ts', '../../../contracts/domain/v1/mural.schema.json', '../../../contracts/domain/v1/operations.json', '../../../contracts/generated/mcr-domain.openapi.json', '../../../contracts/realtime/v1/world.schema.json'])
                report.sourceHashes[p] = hash(await readFile(new URL(p, import.meta.url)));
            c = await loadConfig();
            await loadContracts(c);
            report.candidate = c.releaseRevision;
            report.eventId = c.eventId;
            report.environmentId = c.environmentId;
            report.restoreSnapshotId = c.restoreSnapshotId;
        });
        await step('deployment', async () => {
            const port = runtimeQualification();
            need(port, 'REVIEWED_LIVE_DEPLOYMENT_QUALIFICATION_PORT_REQUIRED');
            const proof = await port.inspect(c!);
            verifyProof(c!, proof);
            report.qualification=proof;
            const contracts = await loadContracts(c!);
            const own = contracts.operation(proof.ownResultOperation);
            need(own.operationId === 'getConcertMuralOwnResult' && own.method === 'get' && own.response === 'mural.OwnPixelResult', 'FROZEN_OWN_RESULT_READ_REQUIRED');
        });
        await step('independentActors', async () => {
            browser = await playwright.chromium.launch({ executablePath: process.env.MCR_CHROME_EXECUTABLE ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true });
            const decode = await loadDecoder(), contracts = await loadContracts(c!);
            for (const account of [...c!.members, c!.operator]) {
                const context = await browser.newContext({ viewport: { width: 390, height: 844 }, serviceWorkers: 'block' });
                contexts.push(context);
                const p = await context.newPage(), a = new Actor(p, context.request, account, c!, contracts);
                actors.push(a);
                if (actors.length <= 2) {
                    const o = new WorldObserver(decode);
                    observers.push(o);
                    o.attach(p, c!.origin);
                }
                await a.login();
            }
            expect(contexts[0]).not.toBe(contexts[1]);
            opState = await operator().admin();
            for (let i = 0; i < 2; i++) {
                const denied = await member(i).call('getConcertAdminMural');
                expect(denied.status).toBe(403);
                expect(reason(denied.value)).toBe('PERMISSION_DENIED');
            }
        });
        await step('baseline', async () => {
            baseline = await fresh();
            need(baseline.state === 'EDITABLE' && baseline.snapshot_id === c!.restoreSnapshotId && opState!.locks.length === 0, 'OWNED_UNLOCKED_EDITABLE_NAMED_BASELINE_REQUIRED');
            need(baseline.cooldown_ms >= 5000 && baseline.cooldown_ms <= c!.maxCooldownMs, 'NONZERO_BOUNDED_SERVER_COOLDOWN_REQUIRED');
            for (let i = 0; i < 2; i++) {
                await expect(page(i).getByRole('button', { name: 'Confirm pixel', exact: true })).toBeVisible();
                await expect.poll(() => observers[i]!.baseline?.selfUserId === c!.members[i]!.userId && observers[i]!.baseline?.eventId === c!.eventId).toBe(true);
                await expect(page(i).locator('[data-world-host] canvas')).toHaveCount(1);
                expect(observers[i]!.peak).toBe(1);
                expect(observers[i]!.invalidFrames).toBe(0);
            }
            await converge();
        });
        await step('confirmedPlacement', async () => {
            const s = await ready(0), body = pixelBody(s, 1, 1), path = member(0).contracts.path('placeConcertPixel', c!.eventId);
            await selectPixel(page(0), body);
            const response = page(0).waitForResponse(r => new URL(r.url()).pathname === path && r.request().method() === 'POST');
            await page(0).getByRole('button', { name: 'Confirm pixel', exact: true }).click();
            const r = await response;
            expect(r.status()).toBe(200);
            const p = member(0).contracts.decode<Pixel>('Domain_mural_PixelOperation', await r.json());
            bindPixel(p, body, c!.eventId);
            ownPixels.set(0, p);
            expect(r.request().postDataJSON()).toEqual(body);
            const key = r.request().headers()['idempotency-key']!;
            report.browserPlacements.push({ actor: c!.members[0].userId, key, body, outcome: 'CONFIRMED_ORIGINAL_RESPONSE' });
            await expect(page(0).getByRole('status', { name: 'Wall command status', exact: true })).toContainText('Server confirmed the pixel');
            const converged = await converge();
            expect(converged.revision).toBe(p.revision);
            expect(converged.pixels[129]).toBe(body.color_index);
        });
        await step('parallelCooldown', async () => {
            const s = await fresh(), body = pixelBody(s, 2, 1);
            expect(s.next_allowed_at).not.toBeNull();
            expect(Date.parse(s.next_allowed_at!)).toBeGreaterThan(Date.parse(s.server_time));
            await Promise.all([reject(0, body, 'MURAL_COOLDOWN'), reject(0, { ...body, x: 3 }, 'MURAL_COOLDOWN')]);
            expect((await fresh()).revision).toBe(s.revision);
        });
        await step('differentActorConvergence', async () => { const s = await ready(1), p = await checkPixel(1, pixelBody(s, 4, 1)); expect((await converge()).revision).toBe(p.revision); });
        // rejectedPreview is recorded only after the real region-lock UI rejection below.
        await step('ownResultRecovery', async () => {
            const s = await ready(0), body = pixelBody(s, 7, 1), path = member(0).contracts.path('placeConcertPixel', c!.eventId);
            let original: Pixel | undefined, key = '', postCount = 0;
            const onRequest = (r: import('@playwright/test').Request) => { if (new URL(r.url()).pathname === path && r.method() === 'POST')
                postCount++; };
            page(0).on('request', onRequest);
            await page(0).route(`${c!.origin}${path}`, async (route) => {
                key = route.request().headers()['idempotency-key']!;
                expect(route.request().postDataJSON()).toEqual(body);
                report.browserPlacements.push({ actor: c!.members[0].userId, key, body, outcome: 'UNKNOWN_TO_BROWSER' });
                const real = await route.fetch({ maxRetries: 0, timeout: 15000 });
                expect(real.status()).toBe(200);
                original = member(0).contracts.decode<Pixel>('Domain_mural_PixelOperation', await real.json());
                bindPixel(original, body, c!.eventId);
                await route.abort('failed'); // Drop only the actual server response. No fixture body or mutation replay.
            }, { times: 1 });
            try {
                await selectPixel(page(0), body);
                await page(0).getByRole('button', { name: 'Confirm pixel', exact: true }).click();
                await expect(page(0).getByRole('status', { name: 'Wall command status', exact: true })).toContainText(/Outcome unknown|Command pending/);
                expect(await page(0).getByRole('button', { name: 'Confirm pixel', exact: true }).isEnabled()).toBe(false);
                const status = await member(0).call('getConcertCommandStatus', undefined, {}, { operation_id: 'placeConcertPixel', idempotency_key: key });
                expect(status.status).toBe(200);
                const receipt = member(0).contracts.decode<Receipt>('Domain_common_CommandReceipt', status.value);
                expect(receipt.state).toBe('SUCCEEDED');
                expect(receipt.result_status).toBe(200);
                expect(receipt.failure_reason).toBeNull();
                const own = await member(0).call('getConcertMuralOwnResult', undefined, { key });
                expect(own.status).toBe(200);
                const envelope = member(0).contracts.decode<OwnPixelResult>('Domain_mural_OwnPixelResult', own.value);
                const serverTime = (await member(0).snapshot()).server_time;
                const p = bindOwnResult(envelope, receipt, body, c!.members[0].userId, c!.eventId, key, serverTime);
                expect(p).toEqual(original);
                report.recoveries.push({receipt,envelope});
                ownPixels.set(0, p);
                const foreign = await member(1).call('getConcertMuralOwnResult', undefined, { key });
                expect(foreign.status).toBe(404);
                expect(reason(foreign.value)).toBe('RESOURCE_UNAVAILABLE');
                await page(0).getByRole('button', { name: 'Check original command', exact: true }).click();
                await expect(page(0).getByRole('status', { name: 'Wall command status', exact: true })).toContainText('Server confirmed the pixel');
                expect(postCount).toBe(1);
                report.browserPlacements.at(-1)!.outcome = 'CONFIRMED_EXACT_OWN_RESULT';
                await converge();
            }
            finally {
                page(0).off('request', onRequest);
                await page(0).unroute(`${c!.origin}${path}`);
            }
        });
        await step('privacy', async () => {
            const d = await member(1).delta(baseline!.revision);
            for (const p of d.operations) {
                expect(Object.hasOwn(p, 'next_allowed_at')).toBe(false);
                expect(Object.hasOwn(p, 'actor_id')).toBe(false);
                expect(Object.hasOwn(p, 'user_id')).toBe(false);
            }
            // Snapshot cooldown is caller-only: compare each to its own recent confirmed placement, never peer metadata.
            for (let i = 0; i < 2; i++) {
                const s = await member(i).snapshot(), own = ownPixels.get(i)!;
                expect(s.next_allowed_at === own.next_allowed_at || s.next_allowed_at === null && Date.parse(s.server_time) >= Date.parse(own.next_allowed_at)).toBe(true);
            }
        });
        await step('unauthorizedModeration', async () => {
            const before = await operator().admin();
            for (const [operation, body] of [
                ['controlConcertMural', { expected_revision: before.revision, action: 'PAUSE', reason: 'IN04 denial probe' }],
                ['lockConcertMuralRegion', { expected_revision: before.revision, x_min: 0, y_min: 0, x_max_exclusive: 1, y_max_exclusive: 1, locked: true, reason: 'IN04 denial probe' }],
                ['restoreConcertMural', { expected_revision: before.revision, snapshot_id: c!.restoreSnapshotId, reason: 'IN04 denial probe' }],
            ] as const) {
                const r = await member(0).call(operation, body);
                expect(r.status).toBe(403);
                expect(reason(r.value)).toBe('PERMISSION_DENIED');
            }
            expect(await operator().admin()).toEqual(before);
        });
        await step('pause', async () => { await control('PAUSE'); const s = await fresh(); await reject(0, pixelBody(s, 8, 1), 'MURAL_PAUSED'); expect((await fresh()).revision).toBe(s.revision); await converge(); });
        await step('regionLock', async () => {
            await control('RESUME');
            await ready(0);
            const state = await operator().admin(), r = await operator().call('lockConcertMuralRegion', { expected_revision: state.revision, x_min: 10, y_min: 10, x_max_exclusive: 11, y_max_exclusive: 11, locked: true, reason: 'IN04 isolated region' });
            expect(r.status).toBe(200);
            const s = await converge(), body = pixelBody(s, 10, 10);
            await selectPixel(page(0), body);
            const response = page(0).waitForResponse(v => v.request().method() === 'POST' && new URL(v.url()).pathname.endsWith('/mural/pixels'));
            await page(0).getByRole('button', { name: 'Confirm pixel', exact: true }).click();
            const denied = await response;
            expect(denied.request().postDataJSON()).toEqual(body);
            expect(denied.status()).toBe(409);
            expect(reason(await denied.json())).toBe('MURAL_REGION_LOCKED');
            await expect(page(0).getByLabel('Selection preview', { exact: true })).toContainText('No pixel selected.');
            const unchanged = await fresh();
            expect(unchanged.pixels[1290]).toBe(s.pixels[1290]);
            await converge();
            checks.rejectedPreview = { status: 'PASS' };
        });
        await step('resume', async () => {
            const state = await operator().admin(), r = await operator().call('lockConcertMuralRegion', { expected_revision: state.revision, x_min: 10, y_min: 10, x_max_exclusive: 11, y_max_exclusive: 11, locked: false, reason: 'IN04 isolated region unlock' });
            expect(r.status).toBe(200);
            const unlocked=operator().contracts.decode<Admin>('Domain_mural_MuralAdminState',r.value);
            expect(unlocked.event_id).toBe(c!.eventId);
            expect(unlocked.state).toBe('EDITABLE');
            expect(unlocked.locks.some(b=>b.x_min<=10 && b.x_max_exclusive>10 && b.y_min<=10 && b.y_max_exclusive>10)).toBe(false);
            await checkPixel(0, pixelBody(await ready(0), 10, 10));
            await converge();
        });
        await step('restore', async () => {
            const before = await fresh(), state = await operator().admin();
            const r = await operator().call('restoreConcertMural', { expected_revision: state.revision, snapshot_id: c!.restoreSnapshotId, reason: 'IN04 restore named fixture baseline' });
            expect(r.status).toBe(200);
            const restored = operator().contracts.decode<Admin>('Domain_mural_MuralAdminState', r.value);
            expect(restored.revision).toBeGreaterThan(before.revision);
            expect(restored.snapshot_id).not.toBe(before.snapshot_id);
            const s = await converge();
            expect(s.revision).toBe(restored.revision);
            expect(s.pixels).toEqual(baseline!.pixels);
        });
        await step('obsoleteDelta', async () => {
            // Delay bytes from a real operation-containing response, never construct a fake delta.
            const path = member(1).contracts.path('getConcertMuralDelta', c!.eventId);
            let observed = false, delayedRevision = 0;
            const oldPixel = pixelBody(await ready(0), 11, 10);
            await page(1).route(`${c!.origin}${path}*`, async (route) => {
                const response = await route.fetch({ maxRetries: 0, timeout: 15000 });
                expect(response.status()).toBe(200);
                const delta = member(1).contracts.decode<import('./mural-helpers').Delta>('Domain_mural_MuralDelta', await response.json());
                if (observed || !delta.operations.some(p => p.revision === oldPixel.expected_revision + 1 && p.x === oldPixel.x && p.y === oldPixel.y && p.color_index === oldPixel.color_index)) {
                    await route.fulfill({ response });
                    return;
                }
                held = route;
                delayedRevision = delta.to_revision;
                const raw = await response.body();
                releaseHeld = async () => { await route.fulfill({ response, body: raw }); held = undefined; };
                observed = true;
            });
            try {
                await checkPixel(0, oldPixel);
                await expect.poll(() => observed, { timeout: 15000 }).toBe(true);
                const before = await fresh(), state = await operator().admin();
                const r = await operator().call('restoreConcertMural', { expected_revision: state.revision, snapshot_id: c!.restoreSnapshotId, reason: 'IN04 obsolete-delta reset barrier' });
                expect(r.status).toBe(200);
                const restored = await fresh();
                expect(restored.revision).toBeGreaterThan(before.revision);
                expect(restored.revision).toBeGreaterThan(delayedRevision);
                const refresh = page(1).getByRole('button', { name: 'Refresh confirmed wall', exact: true });
                need(await refresh.count() === 1, 'REVIEWED_EXPLICIT_WALL_REFRESH_SEAM_REQUIRED');
                await refresh.click();
                await expect.poll(async () => { try {
                    await rendered(page(1), restored);
                    return true;
                }
                catch {
                    return false;
                } }, { timeout: 15000 }).toBe(true);
                // Do not claim obsolete delivery if the pass-through response cannot be released.
                await releaseHeld!();
                await page(1).waitForTimeout(1000);
                await converge();
                expect((await fresh()).pixels).toEqual(baseline!.pixels);
            }
            finally {
                if (held)
                    await held.abort().catch(() => { });
                held = undefined;
                await page(1).unroute(`${c!.origin}${path}*`);
            }
        });
        await step('slowReconnect', async () => {
            const opens = observers[1]!.opens, stream = observers[1]!.baseline!.streamId;
            await contexts[1]!.setOffline(true);
            try {
                await expect.poll(() => observers[1]!.active, { timeout: 15000 }).toBe(0);
                await checkPixel(0, pixelBody(await ready(0), 12, 10));
            }
            finally {
                await contexts[1]!.setOffline(false);
            }
            const reconnect = page(1).getByRole('button', { name: 'Reconnect world', exact: true });
            if (await reconnect.isVisible() && await reconnect.isEnabled())
                await reconnect.click();
            await expect.poll(() => observers[1]!.opens > opens && observers[1]!.active === 1, { timeout: 15000 }).toBe(true);
            await expect.poll(() => observers[1]!.baseline?.streamId !== stream && observers[1]!.baseline?.eventId === c!.eventId && observers[1]!.baseline?.selfUserId === c!.members[1].userId).toBe(true);
            await converge();
            expect(observers[1]!.peak).toBe(1);
        });
        await step('restartEpoch', async () => {
            const port = runtimeQualification();
            need(port, 'REVIEWED_EXTERNAL_RESTART_OBSERVATION_REQUIRED');
            const before = observers[0]!.baseline, oldBindings = observers.map(o => o.baseline);
            need(before && oldBindings.every(Boolean), 'INCUMBENT_WORLD_EPOCH_REQUIRED');
            const restartWindowStart = Date.now();
            const proof = await port.observeRestart(c!, { roomEpoch: before.roomEpoch, streamId: before.streamId });
            need(proof.candidate === c!.releaseRevision && proof.environmentId === c!.environmentId, 'RESTART_CANDIDATE_ISOLATION_MISMATCH');
            need(Date.parse(proof.restartedAt) > restartWindowStart && Date.parse(proof.restartedAt) <= Date.now(), 'FRESH_EXTERNAL_RESTART_REQUIRED');
            await expect.poll(() => observers.every((o, i) => !!o.baseline && o.baseline.roomEpoch !== oldBindings[i]!.roomEpoch && o.baseline.streamId !== oldBindings[i]!.streamId && o.baseline.eventId === c!.eventId && o.baseline.selfUserId === c!.members[i]!.userId && o.active === 1), { timeout: c!.restartObservationTimeoutMs }).toBe(true);
            await converge();
            expect(observers.every(o => o.peak === 1 && o.invalidFrames === 0)).toBe(true);
            report.restart = 'OBSERVED_REAL_NEW_ROOM_EPOCH';
        });
        await step('finalFreeze', async () => {
            await control('FREEZE');
            const s = await converge();
            expect(s.state).toBe('FROZEN');
            await Promise.all([reject(0, pixelBody(s, 13, 10), 'MURAL_FROZEN'), reject(1, pixelBody(s, 14, 10), 'MURAL_FROZEN')]);
            expect((await fresh()).revision).toBe(s.revision);
            for (let i = 0; i < 2; i++)
                expect(await page(i).getByRole('button', { name: 'Confirm pixel', exact: true }).isEnabled()).toBe(false);
            report.finalState = 'FROZEN_OWNED_FIXTURE_EVENT';
        });
    }
    finally {
        await step('teardown', async () => {
            if (held)
                await held.abort().catch(() => { });
            const closed = await Promise.allSettled(contexts.map(context => context.close()));
            try {
                await browser?.close();
            }
            finally {
                actors.forEach(a => a.clear());
            }
            expect(closed.every(result => result.status === 'fulfilled')).toBe(true);
            if (!contexts.length)
                throw new NotRun('NO_REAL_CONTEXTS_ALLOCATED');
            expect(observers.every(o => o.active === 0)).toBe(true);
        });
        report.mutations = actors.flatMap(a => a.commands);
        report.status = resultOf(checks);
        const path = info.outputPath('in04-result.json');
        await writeFile(path, JSON.stringify(report, null, 2) + '\n', { mode: 0o600 });
        await info.attach('in04-result', { path, contentType: 'application/json' });
    }
    if (report.status !== 'PASS')
        throw new Error(`IN04 ${report.status}: ${Object.entries(checks).filter(([, v]) => v.status !== 'PASS').map(([k, v]) => `${k}=${v.code}`).join('; ')}`);
});
