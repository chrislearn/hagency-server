// Controlled Matrix fixture exercises the Rust HTTP backend; no model is called.
import assert from 'node:assert/strict';
import { createServer, request as httpRequest } from 'node:http';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { mkdtemp, writeFile, rm, readFile, readdir } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { randomBytes, randomUUID } from 'node:crypto';
import { fixture } from './fixtures/palpo.mjs';

// Use HTTP/1 explicitly: Fetch normalizes Host, which would hide host-policy regressions.
const fetch = (target, options = {}) => new Promise((resolve, reject) => {
  const req = httpRequest(target, { method: options.method ?? 'GET', headers: options.headers }, res => {
    const chunks = []; res.on('data', chunk => chunks.push(chunk));
    res.on('end', () => resolve(new Response(Buffer.concat(chunks), { status: res.statusCode, headers: res.headers })));
  });
  req.on('error', reject); req.end(options.body);
});

const f = fixture();
const accountConfig = { botMxid: '@owner:example.test', botToken: 'owner-secret', adminToken: 'admin-secret', approvers: ['@admin:example.test'], registrationToken: 'invite-token', passwordKey: 'a'.repeat(64) };
const credentials = new Map(), devices = new Map();
const upstream = createServer(async (req, res) => {
  try {
    const chunks = []; for await (const chunk of req) chunks.push(chunk);
    const raw = Buffer.concat(chunks).toString();
    const url = new URL(req.url, 'http://fixture.invalid'), path = decodeURIComponent(url.pathname), body = raw ? JSON.parse(raw) : null;
    const args = { method: req.method, headers: { Authorization: req.headers.authorization }, ...(raw ? { body: raw } : {}) };
    let response;
    const output = (status, value) => new Response(JSON.stringify(value), { status });
    if (path === '/_matrix/client/v3/register') {
      const mxid = '@' + body.username + ':example.test';
      if (f.users.has(mxid)) response = output(400, { errcode: 'M_USER_IN_USE' });
      else if (!body.auth) response = output(401, { flows: [{ stages: ['m.login.registration_token'] }], session: 'signup-session' });
      else if (body.auth.token !== accountConfig.registrationToken) response = output(403, { errcode: 'M_FORBIDDEN' });
      else {
        credentials.set(mxid, body.password); devices.set(mxid, body.device_id);
        f.actors.set('new-user:' + mxid, mxid); f.users.set(mxid, { name: mxid, admin: false, deactivated: false });
        response = output(200, { user_id: mxid, device_id: body.device_id, access_token: 'new-user:' + mxid });
      }
    } else if (path === '/_matrix/client/v3/account/whoami' && req.headers.authorization==='Bearer guest-test') response=output(200,{user_id:'@owner:example.test',is_guest:true});
    else if (path === '/_matrix/client/v3/account/whoami' && req.headers.authorization==='Bearer foreign-test') response=output(200,{user_id:'@owner:other.test'});
    else if (path === '/_matrix/client/v3/login' && credentials.has(body.identifier.user)) {
      response = output(credentials.get(body.identifier.user) === body.password ? 200 : 403, credentials.get(body.identifier.user) === body.password ? { user_id: body.identifier.user, access_token: 'new-user:' + body.identifier.user } : { errcode: 'M_FORBIDDEN' });
    } else if (path.startsWith('/_palpo/admin/v1/whois/')) {
      const mxid = path.slice('/_palpo/admin/v1/whois/'.length);
      response = output(200, { devices: { [devices.get(mxid)]: {} } });
    } else if (/^\/_matrix\/client\/v3\/profile\/.+\/displayname$/.test(path)) response = output(200, {});
    else if (path === '/_matrix/client/v3/account/whoami' && req.headers.authorization?.startsWith('Bearer new-user:')) response = output(200, { user_id: req.headers.authorization.slice('Bearer new-user:'.length) });
    else if (path.startsWith('/_palpo/admin/') && req.headers.authorization?.startsWith('Bearer new-user:')) response = output(403, { errcode: 'M_FORBIDDEN' });
    else if (/^\/_matrix\/client\/v3\/rooms\/[^/]+\/messages$/.test(path)) {
      const roomId = path.split('/')[5], events = [...f.events.values()].filter(e => e.room_id === roomId);
      const start = Number(url.searchParams.get('from') ?? 0), chunk = events.slice(start, start + 100);
      response = output(200, { start: String(start), end: String(start + chunk.length), chunk });
    } else response = await f.fetch(url, args);
    res.writeHead(response.status, { 'Content-Type': 'application/json' }); res.end(await response.text());
  } catch { res.writeHead(502); res.end('{}'); }
});
upstream.listen(0, '127.0.0.1'); await once(upstream, 'listening');
const fixtureOrigin = `http://127.0.0.1:${upstream.address().port}`;
const reservation = createServer().listen(0, '127.0.0.1'); await once(reservation, 'listening'); const port = reservation.address().port;
await new Promise(r => reservation.close(r));
const base = `http://127.0.0.1:${port}`, dir = await mkdtemp(join(tmpdir(), 'hagency-contract-'));
await writeFile(join(dir, 'accounts.json'), JSON.stringify(accountConfig));
await writeFile(join(dir, 'hagency.toml'), `listen = "127.0.0.1:${port}"\npublic_origin = "${base}"\ndatabase_url = "postgres://unused/hagency"\npalpo_config = "palpo.toml"\ncallback_origins = ["${fixtureOrigin}"]\naccount_config = "accounts.json"\n`);
await writeFile(join(dir, 'palpo.toml'), `server_name = "example.test"\n[db]\nurl = "postgres://unused/palpo"\n[well_known]\nclient = "${base}"\nserver = "example.test"\n`);
const binary = process.env.CONTRACT_SERVER ?? resolve('target/debug/examples/admin_contract_server');
const child = spawn(binary, [join(dir, 'hagency.toml')], { env: { ...process.env, FIXTURE_ORIGIN: fixtureOrigin }, stdio: ['ignore', 'pipe', 'pipe'] });
let stderr = ''; child.stderr.on('data', b => stderr += b);
const delay = ms => new Promise(r => setTimeout(r, ms));
async function until(fn, timeout = 12000) { const end = Date.now() + timeout; let last; while (Date.now() < end) { try { const v = await fn(); if (v) return v; } catch (e) { last = e; } await delay(100); } throw last ?? new Error('Timed out'); }
const checks = [];
async function check(name, fn) { await fn(); checks.push(name); console.log('PASS', name); }
async function api(path, { method = 'GET', body, session, headers = {} } = {}) {
  const response = await fetch(base + '/api' + path, { method, headers: { ...(body ? { 'Content-Type': 'application/json', Origin: base } : {}), ...(session ? { Cookie: session.cookie, 'X-CSRF-Token': session.csrf } : {}), ...headers }, ...(body ? { body: JSON.stringify(body) } : {}) });
  const data = await response.json(); return { status: response.status, data, response };
}
async function login(user = 'admin') { const r = await api('/login', { method: 'POST', body: { username: '@' + user + ':example.test', password: 'correct-password' } }); assert.equal(r.status, 200, JSON.stringify(r.data)); return { ...r.data, cookie: r.response.headers.get('set-cookie').split(';')[0] }; }
const adminHeaders = session => ({ Cookie: session.cookie, 'X-CSRF-Token': session.csrf, Origin: base });
try {
  await until(async () => { if (child.exitCode !== null) throw new Error(stderr); const r = await fetch(base); return r.status === 200; }, 30000);
  const admin = await login(), owner = await login('owner'), other = await login('other');
  await check('Dioxus frontend SPA, runtime config and safe same-origin assets', async () => {
    const html=await readFile('resources/frontend/public/index.html');
    for (const path of ['/','/login','/hagency/projects','/hagency/connections','/users','/pasion/accounts']) {
        const r=await fetch(base+path);assert.equal(r.status,200,path);assert.deepEqual(Buffer.from(await r.arrayBuffer()),html);
        assert.match(r.headers.get('content-security-policy'),/wasm-unsafe-eval/);
    }
    const conf=await(await fetch(base+'/config.json')).json();assert.equal(conf.server_name,'example.test');assert.equal(conf.oauth_enabled,false);assert.equal(conf.pasion_enabled,false);assert.ok(!JSON.stringify(conf).includes('secret'));
    assert.equal((await fetch(base+'/assets/missing.wasm')).status,404);
    assert.equal((await fetch(base+'/assets/%2e%2e%2fCargo.toml')).status,404);
    assert.equal((await fetch(base+'/_matrix/not-a-route')).status,404);
    const wasm="/assets/"+(await readdir("resources/frontend/public/assets")).find(p=>p.endsWith(".wasm"));
    const r=await fetch(base+wasm);assert.equal(r.status,200);assert.equal(r.headers.get('content-type'),'application/wasm');assert.equal(Buffer.from(await r.arrayBuffer()).subarray(0,4).toString('hex'),'0061736d');
  });
  await check('Matrix token bridge validates live identity, administrator role and request Origin',async()=>{
    assert.equal((await api('/login/token',{method:'POST',body:{}})).status,401);
    assert.equal((await api('/login/token',{method:'POST',body:{},headers:{Authorization:'Bearer invalid'}})).status,401);
    assert.equal((await api('/login/token',{method:'POST',body:{},headers:{Authorization:'Bearer owner-secret',Origin:'https://evil.invalid'}})).status,403);
    for(const [token,status] of [['guest-test',403],['foreign-test',400]]) assert.equal((await api('/login/token',{method:'POST',body:{},headers:{Authorization:'Bearer '+token}})).status,status);
    for(const [token,isAdmin] of [['admin-secret',true],['owner-secret',false]]) {
        const r=await api('/login/token',{method:'POST',body:{isAdmin:true},headers:{Authorization:'Bearer '+token}});assert.equal(r.status,200,JSON.stringify(r.data));assert.equal(r.data.isAdmin,isAdmin);
        const session={...r.data,cookie:r.response.headers.get('set-cookie').split(';')[0]};
        assert.equal((await api('/fleets',{session})).status,isAdmin?200:403);
        assert.equal((await api('/projects',{method:'POST',session,body:{},headers:{'X-CSRF-Token':'bad'}})).status,403);
    }
  });
  await check('host, Origin, cookie, CSRF and live administrator checks', async () => {
    assert.equal((await api('/session')).status,401);
    assert.equal((await api('/session',{session:owner})).data.isAdmin,false);
    assert.equal((await api('/fleets',{session:owner})).status,403);
    assert.equal((await api('/login',{method:'POST',body:{},headers:{Origin:'https://evil.invalid'}})).status,403);
    assert.equal((await api('/projects',{method:'POST',body:{},session:owner,headers:{'X-CSRF-Token':'bad'}})).data.code,'csrf_forbidden');
    assert.equal((await api('/session',{session:admin,headers:{Host:'unexpected.invalid'}})).status,403);
  });
  await check('Hagency Operations web adapter and native endpoint retain separate authentication envelopes',async()=>{
    const body={service:'hagency.inbox.list',args:{view:'all'}};
    assert.equal((await api('/operations/call',{method:'POST',body,session:owner})).status,200);
    assert.equal((await api('/operations/call',{method:'POST',body})).status,401);
    assert.equal((await api('/operations/call',{method:'POST',body,session:owner,headers:{'X-CSRF-Token':'bad'}})).status,403);
    assert.equal((await api('/operations/call',{method:'POST',body,session:owner,headers:{Origin:'https://evil.invalid'}})).status,403);
    const input={appId:'im.hagency.operations',bundleDigest:'a'.repeat(64),services:['hagency.inbox.list']};
    const native=await fetch(base+'/_hagency/miniapp/v1/session',{method:'POST',headers:{'Content-Type':'application/json',Authorization:'Bearer owner-secret'},body:JSON.stringify(input)});
    assert.equal(native.status,200,await native.clone().text());
    const session=await native.json();assert.ok(session.sessionToken);assert.ok(!JSON.stringify(session).includes('owner-secret'));
    assert.equal((await fetch(base+'/_hagency/miniapp/v1/session',{method:'POST',headers:{'Content-Type':'application/json',Authorization:'Bearer owner-secret',Origin:base},body:JSON.stringify(input)})).status,403);
    assert.equal((await fetch(base+'/_palpo/miniapp/v1/session',{method:'POST',headers:{'Content-Type':'application/json',Authorization:'Bearer owner-secret'},body:JSON.stringify({...input,appId:'im.palpo.operations',services:['palpo.inbox.list']})})).status,200);
  });
  const legacyInput = { requestId:'legacy-1',name:'Legacy',ownerMxid:'@owner:example.test',transportMode:'callback',callbackUrl:fixtureOrigin + '/matrix' };
  const create = await api('/fleets',{method:'POST',body:legacyInput,session:admin}); assert.equal(create.status,201,JSON.stringify(create.data)); const legacy = create.data.fleet;
  await check('fleet install, content-bound retry, token redaction and owner isolation',async()=>{
    assert.equal((await api('/fleets',{method:'POST',body:legacyInput,session:admin})).data.fleet.id,legacy.id);
    assert.equal((await api('/fleets',{method:'POST',body:{...legacyInput,name:'Changed'},session:admin})).status,409);
    assert.equal((await api('/my/fleets/' + legacy.id + '/pair',{method:'POST',body:{},session:other})).status,404);
    const pair = await api('/my/fleets/' + legacy.id + '/pair',{method:'POST',body:{},session:owner});assert.equal(pair.status,200);assert.ok(pair.data.registration.as_token);
    assert.equal(JSON.stringify((await api('/fleets',{session:admin})).data).includes(pair.data.registration.as_token),false);
  });
  await check('managed identity create, immutable update and verified retirement',async()=>{
    const path='/fleets/' + legacy.id + '/agents'; const a=await api(path,{method:'POST',body:{agentId:'code_01',displayName:'Code',role:'coding',approvedRequestId:'approved-1'},session:admin});assert.equal(a.status,201,JSON.stringify(a.data));assert.equal(a.data.agent.matrixIdentity,'active');
    assert.equal((await api(path+'/code_01',{method:'PATCH',body:{displayName:'Renamed'},session:admin})).data.agent.displayName,'Renamed');
    assert.equal((await api(path+'/code_01',{method:'PATCH',body:{mxid:'@forged:example.test'},session:admin})).data.code,'immutable_identity');
    assert.equal((await api(path+'/code_01/retire',{method:'POST',body:{},session:admin})).data.agent.state,'retired');
  });
  let project, request;
  await check('exact legacy Matrix probe, project and private approval room',async()=>{
    const c=await api('/my/fleets/'+legacy.id+'/connect',{method:'POST',body:{},session:owner});assert.equal(c.status,200,JSON.stringify(c.data));assert.equal(c.data.readiness.ready,true);
    const p=await api('/projects',{method:'POST',body:{requestId:'project-1',fleetId:legacy.id,name:'Project'},session:owner});assert.equal(p.status,201,JSON.stringify(p.data));project=p.data.project;assert.equal(project.ownerApproval,'ready');
    f.putState(f.rooms.get(project.roomId),'m.room.member','@other:example.test',{membership:'join'},'@owner:example.test');
    const view=await api('/projects',{session:other});assert.equal(view.status,200);assert.equal(view.data.projects[0].ownerDmRoomId,undefined);
  });
  await check('request source binding, privacy and actual admission observations',async()=>{
    const r=await api('/requests',{method:'POST',body:{requestId:'agent-request-1',projectId:project.id,role:'coding',requestedTokens:2000,ratePerDay:500},session:owner});assert.equal(r.status,201,JSON.stringify(r.data));request=r.data.request;assert.equal(request.state,'pending');
    const event=f.events.get(request.sourceEventId);assert.equal(event.content.ownerDmRoomId,undefined);
    f.fulfill(legacy.id,'agent-request-1');const list=await api('/requests',{session:owner});assert.equal(list.data.requests[0].usable,true,JSON.stringify(list.data));
    assert.equal((await api('/requests',{method:'POST',body:{requestId:'agent-request-1',projectId:project.id,role:'coding',requestedTokens:2001,ratePerDay:500},session:owner})).status,409);
    f.putState(f.rooms.get(project.roomId),'m.room.member','@owner:example.test',{membership:'leave'},'@owner:example.test');assert.equal((await api('/requests',{session:owner})).data.requests[0].usable,false);
    f.putState(f.rooms.get(project.roomId),'m.room.member','@owner:example.test',{membership:'join'},'@owner:example.test');
  });
  let outbound, pair;
  await check('outbound registration, authenticated queue, leases and ACK',async()=>{
    const r=await api('/fleets',{method:'POST',body:{requestId:'outbound-1',name:'Outbound',ownerMxid:'@owner:example.test'},session:admin});assert.equal(r.status,201,JSON.stringify(r.data));outbound=r.data.fleet;
    pair=(await api('/my/fleets/'+outbound.id+'/pair',{method:'POST',body:{},session:owner})).data;
    const transactionPath='/relay/v2/'+outbound.id+'/transactions/tx-1';const body={events:[]};const relayHeaders={Host:new URL(fixtureOrigin).host,Authorization:'Bearer '+pair.registration.hs_token,Origin:undefined};
    const send=async content=>fetch(base+'/api'+transactionPath,{method:'PUT',headers:{'Content-Type':'application/json',Host:relayHeaders.Host,Authorization:relayHeaders.Authorization},body:JSON.stringify(content)});
    assert.equal((await send(body)).status,200);assert.equal((await send(body)).status,200);assert.equal((await send({events:[{type:'different'}]})).status,409);
    const headers={Authorization:'Bearer '+pair.transport.token,'X-Hagency-Generation':String(pair.transport.generation)};
    assert.equal((await api('/fleet/v2/'+outbound.id+'/poll?lane=matrix&consumer='+randomUUID()+'&wait=0',{headers:{...headers,Origin:base}})).status,403);
    const poll=await api('/fleet/v2/'+outbound.id+'/poll?lane=matrix&consumer='+randomUUID()+'&wait=0',{headers});assert.equal(poll.status,200,JSON.stringify(poll.data));assert.equal(poll.data.delivery.id,'tx-1');
    const ack=await fetch(base+'/api/fleet/v2/'+outbound.id+'/ack',{method:'POST',headers:{...headers,'Content-Type':'application/json'},body:JSON.stringify({lane:'matrix',id:'tx-1',token:poll.data.delivery.token})});assert.equal(ack.status,200);
    const update={v:2,generation:pair.transport.generation,sequence:1,heartbeat:true,capabilities:{v:1,fleetId:outbound.id,serverName:'example.test',representativeMxid:outbound.representativeMxid,approvalBotMxid:'@approvalbot:example.test',offers:[{role:'coding'}]}};
    const publish=async body=>fetch(base+'/api/fleet/v2/'+outbound.id+'/updates',{method:'POST',headers:{...headers,'Content-Type':'application/json'},body:JSON.stringify(body)});
    assert.equal((await publish(update)).status,200);assert.equal((await publish(update)).status,200);assert.equal((await publish({...update,heartbeat:false})).status,400);assert.equal((await publish({...update,statuses:[]})).status,409);
    const rotated=await api('/fleets/'+outbound.id+'/outbound',{method:'POST',body:{requestId:'rotate-1',rotate:true},session:admin});assert.equal(rotated.status,200,JSON.stringify(rotated.data));assert.equal(rotated.data.fleet.transport.generation,2);
    assert.equal((await api('/fleet/v2/'+outbound.id+'/poll?lane=matrix&consumer='+randomUUID()+'&wait=0',{headers})).status,401);
  });
  await check('outbound exact proof, durable project request and actual admission',async()=>{
    pair=(await api('/my/fleets/'+outbound.id+'/pair',{method:'POST',body:{},session:owner})).data;
    const headers={Authorization:'Bearer '+pair.transport.token,'X-Hagency-Generation':String(pair.transport.generation),'Content-Type':'application/json'};
    const machine=async(action,body)=>{const response=await fetch(base+'/api/fleet/v2/'+outbound.id+'/'+action,{method:'POST',headers,body:JSON.stringify(body)});return{status:response.status,data:await response.json()};};
    const capabilities={v:1,fleetId:outbound.id,serverName:'example.test',representativeMxid:outbound.representativeMxid,approvalBotMxid:'@approvalbot:example.test',offers:[{role:'coding'}]};
    assert.equal((await machine('updates',{v:2,generation:2,sequence:1,heartbeat:true,capabilities})).status,200);
    const connect=await api('/my/fleets/'+outbound.id+'/connect',{method:'POST',body:{},session:owner});assert.equal(connect.status,202,JSON.stringify(connect.data));assert.equal(connect.data.readiness.ready,false);
    const probe=connect.data.probe;
    const receipt={received:true,fleetId:outbound.id,sourceRoomId:probe.roomId,sourceEventId:probe.eventId,challenge:probe.challenge};
    assert.equal((await machine('updates',{v:2,generation:2,sequence:2,heartbeat:true,probeReceipts:[receipt]})).status,409);
    const response=await fetch(base+'/api/relay/v2/'+outbound.id+'/transactions/probe-tx',{method:'PUT',headers:{'Content-Type':'application/json',Host:new URL(fixtureOrigin).host,Authorization:'Bearer '+pair.registration.hs_token},body:JSON.stringify({events:[f.events.get(probe.eventId)]})});assert.equal(response.status,200);
    const poll=await api('/fleet/v2/'+outbound.id+'/poll?lane=matrix&consumer='+randomUUID()+'&wait=0',{headers});assert.equal(poll.data.delivery.id,'probe-tx');
    assert.equal((await machine('ack',{lane:'matrix',id:'probe-tx',token:poll.data.delivery.token})).status,200);
    assert.equal((await machine('updates',{v:2,generation:2,sequence:2,heartbeat:true,probeReceipts:[receipt]})).status,200);
    const p=await api('/projects',{method:'POST',body:{requestId:'project-outbound',fleetId:outbound.id,name:'Outbound Project'},session:owner});assert.equal(p.status,201,JSON.stringify(p.data));
    const r=await api('/requests',{method:'POST',body:{requestId:'outbound-request',projectId:p.data.project.id,role:'coding',requestedTokens:2000,ratePerDay:500},session:owner});assert.equal(r.status,201,JSON.stringify(r.data));assert.equal(r.data.request.state,'queued');
    const mxid='@'+outbound.id+'_agent_native:example.test';f.users.set(mxid,{name:mxid,appservice_id:outbound.id,deactivated:false,rooms:[p.data.project.roomId]});f.putState(f.rooms.get(p.data.project.roomId),'m.room.member',mxid,{membership:'join'},mxid);
    const status={v:1,fleetId:outbound.id,requestId:'outbound-request',state:'active',engagementId:'engagement-native',targetProjectId:p.data.project.id,targetRoomId:p.data.project.roomId,sourceRoomId:probe.roomId,sourceEventId:r.data.request.sourceEventId,role:'coding',requestedTokens:2000,allocatedTokens:2000,agentMxid:mxid,bound:true,ready:true,serving:{framework:'codex',model:'fixture-model'},fulfillment:{phase:'ready',incomplete:false},observedAt:new Date().toISOString()};
    assert.equal((await machine('updates',{v:2,generation:2,sequence:3,heartbeat:true,statuses:[status]})).status,200);
    const observed=await api('/requests',{session:owner});assert.equal(observed.data.requests.find(r=>r.requestId==='outbound-request').usable,true,JSON.stringify(observed.data));
    const retirement=await machine('retire-agent',{requestId:'outbound-request',agentMxid:mxid,endedAt:Date.now(),localStopped:true});assert.equal(retirement.status,200,JSON.stringify(retirement.data));assert.equal(retirement.data.agent.localTaskStop,'confirmed');
    assert.equal((await machine('updates',{v:2,generation:2,sequence:4,heartbeat:true,statuses:[status]})).status,200);
    assert.equal((await api('/requests',{session:owner})).data.requests.find(r=>r.requestId==='outbound-request').usable,false);
  });
  await check('account request receipt, private approval and ordinary-user provisioning',async()=>{
    await until(async()=> (await api('/account-access')).data.ready);
    const rooms=(await api('/account-requests',{session:admin})).data;
    f.putState(f.rooms.get(rooms.roomId),'m.room.member','@admin:example.test',{membership:'join'},'@admin:example.test');
    const input={id:randomBytes(16).toString('hex'),receipt:randomBytes(32).toString('hex'),username:'alice',displayName:'Alice 张',reason:'Project work',password:'long-contract-password-2026'};
    const submitted=await api('/account-requests',{method:'POST',body:input});assert.equal(submitted.status,202,JSON.stringify(submitted.data));
    assert.equal((await api('/account-requests/status',{method:'POST',body:{id:input.id,receipt:'b'.repeat(64)}})).status,404);
    assert.equal((await api('/account-requests',{method:'POST',body:{...input,password:'different-long-password'}})).status,409);
    const card=await until(()=> [...f.events.values()].find(e=>e.content['org.octos.approval_request']?.request_id===input.id));
    assert.equal(JSON.stringify(card).includes(input.password),false);
    const approval=card.content['org.octos.approval_request'];
    const decision={event_id:'$account-decision',type:'m.room.message',room_id:rooms.roomId,sender:'@admin:example.test',content:{msgtype:'m.text',body:'Approve','org.octos.approval_response':{request_id:input.id,decision:'approve',source_event_id:card.event_id,tool_args_digest:approval.tool_args_digest},'m.relates_to':{'m.in_reply_to':{event_id:card.event_id}}}};
    f.events.set(decision.event_id,decision);
    await until(async()=> (await api('/account-requests/status',{method:'POST',body:{id:input.id,receipt:input.receipt}})).data.request.status==='registered',25000);
    assert.equal(credentials.get('@alice:example.test'),input.password);
    const login=await api('/login',{method:'POST',body:{username:'@alice:example.test',password:input.password}});assert.equal(login.status,200);assert.equal(login.data.isAdmin,false);
  });
  await check('pause, resume, final revoke and live privilege revocation',async()=>{
    for(const action of ['pause','resume','revoke'])assert.equal((await api('/fleets/'+legacy.id+'/'+action,{method:'POST',body:{},session:admin})).status,200);
    assert.equal((await api('/fleets/'+legacy.id+'/resume',{method:'POST',body:{},session:admin})).data.code,'fleet_revoked');
    f.denyAdmin();assert.equal((await api('/session',{session:admin})).status,403);
  });
  console.log(JSON.stringify({checks:checks.length,passed:true},null,2));
} finally {
  child.kill('SIGTERM'); await Promise.race([once(child,'exit'),delay(5000)]); if(child.exitCode===null)child.kill('SIGKILL');
  upstream.closeAllConnections(); await new Promise(r=>upstream.close(r)); await rm(dir,{recursive:true,force:true});
}
