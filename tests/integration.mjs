// Real Palpo + Rust web-admin + PostgreSQL. Run only with EMPTY dedicated Hagency and Palpo databases.
import assert from 'node:assert/strict';
import { spawn, execFileSync } from 'node:child_process';
import { createServer } from 'node:http';
import { once } from 'node:events';
import { mkdtemp, writeFile, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { randomBytes,randomUUID } from 'node:crypto';
const adminDatabase = process.env.HAGENCY_TEST_DATABASE_URL;
const database = process.env.PALPO_TEST_DATABASE_URL;
assert.ok(adminDatabase && database,'Set HAGENCY_TEST_DATABASE_URL and PALPO_TEST_DATABASE_URL to EMPTY dedicated databases');
// Fail before either component can migrate a populated database.
for (const url of [adminDatabase,database]) {
 const tables=execFileSync('psql',[url,'-Atc',"SELECT count(*) FROM pg_catalog.pg_tables WHERE schemaname NOT IN ('pg_catalog','information_schema')"],{encoding:'utf8'}).trim();
 assert.equal(tables,'0','Integration test requires empty dedicated databases');
}
const reserve=createServer().listen(0,'127.0.0.1');await once(reserve,'listening');const port=reserve.address().port;await new Promise(r=>reserve.close(r));
const dir=await mkdtemp(join(tmpdir(),'hagency-live-')),base=`http://127.0.0.1:${port}`,serverName=`localhost:${port}`;
const password=randomBytes(24).toString('base64url');const passwordFile=join(dir,'admin-password');await writeFile(passwordFile,password,{mode:0o600});
const config=join(dir,'config.toml');await writeFile(config,`listen = "127.0.0.1:${port}"\npublic_origin = "${base}"\ndatabase_url = ${JSON.stringify(adminDatabase)}\n[matrix]\nserver_name = "${serverName}"\nallow_registration = false\n[matrix.db]\nurl = ${JSON.stringify(database)}\npool_size = 10\n[matrix.well_known]\nclient = "${base}"\nserver = "${serverName}"\n[matrix.storage]\nbackend = "fs"\nroot = "data/media"\n`,{mode:0o600});
const binary=process.env.HAGENCY_BINARY??resolve('target/debug/hagency-server');let child,logs='';
const delay=ms=>new Promise(r=>setTimeout(r,ms));
async function until(fn,timeout=45000){let error;const end=Date.now()+timeout;while(Date.now()<end){try{const v=await fn();if(v)return v;}catch(e){error=e;}if(child?.exitCode!==null)throw new Error('Server exited: '+logs.slice(-5000));await delay(100);}throw error??new Error('Timeout: '+logs.slice(-5000));}
async function start(bootstrap){logs='';child=spawn(binary,['--config',config,...(bootstrap?['--bootstrap-admin','admin','--bootstrap-password-file',passwordFile]:[])],{env:{...process.env,RUST_LOG:'warn'},stdio:['ignore','pipe','pipe']});child.stdout.on('data',b=>logs+=b);child.stderr.on('data',b=>logs+=b);await until(async()=> (await fetch(base+'/healthz')).status===200);}
async function stop(){if(child&&child.exitCode===null){child.kill('SIGTERM');await Promise.race([once(child,'exit'),delay(20000)]);if(child.exitCode===null){child.kill('SIGKILL');await once(child,'exit');throw new Error('Graceful shutdown timed out');}assert.equal(child.exitCode,0,'Graceful shutdown should exit cleanly');}}
async function api(path,{method='GET',body,session,headers={}}={}){const response=await fetch(base+'/api'+path,{method,headers:{...(body?{'Content-Type':'application/json',Origin:base}:{}),...(session?{Cookie:session.cookie,'X-CSRF-Token':session.csrf}:{}),...headers},...(body?{body:JSON.stringify(body)}:{})});return{status:response.status,data:await response.json(),response};}
async function login(){const r=await api('/login',{method:'POST',body:{username:`@admin:${serverName}`,password}});assert.equal(r.status,200,JSON.stringify(r.data));assert.equal(r.data.isAdmin,true);return{...r.data,cookie:r.response.headers.get('set-cookie').split(';')[0]};}
let evidence=[];
try{
 await start(true);let session=await login();
 const tables=(url)=>execFileSync('psql',[url,'-Atc',"SELECT tablename FROM pg_catalog.pg_tables WHERE schemaname='public' ORDER BY tablename"],{encoding:'utf8'}).trim().split('\n');
 assert.deepEqual(tables(adminDatabase),['hagency_admin_state']);assert.ok(tables(database).includes('users'));assert.ok(!tables(database).includes('hagency_admin_state'));evidence.push('Hagency and Palpo tables live in separate databases');evidence.push('real administrator login');
 const html=await fetch(base);assert.equal(html.status,200);assert.equal(await html.text(),await readFile('public/index.html','utf8'));evidence.push('unchanged static frontend on same listener');
 assert.equal((await fetch(base+'/_matrix/client/versions')).status,200);const discovery=await(await fetch(base+'/.well-known/matrix/client')).json();assert.equal(discovery['m.homeserver'].base_url,base);evidence.push('mounted Matrix APIs and discovery');
 const fleet=await api('/fleets',{method:'POST',body:{requestId:'real-fleet-1',name:'Real outbound fleet',ownerMxid:`@admin:${serverName}`},session});assert.equal(fleet.status,201,JSON.stringify(fleet.data));const id=fleet.data.fleet.id;
 const pair=(await api('/my/fleets/'+id+'/pair',{method:'POST',body:{},session})).data;assert.ok(pair.transport.token);assert.ok(pair.registration.as_token);evidence.push('real App Service registration and representative provisioning');
 const capabilities={v:1,fleetId:id,serverName,representativeMxid:fleet.data.fleet.representativeMxid,approvalBotMxid:`@approvalbot:${serverName}`,offers:[{role:'coding'}]};
 const headers={Authorization:'Bearer '+pair.transport.token,'X-Hagency-Generation':'1','Content-Type':'application/json'};
 const update=await fetch(base+'/api/fleet/v2/'+id+'/updates',{method:'POST',headers,body:JSON.stringify({v:2,generation:1,sequence:1,heartbeat:true,capabilities})});assert.equal(update.status,200,await update.text());
 const connect=await api('/my/fleets/'+id+'/connect',{method:'POST',body:{},session});assert.equal(connect.status,202,JSON.stringify(connect.data));const probe=connect.data.probe;
 let delivery;
 await until(async()=>{const r=await api('/fleet/v2/'+id+'/poll?lane=matrix&consumer='+randomUUID()+'&wait=0',{headers});assert.equal(r.status,200,JSON.stringify(r.data));const d=r.data.delivery;if(!d)return false;const ack=await fetch(base+'/api/fleet/v2/'+id+'/ack',{method:'POST',headers,body:JSON.stringify({lane:'matrix',id:d.id,token:d.token})});assert.equal(ack.status,200);if(d.payload.body.events.some(e=>e.event_id===probe.eventId)){delivery=d;return true;}return false;});
 const receipt={received:true,fleetId:id,sourceRoomId:probe.roomId,sourceEventId:probe.eventId,challenge:probe.challenge};
 const proof=await fetch(base+'/api/fleet/v2/'+id+'/updates',{method:'POST',headers,body:JSON.stringify({v:2,generation:1,sequence:2,heartbeat:true,probeReceipts:[receipt]})});assert.equal(proof.status,200,await proof.text());
 assert.equal((await api('/fleets/'+id,{session})).data.fleet.readiness.ready,true);evidence.push('real Palpo event -> mounted relay -> durable ACK -> exact connection proof');
 const keys=JSON.parse(await readFile(join(dir,'data/matrix-signing-key.json'),'utf8'));
 const countUsers=()=>Number(execFileSync('psql',[database,'-Atc','SELECT count(*) FROM public.users'],{encoding:'utf8'}).trim());
 const usersBefore=countUsers();assert.ok(usersBefore>=2);
 await stop();assert.equal(countUsers(),usersBefore);await start(false);assert.equal(countUsers(),usersBefore,'Restart must preserve Palpo users even when the DB role is hagency');session=await login();const saved=(await api('/fleets/'+id,{session})).data.fleet;assert.equal(saved.id,id);assert.equal(saved.transport.generation,1);const resumed=(await api('/my/fleets/'+id+'/pair',{method:'POST',body:{},session})).data;assert.equal(resumed.transport.token,pair.transport.token);assert.equal(resumed.registration.as_token,pair.registration.as_token);assert.deepEqual(JSON.parse(await readFile(join(dir,'data/matrix-signing-key.json'),'utf8')),keys);evidence.push('restart retains fleet, transport credentials and Matrix signing key');
 const queue=(await api('/fleets/'+id+'/outbound',{session})).data.queue;assert.ok(queue.records>0);assert.equal((await api('/fleets',{session})).status,200);evidence.push('persistent delivery history and live admin authorization after restart');
 await stop();console.log(JSON.stringify({passed:true,evidence},null,2));
}finally{if(child?.exitCode===null){child.kill('SIGKILL');await once(child,'exit');}await rm(dir,{recursive:true,force:true});}
