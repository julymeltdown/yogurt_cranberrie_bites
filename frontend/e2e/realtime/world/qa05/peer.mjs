// TEST ONLY C05 fault authority. WS framing adapted from the published FE17
// replication-browser.mjs; this is not the Rust service or a capacity oracle.
import assert from 'node:assert/strict'
import {createHash, randomBytes} from 'node:crypto'

const uuid = n => `00000000-0000-4000-8000-${String(n).padStart(12,'0')}`
const wire = data => {
 const body = Buffer.from(JSON.stringify(data)), head = body.length < 126 ? Buffer.from([0x81,body.length]) : Buffer.from([0x81,126,body.length>>8,body.length&255])
 assert(body.length < 65536);return Buffer.concat([head,body])
}
export function faultAuthority() {
 const peers = new Map(), errors = [], connections = new Set();let current, serial = 8000
 const trace = (p,direction,m) => p.trace.push({direction,connection:p.connections,type:m.type,...Object.fromEntries(['streamId','roomEpoch','streamSeq','serverTick','ackInputSeq','seq','axisX','axisY','buttons','reason','requestId','roomId','cause'].filter(k=>k in m).map(k=>[k,m[k]]))})
 const send = (p,m) => { if(!p.socket||p.socket.destroyed)return;trace(p,'server',m);p.socket.write(wire(m)) }
 const delta = (p,upserts=[],removes=[]) => ({protocolVersion:'1.0.0',type:'delta',streamId:p.frame.streamId,roomEpoch:p.frame.roomEpoch,streamSeq:++p.seq,serverTick:++p.tick,ackInputSeq:p.ack,upserts,removes})
 function snapshot(p,cause='INITIAL',requestId=null,roomId=p.frame.roomId) {
  p.seq=0;p.ack=0;p.held=[];p.hold=false
  p.frame={...p.frame,streamId:uuid(++serial),streamSeq:0,cause,requestId,roomId,serverTick:p.tick,ackInputSeq:0,players:structuredClone(p.roster)}
  send(p,p.frame)
 }
 function freshRoster(p,count=2) {
  const self={...p.template.players[0],position:{x:472,y:440}},others=p.template.players.slice(1,count).map((row,i)=>({...row,userId:uuid(++serial),displayName:`Recovered peer ${serial}`,position:{x:504+i*24,y:456}}))
  p.roster=[self,...others]
 }
 function message(p,m) {
  trace(p,'client',m)
  if(m.type==='authenticate'||m.type==='switchRoom') {
   const claim=p.tickets.get(m.ticket)
   if(!claim||claim.used||claim.body.room_id!==m.roomId){p.denials++;p.socket.end(Buffer.from([0x88,2,0x11,0x31]));return}
   claim.used=true
   if(m.type==='authenticate') {
    p.authenticates++;p.ack=0;p.tick=0;p.frame={...p.template,accessExpiresAt:new Date(Date.now()+60000).toISOString(),roomEpoch:uuid(p.epoch),roomId:m.roomId}
    snapshot(p)
   } else if(p.rejectSwitch) {
    p.rejectSwitch=false;send(p,{protocolVersion:'1.0.0',type:'switchRejected',streamId:p.frame.streamId,requestId:m.requestId,reason:'ROOM_FULL'})
   } else snapshot(p,'ROOM_SWITCH',m.requestId,m.roomId)
  } else if(m.type==='input') {
   assert.equal(m.streamId,p.frame.streamId);assert.equal(m.roomEpoch,p.frame.roomEpoch)
   p.ack=m.seq
   const self=p.roster[0];self.moving=!!(m.axisX||m.axisY)
   const d=delta(p,[structuredClone(self)])
   if(p.hold){assert(p.held.length<64);p.held.push(d)}else send(p,d)
  } else if(m.type==='resync') {
   p.resyncs.push(m);p.pending=m
  } else assert.equal(m.type,'pong')
 }
 async function middleware(req,res,next) {
  const u=new URL(req.url,'http://localhost');if(!u.pathname.startsWith('/__qa05/'))return next()
  try {
   assert.equal(req.method,'POST');let bytes=0,parts=[]
   for await(const b of req){bytes+=b.length;assert(bytes<65536);parts.push(b)}
   const body=JSON.parse(Buffer.concat(parts).toString()),id=u.searchParams.get('peer')
   let p=peers.get(id)
   if(u.pathname==='/__qa05/register') {
    assert(body.snapshot.players.length>1)
    p={id,template:body.snapshot,frame:body.snapshot,roster:structuredClone(body.snapshot.players),tickets:new Map(),trace:[],resyncs:[],connections:0,active:0,peak:0,authenticates:0,denials:0,epoch:9000,seq:0,tick:0,ack:0,hold:false,held:[],pending:null,rejectSwitch:false,reuse:false,socket:null}
    peers.set(id,p);current=p;res.end('{}');return
   }
   assert(p);assert.equal(u.pathname,'/__qa05/ticket')
   let token
   if(p.reuse){p.reuse=false;token=[...p.tickets.keys()].at(-1)}else{token=randomBytes(32).toString('base64url');p.tickets.set(token,{body,used:false})}
   const now=Date.now()
   res.setHeader('Content-Type','application/json');res.setHeader('Cache-Control','no-store')
   res.end(JSON.stringify({...body,protocol_version:'1.0.0',event_id:p.template.eventId,ticket:token,server_time:new Date(now).toISOString(),expires_at:new Date(now+30000).toISOString(),ws_path:'/world-ws',auth_timeout_ms:3000}))
  }catch(e){errors.push(String(e));res.statusCode=500;res.end('{}')}
 }
 function attach(http) {
  http.on('upgrade',(request,socket,head)=>{
   if(request.url!=='/world-ws')return
   const p=current;assert(p);assert.equal(request.headers['sec-websocket-protocol'],undefined)
   assert.equal(new URL(request.headers.origin).host,request.headers.host)
   connections.add(socket);p.connections++;p.active++;p.peak=Math.max(p.peak,p.active);p.socket=socket
   socket.on('error',()=>{});socket.on('close',()=>{connections.delete(socket);p.active--;if(p.socket===socket)p.socket=null;clearInterval(heartbeat)})
   const accept=createHash('sha1').update(request.headers['sec-websocket-key']+'258EAFA5-E914-47DA-95CA-C5AB0DC85B11').digest('base64')
   socket.write(`HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: ${accept}\r\n\r\n`)
   const heartbeat=setInterval(()=>{if(p.socket===socket)send(p,{protocolVersion:'1.0.0',type:'ping',heartbeatId:uuid(++serial)})},8000)
   let buffer=head
   socket.on('data',chunk=>{
    try {
     buffer=Buffer.concat([buffer,chunk]);assert(buffer.length<8192)
     while(buffer.length>=2){
      const opcode=buffer[0]&15;assert(buffer[0]&0x80);assert(buffer[1]&0x80)
      let length=buffer[1]&127,offset=2
      if(length===126){if(buffer.length<4)return;length=buffer.readUInt16BE(2);offset=4}
      assert(length<=4096&&length!==127);if(buffer.length<offset+4+length)return
      const mask=buffer.subarray(offset,offset+4),body=Buffer.from(buffer.subarray(offset+4,offset+4+length));buffer=buffer.subarray(offset+4+length)
      for(let i=0;i<body.length;i++)body[i]^=mask[i%4]
      if(opcode===8){socket.end(Buffer.from([0x88,0]));continue}
      assert.equal(opcode,1);message(p,JSON.parse(body))
     }
    }catch(e){errors.push(String(e));socket.destroy()}
   })
  })
 }
 const peer=id=>{const p=peers.get(id);assert(p);return p}
 return {middleware,attach,errors,
  inspect(id){const p=peer(id);return {connections:p.connections,active:p.active,peak:p.peak,tickets:p.tickets.size,authenticates:p.authenticates,denials:p.denials,resyncs:structuredClone(p.resyncs),trace:structuredClone(p.trace),roster:structuredClone(p.roster),held:p.held.length,streamId:p.frame.streamId,roomEpoch:p.frame.roomEpoch,roomId:p.frame.roomId}},
  fault(id,name){const p=peer(id)
   if(name==='gap'){p.seq+=2;send(p,delta(p,[{...p.roster.at(-1),displayName:'Dropped ghost mutation'}]))}
   else if(name==='old'){send(p,delta(p,[{...p.template.players.at(-1),displayName:'Queued ghost mutation'}],[p.template.players[1].userId]))}
   else if(name==='release'){assert(p.pending);const req=p.pending;p.pending=null;freshRoster(p);snapshot(p,'RESYNC',req.requestId)}
   else if(name==='epoch'){p.frame.roomEpoch=uuid(++p.epoch);p.tick=0;freshRoster(p,3);snapshot(p,'EPOCH_CHANGED')}
   else if(name==='remoteMove'){const remote=p.roster[1];remote.position={...remote.position,y:500};remote.moving=true;send(p,delta(p,[structuredClone(remote)]))}
   else if(name==='hold')p.hold=true
   else if(name==='deliver'){assert(p.held.length);const rows=p.held;p.held=[];p.hold=false;for(const d of rows)send(p,d)}
   else if(name==='rejectSwitch')p.rejectSwitch=true
   else if(name==='reuseTicket')p.reuse=true
   else if(name==='restart'){p.epoch++;freshRoster(p,3);p.socket.destroy()}
   else if(name==='drop')p.socket.destroy()
   else if(name==='revoke')send(p,{protocolVersion:'1.0.0',type:'revoke',streamId:p.frame.streamId,reason:'SESSION_REVOKED'})
   else throw Error('Unknown required fault '+name)
  },
  close(){for(const socket of connections)socket.destroy()},
 }
}
