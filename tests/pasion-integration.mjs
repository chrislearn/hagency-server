// Three EMPTY dedicated databases; starts only an isolated test process.
import assert from 'node:assert/strict';
import {spawn, execFileSync} from 'node:child_process';
import {createServer} from 'node:http';
import {once} from 'node:events';
import {mkdtemp, writeFile, readFile, rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {resolve,join} from 'node:path';
const admin = process.env.HAGENCY_TEST_DATABASE_URL, matrix = process.env.PALPO_TEST_DATABASE_URL, pasion = process.env.PASION_TEST_DATABASE_URL;
assert.ok(admin && matrix && pasion, 'Set EMPTY HAGENCY_TEST_DATABASE_URL, PALPO_TEST_DATABASE_URL and PASION_TEST_DATABASE_URL');
for (const database of [admin,matrix,pasion]) assert.equal(execFileSync('psql',[database,'-Atc',"SELECT count(*) FROM pg_catalog.pg_tables WHERE schemaname NOT IN ('pg_catalog','information_schema')"],{encoding:'utf8'}).trim(),'0','Refuses a populated database');
const reserve=createServer().listen(0,'127.0.0.1');await once(reserve,'listening');const port=reserve.address().port;await new Promise(r=>reserve.close(r));
const dir=await mkdtemp(join(tmpdir(),'hagency-pasion-')),base=`http://127.0.0.1:${port}`;
const config=join(dir,'config.toml'),binary=process.env.HAGENCY_BINARY??resolve('target/debug/hagency-server');
const resources=resolve(process.env.PASION_TEST_RESOURCES??'resources/pasion');
let child, logs='',cookie='';
const pause=ms=>new Promise(r=>setTimeout(r,ms));
async function until(fn){for(let i=0;i<180;i++){if(child?.exitCode!==null&&child?.exitCode!==undefined)throw Error(logs);try{if(await fn())return;}catch(e){if(i===179)throw e;}await pause(500);}throw Error('Timed out: '+logs);}
async function configure(delegate=false){await writeFile(config,`listen = "127.0.0.1:${port}"\npublic_origin = "${base}"\ndata_dir = ${JSON.stringify(join(dir,"data"))}\ndatabase_url = ${JSON.stringify(admin)}\n[matrix]\nserver_name = "localhost:${port}"\nallow_registration = false\n[matrix.db]\nurl = ${JSON.stringify(matrix)}\npool_size = 10\n[matrix.well_known]\nclient = "${base}"\nserver = "localhost:${port}"\n[matrix.storage]\nbackend = "fs"\nroot = "data/media"\n[pasion]\ndatabase_url = ${JSON.stringify(pasion)}\nresources_dir = ${JSON.stringify(resources)}\ndelegate_matrix_auth = ${delegate}\n[pasion.settings.account]\npassword_registration_enabled = true\npassword_registration_contact_required = false\n`,{mode:0o600});}
async function start(){child=spawn(binary,['--config',config],{stdio:['ignore','pipe','pipe'],env:{...process.env,RUST_LOG:'warn'}});child.stdout.on('data',b=>logs+=b);child.stderr.on('data',b=>logs+=b);await until(async()=> (await fetch(base+'/_pasion/healthz')).status===200);}
async function stop(){if(child&&child.exitCode===null){const done=once(child,'exit');child.kill('SIGTERM');const result=await Promise.race([done,pause(35000).then(()=>{throw Error('Shutdown timed out')})]);assert.equal(result[0],0,logs);}child=undefined;}
async function api(path,body){const r=await fetch(base+'/_pasion/api/v1'+path,{method:body?'POST':'GET',headers:{'content-type':'application/json',...(cookie?{cookie}:{})},body:body?JSON.stringify(body):undefined});const set=r.headers.getSetCookie();if(set.length)cookie=set.map(s=>s.split(';')[0]).join('; ');const data=await r.json();assert.equal(r.status,200,JSON.stringify(data));return data;}
try{
 await configure();await start();
 const tables=(url)=>execFileSync('psql',[url,'-Atc',"SELECT tablename FROM pg_catalog.pg_tables WHERE schemaname='public' ORDER BY tablename"],{encoding:'utf8'}).trim().split('\n');
 assert.deepEqual(tables(admin),['hagency_admin_state']);assert.ok(tables(matrix).includes('users'));assert.ok(!tables(matrix).includes('hagency_admin_state'));assert.ok(tables(pasion).length>1);assert.ok(!tables(pasion).includes('hagency_admin_state'));
 assert.equal((await fetch(base+'/')).status,200);assert.equal((await fetch(base+'/_matrix/client/versions')).status,200);
 const discovery=await (await fetch(base+'/_pasion/.well-known/openid-configuration')).json();assert.equal(discovery.issuer,base+'/_pasion/');assert.equal(discovery.authorization_endpoint,base+'/_pasion/authorize');assert.equal(discovery.token_endpoint,base+'/_pasion/oauth2/token');
 for(const path of ['/api-doc/openapi.json','/api-doc/admin/openapi.json']) { const spec=await(await fetch(base+'/_pasion'+path)).json();assert.equal(spec.servers[0].url,'/_pasion'); }
 const alias=await fetch(base+'/_pasion/account/',{redirect:'manual'});assert.equal(alias.status,302);assert.equal(alias.headers.get('location'),'/_pasion/');
 const jwks=await(await fetch(base+'/_pasion/oauth2/keys.json')).json();assert.ok(jwks.keys.length);
 const html=await(await fetch(base+'/_pasion/login')).text();assert.ok(html.includes('/_pasion/api/v1'));const script=html.match(/src="([^"]+\.js)"/)[1].replace(/&#x([0-9a-f]+);/gi,(_,h)=>String.fromCodePoint(parseInt(h,16))).replace(/&#([0-9]+);/g,(_,n)=>String.fromCodePoint(Number(n)));assert.ok(script.startsWith('/_pasion/assets/'));
 const js=await(await fetch(base+script)).text();const wasm=js.match(/module_or_path:"([^"]+\.wasm)"/)[1];assert.ok(wasm.startsWith('/_pasion/assets/'));const wr=await fetch(base+wasm);assert.equal(wr.status,200);assert.match(wr.headers.get('content-type'),/application\/wasm/);
 assert.notEqual((await fetch(base+'/oauth2/keys.json')).status,200,'Pasion must not claim root routes');
 const internal=await fetch(base+'/_pasion/api/internal/matrix/password-login',{method:'POST',headers:{'content-type':'application/json'},body:'{}'});assert.equal(internal.status,401);
 const providers=await api('/auth/providers');assert.equal(providers.password_registration_enabled,true);
 const password='Test account! Matrix prefix 93614';const registration=await api('/auth/register',{username:'prefixuser',password,password_confirm:password});assert.equal(registration.status,'success',JSON.stringify(registration));assert.ok(registration.id);
 const display=await api('/auth/register/'+registration.id+'/display-name',{display_name:'Prefix User'});assert.equal(display.status,'success',JSON.stringify(display));
 const finished=await api('/auth/register/'+registration.id+'/finish',{});assert.equal(finished.status,'success',JSON.stringify(finished));
 const login=await api('/auth/login',{username:'prefixuser',password});assert.equal(login.status,'success',JSON.stringify(login));assert.equal(login.viewer.username,'prefixuser');
 assert.ok(cookie,'Pasion browser login must set its cookie');const viewer=await api('/viewer');assert.ok(JSON.stringify(viewer).includes('prefixuser'));
 await until(async()=>execFileSync('psql',[matrix,'-Atc',`SELECT count(*) FROM public.users WHERE id='@prefixuser:localhost:${port}'`],{encoding:'utf8'}).trim()==='1');
 const secrets=await readFile(join(dir,'data/pasion-secrets.json'),'utf8');
 await stop();await configure(true);await start();assert.equal(await readFile(join(dir,'data/pasion-secrets.json'),'utf8'),secrets);
 assert.deepEqual(await(await fetch(base+'/_pasion/oauth2/keys.json')).json(),jwks);
 const authentication=await(await fetch(base+'/.well-known/matrix/client')).json();assert.equal(authentication['m.authentication'].issuer,base+'/_pasion/');
 const matrixLogin=await fetch(base+'/_matrix/client/v3/login',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({type:'m.login.password',identifier:{type:'m.id.user',user:'prefixuser'},password})});const token=await matrixLogin.json();assert.equal(matrixLogin.status,200,JSON.stringify(token));assert.ok(token.access_token);
 const me=await fetch(base+'/_matrix/client/v3/account/whoami',{headers:{authorization:'Bearer '+token.access_token}});assert.equal(me.status,200,await me.clone().text());assert.equal((await me.json()).user_id,`@prefixuser:localhost:${port}`);
 await stop();
 console.log('PASS Pasion: three isolated databases, same listener, prefixed discovery/SPA/JS/WASM/API, browser registration/login, delegated Matrix password/token introspection, stable keys on restart, graceful shutdown');
 if(process.env.HAGENCY_KEEP_TEST_CONFIG)await writeFile(process.env.HAGENCY_KEEP_TEST_CONFIG,await readFile(config),{mode:0o600});
}catch(error){console.error(logs.slice(-10000));throw error;}finally{await stop();if(!process.env.HAGENCY_KEEP_TEST_CONFIG)await rm(dir,{recursive:true,force:true});}
