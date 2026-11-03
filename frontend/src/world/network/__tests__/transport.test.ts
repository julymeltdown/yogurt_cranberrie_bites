import { describe, expect, it, vi } from 'vitest'
import { createMemorySession } from '../../../shared/api/http'
import { MockSocket, MockWorldClock, MockWorldTransport } from '../mock'
import { WebSocketWorldTransport } from '../websocket'
import { decode, validate, WorldTransportError } from '../transport/contract'
import type { ServerMessage, WorldTransport, WorldTransportOptions, WorldResources, WorldEvent, WorldTicketResponse } from '../WorldTransport'
import movement from '../../../../../contracts/fixtures/realtime/movement-heartbeat.json'
import ticketResponse from '../../../../../contracts/fixtures/realtime/ticket-response.json'
const catalog = import.meta.glob('../../../../../contracts/fixtures/realtime/*.json', { eager: true, import: 'default' }) as Record<string, unknown>
const auth = movement.messages[0]!.message
const initial = movement.messages[1]!.message
export const resources: WorldResources = { eventId: auth.eventId!, roomId: auth.roomId!, mapId: auth.mapId!, mapRevision: auth.mapRevision!, avatarManifestId: auth.avatarManifestId!, avatarManifestVersion: auth.avatarManifestVersion! }
const user = movement.actor_id
const flush = async () => { for (let i=0;i<12;i++) await Promise.resolve(); await new Promise(resolve => setTimeout(resolve,5)) }
function setup(lane: 'mock' | 'native', patch: Partial<WorldTransportOptions> = {}) {
  const session = createMemorySession(); session.replace(0,{ userId: user, eventId: resources.eventId, accessToken: 'memory-only-secret' })
  const clock = new MockWorldClock(Date.parse(ticketResponse.server_time))
  const sockets: MockSocket[] = [], urls: string[] = [], events: WorldEvent[] = []
  let issued = 0
  const acquireTicket = vi.fn(async ({ body }: Parameters<WorldTransportOptions['acquireTicket']>[0]) => ({ ...ticketResponse, ...body, ticket: String.fromCharCode(65+issued++).repeat(43) }) as WorldTicketResponse)
  const options = { session, scheduler: clock, origin: 'https://mcr.example', acquireTicket, ...patch }
  const transport = lane === 'mock' ? new MockWorldTransport(options) : new WebSocketWorldTransport(options, url => { urls.push(url); const s = new MockSocket(); sockets.push(s); return s })
  const all = () => lane === 'mock' ? (transport as MockWorldTransport).sockets : sockets
  transport.subscribe(event => events.push(event))
  return { session, clock, transport, all, urls, events, acquireTicket, async opening() { const connected = transport.connect(resources); await flush(); return { connected, socket: all().at(-1)! } }, async live() { const { connected,socket } = await this.opening(); socket.open(); socket.receive(JSON.stringify(initial)); await connected; return socket } }
}
function send(t: WorldTransport, frame: Record<string,unknown>) {
  if (frame.type === 'input') t.sendInput(frame as never)
  else if (frame.type === 'resync') t.resync(frame.requestId as string,frame.reason as never)
  else if (frame.type === 'switchRoom') return t.roomSwitch(frame.roomId as string,frame.requestId as string)
  else if (frame.type === 'pong') t.clientPong(frame.heartbeatId as string)
}
for (const lane of ['mock','native'] as const) describe(`${lane} common lifecycle/fixtures`, () => {
  for (const [path,data] of Object.entries(catalog)) {
    const fixture = data as { actor_id?: string; messages?: { direction: string; message: Record<string,unknown> }[]; kind?: string; payload?: unknown; direction?: 'client'|'server'; binary?: boolean; close_code?: number }
    if (fixture.messages) it(`canonical trace ${path.split('/').at(-1)}`, async () => {
      const s = setup(lane), { connected,socket } = await s.opening(); socket.open()
      for (const row of fixture.messages!.slice(1)) {
        if (row.direction === 'server') socket.receive(JSON.stringify(row.message))
        else { await send(s.transport,row.message); await flush() }
      }
      await connected
      expect(s.events.filter(e => e.type === 'error')).toEqual(path.endsWith('/revoke.json') ? [{ type: 'error',kind:'closed',code:4401 }] : [])
      s.transport.dispose(); expect(s.clock.pending).toBe(0); expect([...socket.listeners.values()].every(x => x.size === 0)).toBe(true)
    })
    if (fixture.payload !== undefined) it(`canonical wire rejection ${path.split('/').at(-1)}`, async () => {
      expect(() => decode(fixture.binary ? new ArrayBuffer(0) : fixture.payload,fixture.direction!)).toThrow(WorldTransportError)
      const s = setup(lane), socket = await s.live()
      socket.receive(fixture.binary ? new ArrayBuffer(0) : fixture.payload)
      expect(s.transport.state).toBe('closed'); expect(s.clock.pending).toBe(0)
    })
  }
  it('requires a fresh ticket and only explicit reconnection', async () => {
    const s = setup(lane), old = await s.live(); old.serverClose(); await flush()
    expect(s.acquireTicket).toHaveBeenCalledTimes(1)
    const next = await s.live(); expect(s.acquireTicket).toHaveBeenCalledTimes(2)
    expect(JSON.parse(old.sent[0]!).ticket).not.toBe(JSON.parse(next.sent[0]!).ticket)
    expect(s.urls.every(u => u === 'wss://mcr.example/world-ws')).toBe(true)
    expect(s.events.some(e => JSON.stringify(e).includes('memory-only-secret'))).toBe(false)
    s.transport.dispose()
  })
  it('rejects second connect and gates sends before admission/after dispose', async () => {
    const s = setup(lane), { connected,socket } = await s.opening()
    await expect(s.transport.connect(resources)).rejects.toThrow()
    expect(() => s.transport.sendInput({seq:1,axisX:0,axisY:0,buttons:[]})).toThrow()
    socket.open(); expect(() => s.transport.resync(user,'GAP')).toThrow()
    s.transport.dispose(); await expect(connected).rejects.toThrow(); await expect(s.transport.connect(resources)).rejects.toThrow('disposed')
    expect(s.clock.pending).toBe(0)
  })
  for (const stage of ['ticket','opening','active'] as const) it(`cancels ${stage} and ignores late generation`, async () => {
    let release: (ticket: WorldTicketResponse) => void = () => {}
    const deferred = new Promise<WorldTicketResponse>(resolve => { release = resolve })
    const s = setup(lane, stage === 'ticket' ? { acquireTicket: () => deferred } : {})
    const abort = new AbortController(), connected = s.transport.connect(resources,abort.signal)
    const rejection = stage === 'active' ? undefined : expect(connected).rejects.toThrow()
    await flush(); const old = s.all()[0]
    if (stage === 'active') { old!.open(); old!.receive(JSON.stringify(initial)); await connected }
    const callbacks = [...old?.listeners.get('message') ?? []]
    abort.abort(); release(ticketResponse as WorldTicketResponse); await flush()
    if (stage !== 'active') await rejection
    expect(s.transport.state).toBe('closed'); expect(s.clock.pending).toBe(0)
    expect(old ? [...old.listeners.values()].every(x => x.size === 0) : true).toBe(true)
    const fresh = await s.opening(); fresh.socket.open(); fresh.socket.receive(JSON.stringify(initial)); await fresh.connected
    callbacks.forEach(fn => fn({data: JSON.stringify({...initial,eventId:user})}))
    expect(s.transport.state).toBe('live'); s.transport.dispose()
  })
  it('session generation change closes immediately and fences pending ticket', async () => {
    const s = setup(lane), socket = await s.live(); s.session.replace(s.session.read().generation,null)
    expect(s.transport.state).toBe('closed'); expect(s.clock.pending).toBe(0); expect(socket.closeCount).toBe(1)
  })
  for (const stage of ['ticket','opening','authenticate'] as const) it(`bounded ${stage} handshake`, async () => {
    const s = setup(lane, stage === 'ticket' ? {acquireTicket: () => new Promise(() => {})} : {})
    const connected = s.transport.connect(resources), rejection = expect(connected).rejects.toThrow('timeout')
    await flush(); if (stage === 'authenticate') s.all()[0]!.open()
    s.clock.advance(3000); await rejection; expect(s.transport.state).toBe('closed'); expect(s.clock.pending).toBe(0)
  })
  for (const field of ['protocolVersion','eventId','roomId','mapId','mapRevision','avatarManifestId','avatarManifestVersion','selfUserId','roomEpoch'] as const) it(`fails closed on wrong ${field}`, async () => {
    const s = setup(lane), { connected,socket } = await s.opening(), rejection = field === 'roomEpoch' ? undefined : expect(connected).rejects.toThrow()
    socket.open()
    if (field === 'roomEpoch') { socket.receive(JSON.stringify(initial)); await connected; socket.receive(JSON.stringify({...movement.messages[3]!.message,roomEpoch:user})) }
    else socket.receive(JSON.stringify({...initial,[field]: typeof initial[field] === 'number' ? 9 : field === 'selfUserId' ? resources.eventId : 'wrong'}))
    if (field !== 'roomEpoch') await rejection
    if(field === 'roomEpoch') { expect(s.transport.state).toBe('resync'); s.transport.dispose() }
    else expect(s.transport.state).toBe('closed')
    expect(s.clock.pending).toBe(0)
  })
  it('rejects malformed, oversized, binary, deeply nested and invalid Unicode', async () => {
    for (const payload of ['{', '{}'.repeat(40000), new ArrayBuffer(0), '\ud800', '[ '.repeat(18)+'0'+']'.repeat(18), '{"type":"ping","type":"ping"}', '{"n":1e999}']) {
      const s = setup(lane), socket = await s.live(); socket.receive(payload); expect(s.transport.state).toBe('closed'); expect(s.clock.pending).toBe(0)
    }
  })
  it('rejects duplicates, gaps, future acknowledgements and regressed tick', async () => {
    for (const patch of [{ackInputSeq:1},{serverTick:-1}]) {
      const s = setup(lane), socket = await s.live(); socket.receive(JSON.stringify({...movement.messages[3]!.message,ackInputSeq:0,...patch})); expect(s.transport.state).toBe('closed')
    }
    const s = setup(lane), socket = await s.live(); const delta = {...movement.messages[3]!.message,ackInputSeq:0}
    socket.receive(JSON.stringify(delta)); socket.receive(JSON.stringify(delta)); expect(s.transport.state).toBe('resync');s.transport.dispose()
  })
  it('suspends input and discards queued deltas until correlated resync', async () => {
    const s = setup(lane), socket = await s.live(); s.transport.resync(user,'GAP')
    expect(() => s.transport.sendInput({seq:1,axisX:0,axisY:0,buttons:[]})).toThrow()
    socket.receive(JSON.stringify({...movement.messages[3]!.message,streamSeq:99,ackInputSeq:99})); expect(s.transport.state).toBe('resync')
    socket.receive(JSON.stringify({...initial,streamId:user,cause:'RESYNC',requestId:user})); expect(s.transport.state).toBe('live')
    s.transport.sendInput({seq:1,axisX:0,axisY:0,buttons:[]}); s.transport.dispose()
  })
  it('ping/pong matching, missing ping and missing pong deadlines', async () => {
    const s = setup(lane), socket = await s.live(); const ping = movement.messages[4]!.message
    expect(() => s.transport.clientPong(user)).toThrow()
    socket.receive(JSON.stringify(ping)); s.transport.clientPong(ping.heartbeatId!); expect(s.transport.state).toBe('live')
    socket.receive(JSON.stringify(ping)); s.clock.advance(10000); expect(s.transport.state).toBe('closed'); expect(s.clock.pending).toBe(0)
    const fresh = setup(lane); await fresh.live(); fresh.clock.advance(20000); expect(fresh.transport.state).toBe('closed')
  })
  it('terminal reset/revoke clears references, listeners and timers', async () => {
    for (const reason of ['MAP_VERSION_MISMATCH','BACKPRESSURE']) {
      const s = setup(lane), socket = await s.live(); socket.receive(JSON.stringify({ protocolVersion:'1.0.0',type:'resyncRequired',streamId: initial.streamId,reason }))
      expect(s.transport.state).toBe('closed'); expect(s.clock.pending).toBe(0); expect([...socket.listeners.values()].every(x => x.size === 0)).toBe(true)
    }
  })
  it('bounds send buffers and rate, never replays old input', async () => {
    const s = setup(lane), socket = await s.live(); socket.bufferedAmount = 262144
    expect(() => s.transport.sendInput({seq:1,axisX:0,axisY:0,buttons:[]})).toThrow('backpressure')
    expect(s.transport.state).toBe('closed'); const next = await s.live(); expect(next.sent).toHaveLength(1)
    for(let seq=1;seq<=60;seq++) s.transport.sendInput({seq,axisX:0,axisY:0,buttons:[]})
    expect(() => s.transport.sendInput({seq:61,axisX:0,axisY:0,buttons:[]})).toThrow('rate-limited')
    s.clock.advance(1000); s.transport.sendInput({seq:61,axisX:0,axisY:0,buttons:[]}); s.transport.dispose()
  })
  for(const patch of [{streamSeq:2},{roomEpoch:user},{streamId:user}]) it(`recovers gap/epoch ${JSON.stringify(patch)} with one notification`, async () => {
    const s=setup(lane),socket=await s.live()
    socket.receive(JSON.stringify({...movement.messages[3]!.message,ackInputSeq:0,...patch}))
    expect(s.transport.state).toBe('resync')
    expect(()=>s.transport.sendInput({seq:1,axisX:0,axisY:0,buttons:[]})).toThrow()
    const notifications=s.events.filter(e=>e.type==='resync-needed');expect(notifications).toHaveLength(1)
    socket.receive(JSON.stringify({...movement.messages[3]!.message,streamSeq:999,ackInputSeq:999}))
    expect(s.events.filter(e=>e.type==='resync-needed')).toHaveLength(1)
    s.transport.resync(user,'GAP');expect(JSON.parse(socket.sent.at(-1)!).lastStreamSeq).toBe(0)
    socket.receive(JSON.stringify({...initial,streamId:user,cause:'RESYNC',requestId:user}))
    expect(s.transport.state).toBe('live');s.transport.sendInput({seq:1,axisX:0,axisY:0,buttons:[]})
    s.transport.dispose();expect(s.clock.pending).toBe(0)
  })
  it('bounds recovery with no consumer response and rejects invalid wire during resync', async () => {
    const s=setup(lane),socket=await s.live()
    socket.receive(JSON.stringify({...movement.messages[3]!.message,streamSeq:2,ackInputSeq:0}))
    s.clock.advance(3000);expect(s.transport.state).toBe('closed');expect(s.clock.pending).toBe(0)
    const other=setup(lane),active=await other.live();other.transport.resync(user,'GAP');active.receive('{')
    expect(other.transport.state).toBe('closed');expect(other.clock.pending).toBe(0)
  })
  it('bounds the native buffer frame count even with small queued bytes', async () => {
    const s=setup(lane),socket=await s.live();socket.bufferedAmount=1
    for(let seq=1;seq<=63;seq++){s.clock.advance(50);s.transport.sendInput({seq,axisX:0,axisY:0,buttons:[]})}
    expect(()=>s.transport.sendInput({seq:64,axisX:0,axisY:0,buttons:[]})).toThrow('backpressure')
    expect(s.transport.state).toBe('closed');expect(s.clock.pending).toBe(0)
  })
  it('ignores stale timer callbacks after reconnect', async () => {
    const callbacks:(()=>void)[]=[]
    const clock=new MockWorldClock(Date.parse(ticketResponse.server_time))
    const scheduler={now:()=>clock.now(),set:(delay:number,fn:()=>void)=>{callbacks.push(fn);return clock.set(delay,fn)},clear:(handle:unknown)=>clock.clear(handle)}
    const s=setup(lane,{scheduler}),old=await s.live();old.serverClose();const active=await s.live()
    callbacks.slice(0,3).forEach(fn=>fn());expect(s.transport.state).toBe('live')
    s.transport.dispose();expect(clock.pending).toBe(0);expect([...active.listeners.values()].every(x=>x.size===0)).toBe(true)
  })
  it('reports safe documented server close codes without copying close reasons', async () => {
    for(const code of [1000,4401,4403,4408,4409,4429,4500]) {
      const s=setup(lane),socket=await s.live();socket.serverClose(code)
      expect(s.events.at(-2)).toEqual({type:'error',kind:'closed',code});expect(s.clock.pending).toBe(0)
    }
  })
  it('rejects a reused ticket response on a new connection', async () => {
    const s = setup(lane,{acquireTicket: async () => ticketResponse as WorldTicketResponse})
    await s.live(); s.transport.disconnect()
    await expect(s.transport.connect(resources)).rejects.toThrow('invalid-frame')
    expect(s.all()).toHaveLength(1); expect(s.clock.pending).toBe(0)
  })
  it('rejects wrong ticket binding, TTL, path and failed acquisition without leaking errors', async () => {
    for (const patch of [{event_id:user},{room_id:user},{map_revision:2},{protocol_version:'2.0.0'},{expires_at:'2026-11-07T10:00:31Z'},{ws_path:'/world-ws?ticket=secret'}]) {
      const s = setup(lane,{acquireTicket: async () => ({...ticketResponse,...patch}) as WorldTicketResponse})
      await expect(s.transport.connect(resources)).rejects.toThrow(); expect(s.all()).toHaveLength(0); expect(s.clock.pending).toBe(0)
    }
    const s = setup(lane,{acquireTicket: async () => { throw Error('SECRET-TICKET') }})
    await expect(s.transport.connect(resources)).rejects.toThrow('unavailable'); expect(JSON.stringify(s.events)).not.toContain('SECRET-TICKET')
  })
  it('fails closed on incompatible native subprotocol and socket errors', async () => {
    for (const fail of ['protocol','error']) {
      const s = setup(lane), {connected,socket} = await s.opening(), rejected = expect(connected).rejects.toThrow()
      if(fail==='protocol') {socket.protocol='unapproved';socket.open()} else socket.fail()
      await rejected; expect(s.clock.pending).toBe(0)
    }
  })
  it('rejects duplicate roster, omitted self, invalid manifest, unsafe numeric and correlation frames', async () => {
    const self = initial.players![0]!
    for (const patch of [
      {players:[self,self]}, {players:[initial.players![1]]}, {ackInputSeq:1}, {streamSeq:1},
      {players:[{...self,avatar:{...self.avatar,config:{...self.avatar.config,manifest_id:'wrong'}}}]},
      {requestId:user}, {serverTick:9007199254740992}, {accessExpiresAt:'2026-02-30T12:00:00Z'},
    ]) {
      const s = setup(lane), {connected,socket}=await s.opening(), rejected=expect(connected).rejects.toThrow()
      socket.open();socket.receive(JSON.stringify({...initial,...patch}));await rejected;expect(s.clock.pending).toBe(0)
    }
  })
  it('validates atomic delta removals, manifests, overlapping and duplicate IDs', async () => {
    const self = initial.players![0]!, ghost = initial.players![1]!
    for(const patch of [{removes:[self.userId]}, {removes:[user+'0']}, {removes:[ghost.userId],upserts:[ghost]},
      {upserts:[self,self]}, {upserts:[{...self,avatar:{...self.avatar,config:{...self.avatar.config,manifest_version:2}}}]}]) {
      const s=setup(lane),socket=await s.live()
      socket.receive(JSON.stringify({...movement.messages[3]!.message,ackInputSeq:0,...patch}));expect(s.transport.state).toBe('closed');expect(s.clock.pending).toBe(0)
    }
  })
  it('preserves captured access deadline and enforces expiry', async () => {
    const s=setup(lane), {connected,socket}=await s.opening();socket.open()
    socket.receive(JSON.stringify({...initial,accessExpiresAt:'2026-11-07T10:00:01Z'}));await connected
    s.clock.advance(1000);expect(s.transport.state).toBe('closed');expect(s.clock.pending).toBe(0)
    const other=setup(lane),active=await other.live()
    active.receive(JSON.stringify({...initial,streamId:user,roomEpoch:user,cause:'EPOCH_CHANGED',accessExpiresAt:'2026-11-07T12:01:00Z'}));expect(other.transport.state).toBe('closed')
  })
  it('bounds control timeouts, reused requests and resync spacing', async () => {
    const s=setup(lane),socket=await s.live();s.transport.resync(user,'GAP')
    socket.receive(JSON.stringify({...initial,streamId:user,cause:'RESYNC',requestId:user}))
    expect(()=>s.transport.resync(resources.eventId,'GAP')).toThrow();s.clock.advance(1000)
    expect(()=>s.transport.resync(user,'GAP')).toThrow();s.transport.resync(resources.eventId,'GAP')
    s.clock.advance(3000);expect(s.transport.state).toBe('closed');expect(s.clock.pending).toBe(0)
  })
  it('fences reentrant disconnect at ticket/opening/live notifications', async () => {
    for(const stage of ['ticket','opening','live']) {
      const s=setup(lane)
      s.transport.subscribe(e=>{if(e.type==='state'&&e.state===stage)s.transport.disconnect()})
      const connected=s.transport.connect(resources); const settled=connected.catch(()=>{})
      await flush();if(stage==='live'){s.all()[0]!.open();s.all()[0]!.receive(JSON.stringify(initial))}
      await settled;expect(s.transport.state).toBe('closed');expect(s.clock.pending).toBe(0)
    }
  })
  it('cancels an outstanding room-ticket request without a late switch send', async () => {
    const s=setup(lane),socket=await s.live()
    let finish:(value:WorldTicketResponse)=>void=()=>{}
    s.acquireTicket.mockImplementationOnce(()=>new Promise(resolve=>{finish=resolve}))
    const switching=s.transport.roomSwitch(user,resources.eventId), rejected=expect(switching).rejects.toThrow()
    s.transport.disconnect();finish({...ticketResponse,room_id:user,ticket:'Z'.repeat(43)} as WorldTicketResponse);await rejected;await flush()
    expect(socket.sent).toHaveLength(1);expect(s.clock.pending).toBe(0)
  })
  it('rejects an unsolicited switch result while ticket acquisition is pending', async () => {
    const s=setup(lane),socket=await s.live()
    s.acquireTicket.mockImplementationOnce(()=>new Promise(()=>{}))
    const switching=s.transport.roomSwitch(user,resources.eventId), rejected=expect(switching).rejects.toThrow()
    socket.receive(JSON.stringify({protocolVersion:'1.0.0',type:'switchRejected',streamId:initial.streamId,requestId:resources.eventId,reason:'ROOM_FULL'}))
    await rejected;expect(s.transport.state).toBe('closed');expect(s.clock.pending).toBe(0)
  })
  it('accepts an unsolicited epoch reset and disposes it cleanly', async () => {
    const s = setup(lane), socket = await s.live(); socket.receive(JSON.stringify({...initial,streamId:user,cause:'EPOCH_CHANGED',roomEpoch:user}))
    expect(s.transport.state).toBe('live'); s.transport.dispose(); expect(s.clock.pending).toBe(0)
  })
})
describe('canonical schema checks', () => {
  it('validates every fixture frame and projection through canonical schemas', () => {
    for (const data of Object.values(catalog)) {
      const fixture = data as { messages?: {direction:'client'|'server';message:unknown}[]; viewers?: {initial:{direction:'client'|'server';message:unknown}[]}[]; expected?: Record<string,ServerMessage[]> }
      for (const row of fixture.messages ?? fixture.viewers?.flatMap(v=>v.initial) ?? []) expect(() => decode(JSON.stringify(row.message),row.direction)).not.toThrow()
      for (const frame of Object.values(fixture.expected ?? {}).flat()) expect(() => validate('ServerMessage',frame)).not.toThrow()
    }
  })
  it('checks HTTP ticket TTL and schema', () => {
    expect(() => validate('WorldTicketResponse',{...ticketResponse,expires_at:'2026-11-07T10:00:31Z'})).toThrow()
    expect(() => validate('WorldTicketResponse',{...ticketResponse,ws_path:'/world-ws?ticket=secret'})).toThrow()
  })
  for (const origin of ['https://mcr.example/','https://mcr.example?ticket=x','https://user:pass@mcr.example','http://mcr.example','wss://mcr.example','null']) it(`rejects unsafe origin ${origin}`, () => {
    expect(() => setup('native',{origin})).toThrow()
  })
})
