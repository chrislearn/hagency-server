// An isolated Compose deployment of the built image; never touches the normal stack.
import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {mkdtemp,mkdir,writeFile,readFile,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
import {createServer} from 'node:http';
import {once} from 'node:events';
import {randomBytes} from 'node:crypto';
import {oauth} from './fixtures/pasion-auth.mjs';
const image=process.env.HAGENCY_TEST_IMAGE??'hagency-server:integration-check';
const reservation=createServer().listen(0,'127.0.0.1');await once(reservation,'listening');const port=reservation.address().port;await new Promise(r=>reservation.close(r));
const base=`http://127.0.0.1:${port}`,name=`localhost:${port}`,password=randomBytes(24).toString('base64url');
const dir=await mkdtemp(join(tmpdir(),'hagency-compose-')),compose=join(dir,'compose.yaml'),project='hagency-test-'+randomBytes(6).toString('hex');
const run=(...args)=>execFileSync('docker',['compose','--project-name',project,'-f',compose,...args],{encoding:'utf8',stdio:['ignore','pipe','pipe']});
const configDir=join(dir,'config');await mkdir(configDir,{mode:0o700});
const hostConfig=`listen = "0.0.0.0:8088"\npublic_origin = "${base}"\ndata_dir = "/app/data"\npublic_dir = "/app/resources/frontend/public"\ndatabase_url = "postgres://hagency:compose_test_only@postgres:5432/hagency"\npalpo_config = "palpo.toml"\npasion_config = "pasion.toml"\n`;
const palpoConfig=`server_name = "${name}"\nallow_registration = false\n[db]\nurl = "postgres://hagency:compose_test_only@postgres:5432/palpo"\npool_size = 10\n[well_known]\nclient = "${base}"\nserver = "${name}"\n[storage]\nbackend = "fs"\nroot = "/app/data/media"\n`;
const pasionConfig=`[database]\nuri = "postgres://hagency:compose_test_only@postgres:5432/pasion"\n[hagency]\nresources_dir = "/app/resources/pasion"\ndelegate_matrix_auth = true\n`;
for(const [component,text] of [['hagency',hostConfig],['palpo',palpoConfig],['pasion',pasionConfig]]) await writeFile(join(configDir,component+'.toml'),text,{mode:0o600});
await writeFile(join(dir,'init.sql'),await readFile(resolve('deploy/databases.sql'),'utf8'));
await writeFile(join(dir,'admin-password'),password,{mode:0o600});
function definition(bootstrap){return `services:\n  postgres:\n    image: postgres:18.6-alpine\n    environment:\n      POSTGRES_USER: hagency\n      POSTGRES_PASSWORD: compose_test_only\n      POSTGRES_DB: hagency\n    volumes: ["database:/var/lib/postgresql", "${dir}/init.sql:/docker-entrypoint-initdb.d/20-components.sql:ro"]\n    healthcheck:\n      test: ["CMD-SHELL", "pg_isready -U hagency -d hagency"]\n      interval: 1s\n      timeout: 3s\n      retries: 30\n  server:\n    image: ${image}\n    depends_on:\n      postgres: {condition: service_healthy}\n    ports: ["127.0.0.1:${port}:8088"]\n    volumes:\n      - "${configDir}:/app/config:ro"\n      - "${dir}/admin-password:/app/bootstrap-password:ro"\n      - "server-data:/app/data"\n    command: ${JSON.stringify(['--config','/app/config/hagency.toml',...(bootstrap?['--bootstrap-admin','admin','--bootstrap-password-file','/run/hagency/bootstrap-password']:[])])}\nvolumes:\n  database: {}\n  server-data: {}\n`;}
async function ready(){let last;for(let i=0;i<600;i++){try{if((await fetch(base+'/healthz')).status===200)return;}catch(e){last=e;}await new Promise(r=>setTimeout(r,100));}throw last??new Error('Startup timed out');}

async function login(){
 // Health means the listener is ready; Pasion provisions the Matrix identity
 // asynchronously through its durable queue after the listener starts.
 let provisioned=false;
 for(let i=0;i<300;i++){
  if(run('exec','-T','postgres','psql','-U','hagency','-d','palpo','-Atc',`SELECT is_admin FROM users WHERE id='@admin:${name}'`).trim()==='t'){provisioned=true;break;}
  await new Promise(r=>setTimeout(r,100));
 }
 assert.ok(provisioned,'Pasion administrator Matrix provisioning timed out');
 const auth=await fetch(base+'/_pasion/api/v1/auth/login',{method:'POST',headers:{'content-type':'application/json',Origin:base},body:JSON.stringify({username:'admin',password})});
 assert.equal(auth.status,200);const data=await auth.json();assert.equal(data.status,'success');
 const cookie=auth.headers.getSetCookie().map(c=>c.split(';')[0]).join('; ');
 const token=await oauth(base,cookie,true);assert.ok(token.access_token);
 const bridge=await fetch(base+'/api/login/token',{method:'POST',headers:{'content-type':'application/json',Origin:base,Authorization:'Bearer '+token.access_token},body:'{}'});
 assert.equal(bridge.status,200);assert.equal((await bridge.json()).isAdmin,true);
}
try{
 await writeFile(compose,definition(true));run('up','-d');await ready();await login();
 const tables=(db)=>run('exec','-T','postgres','psql','-U','hagency','-d',db,'-Atc',"SELECT tablename FROM pg_catalog.pg_tables WHERE schemaname='public' ORDER BY tablename").trim().split('\n');
 assert.deepEqual(tables('hagency'),['hagency_admin_state']);assert.ok(tables('palpo').includes('users'));assert.ok(!tables('palpo').includes('hagency_admin_state'));assert.ok(tables('pasion').length>1);assert.ok(!tables('pasion').includes('hagency_admin_state'));
 const frontend=await(await fetch(base)).text();assert.match(frontend,/<title>Hagency Server<\/title>/);
 assert.equal(await(await fetch(base+'/hagency/projects')).text(),frontend);
 const frontScript=frontend.match(/src="([^"]+\.js)"/)[1];const module=await(await fetch(new URL(frontScript,base))).text();
 const frontWasm=module.match(/module_or_path:"([^"]+\.wasm)"/)[1];const wasmResponse=await fetch(new URL(frontWasm,base));assert.equal(wasmResponse.status,200);assert.match(wasmResponse.headers.get('content-type'),/application\/wasm/);assert.equal(Buffer.from(await wasmResponse.arrayBuffer()).subarray(0,4).toString('hex'),'0061736d');
 const runtime=await(await fetch(base+'/config.json')).json();assert.equal(runtime.pasion_enabled,true);assert.equal(runtime.oauth_enabled,true);
 assert.equal((await fetch(base+'/_matrix/client/versions')).status,200);
 const discovery=await(await fetch(base+'/_pasion/.well-known/openid-configuration')).json();assert.equal(discovery.issuer,base+'/_pasion/');
 assert.equal((await fetch(base+'/_pasion/login')).status,200);
 const oauthKeys=await(await fetch(base+'/_pasion/oauth2/keys.json')).json();assert.ok(oauthKeys.keys.length);
 const uid=run('exec','-T','server','sh','-c',"awk '/^Uid:/ {print $2}' /proc/1/status; stat -c %a /run/hagency/config/*.toml; stat -c %a /run/hagency/config; stat -c %a /app/data/pasion-secrets.json").trim().split('\n');assert.deepEqual(uid,['10001','600','600','600','700','600']);
 const key=run('exec','-T','server','cat','/app/data/matrix-signing-key.json');
 run('stop','server');await writeFile(compose,definition(false));run('up','-d','server');await ready();await login();
 assert.equal(run('exec','-T','server','cat','/app/data/matrix-signing-key.json'),key);
 assert.deepEqual(await(await fetch(base+'/_pasion/oauth2/keys.json')).json(),oauthKeys);
 console.log('PASS isolated Compose: three native component config files and separate databases, non-root Rust process, protected configuration, same-port UI/Matrix/admin/Pasion discovery and login and persistent restart');
}catch(e){console.error(run('logs','--no-color','--tail','60','server'));throw e;}finally{try{run('down','--volumes','--remove-orphans');}finally{await rm(dir,{recursive:true,force:true});}}
