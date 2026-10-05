// Pasion user enrollment against the real Rust adapter and controlled Matrix.
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { once } from 'node:events';
import { spawn } from 'node:child_process';
import { mkdtemp, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { fixture } from './fixtures/palpo.mjs';
const f = fixture();
const owner = 'owner-access-token-0123456789', other = 'other-access-token-0123456789';
f.actors.set(owner,'@owner:example.test'); f.actors.set(other,'@other:example.test');
const upstream = createServer(async(req,res)=>{
  try {
    const chunks=[]; for await(const c of req) chunks.push(c);
    const raw=Buffer.concat(chunks).toString(), url=new URL(req.url,'http://fixture.invalid');
    let response;
    if(url.pathname==='/_pasion/oauth2/introspect') {
      assert.equal(req.headers.authorization,'Bearer service-secret');
      const token=new URLSearchParams(raw).get('token');
      const active=[owner,other,'mismatched-user-token-0123456789','appservice-token-0123456789','wrong-scope-token-0123456789','guest-access-token-0123456789'].includes(token);
      response=new Response(JSON.stringify({active,sub:token===other?'other-sub':'owner-sub',username:token===other?'other':'owner',client_id:token==='appservice-token-0123456789'?null:'native-client',scope:token==='wrong-scope-token-0123456789'?'openid':'urn:matrix:client:api:*'}),{status:200});
    } else if(url.pathname==='/_matrix/client/v3/account/whoami' && f.actors.has(req.headers.authorization?.slice(7))) response=new Response(JSON.stringify({user_id:f.actors.get(req.headers.authorization.slice(7)),device_id:'HagencyDevice',is_guest:req.headers.authorization==='Bearer guest-access-token-0123456789'}),{status:200});
    else response=await f.fetch(url,{method:req.method,headers:{Authorization:req.headers.authorization},...(raw?{body:raw}:{})});
    res.writeHead(response.status,{'Content-Type':'application/json'});res.end(await response.text());
  }catch(error){res.writeHead(502);res.end(JSON.stringify({error:error.message}));}
});
f.actors.set('mismatched-user-token-0123456789','@other:example.test');
f.actors.set('appservice-token-0123456789','@owner:example.test');
f.actors.set('wrong-scope-token-0123456789','@owner:example.test');
f.actors.set('guest-access-token-0123456789','@owner:example.test');
upstream.listen(0,'127.0.0.1');await once(upstream,'listening');
const fixtureOrigin=`http://127.0.0.1:${upstream.address().port}`;
const dir=await mkdtemp(join(tmpdir(),'hagency-native-'));
const delay=ms=>new Promise(r=>setTimeout(r,ms));
async function launch(enabled, policyKey="fleet_access") {
  const reservation=createServer().listen(0,'127.0.0.1');await once(reservation,'listening');const port=reservation.address().port;await new Promise(r=>reservation.close(r));
  const base=`http://127.0.0.1:${port}`;
  await writeFile(join(dir,'hagency.toml'),`listen="127.0.0.1:${port}"\npublic_origin="${base}"\ndatabase_url="postgres://unused/hagency"\npalpo_config="palpo.toml"\npasion_config="pasion.toml"\nretirement_admin_token_file="admin-token"\n[${policyKey}]\nallow_self_service=${enabled}\nmax_per_user=1\n`);
  await writeFile(join(dir,'admin-token'),'admin-secret',{mode:0o600});
  await writeFile(join(dir,'palpo.toml'),`server_name="example.test"\n[db]\nurl="postgres://unused/palpo"\n[admin]\nmas_secret="service-secret"\n[well_known]\nclient="${base}"\nserver="example.test"\n`);
  await writeFile(join(dir,'pasion.toml'),`[database]\nuri="postgres://unused/pasion"\n[hagency]\nresources_dir="${dir}"\ndelegate_matrix_auth=true\n`);
  const child=spawn(process.env.CONTRACT_SERVER??resolve('target/debug/examples/admin_contract_server'),[join(dir,'hagency.toml')],{env:{...process.env,FIXTURE_ORIGIN:fixtureOrigin},stdio:['ignore','pipe','pipe']});
  let logs='';child.stderr.on('data',d=>logs+=d);child.stdout.resume();
  async function api(path,token=owner,body,headers={}) {
    const reply=await fetch(base+'/_hagency/client/v1/'+path,{method:body?'POST':'GET',headers:{...(token?{Authorization:`Bearer ${token}`} :{}),...(body?{'Content-Type':'application/json'}:{}),...headers},...(body?{body:JSON.stringify(body)}:{})});
    return {status:reply.status,data:await reply.json()};
  }
  for(let i=0;i<100;i++){try{if((await api('discovery',null)).status===200)return{base,api,child};}catch{}if(child.exitCode!==null)throw Error(logs);await delay(100);}throw Error(logs||'timeout');
}
async function stop(child){child.kill('SIGTERM');await once(child,'exit');}
let running;
try {
  running=await launch(false);
  assert.equal((await running.api('discovery',null)).data.selfService,false);
  assert.equal((await running.api('fleets',owner,{installationId:'device-one',name:'Workstation'})).data.code,'self_service_disabled');
  await stop(running.child);running=await launch(true,"hafleet_access");
  const {api,base}=running;
  assert.equal((await api('identity')).data.userId,'@owner:example.test');
  assert.equal((await api('identity','guest-access-token-0123456789')).status,403);
  assert.equal((await api('unexpected/discovery')).status,404);
  for(const token of ['inactive-token-0123456789','mismatched-user-token-0123456789','appservice-token-0123456789','wrong-scope-token-0123456789']) assert.ok([401,403].includes((await api('identity',token)).status),token);
  assert.equal((await api('identity',owner,undefined,{Origin:base})).status,403);
  assert.equal((await api('identity',owner,undefined,{Cookie:'session=anything'})).status,403);
  assert.equal((await api('fleets',owner,{installationId:'one',name:'Workstation',ownerMxid:'@other:example.test'})).status,400);
  const created=await api('fleets',owner,{installationId:'device-one',name:'Workstation'});
  assert.equal(created.status,201,JSON.stringify(created.data));
  assert.equal(created.data.fleet.ownerMxid,'@owner:example.test');
  assert.equal(created.data.hafleet,undefined,'canonical response uses Fleet keys');
  assert.equal(created.data.configuration.transport.mode,'outbound');
  const id=created.data.fleet.id;
  assert.equal(f.registrations.size,1,'one App Service per Fleet');
  const again=await api('fleets',owner,{installationId:'device-one',name:'Workstation'});
  assert.deepEqual(again.data.configuration,created.data.configuration,'relogin retains the same credentials');
  assert.equal(f.registrations.size,1,'no duplicate registration');
  const legacy=await api('hafleets',owner,{installationId:'device-one',name:'Workstation'});
  assert.equal(legacy.status,201);assert.equal(legacy.data.hafleet.id,id);
  assert.deepEqual(legacy.data.configuration,created.data.configuration);
  assert.equal((await api(`hafleets/${id}/connect`,other,{})).status,404);
  assert.equal((await api('fleets',owner,{installationId:'device-two',name:'Second'})).data.code,'fleet_limit');
  assert.equal((await api(`fleets/${id}/connect`,other,{})).status,404);
  const second=await api('fleets',other,{installationId:'device-one',name:'Other user'});
  assert.equal(second.status,201,JSON.stringify(second.data));assert.notEqual(second.data.fleet.id,id);
  assert.ok(f.calls.filter(c=>c.path.startsWith('/_palpo/admin/')).every(c=>c.token==='admin-secret'),'native bearer never becomes administrator');
  console.log('PASS native Pasion identity, policy, account isolation, quota, idempotency and service authority');
} finally {if(running?.child.exitCode===null)await stop(running.child);await new Promise(r=>upstream.close(r));await rm(dir,{recursive:true,force:true});}
