// Actual Rust client + Rust server over HTTP; Pasion/Matrix are controlled peers.
// No model, user account or production registration is touched.
import assert from 'node:assert/strict';
import {createServer, request as httpRequest} from 'node:http';
import {once} from 'node:events';
import {spawn} from 'node:child_process';
import {mkdtemp,writeFile,readFile,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {createHash} from 'node:crypto';
import {fixture} from './fixtures/palpo.mjs';
for (const name of ['CONTRACT_SERVER','HAGENCY_CLIENT','CONSOLE_ASSETS']) assert.ok(process.env[name], `${name} is required; see docs/guide.md`);
const token='test-native-owner-token-0123456789', f=fixture({deliverProbe:false});
f.actors.set(token,'@owner:example.test');
const nativeFetch=(target,options)=>new Promise((resolve,reject)=>{
 const req=httpRequest(target,{method:options.method,headers:options.headers},res=>{const chunks=[];res.on('data',c=>chunks.push(c));res.on('end',()=>resolve(new Response(Buffer.concat(chunks),{status:res.statusCode,headers:res.headers})));});req.on('error',reject);req.end(options.body);
});
const dir=await mkdtemp(join(tmpdir(),'hagency-enrollment-e2e-'));
const children=[];const servers=[];let challenge;let lastState;let loginCookie;
const delay=ms=>new Promise(r=>setTimeout(r,ms));
async function port(){const s=createServer().listen(0,'127.0.0.1');await once(s,'listening');const p=s.address().port;await new Promise(r=>s.close(r));return p;}
const backendPort=await port(), clientPort=await port(), proxyPort=await port();
const origin=`http://127.0.0.1:${proxyPort}`, backend=`http://127.0.0.1:${backendPort}`, client=`http://127.0.0.1:${clientPort}`;
let matrix;
const upstream=createServer(async(req,res)=>{
 try {
  const chunks=[];for await(const c of req)chunks.push(c);const raw=Buffer.concat(chunks).toString();const url=new URL(req.url,origin);
  let reply;
  if(url.pathname.startsWith('/api/relay/v2/')) reply=await nativeFetch(backend+req.url,{method:req.method,headers:{Host:new URL(fixtureOrigin).host,Authorization:req.headers.authorization,'Content-Type':'application/json'},body:raw});
  else if(url.pathname==='/_pasion/oauth2/registration')reply=new Response(JSON.stringify({client_id:'test-native-client'}));
  else if(url.pathname==='/_pasion/oauth2/token'){
   const fields=new URLSearchParams(raw);assert.equal(createHash('sha256').update(fields.get('code_verifier')).digest('base64url'),challenge);
   reply=new Response(JSON.stringify({access_token:token,expires_in:900}));
  }else if(url.pathname==='/_pasion/oauth2/introspect'){
   assert.equal(req.headers.authorization,'Bearer service-secret');
   reply=new Response(JSON.stringify({active:new URLSearchParams(raw).get('token')===token,username:'owner',sub:'owner-sub',client_id:'test-native-client',scope:'urn:matrix:client:api:*'}));
  }else if(url.pathname==='/_matrix/client/v3/account/whoami'&&req.headers.authorization===`Bearer ${token}`)reply=new Response(JSON.stringify({user_id:'@owner:example.test',device_id:'HagencyDevice'}));
  else if(/^\/_matrix\/client\/v3\/rooms\/[^/]+\/(event|joined_members|state\/)/.test(decodeURIComponent(url.pathname)) && req.method==='GET') {
   const parts=decodeURIComponent(url.pathname).split('/'), room=f.rooms.get(parts[5]);
   const actor=url.searchParams.get('user_id');
   const reg=[...f.registrations.values()].find(r=>r.as_token===req.headers.authorization?.slice(7));
   if(!room || !reg || !reg.namespaces.users.some(ns=>new RegExp(ns.regex).test(actor)) || !room.state.some(e=>e.type==='m.room.member'&&e.state_key===actor&&e.content.membership==='join'))reply=new Response('{}',{status:403});
   else if(parts[6]==='event')reply=new Response(JSON.stringify(f.events.get(parts[7])),{status:200});
   else if(parts[6]==='joined_members')reply=new Response(JSON.stringify({joined:Object.fromEntries(room.state.filter(e=>e.type==='m.room.member'&&e.content.membership==='join').map(e=>[e.state_key,{}]))}));
   else {const state=room.state.find(e=>e.type===parts[7]&&e.state_key===(parts[8]||''));reply=new Response(JSON.stringify(state?.content||{}),{status:state?200:404});}
  }

  else {
   reply=await f.fetch(url,{method:req.method,headers:{Authorization:req.headers.authorization},...(raw?{body:raw}:{})});
   if(url.pathname.includes('/send/com.hagency.connection.probe.v1/')&&reply.ok){
    const value=await reply.clone().json(),event=f.events.get(value.event_id);
    const reg=[...f.registrations.values()].find(r=>r.namespaces.users.some(ns=>new RegExp(ns.regex).test(event.sender)));
    // Matrix pushes the exact event to the App Service relay; the real host
    // queues and delivers it to the real native client before proof succeeds.
    fetch(`${reg.url}/_matrix/app/v1/transactions/tx-${f.events.size}`,{method:'PUT',headers:{Authorization:`Bearer ${reg.hs_token}`,'Content-Type':'application/json'},body:JSON.stringify({events:[event]})}).then(r=>assert.equal(r.status,200)).catch(e=>matrix=e);
   }
  }
  res.writeHead(reply.status,{'Content-Type':'application/json'});res.end(await reply.text());
 }catch(e){matrix=e;res.writeHead(502);res.end('{}');}
});upstream.listen(0,'127.0.0.1');await once(upstream,'listening');servers.push(upstream);
const fixtureOrigin=`http://127.0.0.1:${upstream.address().port}`;
const proxy=createServer((req,res)=>{
 const target=req.url.startsWith('/_pasion/')||req.url.startsWith('/_matrix/')?fixtureOrigin:backend;
 const forward=httpRequest(target+req.url,{method:req.method,headers:{...req.headers,host:`127.0.0.1:${proxyPort}`}},r=>{res.writeHead(r.statusCode,r.headers);r.pipe(res);});
 forward.on('error',()=>{res.writeHead(502);res.end('{}');});req.pipe(forward);
});proxy.listen(proxyPort,'127.0.0.1');await once(proxy,'listening');servers.push(proxy);
function run(binary,args){const child=spawn(binary,args,{env:{...process.env,FIXTURE_ORIGIN:fixtureOrigin},stdio:['ignore','pipe','pipe']});let logs='';child.stderr.on('data',b=>logs+=b);child.stdout.resume();children.push(child);return {child,logs:()=>logs};}
async function wait(check){let last;for(let i=0;i<600;i++){try{const value=await check();if(value)return value;}catch(error){last=error;}if(matrix)throw matrix;await delay(100);}throw last??Error('Timed out waiting for enrollment');}
try {
 await writeFile(join(dir,'hagency.toml'),`listen="127.0.0.1:${backendPort}"\npublic_origin="${origin}"\ndatabase_url="postgres://unused/hagency"\npalpo_config="palpo.toml"\npasion_config="pasion.toml"\nretirement_admin_token_file="admin-token"\n[fleet_access]\nallow_self_service=true\nmax_per_user=3\n`);
 await writeFile(join(dir,'admin-token'),'admin-secret',{mode:0o600});
 await writeFile(join(dir,'palpo.toml'),`server_name="example.test"\n[db]\nurl="postgres://unused/palpo"\n[admin]\nmas_secret="service-secret"\n[well_known]\nclient="${origin}"\nserver="example.test"\n`);
 await writeFile(join(dir,'pasion.toml'),'[database]\nuri="postgres://unused/pasion"\n[hagency]\ndelegate_matrix_auth=true\n');
 run(process.env.CONTRACT_SERVER,[join(dir,'hagency.toml')]);
 await wait(async()=>{const r=await fetch(origin+'/_hagency/client/v1/discovery');return r.ok;});
 const state=join(dir,'client-state');const initialized=run(process.env.HAGENCY_CLIENT,['init','--state-dir',state]);await once(initialized.child,'exit');assert.equal(initialized.child.exitCode,0,initialized.logs());
 run(process.env.HAGENCY_CLIENT,['serve','--state-dir',state,'--listen',`127.0.0.1:${clientPort}`,'--palpo-transport','--console-assets',process.env.CONSOLE_ASSETS]);
 await wait(async()=>{const r=await fetch(client+'/ready');return r.ok;});
 const operator=(await readFile(join(state,'operator.token'),'utf8')).trim();
 const ticket=(await(await fetch(client+'/api/native/v1/console/access',{method:'POST',headers:{Authorization:`Bearer ${operator}`}})).json()).ticket;
 const exchange=await fetch(client+'/console/session',{method:'POST',headers:{Origin:client,'Sec-Fetch-Site':'same-origin','Content-Type':'application/json'},body:JSON.stringify({ticket})});assert.equal(exchange.status,200);
 const cookie=exchange.headers.getSetCookie()[0].split(';')[0];
 const started=await fetch(client+'/console/server-login/start',{method:'POST',headers:{Origin:client,'Sec-Fetch-Site':'same-origin',Cookie:cookie,'Content-Type':'application/json'},body:JSON.stringify({server:origin,name:'E2E workstation'})});assert.equal(started.status,200);
 const authorization=new URL((await started.json()).url);challenge=authorization.searchParams.get('code_challenge');
 const nonce=started.headers.getSetCookie()[0].split(';')[0];
 const callback=await fetch(`${client}/console/server-login/callback?state=${authorization.searchParams.get('state')}&code=test-code`,{headers:{Cookie:nonce},redirect:'manual'});assert.equal(callback.status,303);
 const login=callback.headers.getSetCookie().find(c=>c.startsWith('hagency_console=')).split(';')[0];
 loginCookie=login;
 const result=await wait(async()=>{const r=await fetch(client+'/console/server-login',{headers:{Cookie:login,'Sec-Fetch-Site':'same-origin'}});const s=await r.json();lastState=s.status;if(s.status.state==='failed')throw Error(s.status.code);return s.status.state==='connected'?s:false;});
 assert.equal(f.registrations.size,1);assert.equal(result.configured,true);
 console.log('PASS actual client + server: PKCE, self-service enrollment, import, outbound poll, Matrix App Service delivery, exact probe and reception connection');
}catch(error){
 console.error('Enrollment state',lastState);
 if(lastState?.fleetId){const reply=await fetch(origin+`/_hagency/client/v1/fleets/${lastState.fleetId}/connect`,{method:'POST',headers:{Authorization:`Bearer ${token}`,'Content-Type':'application/json'},body:'{}'});const result=await reply.json();console.error('Connect diagnostic',reply.status,result.code||result.fleet?.lastError,result.fleet?.readiness);}
 console.error('Matrix fixture calls',f.calls.map(c=>`${c.method} ${c.path}`).slice(-16));
 throw error;
}finally{
 for(const child of children.reverse())if(child.exitCode===null){child.kill('SIGTERM');await Promise.race([once(child,'exit'),delay(5000).then(()=>child.kill('SIGKILL'))]);}
 for(const server of servers.reverse()){server.closeAllConnections();await new Promise(r=>server.close(r));}
 await rm(dir,{recursive:true,force:true});
}
